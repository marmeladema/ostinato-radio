use crate::errors::{AppError, Result};
use reqwest::Client;
use tracing::warn;

#[derive(Debug, Clone)]
pub struct LastfmClient {
    client: Client,
    api_key: String,
    base_url: String,
}

impl Default for LastfmClient {
    fn default() -> Self {
        Self::new("".to_string())
    }
}

impl LastfmClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: "https://ws.audioscrobbler.com/2.0".to_string(),
        }
    }

    #[allow(dead_code)]
    pub async fn get_similar_artists(&self, artist: &str) -> Result<Vec<SimilarArtist>> {
        let params = [
            ("method", "artist.getSimilar"),
            ("artist", artist),
            ("api_key", &self.api_key),
            ("format", "json"),
            ("limit", "20"),
        ];

        let resp = self
            .client
            .get(&self.base_url)
            .query(&params)
            .send()
            .await
            .map_err(|e| AppError::Lastfm(e.to_string()))?;

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Lastfm(e.to_string()))?;

        if data.get("error").is_some() {
            warn!("Last.fm artist.getSimilar error for {}", artist);
            return Ok(Vec::new());
        }

        let artists = data
            .get("similarartists")
            .and_then(|v| v.get("artist"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        Some(SimilarArtist {
                            name: item.get("name")?.as_str()?.to_string(),
                            match_score: item.get("match")?.as_str()?.parse().ok()?,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(artists)
    }

    #[allow(dead_code)]
    pub async fn get_top_tracks(&self, artist: &str) -> Result<Vec<String>> {
        let params = [
            ("method", "artist.getTopTracks"),
            ("artist", artist),
            ("api_key", &self.api_key),
            ("format", "json"),
            ("limit", "10"),
        ];

        let resp = self
            .client
            .get(&self.base_url)
            .query(&params)
            .send()
            .await
            .map_err(|e| AppError::Lastfm(e.to_string()))?;

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Lastfm(e.to_string()))?;

        if data.get("error").is_some() {
            return Ok(Vec::new());
        }

        let tracks = data
            .get("toptracks")
            .and_then(|v| v.get("track"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.get("name")?.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(tracks)
    }

    #[allow(dead_code)]
    pub async fn get_top_tracks_by_tag(&self, tag: &str) -> Result<Vec<TagTrack>> {
        let params = [
            ("method", "tag.getTopTracks"),
            ("tag", tag),
            ("api_key", &self.api_key),
            ("format", "json"),
            ("limit", "50"),
        ];

        let resp = self
            .client
            .get(&self.base_url)
            .query(&params)
            .send()
            .await
            .map_err(|e| AppError::Lastfm(e.to_string()))?;

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Lastfm(e.to_string()))?;

        if data.get("error").is_some() {
            return Ok(Vec::new());
        }

        let tracks = data
            .get("tracks")
            .and_then(|v| v.get("track"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        Some(TagTrack {
                            title: item.get("name")?.as_str()?.to_string(),
                            artist: item.get("artist")?.get("name")?.as_str()?.to_string(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(tracks)
    }

    #[allow(dead_code)]
    pub async fn get_similar_tags(&self, tag: &str) -> Result<Vec<String>> {
        let params = [
            ("method", "tag.getSimilar"),
            ("tag", tag),
            ("api_key", &self.api_key),
            ("format", "json"),
        ];

        let resp = self
            .client
            .get(&self.base_url)
            .query(&params)
            .send()
            .await
            .map_err(|e| AppError::Lastfm(e.to_string()))?;

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Lastfm(e.to_string()))?;

        if data.get("error").is_some() {
            return Ok(Vec::new());
        }

        let tags = data
            .get("similartags")
            .and_then(|v| v.get("tag"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.get("name")?.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(tags)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SimilarArtist {
    pub name: String,
    pub match_score: f32,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TagTrack {
    pub title: String,
    pub artist: String,
}
