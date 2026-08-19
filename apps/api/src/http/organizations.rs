use super::{
    AppState,
    error::ApiError,
    tenant::{AuthIdentity, set_tenant_context},
};
use axum::{Json, Router, extract::State, routing::get};
use multicloud_persistence::entities::{organization_memberships, organizations};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, SqlErr, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(list).post(create))
}

#[derive(Deserialize)]
struct CreateOrganizationRequest {
    slug: String,
    name: String,
}

#[derive(Serialize)]
struct OrganizationResponse {
    id: Uuid,
    slug: String,
    name: String,
    status: String,
}

async fn create(
    identity: AuthIdentity,
    State(state): State<AppState>,
    Json(request): Json<CreateOrganizationRequest>,
) -> Result<(axum::http::StatusCode, Json<OrganizationResponse>), ApiError> {
    let slug = request.slug.trim().to_lowercase();
    if !valid_slug(&slug) {
        return Err(ApiError::BadRequest("invalid organization slug"));
    }
    let name = request.name.trim();
    if name.is_empty() || name.len() > 160 {
        return Err(ApiError::BadRequest("invalid organization name"));
    }
    let organization_id = Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    let transaction = state
        .database
        .begin()
        .await
        .map_err(super::error::internal)?;
    set_tenant_context(&transaction, identity.user_id, Some(organization_id)).await?;
    organizations::ActiveModel {
        id: Set(organization_id),
        slug: Set(slug.clone()),
        name: Set(name.to_owned()),
        status: Set("active".to_owned()),
        settings: Set(serde_json::json!({})),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
    }
    .insert(&transaction)
    .await
    .map_err(|error| {
        if matches!(error.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
            ApiError::Conflict("organization slug is already used")
        } else {
            super::error::internal(error)
        }
    })?;
    organization_memberships::ActiveModel {
        id: Set(Uuid::now_v7()),
        organization_id: Set(organization_id),
        user_id: Set(identity.user_id),
        status: Set("active".to_owned()),
        joined_at: Set(now),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&transaction)
    .await
    .map_err(super::error::internal)?;
    super::authorization::bootstrap_organization_roles(
        &transaction,
        organization_id,
        identity.user_id,
    )
    .await?;
    transaction.commit().await.map_err(super::error::internal)?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(OrganizationResponse {
            id: organization_id,
            slug,
            name: name.to_owned(),
            status: "active".to_owned(),
        }),
    ))
}

async fn list(
    identity: AuthIdentity,
    State(state): State<AppState>,
) -> Result<Json<Vec<OrganizationResponse>>, ApiError> {
    let transaction = state
        .database
        .begin()
        .await
        .map_err(super::error::internal)?;
    set_tenant_context(&transaction, identity.user_id, None).await?;
    let memberships = organization_memberships::Entity::find()
        .filter(organization_memberships::Column::UserId.eq(identity.user_id))
        .filter(organization_memberships::Column::Status.eq("active"))
        .all(&transaction)
        .await
        .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;

    let mut response = Vec::with_capacity(memberships.len());
    for membership in memberships {
        let transaction = state
            .database
            .begin()
            .await
            .map_err(super::error::internal)?;
        set_tenant_context(
            &transaction,
            identity.user_id,
            Some(membership.organization_id),
        )
        .await?;
        if let Some(organization) = organizations::Entity::find_by_id(membership.organization_id)
            .one(&transaction)
            .await
            .map_err(super::error::internal)?
        {
            response.push(OrganizationResponse {
                id: organization.id,
                slug: organization.slug,
                name: organization.name,
                status: organization.status,
            });
        }
        transaction.commit().await.map_err(super::error::internal)?;
    }

    Ok(Json(response))
}

fn valid_slug(value: &str) -> bool {
    let bytes = value.as_bytes();
    (3..=80).contains(&bytes.len())
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_validation_is_strict() {
        assert!(valid_slug("cloud-team-1"));
        assert!(!valid_slug("Cloud Team"));
        assert!(!valid_slug("-cloud"));
    }
}
