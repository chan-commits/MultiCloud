use multicloud_shared_kernel::{OrganizationId, UserId};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt, str::FromStr};
use thiserror::Error;
use uuid::Uuid;

pub mod permissions {
    pub const ORGANIZATION_READ: &str = "organization.organization.read";
    pub const ORGANIZATION_UPDATE: &str = "organization.organization.update";
    pub const MEMBER_READ: &str = "organization.member.read";
    pub const MEMBER_MANAGE: &str = "organization.member.manage";
    pub const INVITATION_MANAGE: &str = "organization.invitation.manage";
    pub const ROLE_READ: &str = "authorization.role.read";
    pub const ROLE_MANAGE: &str = "authorization.role.manage";
    pub const BINDING_MANAGE: &str = "authorization.binding.manage";
    pub const OPERATION_READ: &str = "operation.operation.read";
    pub const OPERATION_CANCEL: &str = "operation.operation.cancel";
    pub const PROVIDER_ACCOUNT_READ: &str = "provider.account.read";
    pub const PROVIDER_ACCOUNT_MANAGE: &str = "provider.account.manage";
    pub const PROVIDER_CONNECTION_TEST: &str = "provider.connection.test";

    pub const ALL: &[(&str, &str)] = &[
        (ORGANIZATION_READ, "Read organization details"),
        (ORGANIZATION_UPDATE, "Update organization settings"),
        (MEMBER_READ, "Read organization members"),
        (MEMBER_MANAGE, "Manage organization members"),
        (INVITATION_MANAGE, "Create and revoke invitations"),
        (ROLE_READ, "Read roles and permissions"),
        (ROLE_MANAGE, "Create and update custom roles"),
        (BINDING_MANAGE, "Assign and remove role bindings"),
        (OPERATION_READ, "Read operation status"),
        (OPERATION_CANCEL, "Cancel queued operations"),
        (PROVIDER_ACCOUNT_READ, "Read provider accounts"),
        (
            PROVIDER_ACCOUNT_MANAGE,
            "Manage provider accounts and credentials",
        ),
        (
            PROVIDER_CONNECTION_TEST,
            "Validate provider credentials and discover capabilities",
        ),
    ];
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PermissionKey(String);

impl PermissionKey {
    /// Parses a permission key in `domain.resource.action` form.
    ///
    /// # Errors
    ///
    /// Returns [`AuthorizationError::InvalidPermissionKey`] for malformed keys.
    pub fn parse(value: impl Into<String>) -> Result<Self, AuthorizationError> {
        let value = value.into();
        let valid = value.split('.').count() == 3
            && value.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '.' | '_' | '-')
            });
        valid
            .then_some(Self(value))
            .ok_or(AuthorizationError::InvalidPermissionKey)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PermissionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for PermissionKey {
    type Err = AuthorizationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Debug, Default)]
pub struct PermissionSet(HashSet<PermissionKey>);

impl PermissionSet {
    #[must_use]
    pub fn new(values: impl IntoIterator<Item = PermissionKey>) -> Self {
        Self(values.into_iter().collect())
    }

    #[must_use]
    pub fn allows(&self, required: &PermissionKey) -> bool {
        self.0.contains(required)
    }

    pub fn values(&self) -> impl Iterator<Item = &PermissionKey> {
        self.0.iter()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectType {
    User,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeType {
    Organization,
}

#[derive(Clone, Debug)]
pub struct RoleBinding {
    pub id: Uuid,
    pub organization_id: OrganizationId,
    pub role_id: Uuid,
    pub subject_type: SubjectType,
    pub subject_id: UserId,
    pub scope_type: ScopeType,
    pub scope_id: OrganizationId,
}

#[derive(Debug, Error)]
pub enum AuthorizationError {
    #[error("permission key is invalid")]
    InvalidPermissionKey,
    #[error("permission is denied")]
    Denied,
    #[error("role was not found")]
    RoleNotFound,
    #[error("authorization persistence failed")]
    Persistence,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_key_requires_three_segments() {
        assert!(PermissionKey::parse("organization.member.read").is_ok());
        assert!(PermissionKey::parse("member.read").is_err());
        assert!(PermissionKey::parse("Organization.member.read").is_err());
    }

    #[test]
    fn permission_set_denies_missing_permission() {
        let granted = PermissionKey::parse(permissions::ORGANIZATION_READ).expect("valid key");
        let required = PermissionKey::parse(permissions::ROLE_MANAGE).expect("valid key");
        let set = PermissionSet::new([granted]);
        assert!(!set.allows(&required));
    }
}
