use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

use crate::errors::{AppError, Result};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct StreamQuery {
    pub session: Option<String>,
    pub format_id: Option<u32>,
}

pub async fn stream_redirect(
    State(state): State<Arc<AppState>>,
    Path(track_id): Path<String>,
    Query(query): Query<StreamQuery>,
) -> Result<Response> {
    let auth = state.qobuz_auth.read().await;

    let format_id = query
        .format_id
        .unwrap_or(state.config.qobuz.preferred_format_id);

    let stream_info = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        state.qobuz.get_file_url(&auth, &track_id, format_id),
    )
    .await
    .map_err(|_| AppError::Qobuz("Timeout getting stream URL".to_string()))?
    .map_err(|e| {
        warn!("Qobuz getFileUrl failed: {}, returning 503", e);
        AppError::Qobuz("Service temporarily unavailable".to_string())
    })?;

    let stream_url = stream_info.url;

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

    // Debug: probe the stream URL to log content-type before redirecting
    match state.qobuz.client.head(&stream_url).send().await {
        Ok(resp) => {
            let _status = resp.status();
            let ct = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok());
            let cors = resp
                .headers()
                .get("access-control-allow-origin")
                .and_then(|v| v.to_str().ok());
            warn!(
                "stream_redirect: track_id={}, requested_format={}, qobuz_format={:?}, content-type={:?}, cors={:?}, url={}",
                track_id, format_id, stream_info.format, ct, cors, stream_url
            );
        }
        Err(e) => {
            warn!(
                "stream_redirect: HEAD probe failed for track_id={}: {}",
                track_id, e
            );
        }
    }

    Ok(Redirect::temporary(&stream_url).into_response())
}

#[derive(Serialize)]
pub struct StreamDebugResponse {
    pub track_id: String,
    pub requested_format_id: u32,
    pub qobuz_format: String,
    pub stream_url: String,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    pub status: u16,
    pub cors: Option<String>,
}

/// Debug endpoint to diagnose stream issues (CORS vs format).
/// Returns metadata about the Qobuz stream without consuming the body.
pub async fn stream_debug(
    State(state): State<Arc<AppState>>,
    Path(track_id): Path<String>,
    Query(query): Query<StreamQuery>,
) -> Result<JsonResponse<StreamDebugResponse>> {
    let auth = state.qobuz_auth.read().await;

    let format_id = query
        .format_id
        .unwrap_or(state.config.qobuz.preferred_format_id);

    let stream_info = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        state.qobuz.get_file_url(&auth, &track_id, format_id),
    )
    .await
    .map_err(|_| AppError::Qobuz("Timeout getting stream URL".to_string()))?
    .map_err(|e| {
        warn!("Qobuz getFileUrl failed in debug: {}", e);
        AppError::Qobuz("Service temporarily unavailable".to_string())
    })?;

    let stream_url = stream_info.url;
    let qobuz_format = stream_info.format;

    drop(auth);

    let (status, ct, cl, cors) = match state.qobuz.client.head(&stream_url).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let ct = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let cl = resp
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok());
            let cors = resp
                .headers()
                .get("access-control-allow-origin")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            info!(
                "Stream debug: status={}, content-type={:?}, content-length={:?}, cors={:?}, url={}",
                status, ct, cl, cors, stream_url
            );
            (status, ct, cl, cors)
        }
        Err(e) => {
            warn!("Stream debug HEAD failed: {}", e);
            (0, Some(format!("HEAD request failed: {}", e)), None, None)
        }
    };

    Ok(JsonResponse(StreamDebugResponse {
        track_id,
        requested_format_id: format_id,
        qobuz_format,
        stream_url,
        content_type: ct,
        content_length: cl,
        status,
        cors,
    }))
}

/// Return stream quality info for a track (format, sampling rate, bit depth).
pub async fn track_stream_info(
    State(state): State<Arc<AppState>>,
    Path(track_id): Path<String>,
    Query(query): Query<StreamQuery>,
) -> Result<JsonResponse<crate::providers::qobuz::StreamInfo>> {
    let auth = state.qobuz_auth.read().await;

    let format_id = query
        .format_id
        .unwrap_or(state.config.qobuz.preferred_format_id);

    let info = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        state.qobuz.get_file_url(&auth, &track_id, format_id),
    )
    .await
    .map_err(|_| AppError::Qobuz("Timeout getting stream URL".to_string()))?
    .map_err(|e| {
        warn!("Qobuz getFileUrl failed for track info: {}", e);
        AppError::Qobuz("Service temporarily unavailable".to_string())
    })?;

    Ok(JsonResponse(info))
}

pub struct JsonResponse<T>(pub T);

impl<T: serde::Serialize> IntoResponse for JsonResponse<T> {
    fn into_response(self) -> Response {
        match serde_json::to_string(&self.0) {
            Ok(body) => Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .unwrap(),
            Err(e) => Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("JSON error: {}", e)))
                .unwrap(),
        }
    }
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
