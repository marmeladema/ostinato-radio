mod config;
mod engine;
mod errors;
mod middleware;
mod providers;
mod routes;
mod state;

use sha2::{Digest, Sha256};
use std::sync::Arc;
use tracing::info;

use axum::{
    Router,
    middleware::from_fn_with_state,
    routing::{get, post},
};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::engine::profile::build_taste_profile;
use crate::providers::ai::DeterministicRanker;
use crate::providers::ai::anthropic::AnthropicProvider;
use crate::providers::ai::openai_compat::OpenAICompatProvider;
use crate::providers::ai::workers_ai::WorkersAIProvider;
use crate::providers::lastfm::LastfmClient;
use crate::providers::linkplay::LinkplayClient;
use crate::providers::qobuz::QobuzClient;
use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<(), errors::AppError> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting ostinato-radio...");

    let config = Config::load()?;
    info!("Configuration loaded");

    // Read optional password hash for remote access protection
    let password_hash = std::env::var("APP_PASSWORD_HASH").ok();
    let password_hash = password_hash.filter(|h| !h.is_empty());

    let jwt_secret = if let Some(ref hash) = password_hash {
        let mut hasher = Sha256::new();
        hasher.update(b"ostinato-jwt:");
        hasher.update(hash.as_bytes());
        format!("{:x}", hasher.finalize())
    } else {
        "ostinato-radio-default-unused".to_string()
    };

    if password_hash.is_some() {
        info!("Password protection enabled (APP_PASSWORD_HASH is set)");
    } else {
        info!("Password protection disabled — set APP_PASSWORD_HASH to enable auth");
    }

    let qobuz = QobuzClient::new();

    let lastfm_api_key = std::env::var("LASTFM_API_KEY").unwrap_or_default();
    let lastfm = LastfmClient::new(lastfm_api_key);

    let linkplay = LinkplayClient::new(config.wiim.ip.clone(), config.wiim.poll_interval_seconds);

    // AI provider selection
    let ai: Box<dyn crate::providers::ai::MusicAI + Send + Sync> = match config.ai.provider.as_str()
    {
        "workers_ai" => {
            let account_id =
                std::env::var("CLOUDFLARE_ACCOUNT_ID").unwrap_or_else(|_| "".to_string());
            let api_token =
                std::env::var("CLOUDFLARE_API_TOKEN").unwrap_or_else(|_| "".to_string());
            if account_id.is_empty() || api_token.is_empty() {
                info!("Workers AI credentials not set, using deterministic fallback");
                Box::new(DeterministicRanker)
            } else {
                info!("Using Workers AI provider");
                Box::new(WorkersAIProvider::new(
                    account_id,
                    api_token,
                    config.ai.model.clone(),
                ))
            }
        }
        "anthropic" => {
            let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_else(|_| "".to_string());
            if api_key.is_empty() {
                info!("Anthropic API key not set, using deterministic fallback");
                Box::new(DeterministicRanker)
            } else {
                info!("Using Anthropic AI provider");
                Box::new(AnthropicProvider::new(api_key, config.ai.model.clone()))
            }
        }
        "openai_compat" => {
            let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "".to_string());
            let base_url = std::env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
            if api_key.is_empty() {
                info!("OpenAI API key not set, using deterministic fallback");
                Box::new(DeterministicRanker)
            } else {
                info!("Using OpenAI-compatible provider");
                Box::new(OpenAICompatProvider::new(
                    api_key,
                    base_url,
                    config.ai.model.clone(),
                ))
            }
        }
        _ => {
            info!(
                "Unknown AI provider '{}', using deterministic fallback",
                config.ai.provider
            );
            Box::new(DeterministicRanker)
        }
    };

    let state = AppState::new(
        config.clone(),
        qobuz.clone(),
        lastfm,
        ai,
        linkplay,
        password_hash,
        jwt_secret,
    );

    // Qobuz credentials (cold boot — OAuth required unless warm-boot token provided)
    info!("Loading Qobuz credentials...");
    let creds = crate::providers::qobuz::auth::load_credentials(&config).await?;

    {
        let mut auth = state.qobuz_auth.write().await;
        auth.app_id = creds.app_id;
        auth.private_key = creds.private_key;
        auth.app_secret = creds.app_secret.into_iter().next().unwrap_or_default();
    }

    // Optional: warm boot if a token is already configured (dev convenience)
    if let Ok(token) = std::env::var("QOBUZ_USER_AUTH_TOKEN")
        && !token.is_empty()
    {
        info!("QOBUZ_USER_AUTH_TOKEN set — attempting warm boot");
        let confirm_creds = {
            let auth = state.qobuz_auth.read().await;
            crate::providers::qobuz::bundle::QobuzCredentials {
                app_id: auth.app_id.clone(),
                private_key: auth.private_key.clone(),
                app_secret: vec![auth.app_secret.clone()],
            }
        };
        match crate::providers::qobuz::auth::confirm_session(&confirm_creds, &token).await {
            Ok(profile) => {
                let mut auth = state.qobuz_auth.write().await;
                auth.user_auth_token = token;
                auth.user_id = Some(profile.user_id);
                auth.display_name = Some(profile.display_name);
                auth.email = Some(profile.email);
                auth.country_code = Some(profile.country_code);
                auth.subscription = profile.subscription;
                auth.obtained_at = Some(std::time::Instant::now());
            }
            Err(e) => {
                tracing::warn!("Warm boot failed: {}. Proceeding with cold boot.", e);
            }
        }
    }

    let token_exists = !state.qobuz_auth.read().await.user_auth_token.is_empty();
    if token_exists {
        info!("Qobuz session active — building taste profile");
        let favorites = qobuz
            .get_user_favorites(&*state.qobuz_auth.read().await)
            .await?;
        let profile = build_taste_profile(favorites).await?;
        {
            let mut tp = state.taste_profile.write().await;
            *tp = profile;
        }
        info!("Taste profile built");
    } else {
        info!("Qobuz not authenticated — visit /auth/start to authorize");
    }

    // Start cache pruning background task
    let prune_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            let before_releases = prune_state.new_releases.len();
            prune_state.new_releases.retain(|_, v| !v.is_expired());
            let after_releases = prune_state.new_releases.len();

            let before_similar = prune_state.similar_artists.len();
            prune_state.similar_artists.retain(|_, v| !v.is_expired());
            let after_similar = prune_state.similar_artists.len();

            info!(
                "Cache pruning: releases {}→{}, similar {}→{}",
                before_releases, after_releases, before_similar, after_similar
            );
        }
    });

    let app = build_router(state.clone(), &config);

    let addr = config.socket_addr();
    info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| errors::AppError::Internal(e.to_string()))?;
    axum::serve(listener, app)
        .await
        .map_err(|e| errors::AppError::Internal(e.to_string()))?;

    Ok(())
}

fn build_router(state: Arc<AppState>, _config: &Config) -> Router {
    // Public routes: no auth required
    let public = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/auth/status", get(routes::auth::status))
        .route("/auth/start", get(routes::auth::start_oauth))
        .route("/auth/callback", get(routes::auth::oauth_callback))
        .route("/auth/login", post(routes::auth::login))
        .route("/auth/logout", post(routes::auth::logout))
        .with_state(state.clone());

    // Protected routes: auth required when password hash is set
    let protected = Router::new()
        .route("/radio/start", post(routes::radio::start_radio))
        .route("/radio/{session_id}", get(routes::radio::session_status))
        .route("/radio/{session_id}/next", post(routes::radio::next_track))
        .route("/stream/{track_id}", get(routes::playback::stream_redirect))
        .route(
            "/playback/wiim/{session_id}",
            get(routes::playback::wiim_m3u),
        )
        .route("/playback/control", get(routes::playback::wiim_control))
        .route(
            "/feedback/{session_id}",
            post(routes::feedback::submit_feedback),
        )
        .layer(from_fn_with_state(
            state.clone(),
            middleware::auth::auth_layer,
        ))
        .with_state(state);

    let api = public
        .merge(protected)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    // Serve frontend static files if they exist
    let static_files = ServeDir::new("frontend/dist").precompressed_gzip();

    Router::new().merge(api).fallback_service(static_files)
}
