use anyhow::Context;
mod http;

use axum::{Router, routing::get};
use multicloud_configuration::Settings;
use serde::Serialize;
use std::{net::SocketAddr, path::PathBuf};
use tokio::net::TcpListener;

#[derive(Serialize)]
struct HealthResponse {
    service: &'static str,
    status: &'static str,
}

async fn health() -> axum::Json<HealthResponse> {
    axum::Json(HealthResponse {
        service: "multicloud-api",
        status: "ok",
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    multicloud_observability::init("multicloud-api");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let settings = Settings::load(root).context("could not load settings")?;
    let address: SocketAddr = format!("{}:{}", settings.http.host, settings.http.port)
        .parse()
        .context("invalid HTTP listen address")?;
    let database =
        multicloud_persistence::connect(&settings.database.url, settings.database.max_connections)
            .await
            .context("could not connect to database")?;
    let state = http::AppState { database };

    let app = Router::new()
        .route("/health", get(health))
        .nest("/api/v1", http::router())
        .with_state(state);
    let listener = TcpListener::bind(address).await?;
    tracing::info!(%address, environment = settings.environment, "API listening");
    axum::serve(listener, app).await?;
    Ok(())
}
