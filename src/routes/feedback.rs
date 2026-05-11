use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::errors::{AppError, Result};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct FeedbackRequest {
    pub track_id: String,
    pub action: String, // "skip" | "complete" | "progress"
    pub progress_ms: Option<u64>,
    pub duration_ms: Option<u64>,
}

#[derive(Serialize)]
pub struct FeedbackResponse {
    pub ok: bool,
}

pub async fn submit_feedback(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(req): Json<FeedbackRequest>,
) -> Result<Json<FeedbackResponse>> {
    let mut session = state
        .sessions
        .get_mut(&session_id)
        .ok_or(AppError::NotFound)?;

    let duration = req.duration_ms.unwrap_or(1);
    let progress = req.progress_ms.unwrap_or(0);

    let ratio = if duration > 0 {
        progress as f32 / duration as f32
    } else {
        0.0
    };

    let completed = match req.action.as_str() {
        "complete" => Some(true),
        "skip" => Some(false),
        "progress" => {
            if ratio > 0.8 {
                Some(true)
            } else {
                None
            }
        }
        _ => return Err(AppError::BadRequest("Unknown action".to_string())),
    };

    // Update history
    if let Some(entry) = session
        .history
        .iter_mut()
        .find(|h| h.track_id == req.track_id)
    {
        entry.completed = completed;
        entry.listened_ms = progress;
    }

    // Update session_delta on artist
    let profile = state.taste_profile.read().await;
    if let Some(artist) = profile.artists.values().find(|_a| {
        // Very rough heuristic: we don't store track->artist mapping in history yet
        // In full implementation we'd derive this from track metadata
        false
    }) {
        let _ = artist;
    }
    drop(profile);

    info!(
        "Feedback for session {} track {}: action={} ratio={:.2}",
        session_id, req.track_id, req.action, ratio
    );

    Ok(Json(FeedbackResponse { ok: true }))
}
