use axum::{Json, extract::State};
use serde::Serialize;
use std::sync::Arc;

use crate::errors::Result;
use crate::state::AppState;

#[derive(Serialize)]
pub struct AuthStatus {
    pub authenticated: bool,
    pub message: String,
}

pub async fn status(State(state): State<Arc<AppState>>) -> Result<Json<AuthStatus>> {
    let auth = state.qobuz_auth.read().await;
    let authenticated = !auth.user_auth_token.is_empty();
    Ok(Json(AuthStatus {
        authenticated,
        message: if authenticated {
            "Qobuz authenticated".to_string()
        } else {
            "Qobuz not authenticated".to_string()
        },
    }))
}
