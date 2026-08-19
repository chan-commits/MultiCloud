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
                CREATE TABLE operations (
                    id uuid PRIMARY KEY,
                    organization_id uuid NOT NULL REFERENCES organizations(id),
                    operation_type varchar(120) NOT NULL,
                    target_type varchar(120) NOT NULL,
                    target_id varchar(255),
                    requested_by uuid NOT NULL REFERENCES users(id),
                    idempotency_key varchar(255) NOT NULL,
                    status varchar(32) NOT NULL DEFAULT 'queued',
                    progress smallint NOT NULL DEFAULT 0,
                    error_code varchar(120),
                    error_message text,
                    started_at timestamptz,
                    completed_at timestamptz,
                    created_at timestamptz NOT NULL DEFAULT now(),
                    updated_at timestamptz NOT NULL DEFAULT now(),
                    CONSTRAINT operations_status_check CHECK (
                        status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled', 'timed_out')
                    ),
                    CONSTRAINT operations_progress_check CHECK (progress BETWEEN 0 AND 100),
                    CONSTRAINT operations_idempotency_uq UNIQUE (organization_id, idempotency_key)
                );
                CREATE INDEX operations_tenant_status_created_idx
                    ON operations (organization_id, status, created_at DESC);

                CREATE TABLE outbox_events (
                    id uuid PRIMARY KEY,
                    organization_id uuid NOT NULL REFERENCES organizations(id),
                    aggregate_type varchar(120) NOT NULL,
                    aggregate_id varchar(255) NOT NULL,
                    event_type varchar(160) NOT NULL,
                    event_version smallint NOT NULL DEFAULT 1,
                    payload jsonb NOT NULL,
                    trace_id varchar(160),
                    occurred_at timestamptz NOT NULL DEFAULT now(),
                    published_at timestamptz,
                    attempt_count integer NOT NULL DEFAULT 0,
                    next_attempt_at timestamptz NOT NULL DEFAULT now(),
                    last_error text,
                    dead_lettered_at timestamptz
                );
                CREATE INDEX outbox_pending_idx
                    ON outbox_events (next_attempt_at, occurred_at)
                    WHERE published_at IS NULL AND dead_lettered_at IS NULL;

                CREATE TABLE inbox_messages (
                    organization_id uuid NOT NULL REFERENCES organizations(id),
                    consumer varchar(160) NOT NULL,
                    message_id uuid NOT NULL,
                    processed_at timestamptz NOT NULL DEFAULT now(),
                    result jsonb,
                    PRIMARY KEY (organization_id, consumer, message_id)
                );
                CREATE INDEX inbox_processed_at_idx ON inbox_messages (processed_at);

                INSERT INTO permissions (id, key, description) VALUES
                    ('10000000-0000-7000-8000-000000000009', 'operation.operation.read', 'Read operation status'),
                    ('10000000-0000-7000-8000-000000000010', 'operation.operation.cancel', 'Cancel queued operations');

                INSERT INTO role_permissions (role_id, permission_id, organization_id)
                SELECT role.id, permission.id, role.organization_id
                FROM roles AS role
                JOIN permissions AS permission
                  ON permission.key IN ('operation.operation.read', 'operation.operation.cancel')
                WHERE role.key IN ('owner', 'admin')
                UNION ALL
                SELECT role.id, permission.id, role.organization_id
                FROM roles AS role
                JOIN permissions AS permission ON permission.key = 'operation.operation.read'
                WHERE role.key = 'member';

                ALTER TABLE operations ENABLE ROW LEVEL SECURITY;
                ALTER TABLE operations FORCE ROW LEVEL SECURITY;
                ALTER TABLE outbox_events ENABLE ROW LEVEL SECURITY;
                ALTER TABLE inbox_messages ENABLE ROW LEVEL SECURITY;

                CREATE POLICY operations_tenant_isolation ON operations
                    USING (organization_id = app.current_organization_id())
                    WITH CHECK (organization_id = app.current_organization_id());
                CREATE POLICY outbox_tenant_isolation ON outbox_events
                    USING (organization_id = app.current_organization_id())
                    WITH CHECK (organization_id = app.current_organization_id());
                CREATE POLICY inbox_tenant_isolation ON inbox_messages
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
                DELETE FROM role_permissions
                WHERE permission_id IN (
                    SELECT id FROM permissions
                    WHERE key IN ('operation.operation.read', 'operation.operation.cancel')
                );
                DELETE FROM permissions
                WHERE key IN ('operation.operation.read', 'operation.operation.cancel');
                DROP TABLE IF EXISTS inbox_messages;
                DROP TABLE IF EXISTS outbox_events;
                DROP TABLE IF EXISTS operations;
                ",
            )
            .await?;
        Ok(())
    }
}
