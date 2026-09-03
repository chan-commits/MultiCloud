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
        attachments, notifications, organization_memberships, sla_policies, ticket_comments,
        tickets,
    },
    reliable_events::enqueue_event,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend, EntityTrait,
    IntoActiveModel, QueryFilter, QueryOrder, QuerySelect, Set, Statement,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route(
            "/sla-policies",
            get(list_sla_policies).post(create_sla_policy),
        )
        .route("/{ticket_id}", get(get_one).patch(update))
        .route(
            "/{ticket_id}/comments",
            get(list_comments).post(add_comment),
        )
        .route("/{ticket_id}/attachments", post(add_attachment))
        .route("/notifications", get(list_notifications))
        .route(
            "/notifications/{notification_id}/read",
            post(read_notification),
        )
}

#[derive(Serialize)]
struct TicketResponse {
    id: Uuid,
    number: i64,
    subject: String,
    description: String,
    status: String,
    priority: String,
    requester_id: Uuid,
    assigned_to: Option<Uuid>,
    #[serde(with = "time::serde::rfc3339::option")]
    response_due_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    resolution_due_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    first_responded_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    resolved_at: Option<OffsetDateTime>,
    version: i32,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    updated_at: OffsetDateTime,
}

impl From<tickets::Model> for TicketResponse {
    fn from(value: tickets::Model) -> Self {
        Self {
            id: value.id,
            number: value.number,
            subject: value.subject,
            description: value.description,
            status: value.status,
            priority: value.priority,
            requester_id: value.requester_id,
            assigned_to: value.assigned_to,
            response_due_at: value.response_due_at,
            resolution_due_at: value.resolution_due_at,
            first_responded_at: value.first_responded_at,
            resolved_at: value.resolved_at,
            version: value.version,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Deserialize)]
struct CreateTicketRequest {
    subject: String,
    description: String,
    priority: Option<String>,
}

#[derive(Deserialize)]
struct UpdateTicketRequest {
    status: Option<String>,
    priority: Option<String>,
    #[allow(clippy::option_option)] // PATCH distinguishes omitted from explicit null/unassign.
    assigned_to: Option<Option<Uuid>>,
    version: i32,
}

async fn list(
    context: TenantContext,
    State(state): State<AppState>,
) -> Result<Json<Vec<TicketResponse>>, ApiError> {
    let tx =
        super::authorization::authorize_transaction(&state, &context, permissions::TICKET_READ)
            .await?;
    let rows = tickets::Entity::find()
        .filter(tickets::Column::OrganizationId.eq(context.organization_id))
        .order_by_desc(tickets::Column::UpdatedAt)
        .limit(200)
        .all(&tx)
        .await
        .map_err(super::error::internal)?;
    tx.commit().await.map_err(super::error::internal)?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn get_one(
    context: TenantContext,
    State(state): State<AppState>,
    Path(ticket_id): Path<Uuid>,
) -> Result<Json<TicketResponse>, ApiError> {
    let tx =
        super::authorization::authorize_transaction(&state, &context, permissions::TICKET_READ)
            .await?;
    let ticket = tenant_ticket(&tx, context.organization_id, ticket_id).await?;
    tx.commit().await.map_err(super::error::internal)?;
    Ok(Json(ticket.into()))
}

async fn create(
    context: TenantContext,
    State(state): State<AppState>,
    Json(request): Json<CreateTicketRequest>,
) -> Result<(axum::http::StatusCode, Json<TicketResponse>), ApiError> {
    let subject = bounded(&request.subject, 1, 200, "invalid ticket subject")?;
    let description = bounded(
        &request.description,
        1,
        20_000,
        "invalid ticket description",
    )?;
    let priority = request.priority.as_deref().unwrap_or("normal");
    multicloud_ticket::TicketPriority::parse(priority)
        .map_err(|_| ApiError::BadRequest("invalid ticket priority"))?;
    let tx =
        super::authorization::authorize_transaction(&state, &context, permissions::TICKET_CREATE)
            .await?;
    let row = tx.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
        "INSERT INTO ticket_counters (organization_id, next_number) VALUES ($1, 2) ON CONFLICT (organization_id) DO UPDATE SET next_number = ticket_counters.next_number + 1 RETURNING next_number - 1 AS number",
        [context.organization_id.into()])).await.map_err(super::error::internal)?.ok_or(ApiError::Internal)?;
    let number: i64 = row.try_get("", "number").map_err(super::error::internal)?;
    let policy = sla_policies::Entity::find()
        .filter(sla_policies::Column::OrganizationId.eq(context.organization_id))
        .filter(sla_policies::Column::IsDefault.eq(true))
        .one(&tx)
        .await
        .map_err(super::error::internal)?;
    let now = OffsetDateTime::now_utc();
    let (response_minutes, resolution_minutes) = policy
        .as_ref()
        .map_or((60, 1440), |p| (p.response_minutes, p.resolution_minutes));
    let target = multicloud_ticket::sla_target(now, response_minutes, resolution_minutes);
    let ticket = tickets::ActiveModel {
        id: Set(Uuid::now_v7()),
        organization_id: Set(context.organization_id),
        number: Set(number),
        subject: Set(subject),
        description: Set(description),
        status: Set("open".to_owned()),
        priority: Set(priority.to_owned()),
        requester_id: Set(context.user_id),
        assigned_to: Set(None),
        sla_policy_id: Set(policy.map(|p| p.id)),
        response_due_at: Set(Some(target.response_due_at)),
        resolution_due_at: Set(Some(target.resolution_due_at)),
        first_responded_at: Set(None),
        resolved_at: Set(None),
        version: Set(1),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&tx)
    .await
    .map_err(super::error::internal)?;
    enqueue_ticket_event(
        &tx,
        &context,
        &ticket,
        "ticket.ticket.created",
        serde_json::json!({"number": number, "priority": priority}),
    )
    .await?;
    tx.commit().await.map_err(super::error::internal)?;
    Ok((axum::http::StatusCode::CREATED, Json(ticket.into())))
}

async fn update(
    context: TenantContext,
    State(state): State<AppState>,
    Path(ticket_id): Path<Uuid>,
    Json(request): Json<UpdateTicketRequest>,
) -> Result<Json<TicketResponse>, ApiError> {
    let tx =
        super::authorization::authorize_transaction(&state, &context, permissions::TICKET_MANAGE)
            .await?;
    let ticket = tenant_ticket(&tx, context.organization_id, ticket_id).await?;
    if ticket.version != request.version {
        return Err(ApiError::Conflict("ticket was modified; refresh and retry"));
    }
    let before = serde_json::json!({"status": ticket.status, "priority": ticket.priority, "assigned_to": ticket.assigned_to});
    let mut active = ticket.clone().into_active_model();
    let now = OffsetDateTime::now_utc();
    if let Some(status) = request.status.as_deref() {
        let current = multicloud_ticket::TicketStatus::parse(&ticket.status)
            .map_err(super::error::internal)?;
        let next = multicloud_ticket::TicketStatus::parse(status)
            .map_err(|_| ApiError::BadRequest("invalid ticket status"))?;
        current
            .transition_to(next)
            .map_err(|_| ApiError::Conflict("invalid ticket status transition"))?;
        active.status = Set(next.as_str().to_owned());
        active.resolved_at = Set(matches!(
            next,
            multicloud_ticket::TicketStatus::Resolved | multicloud_ticket::TicketStatus::Closed
        )
        .then_some(now));
    }
    if let Some(priority) = request.priority.as_deref() {
        multicloud_ticket::TicketPriority::parse(priority)
            .map_err(|_| ApiError::BadRequest("invalid ticket priority"))?;
        active.priority = Set(priority.to_owned());
    }
    if let Some(assigned_to) = request.assigned_to {
        if let Some(user_id) = assigned_to {
            ensure_member(&tx, context.organization_id, user_id).await?;
        }
        active.assigned_to = Set(assigned_to);
    }
    active.version = Set(ticket.version + 1);
    active.updated_at = Set(now);
    let updated = active.update(&tx).await.map_err(super::error::internal)?;
    let after = serde_json::json!({"status": updated.status, "priority": updated.priority, "assigned_to": updated.assigned_to});
    if updated.assigned_to != ticket.assigned_to
        && let Some(recipient) = updated.assigned_to
    {
        notify(
            &tx,
            context.organization_id,
            recipient,
            "ticket.assigned",
            &updated,
        )
        .await?;
    }
    enqueue_ticket_event(
        &tx,
        &context,
        &updated,
        "ticket.ticket.updated",
        serde_json::json!({"before": before, "after": after}),
    )
    .await?;
    tx.commit().await.map_err(super::error::internal)?;
    Ok(Json(updated.into()))
}

#[derive(Serialize)]
struct CommentResponse {
    id: Uuid,
    ticket_id: Uuid,
    author_id: Uuid,
    body: String,
    visibility: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}
impl From<ticket_comments::Model> for CommentResponse {
    fn from(v: ticket_comments::Model) -> Self {
        Self {
            id: v.id,
            ticket_id: v.ticket_id,
            author_id: v.author_id,
            body: v.body,
            visibility: v.visibility,
            created_at: v.created_at,
        }
    }
}
#[derive(Deserialize)]
struct AddCommentRequest {
    body: String,
    visibility: Option<String>,
}

async fn list_comments(
    context: TenantContext,
    State(state): State<AppState>,
    Path(ticket_id): Path<Uuid>,
) -> Result<Json<Vec<CommentResponse>>, ApiError> {
    let can_read_internal = super::authorization::permission_keys_for(&state, &context)
        .await?
        .iter()
        .any(|key| key == permissions::TICKET_MANAGE);
    let tx =
        super::authorization::authorize_transaction(&state, &context, permissions::TICKET_READ)
            .await?;
    tenant_ticket(&tx, context.organization_id, ticket_id).await?;
    let mut query = ticket_comments::Entity::find()
        .filter(ticket_comments::Column::OrganizationId.eq(context.organization_id))
        .filter(ticket_comments::Column::TicketId.eq(ticket_id));
    if !can_read_internal {
        query = query.filter(ticket_comments::Column::Visibility.eq("public"));
    }
    let rows = query
        .order_by_asc(ticket_comments::Column::CreatedAt)
        .all(&tx)
        .await
        .map_err(super::error::internal)?;
    tx.commit().await.map_err(super::error::internal)?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn add_comment(
    context: TenantContext,
    State(state): State<AppState>,
    Path(ticket_id): Path<Uuid>,
    Json(request): Json<AddCommentRequest>,
) -> Result<(axum::http::StatusCode, Json<CommentResponse>), ApiError> {
    let visibility = request.visibility.as_deref().unwrap_or("public");
    if !matches!(visibility, "public" | "internal") {
        return Err(ApiError::BadRequest("invalid comment visibility"));
    }
    let permission = if visibility == "internal" {
        permissions::TICKET_MANAGE
    } else {
        permissions::TICKET_COMMENT
    };
    let body = bounded(&request.body, 1, 20_000, "invalid comment body")?;
    let tx = super::authorization::authorize_transaction(&state, &context, permission).await?;
    let ticket = tenant_ticket(&tx, context.organization_id, ticket_id).await?;
    let now = OffsetDateTime::now_utc();
    let comment = ticket_comments::ActiveModel {
        id: Set(Uuid::now_v7()),
        organization_id: Set(context.organization_id),
        ticket_id: Set(ticket_id),
        author_id: Set(context.user_id),
        body: Set(body),
        visibility: Set(visibility.to_owned()),
        created_at: Set(now),
    }
    .insert(&tx)
    .await
    .map_err(super::error::internal)?;
    if ticket.requester_id != context.user_id {
        notify(
            &tx,
            context.organization_id,
            ticket.requester_id,
            "ticket.comment.created",
            &ticket,
        )
        .await?;
    }
    if let Some(assignee) = ticket.assigned_to
        && assignee != context.user_id
        && assignee != ticket.requester_id
    {
        notify(
            &tx,
            context.organization_id,
            assignee,
            "ticket.comment.created",
            &ticket,
        )
        .await?;
    }
    let mut active = ticket.into_active_model();
    active.updated_at = Set(now);
    if active.first_responded_at.as_ref().is_none()
        && context.user_id != *active.requester_id.as_ref()
    {
        active.first_responded_at = Set(Some(now));
    }
    active.update(&tx).await.map_err(super::error::internal)?;
    enqueue_ticket_event_model(
        &tx,
        &context,
        ticket_id,
        "ticket.comment.created",
        serde_json::json!({"comment_id": comment.id, "visibility": visibility}),
    )
    .await?;
    tx.commit().await.map_err(super::error::internal)?;
    Ok((axum::http::StatusCode::CREATED, Json(comment.into())))
}

#[derive(Deserialize)]
struct AddAttachmentRequest {
    owner_type: String,
    owner_id: Uuid,
    filename: String,
    content_type: String,
    size_bytes: i64,
    checksum_sha256: String,
}
#[derive(Serialize)]
struct AttachmentResponse {
    id: Uuid,
    storage_key: String,
}
async fn add_attachment(
    context: TenantContext,
    State(state): State<AppState>,
    Path(ticket_id): Path<Uuid>,
    Json(request): Json<AddAttachmentRequest>,
) -> Result<(axum::http::StatusCode, Json<AttachmentResponse>), ApiError> {
    let tx =
        super::authorization::authorize_transaction(&state, &context, permissions::TICKET_COMMENT)
            .await?;
    tenant_ticket(&tx, context.organization_id, ticket_id).await?;
    if request.owner_type == "ticket" && request.owner_id != ticket_id {
        return Err(ApiError::BadRequest(
            "attachment owner does not match ticket",
        ));
    }
    if request.owner_type == "ticket_comment" {
        let exists = ticket_comments::Entity::find_by_id(request.owner_id)
            .filter(ticket_comments::Column::TicketId.eq(ticket_id))
            .one(&tx)
            .await
            .map_err(super::error::internal)?
            .is_some();
        if !exists {
            return Err(ApiError::BadRequest("attachment comment was not found"));
        }
    } else if request.owner_type != "ticket" {
        return Err(ApiError::BadRequest("invalid attachment owner"));
    }
    let filename = bounded(&request.filename, 1, 255, "invalid attachment filename")?;
    let content_type = bounded(
        &request.content_type,
        1,
        160,
        "invalid attachment content type",
    )?;
    if !(1..=104_857_600).contains(&request.size_bytes)
        || request.checksum_sha256.len() != 64
        || !request
            .checksum_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(ApiError::BadRequest("invalid attachment metadata"));
    }
    let id = Uuid::now_v7();
    let storage_key = format!("tickets/{}/{id}", context.organization_id);
    let now = OffsetDateTime::now_utc();
    attachments::ActiveModel {
        id: Set(id),
        organization_id: Set(context.organization_id),
        owner_type: Set(request.owner_type),
        owner_id: Set(request.owner_id),
        storage_key: Set(storage_key.clone()),
        filename: Set(filename),
        content_type: Set(content_type),
        size_bytes: Set(request.size_bytes),
        checksum_sha256: Set(request.checksum_sha256),
        uploaded_by: Set(context.user_id),
        created_at: Set(now),
    }
    .insert(&tx)
    .await
    .map_err(super::error::internal)?;
    enqueue_ticket_event_model(
        &tx,
        &context,
        ticket_id,
        "ticket.attachment.registered",
        serde_json::json!({"attachment_id":id,"size_bytes":request.size_bytes}),
    )
    .await?;
    tx.commit().await.map_err(super::error::internal)?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(AttachmentResponse { id, storage_key }),
    ))
}

#[derive(Serialize)]
struct SlaResponse {
    id: Uuid,
    name: String,
    response_minutes: i32,
    resolution_minutes: i32,
    is_default: bool,
}
impl From<sla_policies::Model> for SlaResponse {
    fn from(v: sla_policies::Model) -> Self {
        Self {
            id: v.id,
            name: v.name,
            response_minutes: v.response_minutes,
            resolution_minutes: v.resolution_minutes,
            is_default: v.is_default,
        }
    }
}
#[derive(Deserialize)]
struct CreateSlaRequest {
    name: String,
    response_minutes: i32,
    resolution_minutes: i32,
    is_default: Option<bool>,
}
async fn list_sla_policies(
    context: TenantContext,
    State(state): State<AppState>,
) -> Result<Json<Vec<SlaResponse>>, ApiError> {
    let tx =
        super::authorization::authorize_transaction(&state, &context, permissions::TICKET_READ)
            .await?;
    let rows = sla_policies::Entity::find()
        .filter(sla_policies::Column::OrganizationId.eq(context.organization_id))
        .order_by_asc(sla_policies::Column::Name)
        .all(&tx)
        .await
        .map_err(super::error::internal)?;
    tx.commit().await.map_err(super::error::internal)?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}
async fn create_sla_policy(
    context: TenantContext,
    State(state): State<AppState>,
    Json(request): Json<CreateSlaRequest>,
) -> Result<(axum::http::StatusCode, Json<SlaResponse>), ApiError> {
    if request.response_minutes < 1
        || request.resolution_minutes < request.response_minutes
        || request.resolution_minutes > 525_600
    {
        return Err(ApiError::BadRequest("invalid SLA targets"));
    }
    let name = bounded(&request.name, 1, 120, "invalid SLA name")?;
    let tx = super::authorization::authorize_transaction(
        &state,
        &context,
        permissions::TICKET_SLA_MANAGE,
    )
    .await?;
    if request.is_default.unwrap_or(false) {
        sla_policies::Entity::update_many()
            .col_expr(
                sla_policies::Column::IsDefault,
                sea_orm::sea_query::Expr::value(false),
            )
            .filter(sla_policies::Column::OrganizationId.eq(context.organization_id))
            .exec(&tx)
            .await
            .map_err(super::error::internal)?;
    }
    let now = OffsetDateTime::now_utc();
    let policy = sla_policies::ActiveModel {
        id: Set(Uuid::now_v7()),
        organization_id: Set(context.organization_id),
        name: Set(name),
        response_minutes: Set(request.response_minutes),
        resolution_minutes: Set(request.resolution_minutes),
        is_default: Set(request.is_default.unwrap_or(false)),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&tx)
    .await
    .map_err(super::error::internal)?;
    enqueue_ticket_event_model(&tx,&context,policy.id,"ticket.sla.created",serde_json::json!({"name":policy.name,"response_minutes":policy.response_minutes,"resolution_minutes":policy.resolution_minutes})).await?;
    tx.commit().await.map_err(super::error::internal)?;
    Ok((axum::http::StatusCode::CREATED, Json(policy.into())))
}

#[derive(Serialize)]
struct NotificationResponse {
    id: Uuid,
    notification_type: String,
    payload: serde_json::Value,
    #[serde(with = "time::serde::rfc3339::option")]
    read_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

async fn list_notifications(
    context: TenantContext,
    State(state): State<AppState>,
) -> Result<Json<Vec<NotificationResponse>>, ApiError> {
    let tx =
        super::authorization::authorize_transaction(&state, &context, permissions::TICKET_READ)
            .await?;
    let rows = notifications::Entity::find()
        .filter(notifications::Column::OrganizationId.eq(context.organization_id))
        .filter(notifications::Column::RecipientUserId.eq(context.user_id))
        .order_by_desc(notifications::Column::CreatedAt)
        .limit(100)
        .all(&tx)
        .await
        .map_err(super::error::internal)?;
    tx.commit().await.map_err(super::error::internal)?;
    Ok(Json(
        rows.into_iter()
            .map(|row| NotificationResponse {
                id: row.id,
                notification_type: row.notification_type,
                payload: row.payload,
                read_at: row.read_at,
                created_at: row.created_at,
            })
            .collect(),
    ))
}

async fn read_notification(
    context: TenantContext,
    State(state): State<AppState>,
    Path(notification_id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let tx =
        super::authorization::authorize_transaction(&state, &context, permissions::TICKET_READ)
            .await?;
    let notification = notifications::Entity::find_by_id(notification_id)
        .filter(notifications::Column::OrganizationId.eq(context.organization_id))
        .filter(notifications::Column::RecipientUserId.eq(context.user_id))
        .one(&tx)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::BadRequest("notification was not found"))?;
    let mut active = notification.into_active_model();
    active.read_at = Set(Some(OffsetDateTime::now_utc()));
    active.update(&tx).await.map_err(super::error::internal)?;
    tx.commit().await.map_err(super::error::internal)?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn tenant_ticket<C: ConnectionTrait>(
    db: &C,
    organization_id: Uuid,
    ticket_id: Uuid,
) -> Result<tickets::Model, ApiError> {
    tickets::Entity::find_by_id(ticket_id)
        .filter(tickets::Column::OrganizationId.eq(organization_id))
        .one(db)
        .await
        .map_err(super::error::internal)?
        .ok_or(ApiError::BadRequest("ticket was not found"))
}
async fn ensure_member<C: ConnectionTrait>(
    db: &C,
    organization_id: Uuid,
    user_id: Uuid,
) -> Result<(), ApiError> {
    let found = organization_memberships::Entity::find()
        .filter(organization_memberships::Column::OrganizationId.eq(organization_id))
        .filter(organization_memberships::Column::UserId.eq(user_id))
        .filter(organization_memberships::Column::Status.eq("active"))
        .one(db)
        .await
        .map_err(super::error::internal)?
        .is_some();
    if found {
        Ok(())
    } else {
        Err(ApiError::BadRequest(
            "assignee is not an active organization member",
        ))
    }
}
fn bounded(value: &str, min: usize, max: usize, message: &'static str) -> Result<String, ApiError> {
    let value = value.trim();
    if (min..=max).contains(&value.chars().count()) {
        Ok(value.to_owned())
    } else {
        Err(ApiError::BadRequest(message))
    }
}
async fn notify<C: ConnectionTrait>(
    db: &C,
    organization_id: Uuid,
    recipient: Uuid,
    kind: &str,
    ticket: &tickets::Model,
) -> Result<(), ApiError> {
    notifications::ActiveModel{id:Set(Uuid::now_v7()),organization_id:Set(organization_id),recipient_user_id:Set(recipient),notification_type:Set(kind.to_owned()),payload:Set(serde_json::json!({"ticket_id":ticket.id,"number":ticket.number,"subject":ticket.subject})),read_at:Set(None),created_at:Set(OffsetDateTime::now_utc())}.insert(db).await.map_err(super::error::internal)?;
    Ok(())
}
async fn enqueue_ticket_event(
    db: &DatabaseTransaction,
    context: &TenantContext,
    ticket: &tickets::Model,
    event_type: &str,
    payload: serde_json::Value,
) -> Result<(), ApiError> {
    enqueue_ticket_event_model(db, context, ticket.id, event_type, payload).await
}
async fn enqueue_ticket_event_model(
    db: &DatabaseTransaction,
    context: &TenantContext,
    ticket_id: Uuid,
    event_type: &str,
    mut payload: serde_json::Value,
) -> Result<(), ApiError> {
    if let Some(object) = payload.as_object_mut() {
        object.insert(
            "requested_by".to_owned(),
            serde_json::json!(context.user_id),
        );
    }
    enqueue_event(
        db,
        EventEnvelope {
            id: multicloud_shared_kernel::EventId::new(),
            organization_id: multicloud_shared_kernel::OrganizationId::from_uuid(
                context.organization_id,
            ),
            aggregate_type: "ticket".to_owned(),
            aggregate_id: ticket_id.to_string(),
            event_type: event_type.to_owned(),
            event_version: 1,
            payload,
            trace_id: None,
            occurred_at: OffsetDateTime::now_utc(),
        },
    )
    .await
    .map(|_| ())
    .map_err(super::error::internal)
}
