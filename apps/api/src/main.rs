use anyhow::Context;
mod http;

use axum::{Router, routing::get};
use multicloud_configuration::Settings;
use serde::Serialize;
use std::sync::Arc;
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
    let cipher = multicloud_provider::EnvelopeCipher::from_base64(
        &settings.provider.credential_master_key,
        settings.provider.credential_key_version,
    )
    .context("MULTICLOUD__PROVIDER__CREDENTIAL_MASTER_KEY must contain a base64 32-byte key")?;
    let mut adapters: Vec<Arc<dyn multicloud_provider::ProviderAdapter>> = vec![
        Arc::new(multicloud_provider::CloudflareAdapter::new(
            settings.provider.cloudflare_base_url,
        )),
        Arc::new(multicloud_provider::VultrAdapter::new(
            settings.provider.vultr_base_url,
        )),
        Arc::new(multicloud_provider::OvhAdapter::new(
            settings.provider.ovh_base_url,
        )),
    ];
    if settings.environment == "development" {
        adapters.push(Arc::new(multicloud_provider::FakeProviderAdapter));
    }
    let state = http::AppState {
        database,
        provider_registry: multicloud_provider::ProviderRegistry::new(adapters),
        credential_cipher: Arc::new(cipher),
    };

    let app = Router::new()
        .route("/health", get(health))
        .nest("/api/v1", http::router())
        .with_state(state);
    let listener = TcpListener::bind(address).await?;
    tracing::info!(%address, environment = settings.environment, "API listening");
    axum::serve(listener, app).await?;
    Ok(())
}
