use std::collections::HashMap;
use tracing::info;

use crate::errors::Result;
use crate::providers::qobuz::QobuzFavorites;
use crate::state::{ArtistId, ArtistWeight, TasteProfile};

pub async fn build_taste_profile(favorites: QobuzFavorites) -> Result<TasteProfile> {
    let mut artists: HashMap<ArtistId, ArtistWeight> = HashMap::new();

    for artist in &favorites.artists {
        artists.insert(
            artist.id.clone(),
            ArtistWeight {
                name: artist.name.clone(),
                base_weight: 1.0,
                session_delta: 0.0,
            },
        );
    }

    // Boost artists that appear in favorite tracks/albums
    for track_id in &favorites.tracks {
        // We don't have direct artist mapping here; could be enhanced later
        let _ = track_id;
    }

    info!(
        "Built taste profile: {} artists, {} albums, {} tracks",
        artists.len(),
        favorites.albums.len(),
        favorites.tracks.len()
    );

    Ok(TasteProfile {
        artists,
        albums: favorites.albums,
        tracks: favorites.tracks,
        last_full_refresh: std::time::Instant::now(),
    })
}
