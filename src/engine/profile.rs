use std::collections::HashMap;
use tracing::info;

use crate::errors::Result;
use crate::providers::qobuz::QobuzFavorites;
use crate::state::{AlbumEntry, ArtistId, ArtistWeight, TasteProfile, TrackMetadata};

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
    for track in &favorites.tracks {
        if let Some(ref aid) = track.artist_id
            && let Some(weight) = artists.get_mut(aid)
        {
            weight.base_weight += 0.5;
        }
    }
    for album in &favorites.albums {
        if let Some(weight) = artists.get_mut(&album.artist_id) {
            weight.base_weight += 0.3;
        }
    }

    let albums: Vec<AlbumEntry> = favorites
        .albums
        .into_iter()
        .map(|a| AlbumEntry {
            id: a.id,
            title: a.title,
            artist_id: a.artist_id,
            artist_name: a.artist_name,
            release_date: a.release_date,
        })
        .collect();

    let tracks: Vec<TrackMetadata> = favorites.tracks;

    info!(
        "Built taste profile: {} artists, {} albums, {} tracks",
        artists.len(),
        albums.len(),
        tracks.len()
    );

    Ok(TasteProfile {
        artists,
        albums,
        tracks,
        last_full_refresh: std::time::Instant::now(),
    })
}
