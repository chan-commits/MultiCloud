use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(r"
            CREATE TABLE audit_logs (
                id uuid NOT NULL,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                source_event_id uuid NOT NULL,
                actor_type varchar(32) NOT NULL DEFAULT 'system',
                actor_id uuid,
                action varchar(160) NOT NULL,
                target_type varchar(120) NOT NULL,
                target_id varchar(255) NOT NULL,
                outcome varchar(32) NOT NULL,
                severity varchar(16) NOT NULL DEFAULT 'info',
                trace_id varchar(160),
                request_id varchar(160),
                client_ip inet,
                user_agent varchar(512),
                changes jsonb NOT NULL DEFAULT '{}'::jsonb,
                metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
                occurred_at timestamptz NOT NULL,
                recorded_at timestamptz NOT NULL DEFAULT now(),
                PRIMARY KEY (occurred_at, id),
                CONSTRAINT audit_logs_source_event_uq UNIQUE (occurred_at, source_event_id),
                CONSTRAINT audit_logs_actor_type_check CHECK (actor_type IN ('user', 'agent', 'system')),
                CONSTRAINT audit_logs_outcome_check CHECK (outcome IN ('attempted', 'succeeded', 'failed', 'denied', 'cancelled')),
                CONSTRAINT audit_logs_severity_check CHECK (severity IN ('info', 'warning', 'critical'))
            ) PARTITION BY RANGE (occurred_at);

            CREATE TABLE audit_logs_2026_08 PARTITION OF audit_logs
                FOR VALUES FROM ('2026-08-01 00:00:00+00') TO ('2026-09-01 00:00:00+00');
            CREATE TABLE audit_logs_default PARTITION OF audit_logs DEFAULT;
            CREATE INDEX audit_logs_tenant_time_idx ON audit_logs (organization_id, occurred_at DESC);
            CREATE INDEX audit_logs_tenant_action_idx ON audit_logs (organization_id, action, occurred_at DESC);
            CREATE INDEX audit_logs_tenant_target_idx ON audit_logs (organization_id, target_type, target_id, occurred_at DESC);
            CREATE INDEX audit_logs_actor_idx ON audit_logs (organization_id, actor_id, occurred_at DESC) WHERE actor_id IS NOT NULL;

            CREATE TABLE audit_retention_policies (
                organization_id uuid PRIMARY KEY REFERENCES organizations(id),
                retention_days integer NOT NULL DEFAULT 365,
                export_retention_days integer NOT NULL DEFAULT 7,
                updated_by uuid REFERENCES users(id),
                updated_at timestamptz NOT NULL DEFAULT now(),
                CONSTRAINT audit_retention_days_check CHECK (retention_days BETWEEN 90 AND 3650),
                CONSTRAINT audit_export_retention_days_check CHECK (export_retention_days BETWEEN 1 AND 30)
            );

            CREATE FUNCTION app.reject_audit_mutation() RETURNS trigger
            LANGUAGE plpgsql AS $$ BEGIN
                RAISE EXCEPTION 'audit logs are append-only';
            END $$;
            CREATE TRIGGER audit_logs_append_only
                BEFORE UPDATE OR DELETE ON audit_logs
                FOR EACH ROW EXECUTE FUNCTION app.reject_audit_mutation();

            INSERT INTO permissions (id, key, description) VALUES
                ('10000000-0000-7000-8000-000000000018', 'audit.log.read', 'Read tenant audit logs'),
                ('10000000-0000-7000-8000-000000000019', 'audit.log.export', 'Export tenant audit logs'),
                ('10000000-0000-7000-8000-000000000020', 'audit.retention.manage', 'Manage tenant audit retention');
            INSERT INTO role_permissions (role_id, permission_id, organization_id)
            SELECT role.id, permission.id, role.organization_id
            FROM roles AS role
            JOIN permissions AS permission ON permission.key IN ('audit.log.read', 'audit.log.export', 'audit.retention.manage')
            WHERE role.key IN ('owner', 'admin')
            UNION ALL
            SELECT role.id, permission.id, role.organization_id
            FROM roles AS role
            JOIN permissions AS permission ON permission.key = 'audit.log.read'
            WHERE role.key IN ('member', 'viewer');

            ALTER TABLE audit_logs ENABLE ROW LEVEL SECURITY;
            ALTER TABLE audit_logs FORCE ROW LEVEL SECURITY;
            ALTER TABLE audit_retention_policies ENABLE ROW LEVEL SECURITY;
            ALTER TABLE audit_retention_policies FORCE ROW LEVEL SECURITY;
            CREATE POLICY audit_logs_tenant_isolation ON audit_logs
                USING (organization_id = app.current_organization_id())
                WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY audit_retention_tenant_isolation ON audit_retention_policies
                USING (organization_id = app.current_organization_id())
                WITH CHECK (organization_id = app.current_organization_id());
        ").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r"
            DELETE FROM role_permissions WHERE permission_id IN (
                SELECT id FROM permissions WHERE key IN ('audit.log.read', 'audit.log.export', 'audit.retention.manage')
            );
            DELETE FROM permissions WHERE key IN ('audit.log.read', 'audit.log.export', 'audit.retention.manage');
            DROP TABLE IF EXISTS audit_retention_policies;
            DROP TABLE IF EXISTS audit_logs CASCADE;
            DROP FUNCTION IF EXISTS app.reject_audit_mutation();
        ",
            )
            .await?;
        Ok(())
    }
}
