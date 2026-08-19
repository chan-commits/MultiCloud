use anyhow::Context;
use multicloud_configuration::Settings;
use multicloud_operation::RetryPolicy;
use multicloud_persistence::entities::outbox_events;
use redis::AsyncCommands;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, IntoActiveModel,
    Set, Statement, TransactionTrait,
};
use std::{path::PathBuf, time::Duration as StdDuration};
use time::OffsetDateTime;
use tokio::time::sleep;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    multicloud_observability::init("multicloud-worker");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let settings = Settings::load(root).context("could not load settings")?;
    let database =
        multicloud_persistence::connect(&settings.database.url, settings.database.max_connections)
            .await
            .context("could not connect to database")?;
    let redis_client = multicloud_messaging::client(&settings.redis.url)?;
    let mut redis = redis_client
        .get_multiplexed_async_connection()
        .await
        .context("could not connect to Redis")?;
    tracing::info!(environment = settings.environment, "worker started");

    loop {
        tokio::select! {
            result = dispatch_one(&database, &redis_client, &mut redis) => {
                match result {
                    Ok(true) => {}
                    Ok(false) => sleep(StdDuration::from_millis(500)).await,
                    Err(error) => {
                        tracing::error!(%error, "outbox dispatch iteration failed");
                        sleep(StdDuration::from_secs(1)).await;
                    }
                }
            }
            signal = shutdown_signal() => {
                signal?;
                break;
            }
        }
    }

    tracing::info!("worker stopped");
    Ok(())
}

async fn shutdown_signal() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await
}

async fn dispatch_one(
    database: &DatabaseConnection,
    redis_client: &redis::Client,
    redis: &mut redis::aio::MultiplexedConnection,
) -> anyhow::Result<bool> {
    let transaction = database.begin().await?;
    let row = transaction
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            r"
            SELECT id
            FROM outbox_events
            WHERE published_at IS NULL
              AND dead_lettered_at IS NULL
              AND next_attempt_at <= now()
            ORDER BY occurred_at
            LIMIT 1
            FOR UPDATE SKIP LOCKED
            ",
        ))
        .await?;
    let Some(row) = row else {
        transaction.commit().await?;
        return Ok(false);
    };
    let event_id: Uuid = row.try_get("", "id")?;
    let event = outbox_events::Entity::find_by_id(event_id)
        .one(&transaction)
        .await?
        .context("claimed outbox event disappeared")?;
    let channel = format!("multicloud:events:{}", event.organization_id);
    let envelope = serde_json::json!({
        "id": event.id,
        "organization_id": event.organization_id,
        "aggregate_type": event.aggregate_type,
        "aggregate_id": event.aggregate_id,
        "event_type": event.event_type,
        "event_version": event.event_version,
        "payload": event.payload,
        "trace_id": event.trace_id,
        "occurred_at": event.occurred_at
            .format(&time::format_description::well_known::Rfc3339)?,
    });
    let serialized = serde_json::to_string(&envelope)?;
    let publish_result: Result<usize, redis::RedisError> =
        redis.publish(&channel, serialized).await;
    let mut active = event.into_active_model();
    match publish_result {
        Ok(subscribers) => {
            active.published_at = Set(Some(OffsetDateTime::now_utc()));
            active.last_error = Set(None);
            active.update(&transaction).await?;
            transaction.commit().await?;
            tracing::info!(%event_id, %channel, subscribers, "outbox event published");
        }
        Err(error) => {
            let attempt = active.attempt_count.as_ref().to_owned() + 1;
            let policy = RetryPolicy::default();
            active.attempt_count = Set(attempt);
            active.last_error = Set(Some(truncate_error(&error.to_string())));
            if policy.should_dead_letter(u32::try_from(attempt).unwrap_or(u32::MAX)) {
                active.dead_lettered_at = Set(Some(OffsetDateTime::now_utc()));
            } else {
                active.next_attempt_at = Set(OffsetDateTime::now_utc()
                    + policy.delay_for_attempt(u32::try_from(attempt).unwrap_or(u32::MAX)));
            }
            active.update(&transaction).await?;
            transaction.commit().await?;
            tracing::warn!(%event_id, attempt, %error, "outbox publish scheduled for retry");
            if let Ok(connection) = redis_client.get_multiplexed_async_connection().await {
                *redis = connection;
                tracing::info!("Redis connection restored");
            }
        }
    }
    Ok(true)
}

fn truncate_error(value: &str) -> String {
    value.chars().take(2_000).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_errors_are_bounded() {
        let value = "x".repeat(3_000);
        assert_eq!(truncate_error(&value).len(), 2_000);
    }
}
