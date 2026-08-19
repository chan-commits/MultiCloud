use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use std::time::Duration;
use thiserror::Error;

pub mod entities;
pub mod reliable_events;

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("database connection failed: {0}")]
    Connection(#[from] sea_orm::DbErr),
}

/// Opens a `PostgreSQL` connection pool with bounded connection and timeout settings.
///
/// # Errors
///
/// Returns [`PersistenceError`] when the database connection cannot be established.
pub async fn connect(
    url: &str,
    max_connections: u32,
) -> Result<DatabaseConnection, PersistenceError> {
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(max_connections)
        .connect_timeout(Duration::from_secs(5))
        .sqlx_logging(false);

    Ok(Database::connect(options).await?)
}
