pub mod anthropic;
pub mod openai_compat;
pub mod workers_ai;

use crate::errors::Result;
use crate::state::TrackId;
use async_trait::async_trait;

#[async_trait]
pub trait MusicAI: Send + Sync {
    async fn rank_candidates(&self, ctx: &RankingContext<'_>) -> Result<Vec<RankedTrack>>;
}

#[allow(dead_code)]
pub struct RankingContext<'a> {
    pub theme: &'a str,
    pub taste_profile: &'a crate::state::TasteProfile,
    pub candidates: &'a [Candidate],
    pub already_played: &'a [TrackId],
    pub target_count: usize,
    pub pool_ratios: crate::config::PoolRatios,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: String,
    pub track_title: String,
    pub artist_name: String,
    pub album: String,
    pub duration: Option<u64>,
    pub image_url: Option<String>,
    pub pool: crate::state::Pool,
    pub source_tags: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RankedTrack {
    pub candidate_id: String,
    pub rank: usize,
}

// Simple deterministic fallback ranker: shuffle by pool ratio weights
pub struct DeterministicRanker;

#[async_trait]
impl MusicAI for DeterministicRanker {
    async fn rank_candidates(&self, ctx: &RankingContext<'_>) -> Result<Vec<RankedTrack>> {
        let mut ordered = Vec::with_capacity(ctx.candidates.len());
        for (i, c) in ctx.candidates.iter().enumerate() {
            ordered.push((c, i));
        }

        // Sort by pool priority then by input order
        use crate::state::Pool;
        ordered.sort_by(|a, b| {
            let pool_ord = |p: &Pool| match p {
                Pool::Familiar => 0,
                Pool::NewRelease => 1,
                Pool::Discovery => 2,
            };
            pool_ord(&a.0.pool)
                .cmp(&pool_ord(&b.0.pool))
                .then_with(|| a.1.cmp(&b.1))
        });

        let mut ranked = Vec::new();
        for (rank, (c, _)) in ordered.into_iter().enumerate() {
            ranked.push(RankedTrack {
                candidate_id: c.id.clone(),
                rank,
            });
        }
        Ok(ranked)
    }
}
