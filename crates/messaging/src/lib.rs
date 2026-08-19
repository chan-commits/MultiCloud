use thiserror::Error;

#[derive(Debug, Error)]
pub enum MessagingError {
    #[error("redis client creation failed: {0}")]
    Client(#[from] redis::RedisError),
}

/// Creates a Redis client for the supplied connection URL.
///
/// # Errors
///
/// Returns [`MessagingError`] when the URL is not a valid Redis connection URL.
pub fn client(url: &str) -> Result<redis::Client, MessagingError> {
    Ok(redis::Client::open(url)?)
}
