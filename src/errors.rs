use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;
use tracing::error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Qobuz API error: {0}")]
    Qobuz(String),
    #[error("Last.fm API error: {0}")]
    Lastfm(String),
    #[error("AI provider error: {0}")]
    Ai(String),
    #[error("LinkPlay error: {0}")]
    LinkPlay(String),
    #[allow(dead_code)]
    #[error("Authentication required")]
    Unauthorized,
    #[error("Resource not found")]
    NotFound,
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Qobuz(msg)
            | AppError::Lastfm(msg)
            | AppError::Ai(msg)
            | AppError::LinkPlay(msg)
            | AppError::Internal(msg)
            | AppError::BadRequest(msg) => {
                if matches!(
                    self,
                    AppError::Internal(_)
                        | AppError::Ai(_)
                        | AppError::Qobuz(_)
                        | AppError::Lastfm(_)
                        | AppError::LinkPlay(_)
                ) {
                    error!(error = %self, "Request error");
                }
                let code = match self {
                    AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
                    AppError::NotFound => StatusCode::NOT_FOUND,
                    AppError::Unauthorized => StatusCode::UNAUTHORIZED,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                };
                (code, msg.clone())
            }
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            AppError::NotFound => (StatusCode::NOT_FOUND, "Not found".to_string()),
            AppError::Other(e) => {
                error!(error = %e, "Internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        let body = Json(json!({ "error": message }));
        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
