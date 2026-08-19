use super::{AppState, error::ApiError, tenant::TenantContext};
use axum::{
    Json, Router,
    body::Body,
    extract::{Query, State},
    http::{Response, header},
    routing::get,
};
use multicloud_authorization::permissions;
use multicloud_operation::EventEnvelope;
use multicloud_persistence::{entities::audit_logs, reliable_events::enqueue_event};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list))
        .route("/export", get(export))
}

#[derive(Clone, Deserialize, Serialize)]
struct AuditQuery {
    action: Option<String>,
    target_type: Option<String>,
    outcome: Option<String>,
    actor_id: Option<Uuid>,
    #[serde(with = "time::serde::rfc3339::option", default)]
    occurred_before: Option<OffsetDateTime>,
    limit: Option<u64>,
}

#[derive(Serialize)]
struct AuditResponse {
    id: Uuid,
    source_event_id: Uuid,
    actor_type: String,
    actor_id: Option<Uuid>,
    action: String,
    target_type: String,
    target_id: String,
    outcome: String,
    severity: String,
    trace_id: Option<String>,
    changes: Value,
    metadata: Value,
    #[serde(with = "time::serde::rfc3339")]
    occurred_at: OffsetDateTime,
}

async fn list(
    context: TenantContext,
    State(state): State<AppState>,
    Query(query): Query<AuditQuery>,
) -> Result<Json<Vec<AuditResponse>>, ApiError> {
    validate_query(&query)?;
    let transaction =
        super::authorization::authorize_transaction(&state, &context, permissions::AUDIT_READ)
            .await?;
    let rows = apply_filters(
        audit_logs::Entity::find()
            .filter(audit_logs::Column::OrganizationId.eq(context.organization_id)),
        &query,
    )
    .order_by_desc(audit_logs::Column::OccurredAt)
    .order_by_desc(audit_logs::Column::Id)
    .limit(query.limit.unwrap_or(100).min(200))
    .all(&transaction)
    .await
    .map_err(super::error::internal)?;
    enqueue_event(
        &transaction,
        audit_access_event(&context, "audit.query.executed", &query, rows.len())?,
    )
    .await
    .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn export(
    context: TenantContext,
    State(state): State<AppState>,
    Query(mut query): Query<AuditQuery>,
) -> Result<Response<Body>, ApiError> {
    validate_query(&query)?;
    query.limit = Some(query.limit.unwrap_or(10_000).min(10_000));
    let transaction =
        super::authorization::authorize_transaction(&state, &context, permissions::AUDIT_EXPORT)
            .await?;
    let rows = apply_filters(
        audit_logs::Entity::find()
            .filter(audit_logs::Column::OrganizationId.eq(context.organization_id)),
        &query,
    )
    .order_by_desc(audit_logs::Column::OccurredAt)
    .limit(query.limit.unwrap_or(10_000))
    .all(&transaction)
    .await
    .map_err(super::error::internal)?;
    enqueue_event(
        &transaction,
        audit_access_event(&context, "audit.export.requested", &query, rows.len())?,
    )
    .await
    .map_err(super::error::internal)?;
    transaction.commit().await.map_err(super::error::internal)?;

    let mut csv = String::from(
        "occurred_at,action,outcome,severity,actor_id,target_type,target_id,trace_id\n",
    );
    for row in rows {
        let fields = [
            row.occurred_at
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(super::error::internal)?,
            row.action,
            row.outcome,
            row.severity,
            row.actor_id
                .map_or_else(String::new, |value| value.to_string()),
            row.target_type,
            row.target_id,
            row.trace_id.unwrap_or_default(),
        ];
        csv.push_str(&fields.map(|value| csv_field(&value)).join(","));
        csv.push('\n');
    }
    Response::builder()
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=multicloud-audit.csv",
        )
        .body(Body::from(csv))
        .map_err(super::error::internal)
}

fn apply_filters(
    mut select: sea_orm::Select<audit_logs::Entity>,
    query: &AuditQuery,
) -> sea_orm::Select<audit_logs::Entity> {
    if let Some(action) = &query.action {
        select = select.filter(audit_logs::Column::Action.eq(action));
    }
    if let Some(target_type) = &query.target_type {
        select = select.filter(audit_logs::Column::TargetType.eq(target_type));
    }
    if let Some(outcome) = &query.outcome {
        select = select.filter(audit_logs::Column::Outcome.eq(outcome));
    }
    if let Some(actor_id) = query.actor_id {
        select = select.filter(audit_logs::Column::ActorId.eq(actor_id));
    }
    if let Some(before) = query.occurred_before {
        select = select.filter(audit_logs::Column::OccurredAt.lt(before));
    }
    select
}

fn validate_query(query: &AuditQuery) -> Result<(), ApiError> {
    for value in [query.action.as_deref(), query.target_type.as_deref()]
        .into_iter()
        .flatten()
    {
        if value.is_empty() || value.len() > 160 {
            return Err(ApiError::BadRequest("audit filter is invalid"));
        }
    }
    if query.outcome.as_deref().is_some_and(|value| {
        !matches!(
            value,
            "attempted" | "succeeded" | "failed" | "denied" | "cancelled"
        )
    }) {
        return Err(ApiError::BadRequest("audit outcome is invalid"));
    }
    Ok(())
}

fn csv_field(value: &str) -> String {
    let safe = if value.starts_with(['=', '+', '-', '@']) {
        format!("'{value}")
    } else {
        value.to_owned()
    };
    format!("\"{}\"", safe.replace('"', "\"\""))
}

fn audit_access_event(
    context: &TenantContext,
    event_type: &str,
    query: &AuditQuery,
    row_count: usize,
) -> Result<EventEnvelope, ApiError> {
    Ok(EventEnvelope {
        id: multicloud_shared_kernel::EventId::new(),
        organization_id: multicloud_shared_kernel::OrganizationId::from_uuid(
            context.organization_id,
        ),
        aggregate_type: "audit_access".to_owned(),
        aggregate_id: Uuid::now_v7().to_string(),
        event_type: event_type.to_owned(),
        event_version: 1,
        payload: serde_json::json!({
            "requested_by": context.user_id,
            "row_count": row_count,
            "filters": multicloud_persistence::audit::redact_value(
                &serde_json::to_value(query).map_err(super::error::internal)?
            ),
        }),
        trace_id: None,
        occurred_at: OffsetDateTime::now_utc(),
    })
}

impl From<audit_logs::Model> for AuditResponse {
    fn from(value: audit_logs::Model) -> Self {
        Self {
            id: value.id,
            source_event_id: value.source_event_id,
            actor_type: value.actor_type,
            actor_id: value.actor_id,
            action: value.action,
            target_type: value.target_type,
            target_id: value.target_id,
            outcome: value.outcome,
            severity: value.severity,
            trace_id: value.trace_id,
            changes: value.changes,
            metadata: value.metadata,
            occurred_at: value.occurred_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_fields_escape_formula_and_quotes_as_data() {
        assert_eq!(csv_field("a\"b"), "\"a\"\"b\"");
        assert_eq!(csv_field("=1+1"), "\"'=1+1\"");
    }
}
