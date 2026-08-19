use async_trait::async_trait;
use multicloud_shared_kernel::{InvitationId, MembershipId, OrganizationId, UserId};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationStatus {
    Active,
    Suspended,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Organization {
    pub id: OrganizationId,
    pub slug: String,
    pub name: String,
    pub status: OrganizationStatus,
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MembershipStatus {
    Active,
    Suspended,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Membership {
    pub id: MembershipId,
    pub organization_id: OrganizationId,
    pub user_id: UserId,
    pub status: MembershipStatus,
    pub joined_at: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Invitation {
    pub id: InvitationId,
    pub organization_id: OrganizationId,
    pub email: String,
    pub invited_by: UserId,
    pub expires_at: OffsetDateTime,
    pub accepted_at: Option<OffsetDateTime>,
}

impl Invitation {
    #[must_use]
    pub fn can_be_accepted_at(&self, now: OffsetDateTime) -> bool {
        self.accepted_at.is_none() && self.expires_at > now
    }
}

impl Membership {
    #[must_use]
    pub const fn grants_access(&self) -> bool {
        matches!(self.status, MembershipStatus::Active)
    }
}

#[derive(Debug, Error)]
pub enum OrganizationError {
    #[error("organization was not found")]
    NotFound,
    #[error("organization membership is required")]
    MembershipRequired,
    #[error("organization is unavailable")]
    Unavailable,
    #[error("organization persistence failed")]
    Persistence,
}

#[async_trait]
pub trait OrganizationRepository: Send + Sync {
    async fn find_by_id(
        &self,
        id: OrganizationId,
    ) -> Result<Option<Organization>, OrganizationError>;
}

#[async_trait]
pub trait MembershipRepository: Send + Sync {
    async fn find_active(
        &self,
        organization_id: OrganizationId,
        user_id: UserId,
    ) -> Result<Option<Membership>, OrganizationError>;

    async fn list_for_user(&self, user_id: UserId) -> Result<Vec<Membership>, OrganizationError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_active_membership_grants_access() {
        let membership = Membership {
            id: MembershipId::new(),
            organization_id: OrganizationId::new(),
            user_id: UserId::new(),
            status: MembershipStatus::Suspended,
            joined_at: OffsetDateTime::now_utc(),
        };

        assert!(!membership.grants_access());
    }

    #[test]
    fn expired_invitation_cannot_be_accepted() {
        let invitation = Invitation {
            id: InvitationId::new(),
            organization_id: OrganizationId::new(),
            email: "user@example.com".to_owned(),
            invited_by: UserId::new(),
            expires_at: OffsetDateTime::now_utc() - time::Duration::minutes(1),
            accepted_at: None,
        };

        assert!(!invitation.can_be_accepted_at(OffsetDateTime::now_utc()));
    }
}
