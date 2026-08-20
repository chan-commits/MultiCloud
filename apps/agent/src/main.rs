#[allow(clippy::missing_errors_doc)]
pub async fn run() -> anyhow::Result<()> {
    multicloud_observability::init("multicloud-agent");
    tracing::info!("agent started");
    tokio::signal::ctrl_c().await?;
    tracing::info!("agent stopped");
    Ok(())
}
