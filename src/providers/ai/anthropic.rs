use crate::errors::{AppError, Result};
use crate::providers::ai::{MusicAI, RankedTrack, RankingContext};
use async_trait::async_trait;

#[allow(dead_code)]
pub struct AnthropicProvider {
    api_key: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }
}

#[async_trait]
impl MusicAI for AnthropicProvider {
    async fn rank_candidates(&self, _ctx: &RankingContext<'_>) -> Result<Vec<RankedTrack>> {
        // TODO: Implement Anthropic call
        Err(AppError::Ai("Anthropic not yet implemented".to_string()))
    }
}
