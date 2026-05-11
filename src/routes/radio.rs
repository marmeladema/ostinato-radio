use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::engine::pools::build_pools;
use crate::engine::ranker::rank_candidates;
use crate::engine::window::hydrate_queue;
use crate::errors::{AppError, Result};
use crate::state::{AppState, PlaybackTarget, PoolRatios, RadioSession};

#[derive(Deserialize)]
pub struct StartRadioRequest {
    pub theme: String,
    pub target: String,
    #[serde(default)]
    pub pool_ratios: Option<PoolRatios>,
}

#[derive(Serialize)]
pub struct StartRadioResponse {
    pub session_id: String,
    pub theme_tags: Vec<String>,
    pub queue: Vec<QueuedTrackResponse>,
    pub target: String,
}

#[derive(Serialize)]
pub struct QueuedTrackResponse {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub image_url: Option<String>,
    pub pool: String,
}

#[derive(Serialize)]
pub struct SessionStatusResponse {
    pub session_id: String,
    pub theme: String,
    pub current_track: Option<QueuedTrackResponse>,
    pub queue_remaining: usize,
    pub target: String,
}

pub async fn start_radio(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StartRadioRequest>,
) -> Result<Json<StartRadioResponse>> {
    let theme = req.theme.to_lowercase();
    let target = match req.target.as_str() {
        "wiim" => PlaybackTarget::Wiim,
        _ => PlaybackTarget::Phone,
    };
    let pool_ratios = req
        .pool_ratios
        .unwrap_or_else(|| state.config.radio.default_pool_ratios.clone());

    // Simple tag extraction from theme + dictionary
    let theme_tags = expand_theme_tags(&theme).await;

    info!(
        "Starting radio: theme='{}' tags={:?} target={:?}",
        theme, theme_tags, target
    );

    let candidates = build_pools(
        &state,
        &theme_tags,
        state.config.radio.new_release_max_age_days,
    )
    .await?;

    let ranked = rank_candidates(
        &state,
        &theme,
        candidates,
        &[],
        state.config.radio.window_size,
    )
    .await?;

    let queue = hydrate_queue(&state, ranked).await?;

    let session_id = uuid::Uuid::new_v4().to_string();
    let queue_for_response: Vec<QueuedTrackResponse> = queue
        .iter()
        .map(|q| QueuedTrackResponse {
            track_id: q.track_id.clone(),
            title: q.metadata.title.clone(),
            artist: q.metadata.artist.clone(),
            album: q.metadata.album.clone(),
            image_url: q.metadata.image_url.clone(),
            pool: format!("{:?}", q.pool),
        })
        .collect();

    let session = RadioSession {
        id: session_id.clone(),
        theme_input: theme.clone(),
        theme_tags: theme_tags.clone(),
        pool_ratios,
        queue,
        history: Vec::new(),
        target,
        started_at: std::time::Instant::now(),
    };

    state.sessions.insert(session_id.clone(), session);

    if target == PlaybackTarget::Wiim {
        let m3u_url = format!(
            "{}/playback/wiim/{}.m3u",
            state.config.server.public_base_url, session_id
        );
        if let Err(e) = state.linkplay.play_url(&m3u_url).await {
            tracing::warn!("Failed to start WiiM playback: {}", e);
        }
    }

    Ok(Json(StartRadioResponse {
        session_id,
        theme_tags,
        queue: queue_for_response,
        target: format!("{:?}", target).to_lowercase(),
    }))
}

pub async fn session_status(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionStatusResponse>> {
    let session = state.sessions.get(&session_id).ok_or(AppError::NotFound)?;

    let current = session.queue.front().map(|q| QueuedTrackResponse {
        track_id: q.track_id.clone(),
        title: q.metadata.title.clone(),
        artist: q.metadata.artist.clone(),
        album: q.metadata.album.clone(),
        image_url: q.metadata.image_url.clone(),
        pool: format!("{:?}", q.pool),
    });

    Ok(Json(SessionStatusResponse {
        session_id: session.id.clone(),
        theme: session.theme_input.clone(),
        current_track: current,
        queue_remaining: session.queue.len(),
        target: format!("{:?}", session.target).to_lowercase(),
    }))
}

pub async fn next_track(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<QueuedTrackResponse>> {
    let mut session = state
        .sessions
        .get_mut(&session_id)
        .ok_or(AppError::NotFound)?;

    let next = session.queue.pop_front().ok_or(AppError::NotFound)?;

    // Add to history
    state
        .sessions
        .get_mut(&session_id)
        .unwrap()
        .history
        .push(crate::state::PlayedTrack {
            track_id: next.track_id.clone(),
            pool: next.pool,
            completed: None,
            listened_ms: 0,
        });

    Ok(Json(QueuedTrackResponse {
        track_id: next.track_id,
        title: next.metadata.title,
        artist: next.metadata.artist,
        album: next.metadata.album,
        image_url: next.metadata.image_url,
        pool: format!("{:?}", next.pool),
    }))
}

async fn expand_theme_tags(theme: &str) -> Vec<String> {
    let dictionary: std::collections::HashMap<&str, Vec<&str>> = [
        ("chill", vec!["chill", "mellow", "relaxing"]),
        ("folk", vec!["folk", "acoustic", "indie folk"]),
        ("soir", vec!["evening", "mellow", "chill"]),
        ("running", vec!["energetic", "electronic", "upbeat"]),
        ("shoegaze", vec!["shoegaze", "dream pop", "noise pop"]),
        ("winter", vec!["ambient", "mellow", "acoustic"]),
    ]
    .into_iter()
    .collect();

    let mut tags = Vec::new();
    for (key, vals) in dictionary {
        if theme.contains(key) {
            for v in vals {
                if !tags.contains(&v.to_string()) {
                    tags.push(v.to_string());
                }
            }
        }
    }

    // If no dictionary match, use every word as a tag
    if tags.is_empty() {
        tags = theme.split_whitespace().map(|w| w.to_string()).collect();
    }

    tags
}
