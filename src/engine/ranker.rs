use crate::errors::Result;
use crate::providers::ai::{Candidate, RankedTrack, RankingContext};
use crate::state::AppState;

use std::sync::Arc;

pub async fn rank_candidates(
    state: &Arc<AppState>,
    theme: &str,
    candidates: Vec<Candidate>,
    already_played: &[String],
    target_count: usize,
) -> Result<Vec<RankedTrack>> {
    let profile = state.taste_profile.read().await;
    let ctx = RankingContext {
        theme,
        taste_profile: &profile,
        candidates: &candidates,
        already_played,
        target_count,
        pool_ratios: state.config.radio.default_pool_ratios.clone(),
    };

    let ranked = state.ai.rank_candidates(&ctx).await?;
    Ok(ranked)
}
