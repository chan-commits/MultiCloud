use multicloud_shared_kernel::{EventId, OperationId, OrganizationId, UserId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use time::{Duration, OffsetDateTime};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
}

impl OperationStatus {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::TimedOut
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Operation {
    pub id: OperationId,
    pub organization_id: OrganizationId,
    pub operation_type: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub requested_by: UserId,
    pub idempotency_key: String,
    pub status: OperationStatus,
    pub progress: u8,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub id: EventId,
    pub organization_id: OrganizationId,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub event_version: u16,
    pub payload: Value,
    pub trace_id: Option<String>,
    pub occurred_at: OffsetDateTime,
}

#[derive(Clone, Copy, Debug)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 10,
            initial_delay: Duration::seconds(1),
            max_delay: Duration::minutes(5),
        }
    }
}

impl RetryPolicy {
    #[must_use]
    pub fn delay_for_attempt(self, attempt: u32) -> Duration {
        let exponent = attempt.saturating_sub(1).min(31);
        let multiplier = 1_i64.checked_shl(exponent).unwrap_or(i64::MAX);
        let seconds = self
            .initial_delay
            .whole_seconds()
            .saturating_mul(multiplier)
            .min(self.max_delay.whole_seconds());
        Duration::seconds(seconds)
    }

    #[must_use]
    pub const fn should_dead_letter(self, attempt: u32) -> bool {
        attempt >= self.max_attempts
    }
}

#[derive(Debug, Error)]
pub enum OperationError {
    #[error("operation was not found")]
    NotFound,
    #[error("operation is already in a terminal state")]
    Terminal,
    #[error("operation progress must be between 0 and 100")]
    InvalidProgress,
    #[error("operation persistence failed")]
    Persistence,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_delay_is_exponential_and_capped() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.delay_for_attempt(1), Duration::seconds(1));
        assert_eq!(policy.delay_for_attempt(4), Duration::seconds(8));
        assert_eq!(policy.delay_for_attempt(20), Duration::minutes(5));
    }

    #[test]
    fn terminal_statuses_are_explicit() {
        assert!(!OperationStatus::Running.is_terminal());
        assert!(OperationStatus::Succeeded.is_terminal());
        assert!(OperationStatus::TimedOut.is_terminal());
    }
}
