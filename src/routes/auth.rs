use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{Json, extract::State};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::errors::{AppError, Result};
use crate::state::AppState;

#[derive(Serialize)]
pub struct AuthStatus {
    pub authenticated: bool,
    pub has_password: bool,
    pub message: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
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
    }))
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

pub async fn logout() -> Result<Json<serde_json::Value>> {
    // Stateless logout — client discards token
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iat: usize,
    pub exp: usize,
}
