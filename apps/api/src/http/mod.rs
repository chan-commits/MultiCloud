mod auth;
mod error;
mod invitations;
mod organizations;
mod tenant;

use axum::Router;
use sea_orm::DatabaseConnection;

#[derive(Clone)]
pub struct AppState {
    pub database: DatabaseConnection,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/invitations", invitations::router())
        .nest("/organizations", organizations::router())
        .nest("/tenant", tenant::router())
}
