use crate::errors::Result;
use crate::providers::ai::{MusicAI, RankedTrack, RankingContext};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tracing::{info, warn};

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

    fn build_prompt(&self, ctx: &RankingContext<'_>) -> String {
        let mut prompt = format!(
            "You are a music recommendation engine. Rank the following candidate tracks for a radio themed: '{}'\n\n",
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

        prompt.push_str(
            "Candidates (reply with ONLY a JSON array of {{candidate_id, rank}} objects):\n",
        );
        for c in ctx.candidates {
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
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/ai/run/{}",
            self.account_id, self.model
        );

        let prompt = self.build_prompt(ctx);

        let body = serde_json::json!({
            "prompt": prompt,
        });

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
                warn!("Workers AI request failed: {}", e);
                return self.fallback.rank_candidates(ctx).await;
            }
        };

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            warn!("Workers AI returned error: {}", text);
            return self.fallback.rank_candidates(ctx).await;
        }

        let data: serde_json::Value = match resp.json().await {
            Ok(d) => d,
            Err(e) => {
                warn!("Workers AI JSON parse failed: {}", e);
                return self.fallback.rank_candidates(ctx).await;
            }
        };

        // Workers AI returns { result: { response: "...json string..." } }
        let response_text = data
            .get("result")
            .and_then(|r| r.get("response"))
            .and_then(|v| v.as_str())
            .or_else(|| data.get("response").and_then(|v| v.as_str()))
            .unwrap_or("");

        // Try to extract JSON array from the response text
        let json_str = extract_json_array(response_text);

        let ranked: Vec<WorkersRankItem> = match serde_json::from_str(&json_str) {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    "Workers AI rank parse failed ({}), raw: {}",
                    e, response_text
                );
                return self.fallback.rank_candidates(ctx).await;
            }
        };

        info!("Workers AI ranked {} candidates", ranked.len());

        Ok(ranked
            .into_iter()
            .enumerate()
            .map(|(i, r)| RankedTrack {
                candidate_id: r.candidate_id,
                rank: r.rank.unwrap_or(i),
            })
            .collect())
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
