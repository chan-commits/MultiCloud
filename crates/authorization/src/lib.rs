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
    pub const RESOURCE_READ: &str = "resource.resource.read";
    pub const RESOURCE_MANAGE: &str = "resource.resource.manage";
    pub const RESOURCE_SYNC: &str = "resource.sync.execute";
    pub const RECONCILIATION_MANAGE: &str = "resource.reconciliation.manage";
    pub const AUDIT_READ: &str = "audit.log.read";
    pub const AUDIT_EXPORT: &str = "audit.log.export";
    pub const AUDIT_RETENTION_MANAGE: &str = "audit.retention.manage";
    pub const TICKET_READ: &str = "ticket.ticket.read";
    pub const TICKET_CREATE: &str = "ticket.ticket.create";
    pub const TICKET_COMMENT: &str = "ticket.comment.create";
    pub const TICKET_MANAGE: &str = "ticket.ticket.manage";
    pub const TICKET_SLA_MANAGE: &str = "ticket.sla.manage";

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
        (RESOURCE_READ, "Read canonical resources and state"),
        (
            RESOURCE_MANAGE,
            "Manage resource desired state and lifecycle",
        ),
        (RESOURCE_SYNC, "Execute provider inventory synchronization"),
        (
            RECONCILIATION_MANAGE,
            "Approve and manage reconciliation tasks",
        ),
        (AUDIT_READ, "Read tenant audit logs"),
        (AUDIT_EXPORT, "Export tenant audit logs"),
        (AUDIT_RETENTION_MANAGE, "Manage tenant audit retention"),
        (TICKET_READ, "Read organization tickets"),
        (TICKET_CREATE, "Create support tickets"),
        (TICKET_COMMENT, "Comment on support tickets"),
        (TICKET_MANAGE, "Assign and transition tickets"),
        (TICKET_SLA_MANAGE, "Manage ticket SLA policies"),
    ];
}

pub struct SystemRoleSpec {
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub permission_keys: &'static [&'static str],
}

#[must_use]
pub fn system_role_specs() -> [SystemRoleSpec; 4] {
    use permissions as permission;
    const OWNER_ADMIN: &[&str] = &[
        permission::ORGANIZATION_READ,
        permission::ORGANIZATION_UPDATE,
        permission::MEMBER_READ,
        permission::MEMBER_MANAGE,
        permission::INVITATION_MANAGE,
        permission::ROLE_READ,
        permission::ROLE_MANAGE,
        permission::BINDING_MANAGE,
        permission::OPERATION_READ,
        permission::OPERATION_CANCEL,
        permission::PROVIDER_ACCOUNT_READ,
        permission::PROVIDER_ACCOUNT_MANAGE,
        permission::PROVIDER_CONNECTION_TEST,
        permission::RESOURCE_READ,
        permission::RESOURCE_MANAGE,
        permission::RESOURCE_SYNC,
        permission::RECONCILIATION_MANAGE,
        permission::AUDIT_READ,
        permission::AUDIT_EXPORT,
        permission::AUDIT_RETENTION_MANAGE,
        permission::TICKET_READ,
        permission::TICKET_CREATE,
        permission::TICKET_COMMENT,
        permission::TICKET_MANAGE,
        permission::TICKET_SLA_MANAGE,
    ];
    [
        SystemRoleSpec {
            key: "owner",
            name: "Owner",
            description: "Full organization access",
            permission_keys: OWNER_ADMIN,
        },
        SystemRoleSpec {
            key: "admin",
            name: "Admin",
            description: "Organization administration access",
            permission_keys: OWNER_ADMIN,
        },
        SystemRoleSpec {
            key: "member",
            name: "Member",
            description: "Standard organization access",
            permission_keys: &[
                permission::ORGANIZATION_READ,
                permission::MEMBER_READ,
                permission::OPERATION_READ,
                permission::PROVIDER_ACCOUNT_READ,
                permission::RESOURCE_READ,
                permission::TICKET_READ,
                permission::TICKET_CREATE,
                permission::TICKET_COMMENT,
            ],
        },
        SystemRoleSpec {
            key: "viewer",
            name: "Viewer",
            description: "Read-only organization access",
            permission_keys: &[
                permission::ORGANIZATION_READ,
                permission::PROVIDER_ACCOUNT_READ,
                permission::RESOURCE_READ,
                permission::TICKET_READ,
            ],
        },
    ]
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
