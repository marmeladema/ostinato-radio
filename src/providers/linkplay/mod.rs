use crate::errors::{AppError, Result};
use reqwest::Client;
use tracing::{debug, info};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct LinkplayClient {
    client: Client,
    base_url: Option<String>,
    poll_interval_secs: u64,
}

impl Default for LinkplayClient {
    fn default() -> Self {
        Self::new(None, 5)
    }
}

impl LinkplayClient {
    pub fn new(ip: Option<String>, poll_interval_secs: u64) -> Self {
        let base_url = ip.map(|ip| format!("http://{}/httpapi.asp", ip));
        Self {
            client: Client::new(),
            base_url,
            poll_interval_secs,
        }
    }

    #[allow(dead_code)]
    pub fn set_ip(&mut self, ip: &str) {
        self.base_url = Some(format!("http://{}/httpapi.asp", ip));
    }

    pub async fn play_url(&self, url: &str) -> Result<()> {
        let _base = self
            .base_url
            .as_ref()
            .ok_or_else(|| AppError::LinkPlay("WiiM IP not configured".to_string()))?;
        let command = format!("setPlayerCmd:play:{}", url);
        self.send_command(&command).await
    }

    pub async fn pause(&self) -> Result<()> {
        self.send_command("setPlayerCmd:pause").await
    }

    pub async fn resume(&self) -> Result<()> {
        self.send_command("setPlayerCmd:resume").await
    }

    pub async fn stop(&self) -> Result<()> {
        self.send_command("setPlayerCmd:stop").await
    }

    pub async fn next(&self) -> Result<()> {
        self.send_command("setPlayerCmd:next").await
    }

    pub async fn prev(&self) -> Result<()> {
        self.send_command("setPlayerCmd:prev").await
    }

    pub async fn set_volume(&self, vol: u8) -> Result<()> {
        let command = format!("setPlayerCmd:vol:{}", vol.min(100));
        self.send_command(&command).await
    }

    #[allow(dead_code)]
    pub async fn get_player_status(&self) -> Result<PlayerStatus> {
        let base = self
            .base_url
            .as_ref()
            .ok_or_else(|| AppError::LinkPlay("WiiM IP not configured".to_string()))?;
        let resp = self
            .client
            .get(base)
            .query(&[("command", "getPlayerStatus")])
            .send()
            .await
            .map_err(|e| AppError::LinkPlay(format!("Status request failed: {e}")))?;

        let text = resp
            .text()
            .await
            .map_err(|e| AppError::LinkPlay(e.to_string()))?;
        debug!("LinkPlay status: {}", text);

        // LinkPlay returns a plain text or JSON-like string depending on firmware.
        // We'll attempt a very loose parse.
        let mut status = PlayerStatus::default();

        for part in text.split(';') {
            let kv: Vec<&str> = part.splitn(2, ':').collect();
            if kv.len() != 2 {
                continue;
            }
            match kv[0].trim() {
                "status" => status.state = kv[1].trim().to_string(),
                "curpos" => status.position_ms = kv[1].trim().parse().unwrap_or(0),
                "offset_pts" => status.offset_ms = kv[1].trim().parse().unwrap_or(0),
                _ => {}
            }
        }

        Ok(status)
    }

    async fn send_command(&self, command: &str) -> Result<()> {
        let base = self
            .base_url
            .as_ref()
            .ok_or_else(|| AppError::LinkPlay("WiiM IP not configured".to_string()))?;
        let url = format!("{}?command={}", base, urlencoding::encode(command));
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::LinkPlay(format!("Command failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::LinkPlay(format!(
                "LinkPlay returned status {}",
                resp.status()
            )));
        }

        info!("LinkPlay command sent: {}", command);
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct PlayerStatus {
    pub state: String,
    pub position_ms: u64,
    pub offset_ms: u64,
}
