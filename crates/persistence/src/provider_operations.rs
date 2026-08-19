use crate::{
    entities::{operations, provider_operation_attempts, provider_operation_requests},
    reliable_events::{NewOperation, create_operation_with_event, enqueue_event},
};
use multicloud_operation::{EventEnvelope, RetryPolicy};
use multicloud_provider::{ProviderError, ProviderOperationRequest};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend, DbErr,
    EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, Set, Statement, TryInsertResult,
};
use serde_json::Value;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

pub struct NewProviderOperation<'a> {
    pub organization_id: Uuid,
    pub provider_account_id: Uuid,
    pub requested_by: Uuid,
    pub action: &'a str,
    pub resource_type: &'a str,
    pub external_id: Option<&'a str>,
    pub parameters: Value,
    pub idempotency_key: &'a str,
}

pub struct ClaimedProviderOperation {
    pub operation: operations::Model,
    pub attempt: provider_operation_attempts::Model,
    pub provider_account_id: Uuid,
    pub request: ProviderOperationRequest,
}

/// Creates the generic Operation and immutable Provider request in one transaction.
///
/// # Errors
///
/// Returns [`DbErr`] when either record or the initial outbox event cannot be persisted.
pub async fn create_provider_operation(
    transaction: &DatabaseTransaction,
    command: NewProviderOperation<'_>,
) -> Result<operations::Model, DbErr> {
    let operation = create_operation_with_event(
        transaction,
        NewOperation {
            organization_id: command.organization_id,
            operation_type: "provider.resource_operation",
            target_type: command.resource_type,
            target_id: command.external_id,
            requested_by: command.requested_by,
            idempotency_key: command.idempotency_key,
        },
        "provider.operation.requested",
        serde_json::json!({
            "provider_account_id": command.provider_account_id,
            "action": command.action,
            "resource_type": command.resource_type,
        }),
        None,
    )
    .await?;
    let insertion =
        provider_operation_requests::Entity::insert(provider_operation_requests::ActiveModel {
            operation_id: Set(operation.id),
            organization_id: Set(command.organization_id),
            provider_account_id: Set(command.provider_account_id),
            action: Set(command.action.to_owned()),
            resource_type: Set(command.resource_type.to_owned()),
            external_id: Set(command.external_id.map(str::to_owned)),
            parameters: Set(command.parameters),
            idempotency_key: Set(command.idempotency_key.to_owned()),
            created_at: Set(OffsetDateTime::now_utc()),
        })
        .on_conflict_do_nothing_on([provider_operation_requests::Column::OperationId])
        .exec(transaction)
        .await?;
    match insertion {
        TryInsertResult::Inserted(_) | TryInsertResult::Conflicted | TryInsertResult::Empty => {
            Ok(operation)
        }
    }
}

/// Claims one ready Provider operation and starts a leased attempt.
///
/// This is an internal cross-tenant worker operation and requires a dedicated database role that
/// can read ready Operations while preserving tenant IDs on every write.
///
/// # Errors
///
/// Returns [`DbErr`] for claim, attempt, or Operation persistence failures.
pub async fn claim_next_provider_operation(
    transaction: &DatabaseTransaction,
    worker_id: &str,
    lease_duration: Duration,
) -> Result<Option<ClaimedProviderOperation>, DbErr> {
    let row = transaction
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            r"
            SELECT operation.id
            FROM operations AS operation
            JOIN provider_operation_requests AS request ON request.operation_id = operation.id
            WHERE operation.status = 'queued' AND operation.next_attempt_at <= now()
            ORDER BY operation.created_at
            LIMIT 1
            FOR UPDATE OF operation SKIP LOCKED
            ",
        ))
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let operation_id: Uuid = row.try_get("", "id")?;
    let operation = operations::Entity::find_by_id(operation_id)
        .one(transaction)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("claimed operation disappeared".to_owned()))?;
    let request = provider_operation_requests::Entity::find_by_id(operation_id)
        .one(transaction)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("provider request disappeared".to_owned()))?;
    let attempt_number = provider_operation_attempts::Entity::find()
        .filter(provider_operation_attempts::Column::OperationId.eq(operation_id))
        .order_by_desc(provider_operation_attempts::Column::AttemptNumber)
        .one(transaction)
        .await?
        .map_or(1, |attempt| attempt.attempt_number.saturating_add(1));
    let now = OffsetDateTime::now_utc();
    let attempt = provider_operation_attempts::ActiveModel {
        id: Set(Uuid::now_v7()),
        organization_id: Set(operation.organization_id),
        operation_id: Set(operation.id),
        provider_account_id: Set(request.provider_account_id),
        attempt_number: Set(attempt_number),
        status: Set("running".to_owned()),
        lease_owner: Set(Some(worker_id.to_owned())),
        lease_expires_at: Set(Some(now + lease_duration)),
        provider_request_id: Set(None),
        masked_request: Set(serde_json::json!({
            "action": request.action,
            "resource_type": request.resource_type,
            "external_id": request.external_id,
        })),
        masked_result: Set(None),
        error_category: Set(None),
        error_code: Set(None),
        retryable: Set(None),
        retry_after: Set(None),
        started_at: Set(now),
        completed_at: Set(None),
    }
    .insert(transaction)
    .await?;
    let mut active = operation.clone().into_active_model();
    active.status = Set("running".to_owned());
    active.started_at = Set(operation.started_at.or(Some(now)));
    active.updated_at = Set(now);
    let operation = active.update(transaction).await?;
    Ok(Some(ClaimedProviderOperation {
        operation,
        attempt,
        provider_account_id: request.provider_account_id,
        request: ProviderOperationRequest {
            action: request.action,
            resource_type: request.resource_type,
            external_id: request.external_id,
            parameters: request.parameters,
            idempotency_key: request.idempotency_key,
        },
    }))
}

/// Completes a Provider attempt and its Operation atomically.
///
/// # Errors
///
/// Returns [`DbErr`] when state or completion event persistence fails.
pub async fn complete_provider_operation(
    transaction: &DatabaseTransaction,
    claimed: ClaimedProviderOperation,
    provider_request_id: Option<String>,
    masked_result: Value,
) -> Result<(), DbErr> {
    let now = OffsetDateTime::now_utc();
    let operation_id = claimed.operation.id;
    let organization_id = claimed.operation.organization_id;
    let mut attempt = claimed.attempt.into_active_model();
    attempt.status = Set("succeeded".to_owned());
    attempt.provider_request_id = Set(provider_request_id);
    attempt.masked_result = Set(Some(masked_result));
    attempt.lease_owner = Set(None);
    attempt.lease_expires_at = Set(None);
    attempt.completed_at = Set(Some(now));
    attempt.update(transaction).await?;
    let mut operation = claimed.operation.into_active_model();
    operation.status = Set("succeeded".to_owned());
    operation.progress = Set(100);
    operation.completed_at = Set(Some(now));
    operation.updated_at = Set(now);
    operation.update(transaction).await?;
    enqueue_event(
        transaction,
        operation_event(
            operation_id,
            organization_id,
            "provider.operation.succeeded",
            now,
        ),
    )
    .await?;
    Ok(())
}

/// Records a normalized failure and either schedules retry or terminates the Operation.
///
/// # Errors
///
/// Returns [`DbErr`] when state or failure event persistence fails.
pub async fn fail_provider_operation(
    transaction: &DatabaseTransaction,
    claimed: ClaimedProviderOperation,
    error: &ProviderError,
) -> Result<(), DbErr> {
    let now = OffsetDateTime::now_utc();
    let operation_id = claimed.operation.id;
    let organization_id = claimed.operation.organization_id;
    let policy = RetryPolicy::default();
    let attempt_number = u32::try_from(claimed.attempt.attempt_number).unwrap_or(u32::MAX);
    let retry = error.retryable && !policy.should_dead_letter(attempt_number);
    let retry_after = retry.then(|| {
        error.retry_after_seconds.map_or_else(
            || now + policy.delay_for_attempt(attempt_number),
            |seconds| now + Duration::seconds(i64::try_from(seconds).unwrap_or(i64::MAX)),
        )
    });
    let category = serde_json::to_value(error.category)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned));
    let mut attempt = claimed.attempt.into_active_model();
    attempt.status = Set("failed".to_owned());
    attempt.error_category = Set(category);
    attempt.error_code = Set(Some(error.code.clone()));
    attempt.retryable = Set(Some(error.retryable));
    attempt.retry_after = Set(retry_after);
    attempt.lease_owner = Set(None);
    attempt.lease_expires_at = Set(None);
    attempt.completed_at = Set(Some(now));
    attempt.update(transaction).await?;
    let mut operation = claimed.operation.into_active_model();
    operation.status = Set(if retry { "queued" } else { "failed" }.to_owned());
    operation.error_code = Set(Some(error.code.clone()));
    operation.error_message = Set(Some(error.safe_message.clone()));
    operation.next_attempt_at = Set(retry_after.unwrap_or(now));
    operation.completed_at = Set((!retry).then_some(now));
    operation.updated_at = Set(now);
    operation.update(transaction).await?;
    enqueue_event(
        transaction,
        operation_event(
            operation_id,
            organization_id,
            if retry {
                "provider.operation.retry_scheduled"
            } else {
                "provider.operation.failed"
            },
            now,
        ),
    )
    .await?;
    Ok(())
}

fn operation_event(
    operation_id: Uuid,
    organization_id: Uuid,
    event_type: &str,
    occurred_at: OffsetDateTime,
) -> EventEnvelope {
    EventEnvelope {
        id: multicloud_shared_kernel::EventId::new(),
        organization_id: multicloud_shared_kernel::OrganizationId::from_uuid(organization_id),
        aggregate_type: "operation".to_owned(),
        aggregate_id: operation_id.to_string(),
        event_type: event_type.to_owned(),
        event_version: 1,
        payload: serde_json::json!({ "operation_id": operation_id }),
        trace_id: None,
        occurred_at,
    }
}
