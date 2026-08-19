use super::{AppState, error::ApiError, tenant::TenantContext};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use multicloud_authorization::permissions;
use multicloud_operation::EventEnvelope;
use multicloud_persistence::entities::operations;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list))
        .route("/{operation_id}", get(get_one))
        .route("/{operation_id}/cancel", post(cancel))
}

#[derive(Serialize)]
struct OperationResponse {
    id: Uuid,
    operation_type: String,
    target_type: String,
    target_id: Option<String>,
    status: String,
    progress: i16,
    error_code: Option<String>,
    error_message: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

impl From<operations::Model> for OperationResponse {
    fn from(operation: operations::Model) -> Self {
        Self {
            id: operation.id,
            operation_type: operation.operation_type,
            target_type: operation.target_type,
            target_id: operation.target_id,
            status: operation.status,
            progress: operation.progress,
            error_code: operation.error_code,
            error_message: operation.error_message,
            created_at: operation.created_at,
        }
    }
}

async fn list(
    context: TenantContext,
    State(state): State<AppState>,
) -> Result<Json<Vec<OperationResponse>>, ApiError> {
    let transaction =
        super::authorization::authorize_transaction(&state, &context, permissions::OPERATION_READ)
            .await?;
    let values = operations::Entity::find()
        .filter(operations::Column::OrganizationId.eq(context.organization_id))
        .order_by_desc(operations::Column::CreatedAt)
        .limit(100)
        .all(&transaction)
        .await
        .map_err(super::error::internal)?
        .into_iter()
        .map(Into::into)
        .collect();
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(values))
}

async fn get_one(
    context: TenantContext,
    State(state): State<AppState>,
    Path(operation_id): Path<Uuid>,
) -> Result<Json<OperationResponse>, ApiError> {
    let transaction =
        super::authorization::authorize_transaction(&state, &context, permissions::OPERATION_READ)
            .await?;
    let operation = operations::Entity::find_by_id(operation_id)
        .one(&transaction)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::BadRequest("operation was not found"))?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(operation.into()))
}

async fn cancel(
    context: TenantContext,
    State(state): State<AppState>,
    Path(operation_id): Path<Uuid>,
) -> Result<Json<OperationResponse>, ApiError> {
    let transaction = super::authorization::authorize_transaction(
        &state,
        &context,
        permissions::OPERATION_CANCEL,
    )
    .await?;
    let operation = operations::Entity::find_by_id(operation_id)
        .one(&transaction)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::BadRequest("operation was not found"))?;
    if operation.status != "queued" {
        return Err(ApiError::Conflict(
            "only queued operations can be cancelled",
        ));
    }
    let now = OffsetDateTime::now_utc();
    let mut active = operation.into_active_model();
    active.status = Set("cancelled".to_owned());
    active.completed_at = Set(Some(now));
    active.updated_at = Set(now);
    let operation = active
        .update(&transaction)
        .await
        .map_err(super::error::internal)?;
    multicloud_persistence::reliable_events::enqueue_event(
        &transaction,
        EventEnvelope {
            id: multicloud_shared_kernel::EventId::new(),
            organization_id: multicloud_shared_kernel::OrganizationId::from_uuid(
                context.organization_id,
            ),
            aggregate_type: "operation".to_owned(),
            aggregate_id: operation.id.to_string(),
            event_type: "operation.cancelled".to_owned(),
            event_version: 1,
            payload: serde_json::json!({ "operation_id": operation.id }),
            trace_id: None,
            occurred_at: now,
        },
    )
    .await
    .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(operation.into()))
}
