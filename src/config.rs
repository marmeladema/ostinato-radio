use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub qobuz: QobuzConfig,
    pub lastfm: LastfmConfig,
    pub ai: AiConfig,
    pub radio: RadioConfig,
    pub wiim: WiimConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub public_base_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzConfig {
    pub preferred_format_id: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LastfmConfig {
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AiConfig {
    pub provider: String,
    pub model: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct RadioConfig {
    pub default_pool_ratios: PoolRatios,
    pub window_size: usize,
    pub window_refresh_threshold: usize,
    pub new_release_max_age_days: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PoolRatios {
    pub familiar: f32,
    pub new_release: f32,
    pub discovery: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WiimConfig {
    pub ip: Option<String>,
    pub poll_interval_seconds: u64,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let mut builder = config::Config::builder();

        if let Ok(path) = std::env::var("OSTINATO_CONFIG") {
            builder = builder.add_source(config::File::with_name(&path));
        } else if std::path::Path::new("config.toml").exists() {
            builder = builder.add_source(config::File::with_name("config.toml"));
        }

        let cfg = builder
            .add_source(config::Environment::with_prefix("OSTINATO").separator("__"))
            .build()?;

        Ok(cfg.try_deserialize()?)
    }

    pub fn socket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.server.host, self.server.port)
            .parse()
            .expect("Invalid socket address")
    }
}

// Manual implementation to read env vars with specific names for secrets
// QOBUZ_EMAIL, QOBUZ_PASSWORD handled in qobuz auth
// LASTFM_API_KEY
// CLOUDFLARE_ACCOUNT_ID, CLOUDFLARE_API_TOKEN
// ANTHROPIC_API_KEY
// OPENAI_API_KEY, OPENAI_BASE_URL
