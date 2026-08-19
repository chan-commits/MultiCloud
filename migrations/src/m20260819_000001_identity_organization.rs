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
                CREATE SCHEMA IF NOT EXISTS app;

                CREATE FUNCTION app.current_user_id() RETURNS uuid
                LANGUAGE sql STABLE PARALLEL SAFE
                AS $$ SELECT NULLIF(current_setting('app.user_id', true), '')::uuid $$;

                CREATE FUNCTION app.current_organization_id() RETURNS uuid
                LANGUAGE sql STABLE PARALLEL SAFE
                AS $$ SELECT NULLIF(current_setting('app.organization_id', true), '')::uuid $$;

                CREATE TABLE users (
                    id uuid PRIMARY KEY,
                    email varchar(320) NOT NULL,
                    display_name varchar(120) NOT NULL,
                    status varchar(32) NOT NULL DEFAULT 'active',
                    password_hash text NOT NULL,
                    email_verified_at timestamptz,
                    created_at timestamptz NOT NULL DEFAULT now(),
                    updated_at timestamptz NOT NULL DEFAULT now(),
                    CONSTRAINT users_status_check CHECK (status IN ('active', 'suspended'))
                );
                CREATE UNIQUE INDEX users_email_normalized_uq ON users (lower(email));

                CREATE TABLE organizations (
                    id uuid PRIMARY KEY,
                    slug varchar(80) NOT NULL,
                    name varchar(160) NOT NULL,
                    status varchar(32) NOT NULL DEFAULT 'active',
                    settings jsonb NOT NULL DEFAULT '{}'::jsonb,
                    created_at timestamptz NOT NULL DEFAULT now(),
                    updated_at timestamptz NOT NULL DEFAULT now(),
                    deleted_at timestamptz,
                    CONSTRAINT organizations_status_check CHECK (status IN ('active', 'suspended'))
                );
                CREATE UNIQUE INDEX organizations_slug_uq ON organizations (lower(slug))
                    WHERE deleted_at IS NULL;

                CREATE TABLE organization_memberships (
                    id uuid PRIMARY KEY,
                    organization_id uuid NOT NULL REFERENCES organizations(id),
                    user_id uuid NOT NULL REFERENCES users(id),
                    status varchar(32) NOT NULL DEFAULT 'active',
                    joined_at timestamptz NOT NULL DEFAULT now(),
                    created_at timestamptz NOT NULL DEFAULT now(),
                    updated_at timestamptz NOT NULL DEFAULT now(),
                    CONSTRAINT memberships_status_check CHECK (status IN ('active', 'suspended')),
                    CONSTRAINT memberships_organization_user_uq UNIQUE (organization_id, user_id)
                );
                CREATE INDEX memberships_user_status_idx
                    ON organization_memberships (user_id, status, organization_id);

                CREATE TABLE organization_invitations (
                    id uuid PRIMARY KEY,
                    organization_id uuid NOT NULL REFERENCES organizations(id),
                    email varchar(320) NOT NULL,
                    token_hash bytea NOT NULL UNIQUE,
                    invited_by uuid NOT NULL REFERENCES users(id),
                    expires_at timestamptz NOT NULL,
                    accepted_at timestamptz,
                    created_at timestamptz NOT NULL DEFAULT now()
                );
                CREATE INDEX invitations_organization_email_idx
                    ON organization_invitations (organization_id, lower(email));

                CREATE TABLE sessions (
                    id uuid PRIMARY KEY,
                    user_id uuid NOT NULL REFERENCES users(id),
                    organization_id uuid REFERENCES organizations(id),
                    refresh_token_hash bytea NOT NULL UNIQUE,
                    expires_at timestamptz NOT NULL,
                    revoked_at timestamptz,
                    ip_address inet,
                    user_agent text,
                    created_at timestamptz NOT NULL DEFAULT now()
                );
                CREATE INDEX sessions_user_active_idx ON sessions (user_id, expires_at)
                    WHERE revoked_at IS NULL;

                ALTER TABLE organizations ENABLE ROW LEVEL SECURITY;
                ALTER TABLE organizations FORCE ROW LEVEL SECURITY;
                ALTER TABLE organization_memberships ENABLE ROW LEVEL SECURITY;
                ALTER TABLE organization_memberships FORCE ROW LEVEL SECURITY;
                ALTER TABLE organization_invitations ENABLE ROW LEVEL SECURITY;
                ALTER TABLE organization_invitations FORCE ROW LEVEL SECURITY;

                CREATE POLICY organizations_tenant_isolation ON organizations
                    USING (id = app.current_organization_id())
                    WITH CHECK (id = app.current_organization_id());

                CREATE POLICY memberships_tenant_isolation ON organization_memberships
                    USING (
                        user_id = app.current_user_id()
                        AND (
                            app.current_organization_id() IS NULL
                            OR organization_id = app.current_organization_id()
                        )
                    )
                    WITH CHECK (organization_id = app.current_organization_id());

                CREATE POLICY invitations_tenant_isolation ON organization_invitations
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
                DROP TABLE IF EXISTS sessions;
                DROP TABLE IF EXISTS organization_invitations;
                DROP TABLE IF EXISTS organization_memberships;
                DROP TABLE IF EXISTS organizations;
                DROP TABLE IF EXISTS users;
                DROP FUNCTION IF EXISTS app.current_organization_id();
                DROP FUNCTION IF EXISTS app.current_user_id();
                DROP SCHEMA IF EXISTS app;
                ",
            )
            .await?;
        Ok(())
    }
}
