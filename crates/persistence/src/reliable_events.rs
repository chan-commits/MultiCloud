use crate::entities::{inbox_messages, operations, outbox_events};
use multicloud_operation::EventEnvelope;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, DbErr, EntityTrait, QueryFilter, Set,
    TryInsertResult,
};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

pub struct NewOperation<'a> {
    pub organization_id: Uuid,
    pub operation_type: &'a str,
    pub target_type: &'a str,
    pub target_id: Option<&'a str>,
    pub requested_by: Uuid,
    pub idempotency_key: &'a str,
}

/// Creates an operation and its initial event atomically.
///
/// If the tenant-scoped idempotency key already exists, the existing operation is returned and no
/// duplicate outbox event is created.
///
/// # Errors
///
/// Returns [`DbErr`] when the operation or event cannot be queried or persisted.
pub async fn create_operation_with_event(
    transaction: &DatabaseTransaction,
    operation: NewOperation<'_>,
    event_type: &str,
    mut payload: Value,
    trace_id: Option<&str>,
) -> Result<operations::Model, DbErr> {
    if let Some(existing) = operations::Entity::find()
        .filter(operations::Column::OrganizationId.eq(operation.organization_id))
        .filter(operations::Column::IdempotencyKey.eq(operation.idempotency_key))
        .one(transaction)
        .await?
    {
        return Ok(existing);
    }
    let now = OffsetDateTime::now_utc();
    let insertion = operations::Entity::insert(operations::ActiveModel {
        id: Set(Uuid::now_v7()),
        organization_id: Set(operation.organization_id),
        operation_type: Set(operation.operation_type.to_owned()),
        target_type: Set(operation.target_type.to_owned()),
        target_id: Set(operation.target_id.map(str::to_owned)),
        requested_by: Set(operation.requested_by),
        idempotency_key: Set(operation.idempotency_key.to_owned()),
        status: Set("queued".to_owned()),
        progress: Set(0),
        error_code: Set(None),
        error_message: Set(None),
        started_at: Set(None),
        completed_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        next_attempt_at: Set(now),
    })
    .on_conflict_do_nothing_on([
        operations::Column::OrganizationId,
        operations::Column::IdempotencyKey,
    ])
    .exec_with_returning(transaction)
    .await?;
    let model = match insertion {
        TryInsertResult::Inserted(model) => model,
        TryInsertResult::Conflicted | TryInsertResult::Empty => {
            return operations::Entity::find()
                .filter(operations::Column::OrganizationId.eq(operation.organization_id))
                .filter(operations::Column::IdempotencyKey.eq(operation.idempotency_key))
                .one(transaction)
                .await?
                .ok_or_else(|| DbErr::RecordNotFound("idempotent operation disappeared".into()));
        }
    };
    if let Some(payload) = payload.as_object_mut() {
        payload.insert(
            "requested_by".to_owned(),
            serde_json::json!(operation.requested_by),
        );
    }
    enqueue_event(
        transaction,
        EventEnvelope {
            id: multicloud_shared_kernel::EventId::new(),
            organization_id: multicloud_shared_kernel::OrganizationId::from_uuid(
                operation.organization_id,
            ),
            aggregate_type: "operation".to_owned(),
            aggregate_id: model.id.to_string(),
            event_type: event_type.to_owned(),
            event_version: 1,
            payload,
            trace_id: trace_id.map(str::to_owned),
            occurred_at: now,
        },
    )
    .await?;
    Ok(model)
}

/// Adds a domain event to the transactional outbox.
///
/// # Errors
///
/// Returns [`DbErr`] when the event cannot be persisted.
pub async fn enqueue_event(
    transaction: &DatabaseTransaction,
    event: EventEnvelope,
) -> Result<outbox_events::Model, DbErr> {
    outbox_events::ActiveModel {
        id: Set(event.id.value()),
        organization_id: Set(event.organization_id.value()),
        aggregate_type: Set(event.aggregate_type),
        aggregate_id: Set(event.aggregate_id),
        event_type: Set(event.event_type),
        event_version: Set(i16::try_from(event.event_version).unwrap_or(i16::MAX)),
        payload: Set(event.payload),
        trace_id: Set(event.trace_id),
        occurred_at: Set(event.occurred_at),
        published_at: Set(None),
        attempt_count: Set(0),
        next_attempt_at: Set(event.occurred_at),
        last_error: Set(None),
        dead_lettered_at: Set(None),
    }
    .insert(transaction)
    .await
}

/// Claims an inbox message exactly once for a named consumer.
///
/// # Errors
///
/// Returns [`DbErr`] for persistence failures other than an already-claimed message.
pub async fn claim_inbox_message(
    transaction: &DatabaseTransaction,
    organization_id: Uuid,
    consumer: &str,
    message_id: Uuid,
    result: Option<Value>,
) -> Result<bool, DbErr> {
    let insertion = inbox_messages::Entity::insert(inbox_messages::ActiveModel {
        organization_id: Set(organization_id),
        consumer: Set(consumer.to_owned()),
        message_id: Set(message_id),
        processed_at: Set(OffsetDateTime::now_utc()),
        result: Set(result),
    })
    .on_conflict_do_nothing_on([
        inbox_messages::Column::OrganizationId,
        inbox_messages::Column::Consumer,
        inbox_messages::Column::MessageId,
    ])
    .exec(transaction)
    .await?;
    match insertion {
        TryInsertResult::Inserted(_) => Ok(true),
        TryInsertResult::Conflicted | TryInsertResult::Empty => Ok(false),
    }
}
