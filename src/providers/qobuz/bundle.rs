use base64::{Engine, engine::general_purpose::STANDARD};
use regex::Regex;
use std::collections::HashSet;

use crate::errors::{AppError, Result};

const PLAYER_URL: &str = "https://play.qobuz.com";
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 Firefox/110.0";

#[derive(Debug, Clone)]
pub struct QobuzCredentials {
    pub app_id: String,
    pub private_key: String,
    pub app_secret: Vec<String>, // Multiple candidates for validation
}

/// Scrape Qobuz credentials from the web player bundle.
/// Returns static credentials that can be used to initiate the OAuth flow.
/// The `app_secret` list contains multiple candidates which must be validated later.
pub async fn scrape_credentials() -> Result<QobuzCredentials> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let bundle = fetch_bundle(&client)
        .await
        .map_err(|e| AppError::Qobuz(format!("Failed to fetch bundle: {}", e)))?;

    let app_id = find_app_id(&bundle)
        .ok_or_else(|| AppError::Qobuz("Could not find app_id in bundle".to_string()))?;

    let private_key = find_private_key(&bundle)
        .ok_or_else(|| AppError::Qobuz("Could not find private_key in bundle".to_string()))?;

    let app_secret = find_app_secrets(&bundle);
    if app_secret.is_empty() {
        return Err(AppError::Qobuz(
            "Could not find app_secret candidates in bundle".to_string(),
        ));
    }

    Ok(QobuzCredentials {
        app_id,
        private_key,
        app_secret,
    })
}

async fn fetch_bundle(client: &reqwest::Client) -> Result<String> {
    // Step 1: Fetch login page to get bundle path
    let login_page = client
        .get(format!("{}/login", PLAYER_URL))
        .send()
        .await
        .map_err(|e| AppError::Qobuz(format!("Login page fetch failed: {}", e)))?
        .text()
        .await
        .map_err(|e| AppError::Qobuz(format!("Login page read failed: {}", e)))?;

    // Step 2: Parse bundle path
    let re = Regex::new(r#"<script src="(/resources/\d+\.\d+\.\d+-[a-z]\d{3}/bundle\.js)"#)
        .map_err(|_| AppError::Internal("Bundle regex compilation failed".to_string()))?;

    let caps = re.captures(&login_page).ok_or_else(|| {
        AppError::Qobuz("Could not find bundle.js path in login page".to_string())
    })?;

    let bundle_path = caps
        .get(1)
        .ok_or_else(|| AppError::Qobuz("Could not extract bundle.js path".to_string()))?
        .as_str();

    // Step 3: Fetch bundle content
    let bundle_url = format!("{}{}", PLAYER_URL, bundle_path);
    let bundle = client
        .get(&bundle_url)
        .send()
        .await
        .map_err(|e| AppError::Qobuz(format!("Bundle fetch failed: {}", e)))?
        .text()
        .await
        .map_err(|e| AppError::Qobuz(format!("Bundle read failed: {}", e)))?;

    Ok(bundle)
}

fn find_app_id(bundle: &str) -> Option<String> {
    // Primary: the exact production-appId structure observed in Qobuz bundles
    let primary = Regex::new(r#"production:\{api:\{appId:"([^"]*)""#).ok()?;
    if let Some(cap) = primary.captures(bundle)
        && let Some(m) = cap.get(1)
    {
        let id = m.as_str().to_string();
        if !id.is_empty() {
            return Some(id);
        }
    }

    // Fallbacks: generic patterns suggested by the OAuth spec
    let patterns = [
        Regex::new(r#"ext_app_id["\s:=,]+(\d{7,10})"#).ok()?,
        Regex::new(r#"app_id["\s:=,]+["'](\d{7,10})["']"#).ok()?,
        Regex::new(r#""appId"\s*:\s*"(\d{7,10})""#).ok()?,
        Regex::new(r#"appId\s*=\s*["'](\d{7,10})["']"#).ok()?,
    ];

    for re in &patterns {
        if let Some(cap) = re.captures(bundle)
            && let Some(m) = cap.get(1)
        {
            let id = m.as_str().to_string();
            if id.len() >= 7 && id.len() <= 10 && id.parse::<u64>().is_ok() {
                return Some(id);
            }
        }
    }
    None
}

fn find_private_key(bundle: &str) -> Option<String> {
    let patterns = [
        Regex::new(r#"private_key["\s:=,]+["']([A-Za-z0-9]{8,20})["']"#).ok()?,
        Regex::new(r#"privateKey["\s:=,]+["']([A-Za-z0-9]{8,20})["']"#).ok()?,
        Regex::new(r#""privateKey"\s*:\s*"([A-Za-z0-9]{8,20})""#).ok()?,
        Regex::new(r#"privateKey\s*=\s*["']([A-Za-z0-9]{8,20})["']"#).ok()?,
    ];

    for re in &patterns {
        if let Some(cap) = re.captures(bundle)
            && let Some(m) = cap.get(1)
        {
            let key = m.as_str().to_string();
            if key.len() >= 8 && key.len() <= 20 {
                return Some(key);
            }
        }
    }
    None
}

fn find_app_secrets(bundle: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();

    // Primary: seed/timezone/info/extras extraction (proven method)
    if let Ok(seed_re) = Regex::new(
        r#"\):[a-z]\.initialSeed\("(?P<seed>.*?)",window\.utimezone\.(?P<timezone>[a-z]+)\)"#,
    ) && let Some(seed_caps) = seed_re.captures(bundle)
    {
        let seed = seed_caps.name("seed").map(|m| m.as_str()).unwrap_or("");
        let timezone = seed_caps.name("timezone").map(|m| m.as_str()).unwrap_or("");
        let title_case = capitalize_first_letter(timezone);

        let info_extras_pattern = format!(r#"name:"[^"]*/{}"[^}}]*"#, regex::escape(&title_case));
        if let Ok(info_extras_re) = Regex::new(&info_extras_pattern)
            && let Some(info_extras_caps) = info_extras_re.captures(bundle)
        {
            let timezone_obj = info_extras_caps.get(0).map_or("", |m| m.as_str());

            let info = Regex::new(r#"info:"([^"]*)""#)
                .ok()
                .and_then(|re| re.captures(timezone_obj))
                .and_then(|c| c.get(1))
                .map_or("", |m| m.as_str());

            let extras = Regex::new(r#"extras:"([^"]*)""#)
                .ok()
                .and_then(|re| re.captures(timezone_obj))
                .and_then(|c| c.get(1))
                .map_or("", |m| m.as_str());

            let b64 = format!("{}{}{}", seed, info, extras);
            if b64.len() > 44
                && let Ok(decoded) = STANDARD.decode(&b64[..b64.len() - 44])
                && let Ok(secret) = String::from_utf8(decoded)
                && secret.len() == 32
                && secret.chars().all(|c| c.is_ascii_hexdigit())
                && seen.insert(secret.clone())
            {
                candidates.push(secret);
                return candidates; // Old method yields exactly one correct secret
            }
        }
    }

    // Fallback 1: generic base64-encoded 32-byte secrets (44 chars)
    if candidates.is_empty()
        && let Ok(re) = Regex::new(r#"["']([A-Za-z0-9+/]{44}={0,2})["']"#)
    {
        for cap in re.captures_iter(bundle) {
            if let Some(m) = cap.get(1) {
                let s = m.as_str();
                if let Ok(decoded) = STANDARD.decode(s)
                    && decoded.len() == 32
                {
                    let hex = hex::encode(decoded);
                    if seen.insert(hex.clone()) {
                        candidates.push(hex);
                    }
                }
            }
        }
    }

    // Fallback 2: raw 32-char hex strings
    if candidates.is_empty()
        && let Ok(re) = Regex::new(r#"\b([a-f0-9]{32})\b"#)
    {
        for cap in re.captures_iter(bundle) {
            if let Some(m) = cap.get(1) {
                let s = m.as_str().to_string();
                if seen.insert(s.clone()) {
                    candidates.push(s);
                }
            }
        }
    }

    candidates
}

fn capitalize_first_letter(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_fixture() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/providers/qobuz/bundles/bundle-8.1.0-b019.js");
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path.display(), e))
    }

    #[test]
    fn test_find_app_id() {
        let bundle = load_fixture();
        let id = find_app_id(&bundle).expect("Expected app_id in fixture");
        assert!(!id.is_empty(), "app_id should not be empty");
        assert!(
            id.parse::<u64>().is_ok(),
            "app_id should be numeric: got {}",
            id
        );
    }

    #[test]
    fn test_find_private_key() {
        let bundle = load_fixture();
        let key = find_private_key(&bundle).expect("Expected private_key in fixture");
        assert!(!key.is_empty(), "private_key should not be empty");
        assert!(
            key.chars().all(|c| c.is_alphanumeric()),
            "private_key should be alphanumeric: got {}",
            key
        );
    }

    #[test]
    fn test_find_app_secrets() {
        let bundle = load_fixture();
        let secrets = find_app_secrets(&bundle);
        assert!(
            !secrets.is_empty(),
            "Expected at least one app_secret candidate"
        );
        for s in &secrets {
            assert_eq!(s.len(), 32, "app_secret should be 32 chars: got {}", s);
            assert!(
                s.chars().all(|c| c.is_ascii_hexdigit()),
                "app_secret should be hex: got {}",
                s
            );
        }
    }

    #[test]
    fn test_scrape_credentials_from_fixture() {
        let bundle = load_fixture();

        let app_id = find_app_id(&bundle).expect("app_id");
        let private_key = find_private_key(&bundle).expect("private_key");
        let app_secret = find_app_secrets(&bundle);

        assert!(!app_id.is_empty());
        assert!(!private_key.is_empty());
        assert!(!app_secret.is_empty());

        // Log the extracted values to make test output useful for debugging
        println!("app_id       = {}", app_id);
        println!("private_key  = {}", private_key);
        println!("app_secrets  = {:?}", app_secret);
    }
}
