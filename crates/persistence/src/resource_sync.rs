use crate::{
    entities::{
        external_resource_mappings, reconciliation_tasks, resource_desired_states, resource_drifts,
        resource_metadata, resource_observed_states, resources,
    },
    reliable_events::enqueue_event,
};
use multicloud_operation::EventEnvelope;
use multicloud_provider::{InventoryItem, InventoryPage};
use multicloud_resource::{DesiredState, DriftStatus, ObservedState, detect_drift};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, DbErr, EntityTrait, IntoActiveModel,
    QueryFilter, QueryOrder, Set,
};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub enum ReconciliationPolicy {
    ObserveOnly,
    ManualApproval,
    Automatic,
}

impl ReconciliationPolicy {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ObserveOnly => "observe_only",
            Self::ManualApproval => "manual_approval",
            Self::Automatic => "automatic",
        }
    }
}

#[derive(Default)]
pub struct SyncOutcome {
    pub created: usize,
    pub updated: usize,
    pub drifted: usize,
    pub reconciliation_tasks: usize,
}

/// Applies one normalized Provider inventory page to canonical Resources.
///
/// The caller owns the transaction and cursor update so page persistence and cursor advancement can
/// be committed atomically.
///
/// # Errors
///
/// Returns [`DbErr`] when resource, mapping, state, drift, task, or event persistence fails.
pub async fn apply_inventory_page(
    transaction: &DatabaseTransaction,
    organization_id: Uuid,
    provider_account_id: Uuid,
    provider_kind: &str,
    page: InventoryPage,
    policy: ReconciliationPolicy,
) -> Result<SyncOutcome, DbErr> {
    let mut outcome = SyncOutcome::default();
    for item in page.items {
        apply_inventory_item(
            transaction,
            organization_id,
            provider_account_id,
            provider_kind,
            item,
            policy,
            &mut outcome,
        )
        .await?;
    }
    Ok(outcome)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
async fn apply_inventory_item(
    transaction: &DatabaseTransaction,
    organization_id: Uuid,
    provider_account_id: Uuid,
    provider_kind: &str,
    item: InventoryItem,
    policy: ReconciliationPolicy,
    outcome: &mut SyncOutcome,
) -> Result<(), DbErr> {
    let now = OffsetDateTime::now_utc();
    let existing_mapping = external_resource_mappings::Entity::find()
        .filter(external_resource_mappings::Column::ProviderAccountId.eq(provider_account_id))
        .filter(external_resource_mappings::Column::ExternalType.eq(&item.external_type))
        .filter(external_resource_mappings::Column::ExternalId.eq(&item.external_id))
        .one(transaction)
        .await?;
    let (resource, mapping) = if let Some(mapping) = existing_mapping {
        let resource = resources::Entity::find_by_id(mapping.resource_id)
            .one(transaction)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound("mapped resource disappeared".to_owned()))?;
        let mut resource = resource.into_active_model();
        resource.name = Set(item.name.clone());
        resource.lifecycle = Set(normalize_lifecycle(&item.state).to_owned());
        resource.attributes = Set(item.state.clone());
        resource.updated_at = Set(now);
        let resource = resource.update(transaction).await?;
        let mut mapping = mapping.into_active_model();
        mapping.last_seen_at = Set(now);
        mapping.missing_since = Set(None);
        let mapping = mapping.update(transaction).await?;
        outcome.updated += 1;
        (resource, mapping)
    } else {
        let resource = resources::ActiveModel {
            id: Set(Uuid::now_v7()),
            organization_id: Set(organization_id),
            resource_type: Set(item.external_type.clone()),
            name: Set(item.name.clone()),
            lifecycle: Set(normalize_lifecycle(&item.state).to_owned()),
            region: Set(item
                .metadata
                .get("region")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)),
            attributes: Set(item.state.clone()),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(transaction)
        .await?;
        let mapping = external_resource_mappings::ActiveModel {
            id: Set(Uuid::now_v7()),
            organization_id: Set(organization_id),
            provider_account_id: Set(provider_account_id),
            resource_id: Set(resource.id),
            external_type: Set(item.external_type.clone()),
            external_id: Set(item.external_id.clone()),
            client_reference: Set(None),
            first_seen_at: Set(now),
            last_seen_at: Set(now),
            missing_since: Set(None),
        }
        .insert(transaction)
        .await?;
        outcome.created += 1;
        enqueue_event(
            transaction,
            resource_event(
                organization_id,
                resource.id,
                "provider.resource.discovered",
                now,
            ),
        )
        .await?;
        (resource, mapping)
    };

    let previous = resource_observed_states::Entity::find()
        .filter(resource_observed_states::Column::ResourceId.eq(resource.id))
        .filter(resource_observed_states::Column::IsLatest.eq(true))
        .one(transaction)
        .await?;
    let version = previous.as_ref().map_or(1, |state| state.version + 1);
    if let Some(previous) = previous {
        let mut previous = previous.into_active_model();
        previous.is_latest = Set(false);
        previous.update(transaction).await?;
    }
    let observed = resource_observed_states::ActiveModel {
        id: Set(Uuid::now_v7()),
        organization_id: Set(organization_id),
        resource_id: Set(resource.id),
        mapping_id: Set(mapping.id),
        version: Set(version),
        state: Set(item.state.clone()),
        state_hash: Set(hash_json(&item.state)?),
        is_latest: Set(true),
        observed_at: Set(now),
        created_at: Set(now),
    }
    .insert(transaction)
    .await?;
    if let Some(metadata) = resource_metadata::Entity::find()
        .filter(resource_metadata::Column::ResourceId.eq(resource.id))
        .filter(resource_metadata::Column::Source.eq(provider_kind))
        .filter(resource_metadata::Column::Namespace.eq("provider"))
        .one(transaction)
        .await?
    {
        let mut metadata = metadata.into_active_model();
        metadata.metadata = Set(item.metadata);
        metadata.observed_at = Set(now);
        metadata.update(transaction).await?;
    } else {
        resource_metadata::ActiveModel {
            id: Set(Uuid::now_v7()),
            organization_id: Set(organization_id),
            resource_id: Set(resource.id),
            source: Set(provider_kind.to_owned()),
            namespace: Set("provider".to_owned()),
            metadata: Set(item.metadata),
            observed_at: Set(now),
        }
        .insert(transaction)
        .await?;
    }
    if let Some(desired) = resource_desired_states::Entity::find()
        .filter(resource_desired_states::Column::ResourceId.eq(resource.id))
        .order_by_desc(resource_desired_states::Column::Version)
        .one(transaction)
        .await?
    {
        let managed_fields = serde_json::from_value(desired.managed_fields.clone())
            .map_err(|error| DbErr::Custom(error.to_string()))?;
        let evaluation = detect_drift(
            &DesiredState {
                version: desired.version,
                managed_fields,
                state: desired.state.clone(),
            },
            &ObservedState {
                version: observed.version,
                state: observed.state.clone(),
            },
        )
        .map_err(|error| DbErr::Custom(error.to_string()))?;
        if evaluation.status == DriftStatus::Drifted {
            let drift = resource_drifts::Entity::find()
                .filter(resource_drifts::Column::ResourceId.eq(resource.id))
                .filter(resource_drifts::Column::Fingerprint.eq(&evaluation.fingerprint))
                .filter(resource_drifts::Column::ResolvedAt.is_null())
                .one(transaction)
                .await?;
            let drift = if let Some(drift) = drift {
                drift
            } else {
                outcome.drifted += 1;
                resource_drifts::ActiveModel {
                    id: Set(Uuid::now_v7()),
                    organization_id: Set(organization_id),
                    resource_id: Set(resource.id),
                    desired_state_id: Set(desired.id),
                    observed_state_id: Set(observed.id),
                    fingerprint: Set(evaluation.fingerprint.clone()),
                    status: Set("drifted".to_owned()),
                    differences: Set(serde_json::to_value(evaluation.differences)
                        .map_err(|error| DbErr::Custom(error.to_string()))?),
                    detected_at: Set(now),
                    resolved_at: Set(None),
                }
                .insert(transaction)
                .await?
            };
            if !matches!(policy, ReconciliationPolicy::ObserveOnly)
                && reconciliation_tasks::Entity::find()
                    .filter(reconciliation_tasks::Column::ResourceId.eq(resource.id))
                    .filter(reconciliation_tasks::Column::DriftFingerprint.eq(&drift.fingerprint))
                    .filter(
                        reconciliation_tasks::Column::Status
                            .is_in(["pending", "approved", "running"]),
                    )
                    .one(transaction)
                    .await?
                    .is_none()
            {
                reconciliation_tasks::ActiveModel {
                    id: Set(Uuid::now_v7()),
                    organization_id: Set(organization_id),
                    resource_id: Set(resource.id),
                    drift_id: Set(drift.id),
                    drift_fingerprint: Set(drift.fingerprint),
                    desired_version: Set(desired.version),
                    policy: Set(policy.as_str().to_owned()),
                    status: Set(if matches!(policy, ReconciliationPolicy::Automatic) {
                        "approved".to_owned()
                    } else {
                        "pending".to_owned()
                    }),
                    operation_id: Set(None),
                    attempt_count: Set(0),
                    not_before: Set(now),
                    approved_by: Set(None),
                    approved_at: Set(None),
                    created_at: Set(now),
                    completed_at: Set(None),
                }
                .insert(transaction)
                .await?;
                outcome.reconciliation_tasks += 1;
            }
            enqueue_event(
                transaction,
                resource_event(organization_id, resource.id, "provider.drift.detected", now),
            )
            .await?;
        }
    }
    Ok(())
}

fn normalize_lifecycle(state: &serde_json::Value) -> &'static str {
    match state.get("status").and_then(serde_json::Value::as_str) {
        Some("active" | "running" | "ready") => "active",
        Some("stopped" | "off") => "stopped",
        Some("pending" | "provisioning") => "provisioning",
        Some("error" | "failed") => "error",
        _ => "unknown",
    }
}

fn hash_json(value: &serde_json::Value) -> Result<String, DbErr> {
    let bytes = serde_json::to_vec(value).map_err(|error| DbErr::Custom(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn resource_event(
    organization_id: Uuid,
    resource_id: Uuid,
    event_type: &str,
    occurred_at: OffsetDateTime,
) -> EventEnvelope {
    EventEnvelope {
        id: multicloud_shared_kernel::EventId::new(),
        organization_id: multicloud_shared_kernel::OrganizationId::from_uuid(organization_id),
        aggregate_type: "resource".to_owned(),
        aggregate_id: resource_id.to_string(),
        event_type: event_type.to_owned(),
        event_version: 1,
        payload: serde_json::json!({ "resource_id": resource_id }),
        trace_id: None,
        occurred_at,
    }
}
