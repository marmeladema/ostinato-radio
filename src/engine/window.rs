use crate::errors::Result;
use crate::providers::ai::{Candidate, RankedTrack};
use crate::state::{AppState, QueuedTrack, SessionId, TrackMetadata};

use std::collections::VecDeque;
use std::sync::Arc;

pub async fn hydrate_queue(
    state: &Arc<AppState>,
    ranked: Vec<RankedTrack>,
    candidates: &[Candidate],
) -> Result<VecDeque<QueuedTrack>> {
    let mut queue = VecDeque::new();

    // Build lookup map from candidate id -> metadata
    let candidate_map: std::collections::HashMap<&str, &Candidate> =
        candidates.iter().map(|c| (c.id.as_str(), c)).collect();

    for r in ranked.into_iter().take(20) {
        // candidate_id may contain prefixes; extract real track id after last pipe
        let track_id = r
            .candidate_id
            .rsplit_once('|')
            .map(|(_, id)| id)
            .unwrap_or(&r.candidate_id);

        let metadata = if let Some(c) = candidate_map.get(r.candidate_id.as_str()) {
            TrackMetadata {
                id: track_id.to_string(),
                title: c.track_title.clone(),
                artist: c.artist_name.clone(),
                artist_id: None,
                album: c.album.clone(),
                album_id: None,
                duration: c.duration,
                image_url: c.image_url.clone(),
            }
        } else {
            // Fallback to cached metadata or unknown
            state
                .track_metadata
                .get(track_id)
                .map(|r| r.clone())
                .unwrap_or_else(|| TrackMetadata {
                    id: track_id.to_string(),
                    title: "Unknown".to_string(),
                    artist: "Unknown".to_string(),
                    artist_id: None,
                    album: "Unknown".to_string(),
                    album_id: None,
                    duration: None,
                    image_url: None,
                })
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
