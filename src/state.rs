use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::providers::ai::MusicAI;
use crate::providers::lastfm::LastfmClient;
use crate::providers::linkplay::LinkplayClient;
use crate::providers::qobuz::QobuzClient;

pub type TrackId = String;
pub type ArtistId = String;
pub type SessionId = String;
pub type AlbumId = String;

pub struct AppState {
    pub config: Config,
    pub qobuz_auth: RwLock<QobuzAuth>,
    pub taste_profile: RwLock<TasteProfile>,
    pub new_releases: DashMap<ArtistId, CachedReleases>,
    pub similar_artists: DashMap<ArtistId, CachedSimilar>,
    pub track_metadata: DashMap<TrackId, TrackMetadata>,
    pub sessions: DashMap<SessionId, RadioSession>,
    pub password_hash: Option<String>,
    pub jwt_secret: String,
    pub qobuz: QobuzClient,
    #[allow(dead_code)]
    pub lastfm: LastfmClient,
    pub ai: Box<dyn MusicAI + Send + Sync>,
    pub linkplay: LinkplayClient,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct QobuzAuth {
    pub app_id: String,
    pub app_secret: String,
    pub private_key: String,
    pub user_auth_token: String,
    pub user_id: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub country_code: Option<String>,
    pub subscription: Option<String>,
    /// Unix timestamp (seconds) when this auth was obtained.
    #[serde(default)]
    pub obtained_at_ts: Option<u64>,
}

impl QobuzAuth {
    pub fn is_populated(&self) -> bool {
        !self.app_id.is_empty() && !self.app_secret.is_empty() && !self.user_auth_token.is_empty()
    }

    pub fn maybe_load(path: &std::path::Path) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        let auth: Self = serde_json::from_slice(&bytes).ok()?;
        if auth.is_populated() {
            tracing::info!("Loaded Qobuz auth from {}", path.display());
            Some(auth)
        } else {
            tracing::warn!(
                "Qobuz auth file {} was incomplete, ignoring",
                path.display()
            );
            None
        }
    }

    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(self)?)?;
        tracing::info!("Saved Qobuz auth to {}", path.display());
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TasteProfile {
    pub artists: HashMap<ArtistId, ArtistWeight>,
    pub albums: Vec<AlbumEntry>,
    pub tracks: Vec<TrackMetadata>,
    pub last_full_refresh: Instant,
}

#[derive(Debug, Clone)]
pub struct AlbumEntry {
    pub id: AlbumId,
    pub title: String,
    pub artist_id: ArtistId,
    pub artist_name: String,
    pub release_date: String,
}

#[derive(Debug, Clone)]
pub struct ArtistWeight {
    pub name: String,
    pub base_weight: f32,
    pub session_delta: f32,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CachedReleases {
    pub releases: Vec<Release>,
    pub fetched_at: Instant,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CachedSimilar {
    pub artists: Vec<SimilarArtist>,
    pub fetched_at: Instant,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Release {
    pub id: AlbumId,
    pub title: String,
    pub release_date: String,
    pub tracks: Vec<TrackMetadata>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SimilarArtist {
    pub id: Option<ArtistId>,
    pub name: String,
    pub match_score: f32,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RadioSession {
    pub id: SessionId,
    pub theme_input: String,
    pub theme_tags: Vec<String>,
    pub pool_ratios: PoolRatios,
    pub queue: VecDeque<QueuedTrack>,
    pub history: Vec<PlayedTrack>,
    pub target: PlaybackTarget,
    pub started_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackTarget {
    Phone,
    Wiim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pool {
    Familiar,
    NewRelease,
    Discovery,
}

pub use crate::config::PoolRatios;

#[derive(Debug, Clone)]
pub struct QueuedTrack {
    pub track_id: TrackId,
    pub pool: Pool,
    pub metadata: TrackMetadata,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PlayedTrack {
    pub track_id: TrackId,
    pub pool: Pool,
    pub completed: Option<bool>,
    pub listened_ms: u64,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TrackMetadata {
    pub id: TrackId,
    pub title: String,
    pub artist: String,
    pub artist_id: Option<String>,
    pub album: String,
    pub album_id: Option<String>,
    pub duration: Option<u64>,
    pub image_url: Option<String>,
}

impl AppState {
    pub fn new(
        config: Config,
        qobuz: QobuzClient,
        lastfm: LastfmClient,
        ai: Box<dyn MusicAI + Send + Sync>,
        linkplay: LinkplayClient,
        password_hash: Option<String>,
        jwt_secret: String,
    ) -> Arc<Self> {
        let auth_path = std::path::Path::new("data/qobuz_auth.json");
        let qobuz_auth = QobuzAuth::maybe_load(auth_path).unwrap_or_default();
        Arc::new(Self {
            config,
            qobuz_auth: RwLock::new(qobuz_auth),
            taste_profile: RwLock::new(TasteProfile {
                artists: HashMap::new(),
                albums: Vec::new(),
                tracks: Vec::new(),
                last_full_refresh: Instant::now(),
            }),
            new_releases: DashMap::new(),
            similar_artists: DashMap::new(),
            track_metadata: DashMap::new(),
            sessions: DashMap::new(),
            password_hash,
            jwt_secret,
            qobuz,
            lastfm,
            ai,
            linkplay,
        })
    }
}

impl CachedReleases {
    pub fn is_expired(&self) -> bool {
        self.fetched_at.elapsed() > Duration::from_secs(24 * 3600)
    }
}

impl CachedSimilar {
    pub fn is_expired(&self) -> bool {
        self.fetched_at.elapsed() > Duration::from_secs(7 * 24 * 3600)
    }
}
