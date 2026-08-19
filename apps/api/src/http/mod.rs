mod audit;
mod auth;
mod authorization;
mod error;
mod invitations;
mod operations;
mod organizations;
mod providers;
mod resources;
mod tenant;

use axum::Router;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub database: DatabaseConnection,
    pub provider_registry: multicloud_provider::ProviderRegistry,
    pub credential_cipher: Arc<multicloud_provider::EnvelopeCipher>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/audit-logs", audit::router())
        .nest("/rbac", authorization::router())
        .nest("/invitations", invitations::router())
        .nest("/operations", operations::router())
        .nest("/providers", providers::router())
        .nest("/resources", resources::router())
        .nest("/organizations", organizations::router())
        .nest("/tenant", tenant::router())
}
