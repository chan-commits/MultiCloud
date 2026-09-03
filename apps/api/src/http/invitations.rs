use super::{
    AppState,
    error::ApiError,
    tenant::{AuthIdentity, TenantContext, set_tenant_context},
};
use axum::{Json, Router, extract::State, routing::post};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use multicloud_identity::Email;
use multicloud_operation::EventEnvelope;
use multicloud_persistence::entities::{
    organization_invitations, organization_memberships, role_bindings, roles, users,
};
use rand::Rng;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create))
        .route("/accept", post(accept))
}

#[derive(Deserialize)]
struct CreateInvitationRequest {
    email: String,
}

#[derive(Serialize)]
struct CreateInvitationResponse {
    id: Uuid,
    token: String,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
}

async fn create(
    context: TenantContext,
    State(state): State<AppState>,
    Json(request): Json<CreateInvitationRequest>,
) -> Result<(axum::http::StatusCode, Json<CreateInvitationResponse>), ApiError> {
    let email = Email::parse(request.email).map_err(|_| ApiError::BadRequest("invalid email"))?;
    let mut token_bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut token_bytes);
    let token = URL_SAFE_NO_PAD.encode(token_bytes);
    let expires_at = OffsetDateTime::now_utc() + Duration::days(7);
    let id = Uuid::now_v7();
    let transaction = super::authorization::authorize_transaction(
        &state,
        &context,
        multicloud_authorization::permissions::INVITATION_MANAGE,
    )
    .await?;
    organization_invitations::ActiveModel {
        id: Set(id),
        organization_id: Set(context.organization_id),
        email: Set(email.to_string()),
        token_hash: Set(super::auth::token_hash(&token)),
        invited_by: Set(context.user_id),
        expires_at: Set(expires_at),
        accepted_at: Set(None),
        created_at: Set(OffsetDateTime::now_utc()),
    }
    .insert(&transaction)
    .await
    .map_err(super::error::internal)?;
    multicloud_persistence::reliable_events::enqueue_event(
        &transaction,
        EventEnvelope {
            id: multicloud_shared_kernel::EventId::new(),
            organization_id: multicloud_shared_kernel::OrganizationId::from_uuid(
                context.organization_id,
            ),
            aggregate_type: "organization_invitation".to_owned(),
            aggregate_id: id.to_string(),
            event_type: "organization.invitation.created".to_owned(),
            event_version: 1,
            payload: serde_json::json!({
                "invitation_id": id,
                "requested_by": context.user_id,
                "expires_at": expires_at
                    .format(&time::format_description::well_known::Rfc3339)
                    .map_err(super::error::internal)?,
            }),
            trace_id: None,
            occurred_at: OffsetDateTime::now_utc(),
        },
    )
    .await
    .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(CreateInvitationResponse {
            id,
            token,
            expires_at,
        }),
    ))
}

#[derive(Deserialize)]
struct AcceptInvitationRequest {
    organization_id: Uuid,
    token: String,
}

async fn accept(
    identity: AuthIdentity,
    State(state): State<AppState>,
    Json(request): Json<AcceptInvitationRequest>,
) -> Result<axum::http::StatusCode, ApiError> {
    let transaction = state
        .database
        .begin()
        .await
        .map_err(super::error::internal)?;
    set_tenant_context(
        &transaction,
        identity.user_id,
        Some(request.organization_id),
    )
    .await?;
    let invitation = organization_invitations::Entity::find()
        .filter(organization_invitations::Column::OrganizationId.eq(request.organization_id))
        .filter(
            organization_invitations::Column::TokenHash.eq(super::auth::token_hash(&request.token)),
        )
        .filter(organization_invitations::Column::AcceptedAt.is_null())
        .filter(organization_invitations::Column::ExpiresAt.gt(OffsetDateTime::now_utc()))
        .one(&transaction)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::BadRequest("invitation is invalid or expired"))?;
    let user = users::Entity::find_by_id(identity.user_id)
        .one(&transaction)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::Unauthorized)?;
    if !invitation.email.eq_ignore_ascii_case(&user.email) {
        return Err(ApiError::Forbidden);
    }
    let exists = organization_memberships::Entity::find()
        .filter(organization_memberships::Column::OrganizationId.eq(request.organization_id))
        .filter(organization_memberships::Column::UserId.eq(identity.user_id))
        .one(&transaction)
        .await
        .map_err(super::error::internal)?
        .is_some();
    if !exists {
        let now = OffsetDateTime::now_utc();
        organization_memberships::ActiveModel {
            id: Set(Uuid::now_v7()),
            organization_id: Set(request.organization_id),
            user_id: Set(identity.user_id),
            status: Set("active".to_owned()),
            joined_at: Set(now),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&transaction)
        .await
        .map_err(super::error::internal)?;
        let member_role = roles::Entity::find()
            .filter(roles::Column::OrganizationId.eq(request.organization_id))
            .filter(roles::Column::Key.eq("member"))
            .one(&transaction)
            .await
            .map_err(super::error::internal)?
            .ok_or(ApiError::Internal)?;
        role_bindings::ActiveModel {
            id: Set(Uuid::now_v7()),
            organization_id: Set(request.organization_id),
            role_id: Set(member_role.id),
            subject_type: Set("user".to_owned()),
            subject_id: Set(identity.user_id),
            scope_type: Set("organization".to_owned()),
            scope_id: Set(request.organization_id),
            created_by: Set(invitation.invited_by),
            created_at: Set(now),
        }
        .insert(&transaction)
        .await
        .map_err(super::error::internal)?;
    }
    organization_invitations::Entity::update_many()
        .col_expr(
            organization_invitations::Column::AcceptedAt,
            sea_orm::sea_query::Expr::value(OffsetDateTime::now_utc()),
        )
        .filter(organization_invitations::Column::Id.eq(invitation.id))
        .exec(&transaction)
        .await
        .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
