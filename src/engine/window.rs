use crate::errors::Result;
use crate::providers::ai::RankedTrack;
use crate::state::{AppState, QueuedTrack, SessionId, TrackMetadata};

use std::collections::VecDeque;
use std::sync::Arc;

pub async fn hydrate_queue(
    state: &Arc<AppState>,
    ranked: Vec<RankedTrack>,
) -> Result<VecDeque<QueuedTrack>> {
    let mut queue = VecDeque::new();

    for r in ranked.into_iter().take(20) {
        // candidate_id may contain prefixes; extract real track id after last pipe
        let track_id = r
            .candidate_id
            .rsplit_once('|')
            .map(|(_, id)| id)
            .unwrap_or(&r.candidate_id);
        let auth = state.qobuz_auth.read().await;
        let metadata = match state.qobuz.get_track_metadata(&auth, track_id).await {
            Ok(m) => m,
            Err(_) => {
                // Fallback to cached metadata if available
                state
                    .track_metadata
                    .get(track_id)
                    .map(|e| e.clone())
                    .unwrap_or_else(|| TrackMetadata {
                        id: track_id.to_string(),
                        title: "Unknown".to_string(),
                        artist: "Unknown".to_string(),
                        album: "Unknown".to_string(),
                        duration: None,
                        image_url: None,
                    })
            }
        };
        state
            .track_metadata
            .insert(track_id.to_string(), metadata.clone());

        let pool = if r.candidate_id.starts_with("disc|") {
            crate::state::Pool::Discovery
        } else if r.candidate_id.starts_with("nr|") {
            crate::state::Pool::NewRelease
        } else {
            crate::state::Pool::Familiar
        };

        queue.push_back(QueuedTrack {
            track_id: track_id.to_string(),
            pool,
            metadata,
        });
    }

    Ok(queue)
}

#[allow(dead_code)]
pub async fn append_to_session(
    state: &Arc<AppState>,
    session_id: &SessionId,
    tracks: Vec<QueuedTrack>,
) -> Result<()> {
    let mut session = state
        .sessions
        .get_mut(session_id)
        .ok_or_else(|| crate::errors::AppError::NotFound)?;

    for t in tracks {
        session.queue.push_back(t);
    }

    Ok(())
}
