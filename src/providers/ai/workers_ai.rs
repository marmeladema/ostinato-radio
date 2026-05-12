use crate::errors::Result;
use crate::providers::ai::{MusicAI, RankedTrack, RankingContext};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, info, warn};

pub struct WorkersAIProvider {
    client: Client,
    account_id: String,
    api_token: String,
    model: String,
    fallback: super::DeterministicRanker,
}

impl WorkersAIProvider {
    pub fn new(account_id: String, api_token: String, model: String) -> Self {
        Self {
            client: Client::new(),
            account_id,
            api_token,
            model,
            fallback: super::DeterministicRanker,
        }
    }

    const MAX_AI_CANDIDATES: usize = 40;

    fn build_prompt(&self, ctx: &RankingContext<'_>) -> String {
        let mut prompt = format!(
            "You are a music recommendation engine. Rank the following candidate tracks for a radio themed: '{}\n\n",
            ctx.theme
        );

        prompt.push_str("Taste profile genres/artists (from user favorites):\n");
        for weight in ctx.taste_profile.artists.values() {
            prompt.push_str(&format!(
                "- {} (weight: {:.2})\n",
                weight.name,
                weight.base_weight + weight.session_delta
            ));
        }

        prompt.push_str("\nAlready played track IDs: ");
        if ctx.already_played.is_empty() {
            prompt.push_str("none\n");
        } else {
            prompt.push_str(&ctx.already_played.join(", "));
            prompt.push('\n');
        }

        prompt.push_str(&format!(
            "\nTarget pool ratios: familiar={:.0}%, new_release={:.0}%, discovery={:.0}%\n\n",
            ctx.pool_ratios.familiar * 100.0,
            ctx.pool_ratios.new_release * 100.0,
            ctx.pool_ratios.discovery * 100.0,
        ));

        // Cap candidates to avoid overflowing small model context windows.
        // Prioritize by pool: Familiar > NewRelease > Discovery.
        let mut prioritized: Vec<_> = ctx.candidates.iter().collect();
        prioritized.sort_by(|a, b| {
            use crate::state::Pool;
            let pool_ord = |p: &Pool| match p {
                Pool::Familiar => 0,
                Pool::NewRelease => 1,
                Pool::Discovery => 2,
            };
            pool_ord(&a.pool).cmp(&pool_ord(&b.pool))
        });
        let ai_subset: Vec<_> = prioritized
            .into_iter()
            .take(Self::MAX_AI_CANDIDATES)
            .collect();

        prompt.push_str(
            "Candidates (reply with ONLY a JSON array of {{candidate_id, rank}} objects):\n",
        );
        for c in &ai_subset {
            prompt.push_str(&format!(
                "{{\"id\":\"{}\",\"title\":\"{}\",\"artist\":\"{}\",\"pool\":\"{:?}\"}}\n",
                c.id, c.track_title, c.artist_name, c.pool
            ));
        }

        prompt.push_str(
            "\nRespond ONLY with a JSON array like: [{\"candidate_id\":\"...\",\"rank\":1},...]",
        );
        prompt
    }
}

#[async_trait]
impl MusicAI for WorkersAIProvider {
    async fn rank_candidates(&self, ctx: &RankingContext<'_>) -> Result<Vec<RankedTrack>> {
        // Skip AI call for tiny candidate lists — deterministic is faster
        if ctx.candidates.len() < 5 {
            debug!(
                "Workers AI: only {} candidates, using deterministic fallback",
                ctx.candidates.len()
            );
            return self.fallback.rank_candidates(ctx).await;
        }

        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/ai/run/{}",
            self.account_id, self.model
        );

        let prompt = self.build_prompt(ctx);
        debug!(
            "Workers AI prompt (first 2k chars): {}\n[truncated from {} chars]",
            &prompt[..prompt.len().min(2048)],
            prompt.len()
        );

        let body = serde_json::json!({
            "prompt": prompt,
        });

        debug!("Workers AI request URL: {}", url);

        let resp = match self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("Workers AI transport failed: {}", e);
                return self.fallback.rank_candidates(ctx).await;
            }
        };

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();

        if !status.is_success() {
            warn!("Workers AI returned HTTP {} body: {}", status, text);
            return self.fallback.rank_candidates(ctx).await;
        }

        debug!("Workers AI raw response ({}): {}", status, text);

        let data: serde_json::Value = match serde_json::from_str(&text) {
            Ok(d) => d,
            Err(e) => {
                warn!(
                    "Workers AI response is not valid JSON ({}): raw={}",
                    e, text
                );
                return self.fallback.rank_candidates(ctx).await;
            }
        };

        // Workers AI returns { result: { response: "..." } } for text generation models.
        // But response shape varies by model and gateway config.
        let response_text = data
            .get("result")
            .and_then(|r| r.get("response"))
            .and_then(|v| v.as_str())
            .or_else(|| data.get("response").and_then(|v| v.as_str()))
            .unwrap_or("");

        if response_text.is_empty() {
            warn!(
                "Workers AI returned empty response_text. Full JSON structure: {}",
                serde_json::to_string_pretty(&data).unwrap_or_default()
            );
            return self.fallback.rank_candidates(ctx).await;
        }

        debug!("Workers AI extracted response_text: {}", response_text);

        // Try to extract JSON array from the response text
        let json_str = extract_json_array(response_text);

        let ai_ranked: Vec<WorkersRankItem> = match serde_json::from_str(&json_str) {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    "Workers AI rank parse failed ({}). extracted_json={} raw_response={}",
                    e, json_str, response_text
                );
                return self.fallback.rank_candidates(ctx).await;
            }
        };

        info!("Workers AI ranked {} candidates", ai_ranked.len());

        // AI only saw a subset (MAX_AI_CANDIDATES). Build full rankings:
        // 1) Candidates returned by AI get their AI-assigned rank.
        // 2) Remaining candidates get deterministic ranks after the AI ones.
        let ai_ids: std::collections::HashSet<String> =
            ai_ranked.iter().map(|r| r.candidate_id.clone()).collect();

        let mut result = Vec::with_capacity(ctx.candidates.len());
        let base_rank = ai_ranked.len();

        for (i, r) in ai_ranked.into_iter().enumerate() {
            result.push(RankedTrack {
                candidate_id: r.candidate_id,
                rank: r.rank.unwrap_or(i),
            });
        }

        let mut remaining: Vec<_> = ctx
            .candidates
            .iter()
            .filter(|c| !ai_ids.contains(&c.id))
            .collect();
        // Deterministic order for remainder (pool priority then input order)
        {
            use crate::state::Pool;
            remaining.sort_by(|a, b| {
                let pool_ord = |p: &Pool| match p {
                    Pool::Familiar => 0,
                    Pool::NewRelease => 1,
                    Pool::Discovery => 2,
                };
                pool_ord(&a.pool)
                    .cmp(&pool_ord(&b.pool))
                    .then_with(|| a.id.cmp(&b.id))
            });
        }
        for (i, c) in remaining.into_iter().enumerate() {
            result.push(RankedTrack {
                candidate_id: c.id.clone(),
                rank: base_rank + i,
            });
        }

        Ok(result)
    }
}

#[derive(Deserialize)]
struct WorkersRankItem {
    candidate_id: String,
    rank: Option<usize>,
}

fn extract_json_array(text: &str) -> String {
    // Find the first '[' and last ']'
    let start = text.find('[').unwrap_or(0);
    let end = text.rfind(']').map(|i| i + 1).unwrap_or(text.len());
    text[start..end].to_string()
}
