use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

use crate::routes::auth::Claims;
use crate::state::AppState;

pub async fn auth_layer(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    // If no password hash is configured, allow all requests
    if state.password_hash.is_none() {
        return next.run(request).await;
    }

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => &header[7..],
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                axum::Json(
                    serde_json::json!({ "error": "Missing or invalid Authorization header" }),
                ),
            )
                .into_response();
        }
    };

    let validation = jsonwebtoken::Validation::default();
    let decoding_key = jsonwebtoken::DecodingKey::from_secret(state.jwt_secret.as_bytes());

    match jsonwebtoken::decode::<Claims>(token, &decoding_key, &validation) {
        Ok(_) => next.run(request).await,
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({ "error": "Invalid or expired token" })),
        )
            .into_response(),
    }
}
