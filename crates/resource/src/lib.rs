use multicloud_shared_kernel::{OrganizationId, ResourceId};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    ComputeInstance,
    DnsZone,
    DnsRecord,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceLifecycle {
    Provisioning,
    Active,
    Stopped,
    Deleting,
    Deleted,
    Error,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resource {
    pub id: ResourceId,
    pub organization_id: OrganizationId,
    pub resource_type: ResourceType,
    pub name: String,
    pub lifecycle: ResourceLifecycle,
    pub region: Option<String>,
    pub attributes: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DesiredState {
    pub version: i64,
    pub managed_fields: Vec<String>,
    pub state: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObservedState {
    pub version: i64,
    pub state: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftStatus {
    InSync,
    Drifted,
    Unknown,
    Ignored,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FieldDrift {
    pub field: String,
    pub expected: Value,
    pub actual: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DriftEvaluation {
    pub status: DriftStatus,
    pub fingerprint: String,
    pub differences: Vec<FieldDrift>,
}

#[derive(Debug, Error)]
pub enum ResourceError {
    #[error("desired and observed states must be JSON objects")]
    StateMustBeObject,
    #[error("managed field is invalid")]
    InvalidManagedField,
    #[error("state serialization failed")]
    Serialization,
}

/// Compares only explicitly managed top-level fields and creates a stable fingerprint.
///
/// # Errors
///
/// Returns [`ResourceError`] for non-object states, malformed field names, or serialization
/// failures.
pub fn detect_drift(
    desired: &DesiredState,
    observed: &ObservedState,
) -> Result<DriftEvaluation, ResourceError> {
    let desired_object = desired
        .state
        .as_object()
        .ok_or(ResourceError::StateMustBeObject)?;
    let observed_object = observed
        .state
        .as_object()
        .ok_or(ResourceError::StateMustBeObject)?;
    let fields: BTreeSet<_> = desired.managed_fields.iter().collect();
    if fields
        .iter()
        .any(|field| field.is_empty() || field.contains('.'))
    {
        return Err(ResourceError::InvalidManagedField);
    }
    let null = Value::Null;
    let differences = fields
        .into_iter()
        .filter_map(|field| {
            let expected = desired_object.get(field).unwrap_or(&null);
            let actual = observed_object.get(field).unwrap_or(&null);
            (expected != actual).then(|| FieldDrift {
                field: field.clone(),
                expected: expected.clone(),
                actual: actual.clone(),
            })
        })
        .collect::<Vec<_>>();
    let fingerprint_payload = Value::Object(Map::from_iter([
        ("desired_version".to_owned(), Value::from(desired.version)),
        (
            "differences".to_owned(),
            serde_json::to_value(&differences).map_err(|_| ResourceError::Serialization)?,
        ),
    ]));
    let encoded =
        serde_json::to_vec(&fingerprint_payload).map_err(|_| ResourceError::Serialization)?;
    let fingerprint = format!("{:x}", Sha256::digest(encoded));
    Ok(DriftEvaluation {
        status: if differences.is_empty() {
            DriftStatus::InSync
        } else {
            DriftStatus::Drifted
        },
        fingerprint,
        differences,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn drift_ignores_unmanaged_provider_metadata() {
        let desired = DesiredState {
            version: 4,
            managed_fields: vec!["content".to_owned()],
            state: json!({ "content": "1.1.1.1" }),
        };
        let observed = ObservedState {
            version: 9,
            state: json!({ "content": "1.1.1.1", "provider_timestamp": 123 }),
        };
        assert_eq!(
            detect_drift(&desired, &observed).unwrap().status,
            DriftStatus::InSync
        );
    }

    #[test]
    fn drift_fingerprint_is_stable() {
        let desired = DesiredState {
            version: 2,
            managed_fields: vec!["content".to_owned(), "ttl".to_owned()],
            state: json!({ "content": "1.1.1.1", "ttl": 120 }),
        };
        let observed = ObservedState {
            version: 3,
            state: json!({ "ttl": 60, "content": "2.2.2.2" }),
        };
        let first = detect_drift(&desired, &observed).unwrap();
        let second = detect_drift(&desired, &observed).unwrap();
        assert_eq!(first.status, DriftStatus::Drifted);
        assert_eq!(first.fingerprint, second.fingerprint);
        assert_eq!(first.differences.len(), 2);
    }
}
