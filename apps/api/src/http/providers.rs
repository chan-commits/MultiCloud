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
    provider_operations::{NewProviderOperation, create_provider_operation},
    reliable_events::{NewOperation, create_operation_with_event, enqueue_event},
};
use multicloud_provider::{
    CredentialRiskLevel, CredentialType, EncryptedCredential, ProviderError, ProviderKind,
    decode_credential_envelope,
};
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
        .route("/{account_id}/sync", post(queue_sync))
        .route("/{account_id}/operations", post(queue_operation))
        .route("/{account_id}/disable", post(disable))
}

#[derive(Deserialize)]
struct CreateProviderAccountRequest {
    provider_kind: String,
    name: String,
    #[serde(flatten)]
    credential: CredentialInput,
    #[serde(default = "empty_object")]
    configuration: Value,
}

#[derive(Deserialize)]
struct RotateCredentialRequest {
    #[serde(flatten)]
    credential: CredentialInput,
}

#[derive(Deserialize)]
struct CredentialInput {
    credential_type: Option<CredentialType>,
    api_token: Option<String>,
    email: Option<String>,
    global_api_key: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct StoredCredential {
    schema_version: u16,
    credential_type: CredentialType,
    identity: Option<String>,
    secret: String,
}

struct PreparedCredential {
    stored: StoredCredential,
    risk_level: CredentialRiskLevel,
    masked_identifier: String,
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
    credential_type: Option<String>,
    credential_risk_level: Option<String>,
    credential_masked_identifier: Option<String>,
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

#[derive(Deserialize)]
struct SyncRequest {
    resource_type: String,
    parent_external_id: Option<String>,
    cursor: Option<String>,
    idempotency_key: String,
}

#[derive(Deserialize)]
struct QueueProviderOperationRequest {
    action: String,
    resource_type: String,
    external_id: Option<String>,
    #[serde(default = "empty_object")]
    parameters: Value,
    idempotency_key: String,
}

#[derive(Serialize)]
struct QueuedOperationResponse {
    operation_id: Uuid,
    status: String,
}

fn empty_object() -> Value {
    serde_json::json!({})
}

fn validate_name(name: &str) -> Result<(), ApiError> {
    if name.trim().is_empty() || name.len() > 160 {
        return Err(ApiError::BadRequest("provider account name is invalid"));
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
    let mut response = Vec::with_capacity(accounts.len());
    for account in accounts {
        response.push(account_response(&transaction, account).await?);
    }
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(response))
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
    let response = account_response(&transaction, account).await?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(response))
}

async fn create(
    context: TenantContext,
    State(state): State<AppState>,
    Json(request): Json<CreateProviderAccountRequest>,
) -> Result<Json<ProviderAccountResponse>, ApiError> {
    validate_name(&request.name)?;
    let provider_kind = ProviderKind::parse(&request.provider_kind)
        .map_err(|_| ApiError::BadRequest("provider kind is invalid"))?;
    state
        .provider_registry
        .get(&provider_kind)
        .map_err(|_| ApiError::BadRequest("provider kind is not registered"))?;
    let prepared = prepare_credential(&provider_kind, request.credential)?;
    let plaintext = serde_json::to_string(&prepared.stored).map_err(super::error::internal)?;
    let encrypted = state
        .credential_cipher
        .encrypt(&plaintext)
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
    insert_credential(
        &transaction,
        &context,
        account.id,
        1,
        encrypted,
        &prepared,
        now,
    )
    .await?;
    enqueue_event(
        &transaction,
        provider_event(
            &context,
            account.id,
            "provider.account.created",
            serde_json::json!({
                "provider_account_id": account.id,
                "provider_kind": account.provider_kind,
                "credential_type": credential_type_key(prepared.stored.credential_type),
                "risk_level": risk_level_key(prepared.risk_level),
            }),
            now,
        ),
    )
    .await
    .map_err(super::error::internal)?;
    let response = account_response(&transaction, account).await?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(response))
}

async fn rotate_credential(
    context: TenantContext,
    State(state): State<AppState>,
    Path(account_id): Path<Uuid>,
    Json(request): Json<RotateCredentialRequest>,
) -> Result<Json<ProviderAccountResponse>, ApiError> {
    let transaction = super::authorization::authorize_transaction(
        &state,
        &context,
        permissions::PROVIDER_ACCOUNT_MANAGE,
    )
    .await?;
    let account = find_account(&transaction, context.organization_id, account_id).await?;
    let provider_kind =
        ProviderKind::parse(account.provider_kind.clone()).map_err(super::error::internal)?;
    let prepared = prepare_credential(&provider_kind, request.credential)?;
    let plaintext = serde_json::to_string(&prepared.stored).map_err(super::error::internal)?;
    let encrypted = state
        .credential_cipher
        .encrypt(&plaintext)
        .map_err(super::error::internal)?;
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
        &prepared,
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
            serde_json::json!({
                "provider_account_id": account_id,
                "version": next_version,
                "credential_type": credential_type_key(prepared.stored.credential_type),
                "risk_level": risk_level_key(prepared.risk_level),
            }),
            now,
        ),
    )
    .await
    .map_err(super::error::internal)?;
    let response = account_response(&transaction, account).await?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(response))
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
    let credential_type = credential.credential_type.clone();
    let plaintext = state
        .credential_cipher
        .decrypt(&EncryptedCredential {
            ciphertext: credential.ciphertext,
            nonce: credential.nonce,
            key_version: credential.key_version,
        })
        .map_err(super::error::internal)?;
    let material = decode_credential_envelope(&plaintext, &credential_type);
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

async fn queue_sync(
    context: TenantContext,
    State(state): State<AppState>,
    Path(account_id): Path<Uuid>,
    Json(request): Json<SyncRequest>,
) -> Result<Json<QueuedOperationResponse>, ApiError> {
    if !matches!(request.resource_type.as_str(), "dns_zone" | "dns_record") {
        return Err(ApiError::BadRequest("resource type is not supported"));
    }
    validate_idempotency_key(&request.idempotency_key)?;
    let transaction =
        super::authorization::authorize_transaction(&state, &context, permissions::RESOURCE_SYNC)
            .await?;
    let account = find_account(&transaction, context.organization_id, account_id).await?;
    require_active_capability(&account, "dns")?;
    let operation = create_provider_operation(
        &transaction,
        NewProviderOperation {
            organization_id: context.organization_id,
            provider_account_id: account_id,
            requested_by: context.user_id,
            action: "inventory.sync",
            resource_type: &request.resource_type,
            external_id: request.parent_external_id.as_deref(),
            parameters: serde_json::json!({
                "parent_external_id": request.parent_external_id,
                "cursor": request.cursor,
            }),
            idempotency_key: &request.idempotency_key,
        },
    )
    .await
    .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(QueuedOperationResponse {
        operation_id: operation.id,
        status: operation.status,
    }))
}

async fn queue_operation(
    context: TenantContext,
    State(state): State<AppState>,
    Path(account_id): Path<Uuid>,
    Json(request): Json<QueueProviderOperationRequest>,
) -> Result<Json<QueuedOperationResponse>, ApiError> {
    if !matches!(request.action.as_str(), "create" | "update" | "delete")
        || request.resource_type != "dns_record"
    {
        return Err(ApiError::BadRequest("provider operation is not supported"));
    }
    validate_idempotency_key(&request.idempotency_key)?;
    let transaction =
        super::authorization::authorize_transaction(&state, &context, permissions::RESOURCE_MANAGE)
            .await?;
    let account = find_account(&transaction, context.organization_id, account_id).await?;
    require_active_capability(&account, "dns")?;
    let operation = create_provider_operation(
        &transaction,
        NewProviderOperation {
            organization_id: context.organization_id,
            provider_account_id: account_id,
            requested_by: context.user_id,
            action: &request.action,
            resource_type: &request.resource_type,
            external_id: request.external_id.as_deref(),
            parameters: request.parameters,
            idempotency_key: &request.idempotency_key,
        },
    )
    .await
    .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(QueuedOperationResponse {
        operation_id: operation.id,
        status: operation.status,
    }))
}

fn validate_idempotency_key(value: &str) -> Result<(), ApiError> {
    if value.is_empty() || value.len() > 255 {
        Err(ApiError::BadRequest("idempotency key is invalid"))
    } else {
        Ok(())
    }
}

fn require_active_capability(
    account: &provider_accounts::Model,
    capability: &str,
) -> Result<(), ApiError> {
    if account.status != "active" {
        return Err(ApiError::Conflict("provider account is not active"));
    }
    let has_capability = account.capabilities.as_array().is_some_and(|values| {
        values
            .iter()
            .any(|value| value.as_str() == Some(capability))
    });
    if has_capability {
        Ok(())
    } else {
        Err(ApiError::Conflict("provider capability is not available"))
    }
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
    let response = account_response(&transaction, account).await?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(response))
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
    prepared: &PreparedCredential,
    now: OffsetDateTime,
) -> Result<(), ApiError> {
    provider_credentials::ActiveModel {
        id: Set(Uuid::now_v7()),
        organization_id: Set(context.organization_id),
        provider_account_id: Set(account_id),
        credential_type: Set(credential_type_key(prepared.stored.credential_type).to_owned()),
        risk_level: Set(risk_level_key(prepared.risk_level).to_owned()),
        masked_identifier: Set(Some(prepared.masked_identifier.clone())),
        schema_version: Set(1),
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

async fn account_response(
    transaction: &sea_orm::DatabaseTransaction,
    account: provider_accounts::Model,
) -> Result<ProviderAccountResponse, ApiError> {
    let credential = provider_credentials::Entity::find()
        .filter(provider_credentials::Column::ProviderAccountId.eq(account.id))
        .order_by_desc(provider_credentials::Column::Version)
        .one(transaction)
        .await
        .map_err(super::error::internal)?;
    Ok(ProviderAccountResponse {
        id: account.id,
        provider_kind: account.provider_kind,
        name: account.name,
        status: account.status,
        configuration: account.configuration,
        capabilities: account.capabilities,
        last_validated_at: account.last_validated_at,
        last_error_code: account.last_error_code,
        credential_type: credential
            .as_ref()
            .map(|credential| credential.credential_type.clone()),
        credential_risk_level: credential
            .as_ref()
            .map(|credential| credential.risk_level.clone()),
        credential_masked_identifier: credential
            .and_then(|credential| credential.masked_identifier),
        created_at: account.created_at,
    })
}

fn prepare_credential(
    provider_kind: &ProviderKind,
    input: CredentialInput,
) -> Result<PreparedCredential, ApiError> {
    let credential_type = input.credential_type.unwrap_or_else(|| {
        if input.global_api_key.is_some() {
            CredentialType::GlobalApiKey
        } else {
            CredentialType::ApiToken
        }
    });
    let (identity, secret, risk_level, masked_identifier) = match credential_type {
        CredentialType::ApiToken => {
            let secret = input
                .api_token
                .filter(|value| !value.trim().is_empty() && value.len() <= 8_192)
                .ok_or(ApiError::BadRequest("provider API token is invalid"))?;
            let masked = mask_secret(&secret);
            (None, secret, CredentialRiskLevel::Restricted, masked)
        }
        CredentialType::GlobalApiKey => {
            if provider_kind.as_str() != "cloudflare" {
                return Err(ApiError::BadRequest(
                    "global API key is only supported for Cloudflare",
                ));
            }
            let identity = input
                .email
                .filter(|value| value.contains('@') && value.len() <= 320)
                .ok_or(ApiError::BadRequest("Cloudflare account email is required"))?;
            let secret = input
                .global_api_key
                .filter(|value| !value.trim().is_empty() && value.len() <= 8_192)
                .ok_or(ApiError::BadRequest("Cloudflare Global API Key is invalid"))?;
            let masked = mask_email(&identity);
            (Some(identity), secret, CredentialRiskLevel::High, masked)
        }
        CredentialType::Opaque => {
            return Err(ApiError::BadRequest("credential type is not supported"));
        }
    };
    Ok(PreparedCredential {
        stored: StoredCredential {
            schema_version: 1,
            credential_type,
            identity,
            secret,
        },
        risk_level,
        masked_identifier,
    })
}

const fn credential_type_key(value: CredentialType) -> &'static str {
    match value {
        CredentialType::ApiToken => "api_token",
        CredentialType::GlobalApiKey => "global_api_key",
        CredentialType::Opaque => "opaque",
    }
}

const fn risk_level_key(value: CredentialRiskLevel) -> &'static str {
    match value {
        CredentialRiskLevel::Restricted => "restricted",
        CredentialRiskLevel::High => "high",
    }
}

fn mask_secret(value: &str) -> String {
    let suffix: String = value
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("****{suffix}")
}

fn mask_email(value: &str) -> String {
    value.split_once('@').map_or_else(
        || "***".to_owned(),
        |(local, domain)| {
            let first = local.chars().next().unwrap_or('*');
            format!("{first}***@{domain}")
        },
    )
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
