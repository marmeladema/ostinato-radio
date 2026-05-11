use crate::errors::{AppError, Result};
use crate::providers::ai::{MusicAI, RankedTrack, RankingContext};
use async_trait::async_trait;

#[allow(dead_code)]
pub struct OpenAICompatProvider {
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAICompatProvider {
    pub fn new(api_key: String, base_url: String, model: String) -> Self {
        Self {
            api_key,
            base_url,
            model,
        }
    }
}

#[async_trait]
impl MusicAI for OpenAICompatProvider {
    async fn rank_candidates(&self, _ctx: &RankingContext<'_>) -> Result<Vec<RankedTrack>> {
        // TODO: Implement OpenAI-compatible call
        Err(AppError::Ai("OpenAICompat not yet implemented".to_string()))
    }
}
