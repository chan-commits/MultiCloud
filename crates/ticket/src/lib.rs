use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{Duration, OffsetDateTime};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TicketStatus {
    Open,
    InProgress,
    WaitingOnCustomer,
    Resolved,
    Closed,
}

impl TicketStatus {
    /// Parses the persisted/API status representation.
    ///
    /// # Errors
    ///
    /// Returns [`TicketError::InvalidStatus`] for unknown values.
    pub fn parse(value: &str) -> Result<Self, TicketError> {
        match value {
            "open" => Ok(Self::Open),
            "in_progress" => Ok(Self::InProgress),
            "waiting_on_customer" => Ok(Self::WaitingOnCustomer),
            "resolved" => Ok(Self::Resolved),
            "closed" => Ok(Self::Closed),
            _ => Err(TicketError::InvalidStatus),
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::InProgress => "in_progress",
            Self::WaitingOnCustomer => "waiting_on_customer",
            Self::Resolved => "resolved",
            Self::Closed => "closed",
        }
    }

    /// Validates a lifecycle transition.
    ///
    /// # Errors
    ///
    /// Returns [`TicketError::InvalidTransition`] when the transition is forbidden.
    pub fn transition_to(self, next: Self) -> Result<(), TicketError> {
        let allowed = self == next
            || match self {
                Self::Open => matches!(next, Self::InProgress | Self::Resolved | Self::Closed),
                Self::InProgress => matches!(
                    next,
                    Self::WaitingOnCustomer | Self::Resolved | Self::Closed
                ),
                Self::WaitingOnCustomer => {
                    matches!(next, Self::InProgress | Self::Resolved | Self::Closed)
                }
                Self::Resolved => matches!(next, Self::Open | Self::Closed),
                Self::Closed => next == Self::Open,
            };
        allowed.then_some(()).ok_or(TicketError::InvalidTransition)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TicketPriority {
    Low,
    Normal,
    High,
    Urgent,
}

impl TicketPriority {
    /// Parses the persisted/API priority representation.
    ///
    /// # Errors
    ///
    /// Returns [`TicketError::InvalidPriority`] for unknown values.
    pub fn parse(value: &str) -> Result<Self, TicketError> {
        match value {
            "low" => Ok(Self::Low),
            "normal" => Ok(Self::Normal),
            "high" => Ok(Self::High),
            "urgent" => Ok(Self::Urgent),
            _ => Err(TicketError::InvalidPriority),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SlaTarget {
    pub response_due_at: OffsetDateTime,
    pub resolution_due_at: OffsetDateTime,
}

#[must_use]
pub fn sla_target(
    now: OffsetDateTime,
    response_minutes: i32,
    resolution_minutes: i32,
) -> SlaTarget {
    SlaTarget {
        response_due_at: now + Duration::minutes(i64::from(response_minutes)),
        resolution_due_at: now + Duration::minutes(i64::from(resolution_minutes)),
    }
}

#[derive(Debug, Error)]
pub enum TicketError {
    #[error("invalid ticket status")]
    InvalidStatus,
    #[error("invalid ticket priority")]
    InvalidPriority,
    #[error("invalid ticket status transition")]
    InvalidTransition,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_ticket_must_reopen_before_work_resumes() {
        assert!(
            TicketStatus::Closed
                .transition_to(TicketStatus::Open)
                .is_ok()
        );
        assert!(
            TicketStatus::Closed
                .transition_to(TicketStatus::InProgress)
                .is_err()
        );
    }

    #[test]
    fn sla_deadlines_are_deterministic() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let target = sla_target(now, 30, 240);
        assert_eq!(target.response_due_at, now + Duration::minutes(30));
        assert_eq!(target.resolution_due_at, now + Duration::hours(4));
    }
}
