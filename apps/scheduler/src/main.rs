use anyhow::Context;
use multicloud_configuration::Settings;
use std::path::PathBuf;

#[allow(clippy::missing_errors_doc)]
pub async fn run() -> anyhow::Result<()> {
    multicloud_observability::init("multicloud-scheduler");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let settings = Settings::load(root).context("could not load settings")?;
    tracing::info!(environment = settings.environment, "scheduler started");
    tokio::signal::ctrl_c().await?;
    tracing::info!("scheduler stopped");
    Ok(())
}
