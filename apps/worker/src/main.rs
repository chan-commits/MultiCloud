use anyhow::Context;
use multicloud_configuration::Settings;
use multicloud_operation::RetryPolicy;
use multicloud_persistence::entities::{outbox_events, provider_accounts, provider_credentials};
use multicloud_provider::{
    CloudflareAdapter, EncryptedCredential, EnvelopeCipher, FakeProviderAdapter, InventoryRequest,
    OvhAdapter, ProviderAdapter, ProviderRegistry, VultrAdapter, decode_credential_envelope,
};
use redis::AsyncCommands;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    IntoActiveModel, QueryFilter, Set, Statement, TransactionTrait,
};
use std::{path::PathBuf, sync::Arc, time::Duration as StdDuration};
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
    let cipher = EnvelopeCipher::from_base64(
        &settings.provider.credential_master_key,
        settings.provider.credential_key_version,
    )
    .context("MULTICLOUD__PROVIDER__CREDENTIAL_MASTER_KEY must contain a base64 32-byte key")?;
    let mut adapters: Vec<Arc<dyn ProviderAdapter>> = vec![
        Arc::new(CloudflareAdapter::new(
            settings.provider.cloudflare_base_url,
        )),
        Arc::new(VultrAdapter::new(settings.provider.vultr_base_url)),
        Arc::new(OvhAdapter::new(settings.provider.ovh_base_url)),
    ];
    if settings.environment == "development" {
        adapters.push(Arc::new(FakeProviderAdapter));
    }
    let executor = ProviderExecutor {
        registry: ProviderRegistry::new(adapters),
        cipher,
        worker_id: Uuid::now_v7().to_string(),
    };
    tracing::info!(environment = settings.environment, "worker started");

    loop {
        tokio::select! {
            result = run_iteration(&database, &executor, &redis_client, &mut redis) => {
                match result {
                    Ok(true) => {}
                    Ok(false) => sleep(StdDuration::from_millis(500)).await,
                    Err(error) => {
                        tracing::error!(%error, "worker iteration failed");
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

struct ProviderExecutor {
    registry: ProviderRegistry,
    cipher: EnvelopeCipher,
    worker_id: String,
}

async fn run_iteration(
    database: &DatabaseConnection,
    executor: &ProviderExecutor,
    redis_client: &redis::Client,
    redis: &mut redis::aio::MultiplexedConnection,
) -> anyhow::Result<bool> {
    let provider_work = execute_provider_operation(database, executor).await?;
    let outbox_work = dispatch_one(database, redis_client, redis).await?;
    Ok(provider_work || outbox_work)
}

#[allow(clippy::too_many_lines)]
async fn execute_provider_operation(
    database: &DatabaseConnection,
    executor: &ProviderExecutor,
) -> anyhow::Result<bool> {
    let transaction = database.begin().await?;
    let Some(claimed) = multicloud_persistence::provider_operations::claim_next_provider_operation(
        &transaction,
        &executor.worker_id,
        time::Duration::minutes(2),
    )
    .await?
    else {
        transaction.commit().await?;
        return Ok(false);
    };
    let account = provider_accounts::Entity::find_by_id(claimed.provider_account_id)
        .one(&transaction)
        .await?
        .context("provider account disappeared")?;
    let credential = provider_credentials::Entity::find()
        .filter(provider_credentials::Column::ProviderAccountId.eq(account.id))
        .filter(provider_credentials::Column::Status.eq("active"))
        .one(&transaction)
        .await?
        .context("provider account has no active credential")?;
    let plaintext = executor.cipher.decrypt(&EncryptedCredential {
        ciphertext: credential.ciphertext,
        nonce: credential.nonce,
        key_version: credential.key_version,
    })?;
    let material = decode_credential_envelope(&plaintext, &credential.credential_type);
    let provider_kind = account.provider_kind;
    let kind = multicloud_provider::ProviderKind::parse(provider_kind.clone())?;
    let adapter = executor.registry.get(&kind)?;
    transaction.commit().await?;

    let result = if claimed.request.action == "inventory.sync" {
        let inventory_request = InventoryRequest {
            resource_type: claimed.request.resource_type.clone(),
            parent_external_id: claimed
                .request
                .parameters
                .get("parent_external_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            cursor: claimed
                .request
                .parameters
                .get("cursor")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
        };
        adapter
            .inventory(&material, &inventory_request)
            .await
            .map(ProviderExecution::Inventory)
    } else {
        adapter
            .execute(&material, &claimed.request)
            .await
            .map(ProviderExecution::Operation)
    };
    let transaction = database.begin().await?;
    match result {
        Ok(ProviderExecution::Operation(result)) => {
            if claimed.request.action != "delete"
                && let (Some(external_id), Some(name)) = (
                    result.external_id.clone(),
                    result.state.get("name").and_then(serde_json::Value::as_str),
                )
            {
                multicloud_persistence::resource_sync::apply_inventory_page(
                    &transaction,
                    claimed.operation.organization_id,
                    claimed.provider_account_id,
                    &provider_kind,
                    multicloud_provider::InventoryPage {
                        items: vec![multicloud_provider::InventoryItem {
                            external_type: claimed.request.resource_type.clone(),
                            external_id,
                            name: name.to_owned(),
                            state: result.state.clone(),
                            metadata: result.state.clone(),
                        }],
                        next_cursor: None,
                    },
                    multicloud_persistence::resource_sync::ReconciliationPolicy::ManualApproval,
                )
                .await?;
            }
            multicloud_persistence::provider_operations::complete_provider_operation(
                &transaction,
                claimed,
                None,
                serde_json::json!({
                    "external_id": result.external_id,
                    "state": result.state,
                }),
            )
            .await?;
        }
        Ok(ProviderExecution::Inventory(page)) => {
            let next_cursor = page.next_cursor.clone();
            let outcome = multicloud_persistence::resource_sync::apply_inventory_page(
                &transaction,
                claimed.operation.organization_id,
                claimed.provider_account_id,
                &provider_kind,
                page,
                multicloud_persistence::resource_sync::ReconciliationPolicy::ManualApproval,
            )
            .await?;
            multicloud_persistence::provider_operations::complete_provider_operation(
                &transaction,
                claimed,
                None,
                serde_json::json!({
                    "created": outcome.created,
                    "updated": outcome.updated,
                    "drifted": outcome.drifted,
                    "reconciliation_tasks": outcome.reconciliation_tasks,
                    "next_cursor": next_cursor,
                }),
            )
            .await?;
        }
        Err(error) => {
            multicloud_persistence::provider_operations::fail_provider_operation(
                &transaction,
                claimed,
                &error,
            )
            .await?;
        }
    }
    transaction.commit().await?;
    Ok(true)
}

enum ProviderExecution {
    Operation(multicloud_provider::ProviderOperationResult),
    Inventory(multicloud_provider::InventoryPage),
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
