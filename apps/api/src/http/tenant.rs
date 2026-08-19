use super::{AppState, auth, error::ApiError};
use axum::{Json, Router, extract::FromRequestParts, http::request::Parts, routing::get};
use multicloud_persistence::entities::{organization_memberships, sessions};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend, EntityTrait, QueryFilter,
    Statement, TransactionTrait,
};
use serde::Serialize;
use uuid::Uuid;

const ORGANIZATION_HEADER: &str = "x-organization-id";

pub fn router() -> Router<AppState> {
    Router::new().route("/context", get(context))
}

#[derive(Clone, Copy, Debug)]
pub struct AuthIdentity {
    pub user_id: Uuid,
    pub session_id: Uuid,
}

impl FromRequestParts<AppState> for AuthIdentity {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let authorization = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(ApiError::Unauthorized)?;
        let token = authorization
            .strip_prefix("Bearer ")
            .filter(|token| !token.is_empty())
            .ok_or(ApiError::Unauthorized)?;
        let now = time::OffsetDateTime::now_utc();
        let session = sessions::Entity::find()
            .filter(sessions::Column::RefreshTokenHash.eq(auth::token_hash(token)))
            .filter(sessions::Column::RevokedAt.is_null())
            .filter(sessions::Column::ExpiresAt.gt(now))
            .one(&state.database)
            .await
            .map_err(super::error::internal)?
            .ok_or(ApiError::Unauthorized)?;

        Ok(Self {
            user_id: session.user_id,
            session_id: session.id,
        })
    }
}

#[derive(Clone, Copy, Debug)]
#[allow(clippy::struct_field_names)]
pub struct TenantContext {
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub organization_id: Uuid,
    pub membership_id: Uuid,
}

impl FromRequestParts<AppState> for TenantContext {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let identity = AuthIdentity::from_request_parts(parts, state).await?;
        let organization_id = parts
            .headers
            .get(ORGANIZATION_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or(ApiError::BadRequest("x-organization-id is required"))?;
        let transaction = state
            .database
            .begin()
            .await
            .map_err(super::error::internal)?;
        set_tenant_context(&transaction, identity.user_id, Some(organization_id)).await?;
        let membership = organization_memberships::Entity::find()
            .filter(organization_memberships::Column::OrganizationId.eq(organization_id))
            .filter(organization_memberships::Column::UserId.eq(identity.user_id))
            .filter(organization_memberships::Column::Status.eq("active"))
            .one(&transaction)
            .await
            .map_err(super::error::internal)?
            .ok_or(ApiError::Forbidden)?;
        transaction.commit().await.map_err(super::error::internal)?;

        Ok(Self {
            user_id: identity.user_id,
            session_id: identity.session_id,
            organization_id,
            membership_id: membership.id,
        })
    }
}

pub async fn set_tenant_context(
    transaction: &DatabaseTransaction,
    user_id: Uuid,
    organization_id: Option<Uuid>,
) -> Result<(), ApiError> {
    let organization_id = organization_id.map_or_else(String::new, |id| id.to_string());
    transaction
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT set_config('app.user_id', $1, true), set_config('app.organization_id', $2, true)",
            [user_id.to_string().into(), organization_id.into()],
        ))
        .await
        .map_err(super::error::internal)?;
    Ok(())
}

#[derive(Serialize)]
#[allow(clippy::struct_field_names)]
struct TenantContextResponse {
    user_id: Uuid,
    session_id: Uuid,
    organization_id: Uuid,
    membership_id: Uuid,
}

async fn context(context: TenantContext) -> Json<TenantContextResponse> {
    Json(TenantContextResponse {
        user_id: context.user_id,
        session_id: context.session_id,
        organization_id: context.organization_id,
        membership_id: context.membership_id,
    })
}
