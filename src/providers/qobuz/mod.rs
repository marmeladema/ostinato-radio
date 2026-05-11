use crate::errors::{AppError, Result};
use crate::state::TrackMetadata;
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::Client;
use tracing::info;

pub mod auth;

#[derive(Debug, Clone)]
pub struct QobuzClient {
    client: Client,
    base_url: String,
}

impl Default for QobuzClient {
    fn default() -> Self {
        Self::new()
    }
}

impl QobuzClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://www.qobuz.com/api.json/0.2".to_string(),
        }
    }

    pub async fn login(
        &self,
        app_id: &str,
        app_secret: &str,
        email: &str,
        password: &str,
    ) -> Result<String> {
        let params = vec![("app_id", app_id), ("email", email), ("password", password)];
        let sig = sign_request("user", "login", &params, app_secret)?;
        let url = format!("{}/user/login", self.base_url);

        let resp = self
            .client
            .get(&url)
            .query(&params)
            .query(&[("request_ts", &sig.ts), ("request_sig", &sig.sig)])
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
                    .unwrap_or("login failed")
                    .to_string(),
            ));
        }

        let token = body
            .get("user_auth_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Qobuz("Missing user_auth_token".to_string()))?;

        info!("Qobuz login successful");
        Ok(token.to_string())
    }

    pub async fn get_user_favorites(
        &self,
        auth: &crate::state::QobuzAuth,
    ) -> Result<QobuzFavorites> {
        let params = vec![
            ("app_id", auth.app_id.as_str()),
            ("user_auth_token", auth.user_auth_token.as_str()),
        ];
        let sig = sign_request("favorite", "getUserFavorites", &params, &auth.app_secret)?;
        let url = format!("{}/favorite/getUserFavorites", self.base_url);

        let resp = self
            .client
            .get(&url)
            .query(&params)
            .query(&[("request_ts", &sig.ts), ("request_sig", &sig.sig)])
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
                    .unwrap_or("get favorites failed")
                    .to_string(),
            ));
        }

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
                if let Some(id) = item
                    .get("id")
                    .and_then(|v| v.as_i64())
                    .map(|v| v.to_string())
                {
                    albums.push(id);
                }
            }
        }
        if let Some(arr) = body
            .get("tracks")
            .and_then(|v| v.get("items"))
            .and_then(|v| v.as_array())
        {
            for item in arr {
                if let Some(id) = item
                    .get("id")
                    .and_then(|v| v.as_i64())
                    .map(|v| v.to_string())
                {
                    tracks.push(id);
                }
            }
        }

        Ok(QobuzFavorites {
            artists,
            albums,
            tracks,
        })
    }

    pub async fn get_artist_with_albums(
        &self,
        auth: &crate::state::QobuzAuth,
        artist_id: &str,
    ) -> Result<QobuzArtistDetail> {
        let params = vec![
            ("app_id", auth.app_id.as_str()),
            ("user_auth_token", auth.user_auth_token.as_str()),
            ("artist_id", artist_id),
            ("extra", "albums"),
        ];
        let sig = sign_request("artist", "get", &params, &auth.app_secret)?;
        let url = format!("{}/artist/get", self.base_url);

        let resp = self
            .client
            .get(&url)
            .query(&params)
            .query(&[("request_ts", &sig.ts), ("request_sig", &sig.sig)])
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
                    .unwrap_or("artist get failed")
                    .to_string(),
            ));
        }

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
            ("app_id", auth.app_id.as_str()),
            ("user_auth_token", auth.user_auth_token.as_str()),
            ("type", "tracks"),
            ("query", query),
            ("limit", &limit_str),
        ];
        let sig = sign_request("catalog", "search", &params, &auth.app_secret)?;
        let url = format!("{}/catalog/search", self.base_url);

        let resp = self
            .client
            .get(&url)
            .query(&params)
            .query(&[("request_ts", &sig.ts), ("request_sig", &sig.sig)])
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
                    .unwrap_or("search failed")
                    .to_string(),
            ));
        }

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
                            album: item.get("album")?.get("title")?.as_str()?.to_string(),
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

    pub async fn get_track_metadata(
        &self,
        auth: &crate::state::QobuzAuth,
        track_id: &str,
    ) -> Result<TrackMetadata> {
        let params = vec![
            ("app_id", auth.app_id.as_str()),
            ("user_auth_token", auth.user_auth_token.as_str()),
            ("track_id", track_id),
        ];
        let sig = sign_request("track", "get", &params, &auth.app_secret)?;
        let url = format!("{}/track/get", self.base_url);

        let resp = self
            .client
            .get(&url)
            .query(&params)
            .query(&[("request_ts", &sig.ts), ("request_sig", &sig.sig)])
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
                    .unwrap_or("track get failed")
                    .to_string(),
            ));
        }

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
            album: body
                .get("album")
                .and_then(|v| v.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            duration: body.get("duration").and_then(|v| v.as_u64()),
            image_url: body
                .get("album")
                .and_then(|a| a.get("image"))
                .and_then(|i| i.get("large"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        })
    }

    pub async fn get_file_url(
        &self,
        auth: &crate::state::QobuzAuth,
        track_id: &str,
        format_id: u32,
    ) -> Result<String> {
        let format_id_str = format_id.to_string();
        let params = vec![
            ("app_id", auth.app_id.as_str()),
            ("user_auth_token", auth.user_auth_token.as_str()),
            ("track_id", track_id),
            ("format_id", &format_id_str),
        ];
        let sig = sign_request("track", "getFileUrl", &params, &auth.app_secret)?;
        let url = format!("{}/track/getFileUrl", self.base_url);

        let resp = self
            .client
            .get(&url)
            .query(&params)
            .query(&[("request_ts", &sig.ts), ("request_sig", &sig.sig)])
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

        let url = body
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Qobuz("Missing stream URL".to_string()))?;

        Ok(url.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct QobuzFavorites {
    pub artists: Vec<QobuzArtist>,
    pub albums: Vec<String>,
    pub tracks: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct QobuzArtist {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct QobuzArtistDetail {
    pub albums: Vec<QobuzAlbum>,
}

#[derive(Debug, Clone)]
pub struct QobuzAlbum {
    pub id: String,
    pub title: String,
    pub release_date: String,
}

struct Signature {
    ts: String,
    sig: String,
}

fn sign_request(
    object: &str,
    method: &str,
    params: &[(&str, &str)],
    app_secret: &str,
) -> Result<Signature> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::Internal("Clock error".to_string()))?
        .as_secs()
        .to_string();

    let mut keys: Vec<&str> = params.iter().map(|(k, _)| *k).collect();
    keys.sort_unstable();

    let mut payload = String::new();
    payload.push_str(object);
    payload.push_str(method);

    for k in keys {
        if k == "app_id" || k == "user_auth_token" {
            continue;
        }
        let v = params
            .iter()
            .find(|(key, _)| key == &k)
            .map(|(_, v)| *v)
            .unwrap_or("");
        payload.push_str(k);
        payload.push_str(v);
    }
    payload.push_str(&ts);
    payload.push_str(app_secret);

    let digest = md5::compute(payload);
    let sig = format!("{:x}", digest);

    Ok(Signature { ts, sig })
}
