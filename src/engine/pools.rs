use crate::errors::Result;
use crate::providers::ai::Candidate;
use crate::state::{AppState, ArtistId, Pool};

use std::sync::Arc;

pub async fn build_pools(
    state: &Arc<AppState>,
    theme_tags: &[String],
    max_age_days: u64,
) -> Result<Vec<Candidate>> {
    let profile = state.taste_profile.read().await;
    let mut candidates = Vec::new();

    // --- Familiar pool ---
    for (artist_id, weight) in &profile.artists {
        let auth = state.qobuz_auth.read().await;
        if let Ok(tracks) = state
            .qobuz
            .search_tracks(
                &auth,
                &format!("{} {} ", weight.name, theme_tags.join(" ")),
                10,
            )
            .await
        {
            for track in tracks.into_iter().take(5) {
                candidates.push(Candidate {
                    id: format!("{}|{}", artist_id, track.id),
                    track_title: track.title,
                    artist_name: weight.name.clone(),
                    pool: Pool::Familiar,
                    source_tags: theme_tags.to_vec(),
                });
            }
        }
    }

    // --- New releases pool ---
    for (artist_id, weight) in &profile.artists {
        if let Ok(detail) = state
            .qobuz
            .get_artist_with_albums(&*state.qobuz_auth.read().await, artist_id)
            .await
        {
            let cutoff = chrono::Utc::now() - chrono::Duration::days(max_age_days as i64);
            for album in detail.albums {
                if let Ok(date) = chrono::NaiveDate::parse_from_str(&album.release_date, "%Y-%m-%d")
                    && date >= cutoff.date_naive()
                {
                    // Placeholder: we would fetch album tracks here.
                    // For now, create a candidate from album title.
                    candidates.push(Candidate {
                        id: format!("nr|{}|{}", artist_id, album.id),
                        track_title: album.title,
                        artist_name: weight.name.clone(),
                        pool: Pool::NewRelease,
                        source_tags: theme_tags.to_vec(),
                    });
                }
            }
        }
    }

    // --- Discovery pool ---
    let top_artists: Vec<ArtistId> = profile.artists.keys().take(10).cloned().collect();

    drop(profile); // Release read lock before await loops

    for artist_id in top_artists {
        let artist_name = state
            .taste_profile
            .read()
            .await
            .artists
            .get(&artist_id)
            .map(|w| w.name.clone())
            .unwrap_or_default();

        if let Ok(similar) = state.lastfm.get_similar_artists(&artist_name).await {
            for sim in similar.into_iter().take(5) {
                if let Ok(tracks) = state
                    .qobuz
                    .search_tracks(
                        &*state.qobuz_auth.read().await,
                        &format!("{} {}", sim.name, theme_tags.join(" ")),
                        5,
                    )
                    .await
                {
                    for track in tracks.into_iter().take(3) {
                        candidates.push(Candidate {
                            id: format!("disc|{}|{}", sim.name, track.id),
                            track_title: track.title,
                            artist_name: sim.name.clone(),
                            pool: Pool::Discovery,
                            source_tags: theme_tags.to_vec(),
                        });
                    }
                }
            }
        }
    }

    Ok(candidates)
}
