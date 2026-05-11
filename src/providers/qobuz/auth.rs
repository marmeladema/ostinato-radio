use crate::errors::{AppError, Result};
use regex::Regex;
use reqwest::Client;
use tracing::info;

const BUNDLE_URL: &str = "https://play.qobuz.com/resources/0.5.3-b065/bundle.js";

#[derive(Debug, Clone)]
pub struct ScrapedCredentials {
    pub app_id: String,
    pub app_secret: String,
}

pub async fn scrape_bundle() -> Result<ScrapedCredentials> {
    let client = Client::new();
    let resp = client
        .get(BUNDLE_URL)
        .send()
        .await
        .map_err(|e| AppError::Qobuz(format!("Bundle fetch failed: {e}")))?;

    let body = resp
        .text()
        .await
        .map_err(|e| AppError::Qobuz(format!("Bundle read failed: {e}")))?;

    let app_id = extract_app_id(&body)?;
    let app_secret = extract_app_secret(&body, &app_id)?;

    info!("Scraped Qobuz app_id and app_secret from bundle");
    Ok(ScrapedCredentials { app_id, app_secret })
}

fn extract_app_id(body: &str) -> Result<String> {
    let re = Regex::new(r#"production:\{api:\{appId:"(\d+)""#)
        .or_else(|_| Regex::new(r#"appId:"(\d+)""#))
        .map_err(|_| AppError::Qobuz("Failed to compile app_id regex".to_string()))?;

    re.captures(body)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| AppError::Qobuz("app_id not found in bundle".to_string()))
}

fn extract_app_secret(body: &str, app_id: &str) -> Result<String> {
    let re = Regex::new(&format!(
        r#"{app_id}}}'?\],\w+\[0x\w+\]=0x\w+\}};var \w+="([a-f0-9]{{32}})""#
    ))
    .or_else(|_| Regex::new(r#"secrets?\.([a-f0-9]{32})"#))
    .or_else(|_| Regex::new(r#"([a-f0-9]{32})"#))
    .map_err(|_| AppError::Qobuz("Failed to compile app_secret regex".to_string()))?;

    re.captures(body)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| AppError::Qobuz("app_secret not found in bundle".to_string()))
}

impl ScrapedCredentials {
    pub fn from_env_or_scrape(scraped: Option<ScrapedCredentials>) -> Result<ScrapedCredentials> {
        if let Ok(id) = std::env::var("QOBUZ_APP_ID")
            && let Ok(secret) = std::env::var("QOBUZ_APP_SECRET")
            && !id.is_empty()
            && !secret.is_empty()
        {
            info!("Using Qobuz credentials from env");
            return Ok(ScrapedCredentials {
                app_id: id,
                app_secret: secret,
            });
        }
        scraped.ok_or_else(|| {
            AppError::Qobuz(
                "No Qobuz app credentials available (scrape failed and env vars not set)"
                    .to_string(),
            )
        })
    }
}
