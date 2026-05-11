use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub qobuz: QobuzConfig,
    pub ai: AiConfig,
    pub radio: RadioConfig,
    pub wiim: WiimConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub public_base_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct QobuzConfig {
    pub preferred_format_id: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub provider: String,
    pub model: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
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
#[serde(default)]
pub struct WiimConfig {
    pub ip: Option<String>,
    pub poll_interval_seconds: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            public_base_url: "http://localhost:8080".to_string(),
        }
    }
}

impl Default for QobuzConfig {
    fn default() -> Self {
        Self {
            preferred_format_id: 27,
        }
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "workers_ai".to_string(),
            model: "@cf/google/gemma-4-26b-a4b-it".to_string(),
        }
    }
}

impl Default for RadioConfig {
    fn default() -> Self {
        Self {
            default_pool_ratios: PoolRatios {
                familiar: 0.60,
                new_release: 0.25,
                discovery: 0.15,
            },
            window_size: 20,
            window_refresh_threshold: 5,
            new_release_max_age_days: 180,
        }
    }
}

impl Default for WiimConfig {
    fn default() -> Self {
        Self {
            ip: None,
            poll_interval_seconds: 5,
        }
    }
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
