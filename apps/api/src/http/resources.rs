use super::{AppState, error::ApiError, tenant::TenantContext};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use multicloud_authorization::permissions;
use multicloud_operation::EventEnvelope;
use multicloud_persistence::{
    entities::{
        reconciliation_tasks, resource_desired_states, resource_drifts, resource_observed_states,
        resources,
    },
    reliable_events::enqueue_event,
};
use multicloud_resource::{DesiredState, ObservedState, detect_drift};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list))
        .route("/{resource_id}", get(get_one))
        .route("/{resource_id}/desired-state", post(set_desired_state))
        .route("/{resource_id}/drifts", get(list_drifts))
        .route("/{resource_id}/reconciliations", get(list_reconciliations))
        .route(
            "/{resource_id}/reconciliations/{task_id}/approve",
            post(approve_reconciliation),
        )
}

#[derive(Serialize)]
struct ResourceResponse {
    id: Uuid,
    resource_type: String,
    name: String,
    lifecycle: String,
    region: Option<String>,
    attributes: Value,
    desired_state: Option<StateResponse>,
    observed_state: Option<StateResponse>,
}

#[derive(Serialize)]
struct StateResponse {
    version: i64,
    state: Value,
}

#[derive(Deserialize)]
struct SetDesiredStateRequest {
    managed_fields: Vec<String>,
    state: Value,
}

#[derive(Serialize)]
struct DriftResponse {
    id: Uuid,
    fingerprint: String,
    status: String,
    differences: Value,
    #[serde(with = "time::serde::rfc3339")]
    detected_at: OffsetDateTime,
}

#[derive(Serialize)]
struct ReconciliationResponse {
    id: Uuid,
    drift_id: Uuid,
    policy: String,
    status: String,
    desired_version: i64,
    operation_id: Option<Uuid>,
}

async fn list(
    context: TenantContext,
    State(state): State<AppState>,
) -> Result<Json<Vec<ResourceResponse>>, ApiError> {
    let transaction =
        super::authorization::authorize_transaction(&state, &context, permissions::RESOURCE_READ)
            .await?;
    let models = resources::Entity::find()
        .filter(resources::Column::OrganizationId.eq(context.organization_id))
        .order_by_desc(resources::Column::UpdatedAt)
        .all(&transaction)
        .await
        .map_err(super::error::internal)?;
    let mut response = Vec::with_capacity(models.len());
    for resource in models {
        response.push(resource_response(&transaction, resource).await?);
    }
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(response))
}

async fn get_one(
    context: TenantContext,
    State(state): State<AppState>,
    Path(resource_id): Path<Uuid>,
) -> Result<Json<ResourceResponse>, ApiError> {
    let transaction =
        super::authorization::authorize_transaction(&state, &context, permissions::RESOURCE_READ)
            .await?;
    let resource = find_resource(&transaction, context.organization_id, resource_id).await?;
    let response = resource_response(&transaction, resource).await?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(response))
}

async fn set_desired_state(
    context: TenantContext,
    State(state): State<AppState>,
    Path(resource_id): Path<Uuid>,
    Json(request): Json<SetDesiredStateRequest>,
) -> Result<Json<StateResponse>, ApiError> {
    if request.managed_fields.is_empty() || !request.state.is_object() {
        return Err(ApiError::BadRequest("desired state is invalid"));
    }
    let transaction =
        super::authorization::authorize_transaction(&state, &context, permissions::RESOURCE_MANAGE)
            .await?;
    find_resource(&transaction, context.organization_id, resource_id).await?;
    let previous = resource_desired_states::Entity::find()
        .filter(resource_desired_states::Column::ResourceId.eq(resource_id))
        .order_by_desc(resource_desired_states::Column::Version)
        .one(&transaction)
        .await
        .map_err(super::error::internal)?;
    let version = previous.map_or(1, |state| state.version.saturating_add(1));
    let desired = DesiredState {
        version,
        managed_fields: request.managed_fields,
        state: request.state,
    };
    if let Some(observed) = resource_observed_states::Entity::find()
        .filter(resource_observed_states::Column::ResourceId.eq(resource_id))
        .filter(resource_observed_states::Column::IsLatest.eq(true))
        .one(&transaction)
        .await
        .map_err(super::error::internal)?
    {
        detect_drift(
            &desired,
            &ObservedState {
                version: observed.version,
                state: observed.state,
            },
        )
        .map_err(|_| ApiError::BadRequest("desired state is invalid"))?;
    }
    let now = OffsetDateTime::now_utc();
    let model = resource_desired_states::ActiveModel {
        id: Set(Uuid::now_v7()),
        organization_id: Set(context.organization_id),
        resource_id: Set(resource_id),
        version: Set(version),
        managed_fields: Set(
            serde_json::to_value(&desired.managed_fields).map_err(super::error::internal)?
        ),
        state: Set(desired.state.clone()),
        state_hash: Set(hash_json(&desired.state)?),
        created_by: Set(context.user_id),
        created_at: Set(now),
    }
    .insert(&transaction)
    .await
    .map_err(super::error::internal)?;
    enqueue_event(
        &transaction,
        resource_event(&context, resource_id, "resource.desired_state.changed", now),
    )
    .await
    .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(StateResponse {
        version: model.version,
        state: model.state,
    }))
}

async fn list_drifts(
    context: TenantContext,
    State(state): State<AppState>,
    Path(resource_id): Path<Uuid>,
) -> Result<Json<Vec<DriftResponse>>, ApiError> {
    let transaction =
        super::authorization::authorize_transaction(&state, &context, permissions::RESOURCE_READ)
            .await?;
    find_resource(&transaction, context.organization_id, resource_id).await?;
    let values = resource_drifts::Entity::find()
        .filter(resource_drifts::Column::ResourceId.eq(resource_id))
        .order_by_desc(resource_drifts::Column::DetectedAt)
        .all(&transaction)
        .await
        .map_err(super::error::internal)?
        .into_iter()
        .map(|drift| DriftResponse {
            id: drift.id,
            fingerprint: drift.fingerprint,
            status: drift.status,
            differences: drift.differences,
            detected_at: drift.detected_at,
        })
        .collect();
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(values))
}

async fn list_reconciliations(
    context: TenantContext,
    State(state): State<AppState>,
    Path(resource_id): Path<Uuid>,
) -> Result<Json<Vec<ReconciliationResponse>>, ApiError> {
    let transaction =
        super::authorization::authorize_transaction(&state, &context, permissions::RESOURCE_READ)
            .await?;
    find_resource(&transaction, context.organization_id, resource_id).await?;
    let values = reconciliation_tasks::Entity::find()
        .filter(reconciliation_tasks::Column::ResourceId.eq(resource_id))
        .order_by_desc(reconciliation_tasks::Column::CreatedAt)
        .all(&transaction)
        .await
        .map_err(super::error::internal)?
        .into_iter()
        .map(reconciliation_response)
        .collect();
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(values))
}

async fn approve_reconciliation(
    context: TenantContext,
    State(state): State<AppState>,
    Path((resource_id, task_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ReconciliationResponse>, ApiError> {
    let transaction = super::authorization::authorize_transaction(
        &state,
        &context,
        permissions::RECONCILIATION_MANAGE,
    )
    .await?;
    find_resource(&transaction, context.organization_id, resource_id).await?;
    let task = reconciliation_tasks::Entity::find_by_id(task_id)
        .filter(reconciliation_tasks::Column::ResourceId.eq(resource_id))
        .one(&transaction)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::BadRequest("reconciliation task was not found"))?;
    if task.policy != "manual_approval" || task.status != "pending" {
        return Err(ApiError::Conflict("reconciliation task cannot be approved"));
    }
    let now = OffsetDateTime::now_utc();
    let mut active = task.into_active_model();
    active.status = Set("approved".to_owned());
    active.approved_by = Set(Some(context.user_id));
    active.approved_at = Set(Some(now));
    let task = active
        .update(&transaction)
        .await
        .map_err(super::error::internal)?;
    enqueue_event(
        &transaction,
        resource_event(
            &context,
            resource_id,
            "provider.reconciliation.approved",
            now,
        ),
    )
    .await
    .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(reconciliation_response(task)))
}

async fn find_resource(
    transaction: &sea_orm::DatabaseTransaction,
    organization_id: Uuid,
    resource_id: Uuid,
) -> Result<resources::Model, ApiError> {
    resources::Entity::find_by_id(resource_id)
        .filter(resources::Column::OrganizationId.eq(organization_id))
        .one(transaction)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::BadRequest("resource was not found"))
}

async fn resource_response(
    transaction: &sea_orm::DatabaseTransaction,
    resource: resources::Model,
) -> Result<ResourceResponse, ApiError> {
    let desired = resource_desired_states::Entity::find()
        .filter(resource_desired_states::Column::ResourceId.eq(resource.id))
        .order_by_desc(resource_desired_states::Column::Version)
        .one(transaction)
        .await
        .map_err(super::error::internal)?;
    let observed = resource_observed_states::Entity::find()
        .filter(resource_observed_states::Column::ResourceId.eq(resource.id))
        .filter(resource_observed_states::Column::IsLatest.eq(true))
        .one(transaction)
        .await
        .map_err(super::error::internal)?;
    Ok(ResourceResponse {
        id: resource.id,
        resource_type: resource.resource_type,
        name: resource.name,
        lifecycle: resource.lifecycle,
        region: resource.region,
        attributes: resource.attributes,
        desired_state: desired.map(|state| StateResponse {
            version: state.version,
            state: state.state,
        }),
        observed_state: observed.map(|state| StateResponse {
            version: state.version,
            state: state.state,
        }),
    })
}

fn reconciliation_response(task: reconciliation_tasks::Model) -> ReconciliationResponse {
    ReconciliationResponse {
        id: task.id,
        drift_id: task.drift_id,
        policy: task.policy,
        status: task.status,
        desired_version: task.desired_version,
        operation_id: task.operation_id,
    }
}

fn hash_json(value: &Value) -> Result<String, ApiError> {
    let bytes = serde_json::to_vec(value).map_err(super::error::internal)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn resource_event(
    context: &TenantContext,
    resource_id: Uuid,
    event_type: &str,
    occurred_at: OffsetDateTime,
) -> EventEnvelope {
    EventEnvelope {
        id: multicloud_shared_kernel::EventId::new(),
        organization_id: multicloud_shared_kernel::OrganizationId::from_uuid(
            context.organization_id,
        ),
        aggregate_type: "resource".to_owned(),
        aggregate_id: resource_id.to_string(),
        event_type: event_type.to_owned(),
        event_version: 1,
        payload: serde_json::json!({ "resource_id": resource_id }),
        trace_id: None,
        occurred_at,
    }
}
