use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::warn;

use crate::errors::{AppError, Result};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct StreamQuery {
    pub session: Option<String>,
}

pub async fn stream_redirect(
    State(state): State<Arc<AppState>>,
    Path(track_id): Path<String>,
    Query(query): Query<StreamQuery>,
) -> Result<Response> {
    let auth = state.qobuz_auth.read().await;

    let stream_url = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        state
            .qobuz
            .get_file_url(&auth, &track_id, state.config.qobuz.preferred_format_id),
    )
    .await
    .map_err(|_| AppError::Qobuz("Timeout getting stream URL".to_string()))?
    .map_err(|e| {
        warn!("Qobuz getFileUrl failed: {}, returning 503", e);
        AppError::Qobuz("Service temporarily unavailable".to_string())
    })?;

    drop(auth);

    if let Some(session_id) = &query.session
        && let Some(mut session) = state.sessions.get_mut(session_id)
    {
        // Track that this track started playing
        if let Some(last) = session.history.last_mut() {
            if last.track_id == track_id {
                // Already in history
            } else {
                session.history.push(crate::state::PlayedTrack {
                    track_id: track_id.clone(),
                    pool: crate::state::Pool::Familiar, // placeholder
                    completed: None,
                    listened_ms: 0,
                });
            }
        }
    }

    Ok(Redirect::temporary(&stream_url).into_response())
}

pub async fn wiim_m3u(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Response> {
    let session = state.sessions.get(&session_id).ok_or(AppError::NotFound)?;

    let mut lines = vec!["#EXTM3U".to_string()];

    for track in session.queue.iter() {
        let url = format!(
            "{}/stream/{}?session={}",
            state.config.server.public_base_url, track.track_id, session_id
        );
        lines.push(format!(
            "#EXTINF:{},{} - {}",
            track.metadata.duration.unwrap_or(0),
            track.metadata.artist,
            track.metadata.title
        ));
        lines.push(url);
    }

    let body = lines.join("\n");

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "audio/x-mpegurl")
        .body(Body::from(body))
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[derive(Deserialize)]
pub struct WiimControlQuery {
    pub command: String,
}

pub async fn wiim_control(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WiimControlQuery>,
) -> Result<StatusCode> {
    match query.command.as_str() {
        "play" | "resume" => state.linkplay.resume().await?,
        "pause" => state.linkplay.pause().await?,
        "stop" => state.linkplay.stop().await?,
        "next" => state.linkplay.next().await?,
        "prev" => state.linkplay.prev().await?,
        other => {
            if other.starts_with("vol:") {
                let vol: u8 = other
                    .split(':')
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(50);
                state.linkplay.set_volume(vol).await?;
            } else {
                return Err(AppError::BadRequest(format!("Unknown command: {}", other)));
            }
        }
    }

    Ok(StatusCode::NO_CONTENT)
}
