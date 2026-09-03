use super::{AppState, error::ApiError};
use axum::{Json, Router, extract::State, routing::get};
use multicloud_operation::EventEnvelope;
use multicloud_persistence::{
    entities::{platform_settings, users},
    reliable_events::enqueue_event,
};
use sea_orm::{ActiveModelTrait, EntityTrait, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub fn router() -> Router<AppState> {
    Router::new().route("/logging", get(get_logging).put(update_logging))
}

#[derive(Serialize)]
struct LoggingSettingsResponse {
    log_level: String,
}

#[derive(Deserialize)]
struct UpdateLoggingSettingsRequest {
    log_level: String,
}

async fn get_logging(
    context: super::tenant::TenantContext,
    State(state): State<AppState>,
) -> Result<Json<LoggingSettingsResponse>, ApiError> {
    require_platform_admin(&state, context.user_id).await?;
    let settings = platform_settings::Entity::find_by_id(1_i16)
        .one(&state.database)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::Internal)?;
    Ok(Json(LoggingSettingsResponse {
        log_level: settings.log_level,
    }))
}

async fn update_logging(
    context: super::tenant::TenantContext,
    State(state): State<AppState>,
    Json(request): Json<UpdateLoggingSettingsRequest>,
) -> Result<Json<LoggingSettingsResponse>, ApiError> {
    require_platform_admin(&state, context.user_id).await?;
    validate_log_level(&request.log_level)?;

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
    let previous_level = settings.log_level.as_ref().to_owned();
    settings.log_level = Set(request.log_level.clone());
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
            aggregate_id: "logging".to_owned(),
            event_type: "platform.logging.updated".to_owned(),
            event_version: 1,
            payload: serde_json::json!({
                "requested_by": context.user_id,
                "before": { "log_level": previous_level },
                "after": { "log_level": request.log_level },
            }),
            trace_id: None,
            occurred_at: OffsetDateTime::now_utc(),
        },
    )
    .await
    .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;

    multicloud_observability::set_log_level(&request.log_level).map_err(super::error::internal)?;
    Ok(Json(LoggingSettingsResponse {
        log_level: request.log_level,
    }))
}

async fn require_platform_admin(state: &AppState, user_id: uuid::Uuid) -> Result<(), ApiError> {
    let user = users::Entity::find_by_id(user_id)
        .one(&state.database)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::Unauthorized)?;
    if !user.is_platform_admin {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

fn validate_log_level(level: &str) -> Result<(), ApiError> {
    if matches!(level, "error" | "warn" | "info" | "debug" | "trace") {
        Ok(())
    } else {
        Err(ApiError::BadRequest("unsupported log level"))
    }
}

#[cfg(test)]
mod tests {
    use super::validate_log_level;

    #[test]
    fn log_level_is_restricted_to_supported_values() {
        for level in ["error", "warn", "info", "debug", "trace"] {
            assert!(validate_log_level(level).is_ok());
        }
        assert!(validate_log_level("multicloud=trace").is_err());
        assert!(validate_log_level("off").is_err());
    }
}
