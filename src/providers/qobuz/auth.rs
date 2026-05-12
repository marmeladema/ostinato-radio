use crate::errors::{AppError, Result};
use crate::providers::qobuz::bundle::QobuzCredentials;
use tracing::{info, warn};

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 Firefox/110.0";

#[derive(Debug, Clone)]
pub struct QobuzUserProfile {
    pub user_id: String,
    pub display_name: String,
    pub email: String,
    pub country_code: String,
    pub subscription: Option<String>,
}

/// Build the Qobuz OAuth authorization URL.
pub fn build_oauth_url(app_id: &str, redirect_url: &str) -> String {
    let encoded = urlencoding::encode(redirect_url);
    format!(
        "https://www.qobuz.com/signin/oauth?ext_app_id={}&redirect_url={}",
        app_id, encoded
    )
}

/// Exchange a short-lived authorization code for a persistent user auth token.
pub async fn exchange_code(creds: &QobuzCredentials, code: &str) -> Result<(String, String)> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let url = format!(
        "https://www.qobuz.com/api.json/0.2/oauth/callback?code={}&private_key={}",
        urlencoding::encode(code),
        urlencoding::encode(&creds.private_key)
    );

    let resp = client
        .get(&url)
        .header("X-App-Id", &creds.app_id)
        .header("Origin", "https://play.qobuz.com")
        .header("Referer", "https://play.qobuz.com/")
        .send()
        .await
        .map_err(|e| AppError::Qobuz(format!("oauth exchange request failed: {}", e)))?;

    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Qobuz(format!("oauth exchange parse failed: {}", e)))?;

    if !status.is_success() {
        let msg = body
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("oauth exchange failed");
        return Err(AppError::Qobuz(msg.to_string()));
    }

    let token = body
        .get("token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Qobuz("Missing token in oauth response".to_string()))?;
    let user_id = body
        .get("user_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::Qobuz("Missing user_id in oauth response".to_string()))?;

    Ok((token.to_string(), user_id.to_string()))
}

/// Confirm the session by calling user/login and retrieve the user profile.
pub async fn confirm_session(
    creds: &QobuzCredentials,
    auth_token: &str,
) -> Result<QobuzUserProfile> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let url = "https://www.qobuz.com/api.json/0.2/user/login";

    let resp = client
        .post(url)
        .header("X-App-Id", &creds.app_id)
        .header("X-User-Auth-Token", auth_token)
        .header("Origin", "https://play.qobuz.com")
        .header("Referer", "https://play.qobuz.com/")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("extra=partner")
        .send()
        .await
        .map_err(|e| AppError::Qobuz(format!("user/login confirm request failed: {}", e)))?;

    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Qobuz(format!("user/login confirm parse failed: {}", e)))?;

    if !status.is_success() {
        let msg = body
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("session confirmation failed");
        return Err(AppError::Qobuz(msg.to_string()));
    }

    let user_id = body
        .get("id")
        .and_then(|v| v.as_str())
        .or_else(|| body.get("user_id").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    let display_name = format!(
        "{} {}",
        body.get("firstname").and_then(|v| v.as_str()).unwrap_or(""),
        body.get("lastname").and_then(|v| v.as_str()).unwrap_or("")
    )
    .trim()
    .to_string();

    let email = body
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let country_code = body
        .get("country_code")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let subscription = body
        .get("credential")
        .and_then(|v| v.get("label"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(QobuzUserProfile {
        user_id,
        display_name,
        email,
        country_code,
        subscription,
    })
}

/// Load Qobuz credentials with the following precedence:
/// 1. Environment variables (`QOBUZ_APP_ID`, `QOBUZ_PRIVATE_KEY`, `QOBUZ_APP_SECRET`)
/// 2. Scraped from the Qobuz web player bundle
/// 3. Fallback values from config (TOML / env)
pub async fn load_credentials(config: &crate::config::Config) -> Result<QobuzCredentials> {
    // 1. Environment variables (highest priority)
    if let (Ok(id), Ok(key), Ok(secret)) = (
        std::env::var("QOBUZ_APP_ID"),
        std::env::var("QOBUZ_PRIVATE_KEY"),
        std::env::var("QOBUZ_APP_SECRET"),
    ) && !id.is_empty()
        && !key.is_empty()
        && !secret.is_empty()
    {
        info!("Using Qobuz credentials from environment variables");
        return Ok(QobuzCredentials {
            app_id: id,
            private_key: key,
            app_secret: vec![secret],
        });
    }

    // 2. Scrape from bundle
    match crate::providers::qobuz::bundle::scrape_credentials().await {
        Ok(creds) => {
            info!("Using scraped Qobuz credentials");
            return Ok(creds);
        }
        Err(e) => {
            warn!("Failed to scrape Qobuz credentials: {}", e);
        }
    }

    // 3. Config fallback
    let qcfg = &config.qobuz;
    if !qcfg.fallback_app_id.is_empty()
        && !qcfg.fallback_private_key.is_empty()
        && !qcfg.fallback_app_secret.is_empty()
    {
        info!("Using fallback Qobuz credentials from config");
        return Ok(QobuzCredentials {
            app_id: qcfg.fallback_app_id.clone(),
            private_key: qcfg.fallback_private_key.clone(),
            app_secret: vec![qcfg.fallback_app_secret.clone()],
        });
    }

    Err(AppError::Qobuz(
        "No Qobuz credentials available (scrape failed and no fallback configured)".to_string(),
    ))
}
