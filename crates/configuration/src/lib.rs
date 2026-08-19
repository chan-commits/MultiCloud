use config::{Config, Environment, File};
use serde::Deserialize;
use std::path::Path;
use thiserror::Error;

#[derive(Clone, Debug, Deserialize)]
pub struct Settings {
    pub environment: String,
    pub http: HttpSettings,
    pub database: DatabaseSettings,
    pub redis: RedisSettings,
    #[serde(default)]
    pub provider: ProviderSettings,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HttpSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DatabaseSettings {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RedisSettings {
    pub url: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ProviderSettings {
    #[serde(default)]
    pub credential_master_key: String,
    #[serde(default = "default_key_version")]
    pub credential_key_version: i32,
    #[serde(default = "default_cloudflare_base_url")]
    pub cloudflare_base_url: String,
    #[serde(default = "default_vultr_base_url")]
    pub vultr_base_url: String,
}

const fn default_key_version() -> i32 {
    1
}

fn default_cloudflare_base_url() -> String {
    "https://api.cloudflare.com/client/v4".to_owned()
}

fn default_vultr_base_url() -> String {
    "https://api.vultr.com/v2".to_owned()
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("failed to load configuration: {0}")]
    Load(#[from] config::ConfigError),
}

impl Settings {
    /// Loads the layered application settings from the project root and environment.
    ///
    /// # Errors
    ///
    /// Returns [`SettingsError`] when a source cannot be read or deserialized.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, SettingsError> {
        let root = root.as_ref();
        let settings = Config::builder()
            .add_source(File::from(root.join("config/default.toml")))
            .add_source(
                Environment::with_prefix("MULTICLOUD")
                    .prefix_separator("__")
                    .separator("__"),
            )
            .build()?
            .try_deserialize()?;

        Ok(settings)
    }
}
