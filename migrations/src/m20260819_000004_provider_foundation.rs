use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r"
                CREATE TABLE provider_accounts (
                    id uuid PRIMARY KEY,
                    organization_id uuid NOT NULL REFERENCES organizations(id),
                    provider_kind varchar(64) NOT NULL,
                    name varchar(160) NOT NULL,
                    status varchar(32) NOT NULL DEFAULT 'pending_validation',
                    configuration jsonb NOT NULL DEFAULT '{}'::jsonb,
                    capabilities jsonb NOT NULL DEFAULT '[]'::jsonb,
                    last_validated_at timestamptz,
                    last_error_code varchar(160),
                    created_by uuid NOT NULL REFERENCES users(id),
                    created_at timestamptz NOT NULL DEFAULT now(),
                    updated_at timestamptz NOT NULL DEFAULT now(),
                    CONSTRAINT provider_accounts_status_check CHECK (
                        status IN ('pending_validation', 'active', 'invalid', 'disabled')
                    ),
                    CONSTRAINT provider_accounts_name_uq UNIQUE (organization_id, name)
                );
                CREATE INDEX provider_accounts_tenant_kind_idx
                    ON provider_accounts (organization_id, provider_kind, status);

                CREATE TABLE provider_credentials (
                    id uuid PRIMARY KEY,
                    organization_id uuid NOT NULL REFERENCES organizations(id),
                    provider_account_id uuid NOT NULL REFERENCES provider_accounts(id) ON DELETE CASCADE,
                    credential_type varchar(64) NOT NULL,
                    ciphertext bytea NOT NULL,
                    nonce bytea NOT NULL,
                    key_version integer NOT NULL,
                    version integer NOT NULL,
                    status varchar(32) NOT NULL DEFAULT 'active',
                    created_by uuid NOT NULL REFERENCES users(id),
                    created_at timestamptz NOT NULL DEFAULT now(),
                    activated_at timestamptz NOT NULL DEFAULT now(),
                    revoked_at timestamptz,
                    CONSTRAINT provider_credentials_status_check CHECK (status IN ('active', 'revoked')),
                    CONSTRAINT provider_credentials_version_uq UNIQUE (provider_account_id, version)
                );
                CREATE UNIQUE INDEX provider_credentials_one_active_idx
                    ON provider_credentials (provider_account_id) WHERE status = 'active';
                CREATE INDEX provider_credentials_tenant_account_idx
                    ON provider_credentials (organization_id, provider_account_id, version DESC);

                INSERT INTO permissions (id, key, description) VALUES
                    ('10000000-0000-7000-8000-000000000011', 'provider.account.read', 'Read provider accounts'),
                    ('10000000-0000-7000-8000-000000000012', 'provider.account.manage', 'Manage provider accounts and credentials'),
                    ('10000000-0000-7000-8000-000000000013', 'provider.connection.test', 'Validate provider credentials and discover capabilities');

                INSERT INTO role_permissions (role_id, permission_id, organization_id)
                SELECT role.id, permission.id, role.organization_id
                FROM roles AS role
                JOIN permissions AS permission ON permission.key IN (
                    'provider.account.read', 'provider.account.manage', 'provider.connection.test'
                )
                WHERE role.key IN ('owner', 'admin')
                UNION ALL
                SELECT role.id, permission.id, role.organization_id
                FROM roles AS role
                JOIN permissions AS permission ON permission.key = 'provider.account.read'
                WHERE role.key IN ('member', 'viewer');

                ALTER TABLE provider_accounts ENABLE ROW LEVEL SECURITY;
                ALTER TABLE provider_accounts FORCE ROW LEVEL SECURITY;
                ALTER TABLE provider_credentials ENABLE ROW LEVEL SECURITY;
                ALTER TABLE provider_credentials FORCE ROW LEVEL SECURITY;

                CREATE POLICY provider_accounts_tenant_isolation ON provider_accounts
                    USING (organization_id = app.current_organization_id())
                    WITH CHECK (organization_id = app.current_organization_id());
                CREATE POLICY provider_credentials_tenant_isolation ON provider_credentials
                    USING (organization_id = app.current_organization_id())
                    WITH CHECK (organization_id = app.current_organization_id());
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
                DELETE FROM role_permissions WHERE permission_id IN (
                    SELECT id FROM permissions WHERE key IN (
                        'provider.account.read', 'provider.account.manage', 'provider.connection.test'
                    )
                );
                DELETE FROM permissions WHERE key IN (
                    'provider.account.read', 'provider.account.manage', 'provider.connection.test'
                );
                DROP TABLE IF EXISTS provider_credentials;
                DROP TABLE IF EXISTS provider_accounts;
                ",
            )
            .await?;
        Ok(())
    }
}
