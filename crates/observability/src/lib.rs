use std::sync::OnceLock;
use thiserror::Error;
use tracing_subscriber::{
    EnvFilter, Registry, layer::SubscriberExt, reload, util::SubscriberInitExt,
};

type FilterHandle = reload::Handle<EnvFilter, Registry>;

static FILTER_HANDLE: OnceLock<FilterHandle> = OnceLock::new();

#[derive(Debug, Error)]
pub enum LogFilterError {
    #[error("unsupported log level")]
    UnsupportedLevel,
    #[error("tracing subscriber is not initialized")]
    NotInitialized,
    #[error("failed to reload log filter: {0}")]
    Reload(#[from] reload::Error),
}

pub fn init(service_name: &'static str) {
    FILTER_HANDLE.get_or_init(|| {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
        let (filter, handle) = reload::Layer::new(filter);
        tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_current_span(true)
                    .with_target(true),
            )
            .init();
        handle
    });

    tracing::info!(service.name = service_name, "observability initialized");
}

/// Changes the process-wide tracing level without restarting the service.
///
/// # Errors
///
/// Returns an error for unsupported levels or when the subscriber cannot be reloaded.
pub fn set_log_level(level: &str) -> Result<(), LogFilterError> {
    if !matches!(level, "error" | "warn" | "info" | "debug" | "trace") {
        return Err(LogFilterError::UnsupportedLevel);
    }
    let handle = FILTER_HANDLE.get().ok_or(LogFilterError::NotInitialized)?;
    handle.reload(EnvFilter::new(level))?;
    tracing::info!(log.level = level, "runtime log level updated");
    Ok(())
}
