use anyhow::Context;
use multicloud_configuration::Settings;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    multicloud_observability::init("multicloud-worker");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let settings = Settings::load(root).context("could not load settings")?;
    tracing::info!(environment = settings.environment, "worker started");
    tokio::signal::ctrl_c().await?;
    tracing::info!("worker stopped");
    Ok(())
}
