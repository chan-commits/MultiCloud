use super::{
    AppState,
    error::ApiError,
    tenant::{TenantContext, set_tenant_context},
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{delete, get, post},
};
use multicloud_authorization::{PermissionKey, permissions as permission_keys, system_role_specs};
use multicloud_operation::EventEnvelope;
use multicloud_persistence::entities::{
    organization_memberships, permissions, role_bindings, role_permissions, roles,
};
use multicloud_persistence::reliable_events::enqueue_event;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, QueryTrait, Set, SqlErr, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/permissions", get(list_permissions))
        .route("/roles", get(list_roles).post(create_role))
        .route("/bindings", post(create_binding))
        .route("/bindings/{binding_id}", delete(delete_binding))
}

pub async fn permission_keys_for(
    state: &AppState,
    context: &TenantContext,
) -> Result<Vec<String>, ApiError> {
    let transaction = state
        .database
        .begin()
        .await
        .map_err(super::error::internal)?;
    set_tenant_context(&transaction, context.user_id, Some(context.organization_id)).await?;
    let keys = load_permission_keys(&transaction, context).await?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(keys)
}

pub async fn authorize_transaction(
    state: &AppState,
    context: &TenantContext,
    required: &str,
) -> Result<DatabaseTransaction, ApiError> {
    let required = PermissionKey::parse(required).map_err(super::error::internal)?;
    let transaction = state
        .database
        .begin()
        .await
        .map_err(super::error::internal)?;
    set_tenant_context(&transaction, context.user_id, Some(context.organization_id)).await?;
    let permissions = load_permission_keys(&transaction, context).await?;
    if !permissions.iter().any(|key| key == required.as_str()) {
        enqueue_authorization_event(
            &transaction,
            context,
            "authorization.access.denied",
            "permission",
            context.membership_id,
            serde_json::json!({
                "requested_by": context.user_id,
                "required_permission": required.as_str(),
            }),
        )
        .await?;
        transaction.commit().await.map_err(super::error::internal)?;
        return Err(ApiError::Forbidden);
    }
    Ok(transaction)
}

async fn load_permission_keys(
    transaction: &DatabaseTransaction,
    context: &TenantContext,
) -> Result<Vec<String>, ApiError> {
    let rows = transaction
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r"
            SELECT DISTINCT permission.key
            FROM permissions AS permission
            JOIN role_permissions AS role_permission
              ON role_permission.permission_id = permission.id
            JOIN role_bindings AS binding
              ON binding.role_id = role_permission.role_id
             AND binding.organization_id = role_permission.organization_id
            WHERE binding.organization_id = $1
              AND binding.subject_type = 'user'
              AND binding.subject_id = $2
              AND binding.scope_type = 'organization'
              AND binding.scope_id = $1
            ORDER BY permission.key
            ",
            [context.organization_id.into(), context.user_id.into()],
        ))
        .await
        .map_err(super::error::internal)?;
    rows.into_iter()
        .map(|row| row.try_get("", "key").map_err(super::error::internal))
        .collect()
}

pub async fn bootstrap_organization_roles(
    transaction: &DatabaseTransaction,
    organization_id: Uuid,
    creator_id: Uuid,
) -> Result<(), ApiError> {
    let catalog = permissions::Entity::find()
        .all(transaction)
        .await
        .map_err(super::error::internal)?;
    let permission_by_key: HashMap<_, _> = catalog
        .iter()
        .map(|permission| (permission.key.as_str(), permission.id))
        .collect();
    let now = OffsetDateTime::now_utc();
    let mut owner_role_id = None;
    for spec in system_role_specs() {
        let role_id = Uuid::now_v7();
        if spec.key == "owner" {
            owner_role_id = Some(role_id);
        }
        roles::ActiveModel {
            id: Set(role_id),
            organization_id: Set(organization_id),
            key: Set(spec.key.to_owned()),
            name: Set(spec.name.to_owned()),
            description: Set(spec.description.to_owned()),
            is_system: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(transaction)
        .await
        .map_err(super::error::internal)?;
        for permission_key in spec.permission_keys {
            let permission_id = permission_by_key
                .get(permission_key)
                .copied()
                .ok_or(ApiError::Internal)?;
            role_permissions::ActiveModel {
                role_id: Set(role_id),
                permission_id: Set(permission_id),
                organization_id: Set(organization_id),
            }
            .insert(transaction)
            .await
            .map_err(super::error::internal)?;
        }
    }
    role_bindings::ActiveModel {
        id: Set(Uuid::now_v7()),
        organization_id: Set(organization_id),
        role_id: Set(owner_role_id.ok_or(ApiError::Internal)?),
        subject_type: Set("user".to_owned()),
        subject_id: Set(creator_id),
        scope_type: Set("organization".to_owned()),
        scope_id: Set(organization_id),
        created_by: Set(creator_id),
        created_at: Set(now),
    }
    .insert(transaction)
    .await
    .map_err(super::error::internal)?;
    Ok(())
}

#[derive(Serialize)]
struct PermissionResponse {
    key: String,
    description: String,
}

async fn list_permissions(
    context: TenantContext,
    State(state): State<AppState>,
) -> Result<Json<Vec<PermissionResponse>>, ApiError> {
    let transaction = authorize_transaction(&state, &context, permission_keys::ROLE_READ).await?;
    let values = permissions::Entity::find()
        .order_by_asc(permissions::Column::Key)
        .all(&transaction)
        .await
        .map_err(super::error::internal)?
        .into_iter()
        .map(|permission| PermissionResponse {
            key: permission.key,
            description: permission.description,
        })
        .collect();
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(values))
}

#[derive(Serialize)]
struct RoleResponse {
    id: Uuid,
    key: String,
    name: String,
    description: String,
    is_system: bool,
    permissions: Vec<String>,
}

async fn list_roles(
    context: TenantContext,
    State(state): State<AppState>,
) -> Result<Json<Vec<RoleResponse>>, ApiError> {
    let transaction = authorize_transaction(&state, &context, permission_keys::ROLE_READ).await?;
    let role_models = roles::Entity::find()
        .filter(roles::Column::OrganizationId.eq(context.organization_id))
        .order_by_asc(roles::Column::Name)
        .all(&transaction)
        .await
        .map_err(super::error::internal)?;
    let mut response = Vec::with_capacity(role_models.len());
    for role in role_models {
        let permission_models = permissions::Entity::find()
            .filter(
                permissions::Column::Id.in_subquery(
                    role_permissions::Entity::find()
                        .select_only()
                        .column(role_permissions::Column::PermissionId)
                        .filter(role_permissions::Column::RoleId.eq(role.id))
                        .into_query(),
                ),
            )
            .order_by_asc(permissions::Column::Key)
            .all(&transaction)
            .await
            .map_err(super::error::internal)?;
        response.push(RoleResponse {
            id: role.id,
            key: role.key,
            name: role.name,
            description: role.description,
            is_system: role.is_system,
            permissions: permission_models
                .into_iter()
                .map(|permission| permission.key)
                .collect(),
        });
    }
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(response))
}

#[derive(Deserialize)]
struct CreateRoleRequest {
    key: String,
    name: String,
    description: Option<String>,
    permissions: Vec<String>,
}

async fn create_role(
    context: TenantContext,
    State(state): State<AppState>,
    Json(request): Json<CreateRoleRequest>,
) -> Result<(axum::http::StatusCode, Json<RoleResponse>), ApiError> {
    let key = request.key.trim().to_lowercase();
    if !valid_role_key(&key) || matches!(key.as_str(), "owner" | "admin" | "member" | "viewer") {
        return Err(ApiError::BadRequest("invalid or reserved role key"));
    }
    let name = request.name.trim();
    if name.is_empty() || name.len() > 120 || request.permissions.is_empty() {
        return Err(ApiError::BadRequest("invalid role definition"));
    }
    let transaction = authorize_transaction(&state, &context, permission_keys::ROLE_MANAGE).await?;
    let permission_models = permissions::Entity::find()
        .filter(permissions::Column::Key.is_in(request.permissions.clone()))
        .all(&transaction)
        .await
        .map_err(super::error::internal)?;
    if permission_models.len() != request.permissions.len() {
        return Err(ApiError::BadRequest("unknown or duplicate permission"));
    }
    let role_id = Uuid::now_v7();
    let description = request.description.unwrap_or_default();
    let now = OffsetDateTime::now_utc();
    roles::ActiveModel {
        id: Set(role_id),
        organization_id: Set(context.organization_id),
        key: Set(key.clone()),
        name: Set(name.to_owned()),
        description: Set(description.clone()),
        is_system: Set(false),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&transaction)
    .await
    .map_err(|error| {
        if matches!(error.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
            ApiError::Conflict("role key is already used")
        } else {
            super::error::internal(error)
        }
    })?;
    for permission in &permission_models {
        role_permissions::ActiveModel {
            role_id: Set(role_id),
            permission_id: Set(permission.id),
            organization_id: Set(context.organization_id),
        }
        .insert(&transaction)
        .await
        .map_err(super::error::internal)?;
    }
    enqueue_authorization_event(
        &transaction,
        &context,
        "authorization.role.created",
        "role",
        role_id,
        serde_json::json!({
            "requested_by": context.user_id,
            "key": key,
            "permissions": request.permissions,
        }),
    )
    .await?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(RoleResponse {
            id: role_id,
            key,
            name: name.to_owned(),
            description,
            is_system: false,
            permissions: permission_models
                .into_iter()
                .map(|permission| permission.key)
                .collect(),
        }),
    ))
}

#[derive(Deserialize)]
struct CreateBindingRequest {
    role_id: Uuid,
    user_id: Uuid,
}

#[derive(Serialize)]
struct BindingResponse {
    id: Uuid,
}

async fn create_binding(
    context: TenantContext,
    State(state): State<AppState>,
    Json(request): Json<CreateBindingRequest>,
) -> Result<(axum::http::StatusCode, Json<BindingResponse>), ApiError> {
    let transaction =
        authorize_transaction(&state, &context, permission_keys::BINDING_MANAGE).await?;
    let role_exists = roles::Entity::find_by_id(request.role_id)
        .filter(roles::Column::OrganizationId.eq(context.organization_id))
        .one(&transaction)
        .await
        .map_err(super::error::internal)?
        .is_some();
    let member_exists = organization_memberships::Entity::find()
        .filter(organization_memberships::Column::OrganizationId.eq(context.organization_id))
        .filter(organization_memberships::Column::UserId.eq(request.user_id))
        .filter(organization_memberships::Column::Status.eq("active"))
        .one(&transaction)
        .await
        .map_err(super::error::internal)?
        .is_some();
    if !role_exists || !member_exists {
        return Err(ApiError::BadRequest("role or member was not found"));
    }
    let id = Uuid::now_v7();
    role_bindings::ActiveModel {
        id: Set(id),
        organization_id: Set(context.organization_id),
        role_id: Set(request.role_id),
        subject_type: Set("user".to_owned()),
        subject_id: Set(request.user_id),
        scope_type: Set("organization".to_owned()),
        scope_id: Set(context.organization_id),
        created_by: Set(context.user_id),
        created_at: Set(OffsetDateTime::now_utc()),
    }
    .insert(&transaction)
    .await
    .map_err(|error| {
        if matches!(error.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
            ApiError::Conflict("role binding already exists")
        } else {
            super::error::internal(error)
        }
    })?;
    enqueue_authorization_event(
        &transaction,
        &context,
        "authorization.binding.created",
        "role_binding",
        id,
        serde_json::json!({
            "requested_by": context.user_id,
            "role_id": request.role_id,
            "subject_id": request.user_id,
        }),
    )
    .await?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(BindingResponse { id }),
    ))
}

async fn delete_binding(
    context: TenantContext,
    State(state): State<AppState>,
    Path(binding_id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let transaction =
        authorize_transaction(&state, &context, permission_keys::BINDING_MANAGE).await?;
    let binding = role_bindings::Entity::find_by_id(binding_id)
        .one(&transaction)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::BadRequest("role binding was not found"))?;
    let role = roles::Entity::find_by_id(binding.role_id)
        .one(&transaction)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::BadRequest("role was not found"))?;
    if role.key == "owner" && binding.subject_id == context.user_id {
        return Err(ApiError::BadRequest("cannot remove your own owner binding"));
    }
    role_bindings::Entity::delete_by_id(binding_id)
        .exec(&transaction)
        .await
        .map_err(super::error::internal)?;
    enqueue_authorization_event(
        &transaction,
        &context,
        "authorization.binding.deleted",
        "role_binding",
        binding_id,
        serde_json::json!({
            "requested_by": context.user_id,
            "role_id": binding.role_id,
            "subject_id": binding.subject_id,
        }),
    )
    .await?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

fn valid_role_key(value: &str) -> bool {
    let bytes = value.as_bytes();
    (3..=80).contains(&bytes.len())
        && bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

async fn enqueue_authorization_event(
    transaction: &DatabaseTransaction,
    context: &TenantContext,
    event_type: &str,
    aggregate_type: &str,
    aggregate_id: Uuid,
    payload: serde_json::Value,
) -> Result<(), ApiError> {
    enqueue_event(
        transaction,
        EventEnvelope {
            id: multicloud_shared_kernel::EventId::new(),
            organization_id: multicloud_shared_kernel::OrganizationId::from_uuid(
                context.organization_id,
            ),
            aggregate_type: aggregate_type.to_owned(),
            aggregate_id: aggregate_id.to_string(),
            event_type: event_type.to_owned(),
            event_version: 1,
            payload,
            trace_id: None,
            occurred_at: OffsetDateTime::now_utc(),
        },
    )
    .await
    .map_err(super::error::internal)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_role_key_rejects_invalid_values() {
        assert!(valid_role_key("support-agent"));
        assert!(!valid_role_key("Owner"));
        assert!(!valid_role_key("-support"));
    }
}
