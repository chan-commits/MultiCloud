use super::{AppState, error::ApiError};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use multicloud_identity::Email;
use multicloud_operation::EventEnvelope;
use multicloud_persistence::{
    entities::{platform_settings, sessions, users},
    reliable_events::enqueue_event,
};
use rand::Rng;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route(
            "/registration-settings",
            get(registration_settings).put(update_registration_settings),
        )
}

#[derive(Deserialize)]
struct RegisterRequest {
    email: String,
    password: String,
    display_name: String,
}

#[derive(Serialize)]
struct UserResponse {
    id: Uuid,
    email: String,
    display_name: String,
}

#[derive(Deserialize)]
struct UpdateRegistrationSettingsRequest {
    registration_enabled: bool,
}

#[derive(Serialize)]
struct RegistrationSettingsResponse {
    initialized: bool,
    registration_enabled: bool,
}

async fn registration_settings(
    State(state): State<AppState>,
) -> Result<Json<RegistrationSettingsResponse>, ApiError> {
    let initialized = users::Entity::find()
        .one(&state.database)
        .await
        .map_err(super::error::internal)?
        .is_some();
    let settings = platform_settings::Entity::find_by_id(1_i16)
        .one(&state.database)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::Internal)?;
    Ok(Json(RegistrationSettingsResponse {
        initialized,
        registration_enabled: initialized && settings.registration_enabled,
    }))
}

async fn update_registration_settings(
    context: super::tenant::TenantContext,
    State(state): State<AppState>,
    Json(request): Json<UpdateRegistrationSettingsRequest>,
) -> Result<Json<RegistrationSettingsResponse>, ApiError> {
    let user = users::Entity::find_by_id(context.user_id)
        .one(&state.database)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::Unauthorized)?;
    if !user.is_platform_admin {
        return Err(ApiError::Forbidden);
    }
    let transaction = state
        .database
        .begin()
        .await
        .map_err(super::error::internal)?;
    super::tenant::set_tenant_context(&transaction, context.user_id, Some(context.organization_id))
        .await?;
    let mut settings: platform_settings::ActiveModel = platform_settings::Entity::find_by_id(1_i16)
        .one(&transaction)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::Internal)?
        .into();
    let previous_enabled = settings.registration_enabled.as_ref().to_owned();
    settings.registration_enabled = Set(request.registration_enabled);
    settings.updated_by = Set(Some(context.user_id));
    settings.updated_at = Set(OffsetDateTime::now_utc());
    settings
        .update(&transaction)
        .await
        .map_err(super::error::internal)?;
    enqueue_event(
        &transaction,
        EventEnvelope {
            id: multicloud_shared_kernel::EventId::new(),
            organization_id: multicloud_shared_kernel::OrganizationId::from_uuid(
                context.organization_id,
            ),
            aggregate_type: "platform_settings".to_owned(),
            aggregate_id: "registration".to_owned(),
            event_type: "identity.registration.updated".to_owned(),
            event_version: 1,
            payload: serde_json::json!({
                "requested_by": context.user_id,
                "registration_enabled": request.registration_enabled,
                "before": { "registration_enabled": previous_enabled },
                "after": { "registration_enabled": request.registration_enabled },
            }),
            trace_id: None,
            occurred_at: OffsetDateTime::now_utc(),
        },
    )
    .await
    .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(RegistrationSettingsResponse {
        initialized: true,
        registration_enabled: request.registration_enabled,
    }))
}

async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<(axum::http::StatusCode, Json<UserResponse>), ApiError> {
    if users::Entity::find()
        .one(&state.database)
        .await
        .map_err(super::error::internal)?
        .is_none()
    {
        return Err(ApiError::Unavailable(
            "platform administrator must be initialized before public registration",
        ));
    }
    let registration_enabled = platform_settings::Entity::find_by_id(1_i16)
        .one(&state.database)
        .await
        .map_err(super::error::internal)?
        .is_some_and(|settings| settings.registration_enabled);
    if !registration_enabled {
        return Err(ApiError::Forbidden);
    }
    let email = Email::parse(request.email).map_err(|_| ApiError::BadRequest("invalid email"))?;
    if request.password.len() < 12 {
        return Err(ApiError::BadRequest(
            "password must contain at least 12 characters",
        ));
    }
    let display_name = request.display_name.trim();
    if display_name.is_empty() || display_name.len() > 120 {
        return Err(ApiError::BadRequest("invalid display name"));
    }
    let exists = users::Entity::find()
        .filter(users::Column::Email.eq(email.as_str()))
        .one(&state.database)
        .await
        .map_err(super::error::internal)?
        .is_some();
    if exists {
        return Err(ApiError::Conflict("email is already registered"));
    }

    let mut salt_bytes = [0_u8; 16];
    rand::rng().fill_bytes(&mut salt_bytes);
    let salt = SaltString::encode_b64(&salt_bytes).map_err(super::error::internal)?;
    let password_hash = Argon2::default()
        .hash_password(request.password.as_bytes(), &salt)
        .map_err(super::error::internal)?
        .to_string();
    let id = Uuid::now_v7();
    let now = OffsetDateTime::now_utc();
    users::ActiveModel {
        id: Set(id),
        email: Set(email.to_string()),
        display_name: Set(display_name.to_owned()),
        status: Set("active".to_owned()),
        password_hash: Set(password_hash),
        is_platform_admin: Set(false),
        email_verified_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&state.database)
    .await
    .map_err(super::error::internal)?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(UserResponse {
            id,
            email: email.to_string(),
            display_name: display_name.to_owned(),
        }),
    ))
}

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    access_token: String,
    token_type: &'static str,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
    is_platform_admin: bool,
}

async fn logout(
    identity: super::tenant::AuthIdentity,
    State(state): State<AppState>,
) -> Result<axum::http::StatusCode, ApiError> {
    sessions::Entity::update_many()
        .col_expr(
            sessions::Column::RevokedAt,
            sea_orm::sea_query::Expr::value(OffsetDateTime::now_utc()),
        )
        .filter(sessions::Column::Id.eq(identity.session_id))
        .exec(&state.database)
        .await
        .map_err(super::error::internal)?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let email = Email::parse(request.email).map_err(|_| ApiError::Unauthorized)?;
    let user = users::Entity::find()
        .filter(users::Column::Email.eq(email.as_str()))
        .one(&state.database)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::Unauthorized)?;
    if user.status != "active" {
        return Err(ApiError::Unauthorized);
    }
    let parsed_hash = PasswordHash::new(&user.password_hash).map_err(super::error::internal)?;
    Argon2::default()
        .verify_password(request.password.as_bytes(), &parsed_hash)
        .map_err(|_| ApiError::Unauthorized)?;

    let mut token_bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut token_bytes);
    let access_token = URL_SAFE_NO_PAD.encode(token_bytes);
    let token_hash = token_hash(&access_token);
    let expires_at = OffsetDateTime::now_utc() + Duration::days(30);
    sessions::ActiveModel {
        id: Set(Uuid::now_v7()),
        user_id: Set(user.id),
        organization_id: Set(None),
        refresh_token_hash: Set(token_hash),
        expires_at: Set(expires_at),
        revoked_at: Set(None),
        ip_address: Set(None),
        user_agent: Set(None),
        created_at: Set(OffsetDateTime::now_utc()),
    }
    .insert(&state.database)
    .await
    .map_err(super::error::internal)?;

    Ok(Json(LoginResponse {
        access_token,
        token_type: "Bearer",
        expires_at,
        is_platform_admin: user.is_platform_admin,
    }))
}

pub(super) fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}
