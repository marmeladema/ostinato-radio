use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    Json,
    extract::{Query, State},
    response::Html,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::engine::profile::build_taste_profile;
use crate::errors::{AppError, Result};
use crate::providers::qobuz::bundle::QobuzCredentials;
use crate::state::AppState;

#[derive(Serialize)]
pub struct AuthStatus {
    pub authenticated: bool,
    pub has_password: bool,
    pub message: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub country_code: Option<String>,
    pub subscription: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Serialize)]
pub struct OauthStartResponse {
    pub oauth_url: String,
}

#[derive(Deserialize)]
pub struct OauthCallbackQuery {
    #[serde(rename = "code_autorisation")]
    pub code_autorisation: String,
}

pub async fn status(State(state): State<Arc<AppState>>) -> Result<Json<AuthStatus>> {
    let auth = state.qobuz_auth.read().await;
    let authenticated = !auth.user_auth_token.is_empty();
    let has_password = state.password_hash.is_some();

    Ok(Json(AuthStatus {
        authenticated,
        has_password,
        message: if authenticated {
            "Qobuz authenticated".to_string()
        } else {
            "Qobuz not authenticated".to_string()
        },
        display_name: auth.display_name.clone(),
        email: auth.email.clone(),
        country_code: auth.country_code.clone(),
        subscription: auth.subscription.clone(),
    }))
}

pub async fn start_oauth(State(state): State<Arc<AppState>>) -> Result<Json<OauthStartResponse>> {
    let auth = state.qobuz_auth.read().await;
    if auth.app_id.is_empty() || auth.private_key.is_empty() {
        return Err(AppError::Qobuz("Qobuz credentials not loaded".to_string()));
    }

    let redirect_url = format!("{}/auth/callback", state.config.server.public_base_url);
    let oauth_url = crate::providers::qobuz::auth::build_oauth_url(&auth.app_id, &redirect_url);

    Ok(Json(OauthStartResponse { oauth_url }))
}

pub async fn oauth_callback(
    State(state): State<Arc<AppState>>,
    Query(params): Query<OauthCallbackQuery>,
) -> Result<Html<String>> {
    let code = &params.code_autorisation;

    // Basic validation
    if code.len() < 4 || code.len() > 16 || !code.chars().all(|c| c.is_alphanumeric()) {
        return Ok(Html(
            "<html><body><h1>Invalid authorization code</h1></body></html>".to_string(),
        ));
    }

    // Load current credentials from state
    let creds = {
        let auth = state.qobuz_auth.read().await;
        if auth.app_id.is_empty() || auth.private_key.is_empty() {
            return Ok(Html(
                "<html><body><h1>Qobuz credentials not configured</h1></body></html>".to_string(),
            ));
        }
        QobuzCredentials {
            app_id: auth.app_id.clone(),
            private_key: auth.private_key.clone(),
            app_secret: vec![auth.app_secret.clone()],
        }
    };

    // Exchange code for token
    let (token, _user_id) =
        match crate::providers::qobuz::auth::exchange_code(&creds, code).await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("OAuth exchange failed: {}", e);
                return Ok(Html(format!(
                    "<html><body><h1>Authentication failed</h1><p>{}</p></body></html>",
                    e
                )));
            }
        };

    // Confirm session and get profile
    let profile =
        match crate::providers::qobuz::auth::confirm_session(&creds, &token).await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Session confirmation failed: {}", e);
                return Ok(Html(format!(
                    "<html><body><h1>Authentication failed</h1><p>{}</p></body></html>",
                    e
                )));
            }
        };

    // Store session in state
    {
        let mut auth = state.qobuz_auth.write().await;
        auth.user_auth_token = token;
        auth.user_id = Some(profile.user_id);
        auth.display_name = Some(profile.display_name);
        auth.email = Some(profile.email);
        auth.country_code = Some(profile.country_code);
        auth.subscription = profile.subscription;
        auth.obtained_at = Some(std::time::Instant::now());
    }

    tracing::info!("Qobuz OAuth successful for user {}", _user_id);

    // Build taste profile in background
    let state_clone = state.clone();
    tokio::spawn(async move {
        build_taste_profile_background(state_clone).await;
    });

    Ok(Html(
        r#"<html><body style="font-family:sans-serif;text-align:center;padding-top:40px">
            <h1>Authentication successful</h1>
            <p>You can close this tab.</p>
            <script>window.close();</script>
        </body></html>"#
        .to_string(),
    ))
}

async fn build_taste_profile_background(state: Arc<AppState>) {
    let auth = state.qobuz_auth.read().await;
    if auth.user_auth_token.is_empty() {
        tracing::warn!("Cannot build taste profile: no auth token");
        return;
    }

    match state.qobuz.get_user_favorites(&auth).await {
        Ok(favorites) => match build_taste_profile(favorites).await {
            Ok(profile) => {
                let mut tp = state.taste_profile.write().await;
                *tp = profile;
                tracing::info!("Taste profile built successfully");
            }
            Err(e) => {
                tracing::error!("Failed to build taste profile: {}", e);
            }
        },
        Err(e) => {
            tracing::error!("Failed to fetch favorites: {}", e);
        }
    }
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>> {
    let password_hash = state.password_hash.as_ref().ok_or(AppError::Unauthorized)?;

    let parsed_hash =
        PasswordHash::new(password_hash).map_err(|e| AppError::Internal(e.to_string()))?;

    Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed_hash)
        .map_err(|_| AppError::Unauthorized)?;

    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &Claims {
            sub: "ostinato".to_string(),
            iat: Utc::now().timestamp() as usize,
            exp: (Utc::now() + chrono::Duration::days(30)).timestamp() as usize,
        },
        &jsonwebtoken::EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(LoginResponse { token }))
}

pub async fn logout(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>> {
    // Clear Qobuz session (admin logout)
    let mut auth = state.qobuz_auth.write().await;
    auth.user_auth_token.clear();
    auth.user_id = None;
    auth.display_name = None;
    auth.email = None;
    auth.country_code = None;
    auth.subscription = None;
    auth.obtained_at = None;

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iat: usize,
    pub exp: usize,
}
