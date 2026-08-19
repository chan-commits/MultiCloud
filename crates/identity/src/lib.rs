use async_trait::async_trait;
use multicloud_shared_kernel::{OrganizationId, SessionId, UserId};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use time::OffsetDateTime;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Email(String);

impl Email {
    /// Creates a normalized email value.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::InvalidEmail`] for malformed input.
    pub fn parse(value: impl Into<String>) -> Result<Self, IdentityError> {
        let normalized = value.into().trim().to_lowercase();
        let valid = normalized.len() <= 320
            && normalized
                .split_once('@')
                .is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.'));

        valid
            .then_some(Self(normalized))
            .ok_or(IdentityError::InvalidEmail)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Email {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Suspended,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub email: Email,
    pub display_name: String,
    pub status: UserStatus,
    pub email_verified_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub id: SessionId,
    pub user_id: UserId,
    pub organization_id: Option<OrganizationId>,
    pub expires_at: OffsetDateTime,
    pub revoked_at: Option<OffsetDateTime>,
}

impl Session {
    #[must_use]
    pub fn is_active_at(&self, now: OffsetDateTime) -> bool {
        self.revoked_at.is_none() && self.expires_at > now
    }
}

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("email is invalid")]
    InvalidEmail,
    #[error("user was not found")]
    UserNotFound,
    #[error("session is invalid or expired")]
    InvalidSession,
    #[error("identity persistence failed")]
    Persistence,
}

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, IdentityError>;
    async fn find_by_email(&self, email: &Email) -> Result<Option<User>, IdentityError>;
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn find_by_token_hash(&self, token_hash: &[u8])
    -> Result<Option<Session>, IdentityError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_is_normalized() {
        let email = Email::parse("  User@Example.COM ").expect("valid email");
        assert_eq!(email.as_str(), "user@example.com");
    }

    #[test]
    fn malformed_email_is_rejected() {
        assert!(matches!(
            Email::parse("invalid"),
            Err(IdentityError::InvalidEmail)
        ));
    }
}
