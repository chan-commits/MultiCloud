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
