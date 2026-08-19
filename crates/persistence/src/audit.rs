use crate::entities::{audit_logs, outbox_events};
use sea_orm::{DatabaseTransaction, DbErr, EntityTrait, Set, sea_query::OnConflict};
use serde_json::{Map, Value, json};
use time::OffsetDateTime;
use uuid::Uuid;

/// Projects an outbox event into an immutable, sanitized audit record.
/// Duplicate delivery is ignored using the source event identity and timestamp.
///
/// # Errors
///
/// Returns [`DbErr`] when the audit projection cannot be persisted.
pub async fn project_event(
    transaction: &DatabaseTransaction,
    event: &outbox_events::Model,
) -> Result<(), DbErr> {
    let sanitized = redact_value(&event.payload);
    let actor_id = ["actor_id", "requested_by", "user_id", "created_by"]
        .iter()
        .find_map(|key| {
            event
                .payload
                .get(key)
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
        });
    let changes = json!({
        "before": sanitized.get("before").cloned().unwrap_or(Value::Null),
        "after": sanitized.get("after").cloned().unwrap_or(Value::Null),
        "differences": sanitized.get("differences").cloned().unwrap_or(Value::Null),
    });
    let active = audit_logs::ActiveModel {
        occurred_at: Set(event.occurred_at),
        id: Set(Uuid::now_v7()),
        organization_id: Set(event.organization_id),
        source_event_id: Set(event.id),
        actor_type: Set(if actor_id.is_some() { "user" } else { "system" }.to_owned()),
        actor_id: Set(actor_id),
        action: Set(event.event_type.clone()),
        target_type: Set(event.aggregate_type.clone()),
        target_id: Set(event.aggregate_id.clone()),
        outcome: Set(outcome_for(&event.event_type).to_owned()),
        severity: Set(severity_for(&event.event_type).to_owned()),
        trace_id: Set(event.trace_id.clone()),
        request_id: Set(sanitized
            .get("request_id")
            .and_then(Value::as_str)
            .map(str::to_owned)),
        client_ip: Set(None),
        user_agent: Set(None),
        changes: Set(changes),
        metadata: Set(sanitized),
        recorded_at: Set(OffsetDateTime::now_utc()),
    };
    audit_logs::Entity::insert(active)
        .on_conflict(
            OnConflict::columns([
                audit_logs::Column::OccurredAt,
                audit_logs::Column::SourceEventId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(transaction)
        .await?;
    Ok(())
}

#[must_use]
pub fn redact_value(value: &Value) -> Value {
    redact_at_depth(value, 0)
}

fn redact_at_depth(value: &Value, depth: usize) -> Value {
    if depth >= 16 {
        return Value::String("[TRUNCATED]".to_owned());
    }
    match value {
        Value::Object(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| {
                    let value = if is_sensitive_key(key) {
                        Value::String("[REDACTED]".to_owned())
                    } else {
                        redact_at_depth(value, depth + 1)
                    };
                    (key.clone(), value)
                })
                .collect::<Map<_, _>>(),
        ),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .take(1_000)
                .map(|value| redact_at_depth(value, depth + 1))
                .collect(),
        ),
        Value::String(value) if value.len() > 8_192 => Value::String(format!(
            "{}…[TRUNCATED]",
            value.chars().take(8_192).collect::<String>()
        )),
        _ => value.clone(),
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace('-', "_");
    [
        "password",
        "secret",
        "token",
        "api_key",
        "consumer_key",
        "authorization",
        "credential",
        "ciphertext",
        "nonce",
        "private_key",
    ]
    .iter()
    .any(|sensitive| normalized.contains(sensitive))
}

fn outcome_for(event_type: &str) -> &'static str {
    if event_type.ends_with(".failed") || event_type.ends_with(".invalid") {
        "failed"
    } else if event_type.ends_with(".denied") {
        "denied"
    } else if event_type.ends_with(".cancelled") {
        "cancelled"
    } else if event_type.ends_with(".requested") || event_type.ends_with(".created") {
        "attempted"
    } else {
        "succeeded"
    }
}

fn severity_for(event_type: &str) -> &'static str {
    if event_type.contains("credential")
        || event_type.ends_with(".denied")
        || event_type.ends_with(".failed")
    {
        "warning"
    } else {
        "info"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recursively_redacts_sensitive_values() {
        let value = json!({
            "api_token": "plain",
            "nested": { "applicationSecret": "plain", "safe": "visible" },
            "items": [{ "Authorization": "Bearer plain" }]
        });
        let redacted = redact_value(&value);
        assert_eq!(redacted["api_token"], "[REDACTED]");
        assert_eq!(redacted["nested"]["applicationSecret"], "[REDACTED]");
        assert_eq!(redacted["nested"]["safe"], "visible");
        assert_eq!(redacted["items"][0]["Authorization"], "[REDACTED]");
    }
}
