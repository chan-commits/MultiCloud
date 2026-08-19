use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
#[allow(clippy::too_many_lines)]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r"
                CREATE TABLE permissions (
                    id uuid PRIMARY KEY,
                    key varchar(160) NOT NULL UNIQUE,
                    description text NOT NULL,
                    created_at timestamptz NOT NULL DEFAULT now()
                );

                CREATE TABLE roles (
                    id uuid PRIMARY KEY,
                    organization_id uuid NOT NULL REFERENCES organizations(id),
                    key varchar(80) NOT NULL,
                    name varchar(120) NOT NULL,
                    description text NOT NULL DEFAULT '',
                    is_system boolean NOT NULL DEFAULT false,
                    created_at timestamptz NOT NULL DEFAULT now(),
                    updated_at timestamptz NOT NULL DEFAULT now(),
                    CONSTRAINT roles_organization_key_uq UNIQUE (organization_id, key)
                );

                CREATE TABLE role_permissions (
                    role_id uuid NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
                    permission_id uuid NOT NULL REFERENCES permissions(id),
                    organization_id uuid NOT NULL REFERENCES organizations(id),
                    PRIMARY KEY (role_id, permission_id)
                );
                CREATE INDEX role_permissions_organization_idx
                    ON role_permissions (organization_id, role_id);

                CREATE TABLE role_bindings (
                    id uuid PRIMARY KEY,
                    organization_id uuid NOT NULL REFERENCES organizations(id),
                    role_id uuid NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
                    subject_type varchar(32) NOT NULL,
                    subject_id uuid NOT NULL,
                    scope_type varchar(32) NOT NULL,
                    scope_id uuid NOT NULL,
                    created_by uuid NOT NULL REFERENCES users(id),
                    created_at timestamptz NOT NULL DEFAULT now(),
                    CONSTRAINT role_bindings_subject_check CHECK (subject_type IN ('user')),
                    CONSTRAINT role_bindings_scope_check CHECK (scope_type IN ('organization')),
                    CONSTRAINT role_bindings_unique_assignment_uq UNIQUE (
                        organization_id, role_id, subject_type, subject_id, scope_type, scope_id
                    )
                );
                CREATE INDEX role_bindings_subject_scope_idx
                    ON role_bindings (organization_id, subject_type, subject_id, scope_type, scope_id);

                INSERT INTO permissions (id, key, description) VALUES
                    ('10000000-0000-7000-8000-000000000001', 'organization.organization.read', 'Read organization details'),
                    ('10000000-0000-7000-8000-000000000002', 'organization.organization.update', 'Update organization settings'),
                    ('10000000-0000-7000-8000-000000000003', 'organization.member.read', 'Read organization members'),
                    ('10000000-0000-7000-8000-000000000004', 'organization.member.manage', 'Manage organization members'),
                    ('10000000-0000-7000-8000-000000000005', 'organization.invitation.manage', 'Create and revoke invitations'),
                    ('10000000-0000-7000-8000-000000000006', 'authorization.role.read', 'Read roles and permissions'),
                    ('10000000-0000-7000-8000-000000000007', 'authorization.role.manage', 'Create and update custom roles'),
                    ('10000000-0000-7000-8000-000000000008', 'authorization.binding.manage', 'Assign and remove role bindings');

                INSERT INTO roles (id, organization_id, key, name, description, is_system)
                SELECT gen_random_uuid(), id, role.key, role.name, role.description, true
                FROM organizations
                CROSS JOIN (VALUES
                    ('owner', 'Owner', 'Full organization access'),
                    ('admin', 'Admin', 'Organization administration access'),
                    ('member', 'Member', 'Standard organization access'),
                    ('viewer', 'Viewer', 'Read-only organization access')
                ) AS role(key, name, description);

                INSERT INTO role_permissions (role_id, permission_id, organization_id)
                SELECT roles.id, permissions.id, roles.organization_id
                FROM roles
                CROSS JOIN permissions
                WHERE roles.key IN ('owner', 'admin')
                   OR (roles.key = 'member' AND permissions.key IN (
                       'organization.organization.read', 'organization.member.read'
                   ))
                   OR (roles.key = 'viewer' AND permissions.key = 'organization.organization.read');

                WITH ranked_memberships AS (
                    SELECT organization_id, user_id,
                           row_number() OVER (
                               PARTITION BY organization_id ORDER BY joined_at, created_at, id
                           ) AS member_order
                    FROM organization_memberships
                    WHERE status = 'active'
                )
                INSERT INTO role_bindings (
                    id, organization_id, role_id, subject_type, subject_id,
                    scope_type, scope_id, created_by
                )
                SELECT gen_random_uuid(), membership.organization_id, roles.id, 'user',
                       membership.user_id, 'organization', membership.organization_id,
                       membership.user_id
                FROM ranked_memberships AS membership
                JOIN roles ON roles.organization_id = membership.organization_id
                          AND roles.key = CASE
                              WHEN membership.member_order = 1 THEN 'owner'
                              ELSE 'member'
                          END;

                ALTER TABLE roles ENABLE ROW LEVEL SECURITY;
                ALTER TABLE roles FORCE ROW LEVEL SECURITY;
                ALTER TABLE role_permissions ENABLE ROW LEVEL SECURITY;
                ALTER TABLE role_permissions FORCE ROW LEVEL SECURITY;
                ALTER TABLE role_bindings ENABLE ROW LEVEL SECURITY;
                ALTER TABLE role_bindings FORCE ROW LEVEL SECURITY;

                CREATE POLICY roles_tenant_isolation ON roles
                    USING (organization_id = app.current_organization_id())
                    WITH CHECK (organization_id = app.current_organization_id());
                CREATE POLICY role_permissions_tenant_isolation ON role_permissions
                    USING (organization_id = app.current_organization_id())
                    WITH CHECK (organization_id = app.current_organization_id());
                CREATE POLICY role_bindings_tenant_isolation ON role_bindings
                    USING (organization_id = app.current_organization_id())
                    WITH CHECK (
                        organization_id = app.current_organization_id()
                        AND scope_id = app.current_organization_id()
                    );
                ",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r"
                DROP TABLE IF EXISTS role_bindings;
                DROP TABLE IF EXISTS role_permissions;
                DROP TABLE IF EXISTS roles;
                DROP TABLE IF EXISTS permissions;
                ",
            )
            .await?;
        Ok(())
    }
}
