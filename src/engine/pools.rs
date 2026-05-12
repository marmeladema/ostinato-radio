use crate::errors::Result;
use crate::providers::ai::Candidate;
use crate::state::{AppState, Pool};

use std::collections::HashSet;
use std::sync::Arc;

const MIN_SEARCH_RESULTS: usize = 10;
const MAX_ARTIST_FALLBACKS: usize = 10;

/// Build candidate pools.
///
/// Strategy:
/// 1. Try one broad search with theme tags.
/// 2. If too few results, fall back to per-artist searches for top favorite artists.
/// 3. New Release comes from cached album data (0 API calls).
pub async fn build_pools(
    state: &Arc<AppState>,
    theme_tags: &[String],
    max_age_days: u64,
) -> Result<Vec<Candidate>> {
    let start = std::time::Instant::now();

    let auth = state.qobuz_auth.read().await;
    let query = theme_tags.join(" ");

    // Snapshot favorite artist IDs
    let (favorite_artist_ids, top_artists) = {
        let profile = state.taste_profile.read().await;
        let ids: HashSet<String> = profile.artists.keys().cloned().collect();
        let mut sorted: Vec<_> = profile.artists.values().collect();
        sorted.sort_by(|a, b| {
            b.base_weight
                .partial_cmp(&a.base_weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let top: Vec<String> = sorted
            .into_iter()
            .take(MAX_ARTIST_FALLBACKS)
            .map(|w| w.name.clone())
            .collect();
        (ids, top)
    };

    // --- Broad search ---
    tracing::info!("Searching Qobuz for theme '{}'...", query);
    let mut search_results = state
        .qobuz
        .search_tracks(&auth, &query, 100)
        .await
        .unwrap_or_default();
    tracing::info!("Broad search returned {} tracks", search_results.len());

    // --- Fallback: per-artist search if broad search is too sparse ---
    if search_results.len() < MIN_SEARCH_RESULTS {
        tracing::info!(
            "Broad search sparse ({} < {}); trying per-artist fallback for top {} artists",
            search_results.len(),
            MIN_SEARCH_RESULTS,
            top_artists.len()
        );
        for artist_name in &top_artists {
            let q = format!("{} {}", artist_name, query);
            if let Ok(mut tracks) = state.qobuz.search_tracks(&auth, &q, 10).await {
                search_results.append(&mut tracks);
            }
        }
        tracing::info!("After fallback: {} tracks total", search_results.len());
    }

    // Deduplicate by track id
    let mut seen_ids = HashSet::new();
    search_results.retain(|t| seen_ids.insert(t.id.clone()));

    let mut candidates = Vec::new();

    // --- Familiar pool: search results from favorite artists ---
    for track in &search_results {
        if let Some(ref aid) = track.artist_id
            && favorite_artist_ids.contains(aid)
        {
            candidates.push(Candidate {
                id: format!("{}|{}", aid, track.id),
                track_title: track.title.clone(),
                artist_name: track.artist.clone(),
                album: track.album.clone(),
                duration: track.duration,
                image_url: track.image_url.clone(),
                pool: Pool::Familiar,
                source_tags: theme_tags.to_vec(),
            });
        }
    }

    // --- Discovery pool: search results from non-favorite artists ---
    for track in &search_results {
        let is_favorite = track
            .artist_id
            .as_ref()
            .map(|aid| favorite_artist_ids.contains(aid))
            .unwrap_or(false);
        if !is_favorite {
            candidates.push(Candidate {
                id: format!("disc|{}|{}", track.artist, track.id),
                track_title: track.title.clone(),
                artist_name: track.artist.clone(),
                album: track.album.clone(),
                duration: track.duration,
                image_url: track.image_url.clone(),
                pool: Pool::Discovery,
                source_tags: theme_tags.to_vec(),
            });
        }
    }

    // --- New Release pool: cached favorite albums filtered by date ---
    {
        let profile = state.taste_profile.read().await;
        let cutoff = chrono::Utc::now() - chrono::Duration::days(max_age_days as i64);
        for album in &profile.albums {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(&album.release_date, "%Y-%m-%d")
                && date >= cutoff.date_naive()
            {
                candidates.push(Candidate {
                    id: format!("nr|{}|{}", album.artist_id, album.id),
                    track_title: album.title.clone(),
                    artist_name: album.artist_name.clone(),
                    album: album.title.clone(),
                    duration: None,
                    image_url: None,
                    pool: Pool::NewRelease,
                    source_tags: Vec::new(),
                });
            }
        }
    }

    let familiar_count = candidates
        .iter()
        .filter(|c| matches!(c.pool, Pool::Familiar))
        .count();
    let discovery_count = candidates
        .iter()
        .filter(|c| matches!(c.pool, Pool::Discovery))
        .count();
    let new_release_count = candidates
        .iter()
        .filter(|c| matches!(c.pool, Pool::NewRelease))
        .count();

    tracing::info!(
        "Built {} candidates in {:.2}s (familiar={}, discovery={}, new_release={})",
        candidates.len(),
        start.elapsed().as_secs_f64(),
        familiar_count,
        discovery_count,
        new_release_count,
    );

    Ok(candidates)
}
