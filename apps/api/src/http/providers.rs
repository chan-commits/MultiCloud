use super::{AppState, error::ApiError, tenant::TenantContext};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use multicloud_authorization::permissions;
use multicloud_operation::EventEnvelope;
use multicloud_persistence::{
    entities::{operations, provider_accounts, provider_credentials},
    reliable_events::{NewOperation, create_operation_with_event, enqueue_event},
};
use multicloud_provider::{CredentialMaterial, EncryptedCredential, ProviderError, ProviderKind};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{account_id}", get(get_one))
        .route("/{account_id}/credentials", post(rotate_credential))
        .route("/{account_id}/connection-test", post(test_connection))
        .route("/{account_id}/disable", post(disable))
}

#[derive(Deserialize)]
struct CreateProviderAccountRequest {
    provider_kind: String,
    name: String,
    api_token: String,
    #[serde(default = "empty_object")]
    configuration: Value,
}

#[derive(Deserialize)]
struct RotateCredentialRequest {
    api_token: String,
}

#[derive(Serialize)]
struct ProviderAccountResponse {
    id: Uuid,
    provider_kind: String,
    name: String,
    status: String,
    configuration: Value,
    capabilities: Value,
    #[serde(with = "time::serde::rfc3339::option")]
    last_validated_at: Option<OffsetDateTime>,
    last_error_code: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

#[derive(Serialize)]
struct ConnectionTestResponse {
    operation_id: Uuid,
    valid: bool,
    capabilities: Value,
    error_code: Option<String>,
}

impl From<provider_accounts::Model> for ProviderAccountResponse {
    fn from(account: provider_accounts::Model) -> Self {
        Self {
            id: account.id,
            provider_kind: account.provider_kind,
            name: account.name,
            status: account.status,
            configuration: account.configuration,
            capabilities: account.capabilities,
            last_validated_at: account.last_validated_at,
            last_error_code: account.last_error_code,
            created_at: account.created_at,
        }
    }
}

fn empty_object() -> Value {
    serde_json::json!({})
}

fn validate_name_and_token(name: &str, token: &str) -> Result<(), ApiError> {
    if name.trim().is_empty() || name.len() > 160 {
        return Err(ApiError::BadRequest("provider account name is invalid"));
    }
    if token.trim().is_empty() || token.len() > 8_192 {
        return Err(ApiError::BadRequest("provider API token is invalid"));
    }
    Ok(())
}

async fn list(
    context: TenantContext,
    State(state): State<AppState>,
) -> Result<Json<Vec<ProviderAccountResponse>>, ApiError> {
    let transaction = super::authorization::authorize_transaction(
        &state,
        &context,
        permissions::PROVIDER_ACCOUNT_READ,
    )
    .await?;
    let accounts = provider_accounts::Entity::find()
        .filter(provider_accounts::Column::OrganizationId.eq(context.organization_id))
        .order_by_desc(provider_accounts::Column::CreatedAt)
        .all(&transaction)
        .await
        .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(accounts.into_iter().map(Into::into).collect()))
}

async fn get_one(
    context: TenantContext,
    State(state): State<AppState>,
    Path(account_id): Path<Uuid>,
) -> Result<Json<ProviderAccountResponse>, ApiError> {
    let transaction = super::authorization::authorize_transaction(
        &state,
        &context,
        permissions::PROVIDER_ACCOUNT_READ,
    )
    .await?;
    let account = find_account(&transaction, context.organization_id, account_id).await?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(account.into()))
}

async fn create(
    context: TenantContext,
    State(state): State<AppState>,
    Json(request): Json<CreateProviderAccountRequest>,
) -> Result<Json<ProviderAccountResponse>, ApiError> {
    validate_name_and_token(&request.name, &request.api_token)?;
    let provider_kind = ProviderKind::parse(&request.provider_kind)
        .map_err(|_| ApiError::BadRequest("provider kind is invalid"))?;
    state
        .provider_registry
        .get(&provider_kind)
        .map_err(|_| ApiError::BadRequest("provider kind is not registered"))?;
    let encrypted = state
        .credential_cipher
        .encrypt(&request.api_token)
        .map_err(super::error::internal)?;
    let transaction = super::authorization::authorize_transaction(
        &state,
        &context,
        permissions::PROVIDER_ACCOUNT_MANAGE,
    )
    .await?;
    let now = OffsetDateTime::now_utc();
    let account = provider_accounts::ActiveModel {
        id: Set(Uuid::now_v7()),
        organization_id: Set(context.organization_id),
        provider_kind: Set(provider_kind.to_string()),
        name: Set(request.name.trim().to_owned()),
        status: Set("pending_validation".to_owned()),
        configuration: Set(request.configuration),
        capabilities: Set(serde_json::json!([])),
        last_validated_at: Set(None),
        last_error_code: Set(None),
        created_by: Set(context.user_id),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&transaction)
    .await
    .map_err(super::error::internal)?;
    insert_credential(&transaction, &context, account.id, 1, encrypted, now).await?;
    enqueue_event(
        &transaction,
        provider_event(
            &context,
            account.id,
            "provider.account.created",
            serde_json::json!({
                "provider_account_id": account.id,
                "provider_kind": account.provider_kind,
            }),
            now,
        ),
    )
    .await
    .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(account.into()))
}

async fn rotate_credential(
    context: TenantContext,
    State(state): State<AppState>,
    Path(account_id): Path<Uuid>,
    Json(request): Json<RotateCredentialRequest>,
) -> Result<Json<ProviderAccountResponse>, ApiError> {
    validate_name_and_token("credential", &request.api_token)?;
    let encrypted = state
        .credential_cipher
        .encrypt(&request.api_token)
        .map_err(super::error::internal)?;
    let transaction = super::authorization::authorize_transaction(
        &state,
        &context,
        permissions::PROVIDER_ACCOUNT_MANAGE,
    )
    .await?;
    let account = find_account(&transaction, context.organization_id, account_id).await?;
    let active = active_credential(&transaction, context.organization_id, account_id).await?;
    let next_version = active.version.checked_add(1).ok_or(ApiError::Internal)?;
    let now = OffsetDateTime::now_utc();
    let mut revoked = active.into_active_model();
    revoked.status = Set("revoked".to_owned());
    revoked.revoked_at = Set(Some(now));
    revoked
        .update(&transaction)
        .await
        .map_err(super::error::internal)?;
    insert_credential(
        &transaction,
        &context,
        account_id,
        next_version,
        encrypted,
        now,
    )
    .await?;
    let mut pending = account.into_active_model();
    pending.status = Set("pending_validation".to_owned());
    pending.capabilities = Set(serde_json::json!([]));
    pending.last_error_code = Set(None);
    pending.updated_at = Set(now);
    let account = pending
        .update(&transaction)
        .await
        .map_err(super::error::internal)?;
    enqueue_event(
        &transaction,
        provider_event(
            &context,
            account_id,
            "provider.credential.rotated",
            serde_json::json!({ "provider_account_id": account_id, "version": next_version }),
            now,
        ),
    )
    .await
    .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(account.into()))
}

async fn test_connection(
    context: TenantContext,
    State(state): State<AppState>,
    Path(account_id): Path<Uuid>,
) -> Result<Json<ConnectionTestResponse>, ApiError> {
    let transaction = super::authorization::authorize_transaction(
        &state,
        &context,
        permissions::PROVIDER_CONNECTION_TEST,
    )
    .await?;
    let account = find_account(&transaction, context.organization_id, account_id).await?;
    let credential = active_credential(&transaction, context.organization_id, account_id).await?;
    let secret = state
        .credential_cipher
        .decrypt(&EncryptedCredential {
            ciphertext: credential.ciphertext,
            nonce: credential.nonce,
            key_version: credential.key_version,
        })
        .map_err(super::error::internal)?;
    let operation = create_operation_with_event(
        &transaction,
        NewOperation {
            organization_id: context.organization_id,
            operation_type: "provider.connection_test",
            target_type: "provider_account",
            target_id: Some(&account_id.to_string()),
            requested_by: context.user_id,
            idempotency_key: &format!("connection-test:{account_id}:{}", Uuid::now_v7()),
        },
        "provider.connection_test.requested",
        serde_json::json!({ "provider_account_id": account_id }),
        None,
    )
    .await
    .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;

    let kind =
        ProviderKind::parse(account.provider_kind.clone()).map_err(super::error::internal)?;
    let adapter = state
        .provider_registry
        .get(&kind)
        .map_err(|error| map_provider_error(&error))?;
    let material = CredentialMaterial { secret };
    let result = async {
        adapter.validate_credential(&material).await?;
        adapter.discover_capabilities(&material).await
    }
    .await;
    persist_connection_result(&state, &context, account, operation, result).await
}

async fn persist_connection_result(
    state: &AppState,
    context: &TenantContext,
    account: provider_accounts::Model,
    operation: operations::Model,
    result: Result<Vec<multicloud_provider::Capability>, ProviderError>,
) -> Result<Json<ConnectionTestResponse>, ApiError> {
    let transaction = super::authorization::authorize_transaction(
        state,
        context,
        permissions::PROVIDER_CONNECTION_TEST,
    )
    .await?;
    let now = OffsetDateTime::now_utc();
    let (valid, capabilities, error_code) = match result {
        Ok(capabilities) => (
            true,
            serde_json::to_value(capabilities).map_err(super::error::internal)?,
            None,
        ),
        Err(error) => (false, serde_json::json!([]), Some(error.code)),
    };
    let account_id = account.id;
    let operation_id = operation.id;
    let mut account = account.into_active_model();
    account.status = Set(if valid { "active" } else { "invalid" }.to_owned());
    account.capabilities = Set(capabilities.clone());
    account.last_validated_at = Set(Some(now));
    account.last_error_code = Set(error_code.clone());
    account.updated_at = Set(now);
    account
        .update(&transaction)
        .await
        .map_err(super::error::internal)?;
    let mut operation = operation.into_active_model();
    operation.status = Set(if valid { "succeeded" } else { "failed" }.to_owned());
    operation.progress = Set(100);
    operation.error_code = Set(error_code.clone());
    operation.completed_at = Set(Some(now));
    operation.updated_at = Set(now);
    operation
        .update(&transaction)
        .await
        .map_err(super::error::internal)?;
    enqueue_event(
        &transaction,
        provider_event(
            context,
            account_id,
            "provider.connection_test.completed",
            serde_json::json!({
                "provider_account_id": account_id,
                "operation_id": operation_id,
                "valid": valid,
                "capabilities": capabilities,
                "error_code": error_code,
            }),
            now,
        ),
    )
    .await
    .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(ConnectionTestResponse {
        operation_id,
        valid,
        capabilities,
        error_code,
    }))
}

async fn disable(
    context: TenantContext,
    State(state): State<AppState>,
    Path(account_id): Path<Uuid>,
) -> Result<Json<ProviderAccountResponse>, ApiError> {
    let transaction = super::authorization::authorize_transaction(
        &state,
        &context,
        permissions::PROVIDER_ACCOUNT_MANAGE,
    )
    .await?;
    let account = find_account(&transaction, context.organization_id, account_id).await?;
    let now = OffsetDateTime::now_utc();
    if let Some(credential) = provider_credentials::Entity::find()
        .filter(provider_credentials::Column::OrganizationId.eq(context.organization_id))
        .filter(provider_credentials::Column::ProviderAccountId.eq(account_id))
        .filter(provider_credentials::Column::Status.eq("active"))
        .one(&transaction)
        .await
        .map_err(super::error::internal)?
    {
        let mut revoked = credential.into_active_model();
        revoked.status = Set("revoked".to_owned());
        revoked.revoked_at = Set(Some(now));
        revoked
            .update(&transaction)
            .await
            .map_err(super::error::internal)?;
    }
    let mut active = account.into_active_model();
    active.status = Set("disabled".to_owned());
    active.updated_at = Set(now);
    let account = active
        .update(&transaction)
        .await
        .map_err(super::error::internal)?;
    enqueue_event(
        &transaction,
        provider_event(
            &context,
            account_id,
            "provider.account.disabled",
            serde_json::json!({ "provider_account_id": account_id }),
            now,
        ),
    )
    .await
    .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(account.into()))
}

async fn find_account(
    transaction: &sea_orm::DatabaseTransaction,
    organization_id: Uuid,
    account_id: Uuid,
) -> Result<provider_accounts::Model, ApiError> {
    provider_accounts::Entity::find_by_id(account_id)
        .filter(provider_accounts::Column::OrganizationId.eq(organization_id))
        .one(transaction)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::BadRequest("provider account was not found"))
}

async fn active_credential(
    transaction: &sea_orm::DatabaseTransaction,
    organization_id: Uuid,
    account_id: Uuid,
) -> Result<provider_credentials::Model, ApiError> {
    provider_credentials::Entity::find()
        .filter(provider_credentials::Column::OrganizationId.eq(organization_id))
        .filter(provider_credentials::Column::ProviderAccountId.eq(account_id))
        .filter(provider_credentials::Column::Status.eq("active"))
        .one(transaction)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::BadRequest(
            "provider account has no active credential",
        ))
}

async fn insert_credential(
    transaction: &sea_orm::DatabaseTransaction,
    context: &TenantContext,
    account_id: Uuid,
    version: i32,
    encrypted: EncryptedCredential,
    now: OffsetDateTime,
) -> Result<(), ApiError> {
    provider_credentials::ActiveModel {
        id: Set(Uuid::now_v7()),
        organization_id: Set(context.organization_id),
        provider_account_id: Set(account_id),
        credential_type: Set("api_token".to_owned()),
        ciphertext: Set(encrypted.ciphertext),
        nonce: Set(encrypted.nonce),
        key_version: Set(encrypted.key_version),
        version: Set(version),
        status: Set("active".to_owned()),
        created_by: Set(context.user_id),
        created_at: Set(now),
        activated_at: Set(now),
        revoked_at: Set(None),
    }
    .insert(transaction)
    .await
    .map_err(super::error::internal)?;
    Ok(())
}

fn provider_event(
    context: &TenantContext,
    account_id: Uuid,
    event_type: &str,
    payload: Value,
    occurred_at: OffsetDateTime,
) -> EventEnvelope {
    EventEnvelope {
        id: multicloud_shared_kernel::EventId::new(),
        organization_id: multicloud_shared_kernel::OrganizationId::from_uuid(
            context.organization_id,
        ),
        aggregate_type: "provider_account".to_owned(),
        aggregate_id: account_id.to_string(),
        event_type: event_type.to_owned(),
        event_version: 1,
        payload,
        trace_id: None,
        occurred_at,
    }
}

fn map_provider_error(error: &ProviderError) -> ApiError {
    tracing::warn!(
        code = error.code,
        retryable = error.retryable,
        "provider request failed"
    );
    ApiError::BadRequest("provider request failed")
}
