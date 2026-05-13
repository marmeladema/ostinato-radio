use crate::errors::{AppError, Result};
use crate::state::TrackMetadata;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::Client;

pub mod auth;
pub mod bundle;

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 Firefox/110.0";

#[derive(Debug, Clone)]
pub struct QobuzClient {
    pub client: Client,
    base_url: String,
}

impl Default for QobuzClient {
    fn default() -> Self {
        Self::new()
    }
}

impl QobuzClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            base_url: "https://www.qobuz.com/api.json/0.2".to_string(),
        }
    }

    pub async fn get_user_favorites(
        &self,
        auth: &crate::state::QobuzAuth,
    ) -> Result<QobuzFavorites> {
        tracing::info!("Fetching Qobuz user favorites...");
        let params = vec![("limit", "5000"), ("offset", "0")];
        let body: serde_json::Value = self
            .signed_get("/favorite/getUserFavorites", &params, auth)
            .await?;

        use crate::state::TrackMetadata;

        let mut artists = Vec::new();
        let mut albums = Vec::new();
        let mut tracks = Vec::new();

        if let Some(arr) = body
            .get("artists")
            .and_then(|v| v.get("items"))
            .and_then(|v| v.as_array())
        {
            for item in arr {
                if let Some(id) = item
                    .get("id")
                    .and_then(|v| v.as_i64())
                    .map(|v| v.to_string())
                {
                    artists.push(QobuzArtist {
                        id,
                        name: item
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    });
                }
            }
        }
        if let Some(arr) = body
            .get("albums")
            .and_then(|v| v.get("items"))
            .and_then(|v| v.as_array())
        {
            for item in arr {
                let id = item
                    .get("id")
                    .and_then(|v| v.as_i64().map(|i| i.to_string()))
                    .or_else(|| {
                        item.get("id")
                            .and_then(|v| v.as_str().map(|s| s.to_string()))
                    });
                if let Some(id) = id {
                    let artist_id = item
                        .get("artist")
                        .and_then(|v| v.get("id"))
                        .and_then(|v| v.as_i64().map(|i| i.to_string()))
                        .or_else(|| {
                            item.get("artist_id")
                                .and_then(|v| v.as_str().map(|s| s.to_string()))
                        })
                        .unwrap_or_default();
                    let artist_name = item
                        .get("artist")
                        .and_then(|v| v.get("name"))
                        .and_then(|v| v.as_str())
                        .or_else(|| item.get("artist").and_then(|v| v.as_str()))
                        .unwrap_or("")
                        .to_string();
                    albums.push(QobuzAlbum {
                        id,
                        title: item
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        artist_id,
                        artist_name,
                        release_date: item
                            .get("release_date")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    });
                } else {
                    tracing::warn!(
                        "Skipping album item without parseable id: {:?}",
                        item.get("id")
                    );
                }
            }
        } else {
            tracing::warn!(
                "No albums array in favorites response. Keys: {:?}",
                body.as_object().map(|o| o.keys().collect::<Vec<_>>())
            );
        }
        if let Some(arr) = body
            .get("tracks")
            .and_then(|v| v.get("items"))
            .and_then(|v| v.as_array())
        {
            for item in arr {
                if let Some(id) = item.get("id").and_then(|v| v.as_i64()) {
                    tracks.push(TrackMetadata {
                        id: id.to_string(),
                        title: item
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        artist: item
                            .get("performer")
                            .and_then(|v| v.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        artist_id: item
                            .get("performer")
                            .and_then(|v| v.get("id"))
                            .and_then(|v| v.as_i64())
                            .map(|v| v.to_string()),
                        album: item
                            .get("album")
                            .and_then(|v| v.get("title"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        album_id: item
                            .get("album")
                            .and_then(|v| v.get("id"))
                            .and_then(|v| v.as_i64())
                            .map(|v| v.to_string()),
                        duration: item.get("duration").and_then(|v| v.as_u64()),
                        image_url: item
                            .get("album")
                            .and_then(|a| a.get("image"))
                            .and_then(|i| i.get("large"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                    });
                }
            }
        }

        tracing::info!(
            "Fetched Qobuz favorites: {} artists, {} albums, {} tracks",
            artists.len(),
            albums.len(),
            tracks.len()
        );
        Ok(QobuzFavorites {
            artists,
            albums,
            tracks,
        })
    }

    #[allow(dead_code)]
    pub async fn get_artist_with_albums(
        &self,
        auth: &crate::state::QobuzAuth,
        artist_id: &str,
    ) -> Result<QobuzArtistDetail> {
        let params = vec![("artist_id", artist_id), ("extra", "albums")];
        let body: serde_json::Value = self.get("/artist/get", &params, Some(auth)).await?;

        let artist_name = body
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let albums = body
            .get("albums")
            .and_then(|v| v.get("items"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        Some(QobuzAlbum {
                            id: item.get("id")?.as_i64()?.to_string(),
                            title: item.get("title")?.as_str()?.to_string(),
                            artist_id: artist_id.to_string(),
                            artist_name: artist_name.clone(),
                            release_date: item
                                .get("release_date")?
                                .as_str()
                                .unwrap_or("")
                                .to_string(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(QobuzArtistDetail { albums })
    }

    pub async fn search_tracks(
        &self,
        auth: &crate::state::QobuzAuth,
        query: &str,
        limit: usize,
    ) -> Result<Vec<TrackMetadata>> {
        let limit_str = limit.to_string();
        let params = vec![
            ("query", query),
            ("type", "tracks"),
            ("limit", &limit_str),
            ("offset", "0"),
        ];
        let body: serde_json::Value = self.get("/catalog/search", &params, Some(auth)).await?;

        let tracks = body
            .get("tracks")
            .and_then(|v| v.get("items"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        Some(TrackMetadata {
                            id: item.get("id")?.as_i64()?.to_string(),
                            title: item.get("title")?.as_str()?.to_string(),
                            artist: item.get("performer")?.get("name")?.as_str()?.to_string(),
                            artist_id: item
                                .get("performer")
                                .and_then(|v| v.get("id"))
                                .and_then(|v| v.as_i64())
                                .map(|v| v.to_string()),
                            album: item.get("album")?.get("title")?.as_str()?.to_string(),
                            album_id: item
                                .get("album")
                                .and_then(|v| v.get("id"))
                                .and_then(|v| v.as_i64())
                                .map(|v| v.to_string()),
                            duration: item.get("duration").and_then(|v| v.as_u64()),
                            image_url: item
                                .get("album")
                                .and_then(|a| a.get("image"))
                                .and_then(|i| i.get("large"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(tracks)
    }

    #[allow(dead_code)]
    pub async fn get_track_metadata(
        &self,
        auth: &crate::state::QobuzAuth,
        track_id: &str,
    ) -> Result<TrackMetadata> {
        let params = vec![("track_id", track_id)];
        let body: serde_json::Value = self.get("/track/get", &params, Some(auth)).await?;

        Ok(TrackMetadata {
            id: track_id.to_string(),
            title: body
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            artist: body
                .get("performer")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            artist_id: body
                .get("performer")
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_i64())
                .map(|v| v.to_string()),
            album: body
                .get("album")
                .and_then(|v| v.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            album_id: body
                .get("album")
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_i64())
                .map(|v| v.to_string()),
            duration: body.get("duration").and_then(|v| v.as_u64()),
            image_url: body
                .get("album")
                .and_then(|a| a.get("image"))
                .and_then(|i| i.get("large"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamInfo {
    pub url: String,
    pub format_id: u32,
    pub format: String,
    pub sampling_rate: Option<f64>,
    pub bit_depth: Option<u32>,
}

impl StreamInfo {
    fn format_name(format_id: u32) -> &'static str {
        match format_id {
            5 => "MP3 320",
            6 => "FLAC 16-bit",
            7 => "FLAC 24-bit",
            27 => "FLAC 24-bit >96kHz",
            _ => "Unknown",
        }
    }
}

impl QobuzClient {
    pub async fn get_file_url(
        &self,
        auth: &crate::state::QobuzAuth,
        track_id: &str,
        format_id: u32,
    ) -> Result<StreamInfo> {
        let format_id_str = format_id.to_string();
        let ts = current_timestamp();

        // Old-style Qobuz signature for getFileUrl
        let sig_payload = format!(
            "trackgetFileUrlformat_id{}intentstreamtrack_id{}{}{}",
            format_id_str, track_id, ts, auth.app_secret
        );
        let sig = format!("{:x}", md5::compute(sig_payload));

        let url = format!("{}/track/getFileUrl", self.base_url);
        let resp = self
            .client
            .get(&url)
            .query(&[
                ("app_id", auth.app_id.as_str()),
                ("user_auth_token", auth.user_auth_token.as_str()),
                ("track_id", track_id),
                ("format_id", &format_id_str),
                ("intent", "stream"),
                ("request_ts", &ts),
                ("request_sig", &sig),
            ])
            .send()
            .await
            .map_err(|e| AppError::Qobuz(e.to_string()))?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Qobuz(e.to_string()))?;

        if !status.is_success() {
            return Err(AppError::Qobuz(
                body.get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("getFileUrl failed")
                    .to_string(),
            ));
        }

        tracing::debug!(
            "Qobuz getFileUrl raw response for track {}: {}",
            track_id,
            serde_json::to_string_pretty(&body).unwrap_or_default()
        );

        let stream_url = body
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Qobuz("Missing stream URL".to_string()))?;

        if stream_url.is_empty() {
            return Err(AppError::Qobuz(format!(
                "Qobuz returned empty stream URL for track {} (format_id={})",
                track_id, format_id
            )));
        }

        let sampling_rate = body
            .get("sampling_rate")
            .and_then(|v| v.as_f64())
            .or_else(|| {
                body.get("sampling_rate")
                    .and_then(|v| v.as_i64())
                    .map(|i| i as f64)
            });

        let bit_depth = body
            .get("bit_depth")
            .and_then(|v| v.as_u64())
            .map(|d| d as u32);

        Ok(StreamInfo {
            url: stream_url.to_string(),
            format_id,
            format: StreamInfo::format_name(format_id).to_string(),
            sampling_rate,
            bit_depth,
        })
    }

    // Plain GET for open endpoints (catalog search, track get, etc.)
    async fn get(
        &self,
        endpoint: &str,
        params: &[(&str, &str)],
        auth: Option<&crate::state::QobuzAuth>,
    ) -> Result<serde_json::Value> {
        let url = format!("{}{}", self.base_url, endpoint);
        let mut req = self.client.get(&url);

        // Always add app_id if we have auth
        let mut all_params = params.to_vec();
        if let Some(a) = auth {
            all_params.push(("app_id", a.app_id.as_str()));
            all_params.push(("user_auth_token", a.user_auth_token.as_str()));
        }

        req = req.query(&all_params);

        let resp = req
            .send()
            .await
            .map_err(|e| AppError::Qobuz(e.to_string()))?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Qobuz(e.to_string()))?;

        if !status.is_success() {
            return Err(AppError::Qobuz(
                body.get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("API request failed")
                    .to_string(),
            ));
        }

        Ok(body)
    }

    // Signed GET for protected endpoints (favorites)
    async fn signed_get(
        &self,
        endpoint: &str,
        params: &[(&str, &str)],
        auth: &crate::state::QobuzAuth,
    ) -> Result<serde_json::Value> {
        let ts = current_timestamp();

        // New-style Qobuz signature
        let mut sig_params: Vec<(String, String)> = params
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        sig_params.push(("app_id".to_string(), auth.app_id.clone()));
        sig_params.push(("method".to_string(), "GET".to_string()));
        sig_params.push(("timestamp".to_string(), ts.clone()));
        sig_params.push(("user_auth_token".to_string(), auth.user_auth_token.clone()));
        sig_params.sort_by(|a, b| a.0.cmp(&b.0));

        let mut sig_string = format!("GET{}", endpoint);
        for (k, v) in &sig_params {
            sig_string.push_str(&format!("{}{}", k, v));
        }
        sig_string.push_str(&auth.app_secret);
        let sig = format!("{:x}", md5::compute(sig_string));

        let url = format!("{}{}", self.base_url, endpoint);
        let mut all_params: Vec<(&str, &str)> = params.to_vec();
        all_params.push(("app_id", auth.app_id.as_str()));
        all_params.push(("user_auth_token", auth.user_auth_token.as_str()));
        all_params.push(("request_ts", &ts));
        all_params.push(("request_sig", &sig));

        let resp = self
            .client
            .get(&url)
            .query(&all_params)
            .send()
            .await
            .map_err(|e| AppError::Qobuz(e.to_string()))?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Qobuz(e.to_string()))?;

        if !status.is_success() {
            return Err(AppError::Qobuz(
                body.get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("API request failed")
                    .to_string(),
            ));
        }

        Ok(body)
    }
}

#[derive(Debug, Clone)]
pub struct QobuzFavorites {
    pub artists: Vec<QobuzArtist>,
    pub albums: Vec<QobuzAlbum>,
    pub tracks: Vec<crate::state::TrackMetadata>,
}

#[derive(Debug, Clone)]
pub struct QobuzArtist {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct QobuzArtistDetail {
    pub albums: Vec<QobuzAlbum>,
}

#[derive(Debug, Clone)]
pub struct QobuzAlbum {
    pub id: String,
    pub title: String,
    pub artist_id: String,
    pub artist_name: String,
    pub release_date: String,
}

fn current_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
