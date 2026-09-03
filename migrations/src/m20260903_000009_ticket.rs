use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
#[allow(clippy::too_many_lines)]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(r"
            CREATE TABLE ticket_counters (
                organization_id uuid PRIMARY KEY REFERENCES organizations(id) ON DELETE CASCADE,
                next_number bigint NOT NULL DEFAULT 1 CHECK (next_number > 0)
            );
            CREATE TABLE sla_policies (
                id uuid PRIMARY KEY,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                name varchar(120) NOT NULL,
                response_minutes integer NOT NULL CHECK (response_minutes BETWEEN 1 AND 43200),
                resolution_minutes integer NOT NULL CHECK (resolution_minutes BETWEEN 1 AND 525600),
                is_default boolean NOT NULL DEFAULT false,
                created_at timestamptz NOT NULL DEFAULT now(),
                updated_at timestamptz NOT NULL DEFAULT now(),
                UNIQUE (organization_id, name),
                CHECK (resolution_minutes >= response_minutes)
            );
            CREATE UNIQUE INDEX sla_policies_one_default_uq
                ON sla_policies (organization_id) WHERE is_default;
            CREATE TABLE tickets (
                id uuid PRIMARY KEY,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                number bigint NOT NULL,
                subject varchar(200) NOT NULL,
                description text NOT NULL,
                status varchar(32) NOT NULL DEFAULT 'open',
                priority varchar(16) NOT NULL DEFAULT 'normal',
                requester_id uuid NOT NULL REFERENCES users(id),
                assigned_to uuid REFERENCES users(id),
                sla_policy_id uuid REFERENCES sla_policies(id),
                response_due_at timestamptz,
                resolution_due_at timestamptz,
                first_responded_at timestamptz,
                resolved_at timestamptz,
                version integer NOT NULL DEFAULT 1,
                created_at timestamptz NOT NULL DEFAULT now(),
                updated_at timestamptz NOT NULL DEFAULT now(),
                UNIQUE (organization_id, number),
                CHECK (status IN ('open', 'in_progress', 'waiting_on_customer', 'resolved', 'closed')),
                CHECK (priority IN ('low', 'normal', 'high', 'urgent')),
                CHECK (version > 0)
            );
            CREATE INDEX tickets_tenant_status_idx ON tickets (organization_id, status, updated_at DESC);
            CREATE INDEX tickets_tenant_assignee_idx ON tickets (organization_id, assigned_to, updated_at DESC);
            CREATE INDEX tickets_sla_idx ON tickets (organization_id, resolution_due_at)
                WHERE status NOT IN ('resolved', 'closed');
            CREATE TABLE ticket_comments (
                id uuid PRIMARY KEY,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                ticket_id uuid NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
                author_id uuid NOT NULL REFERENCES users(id),
                body text NOT NULL,
                visibility varchar(16) NOT NULL DEFAULT 'public',
                created_at timestamptz NOT NULL DEFAULT now(),
                CHECK (visibility IN ('public', 'internal')),
                CHECK (length(body) BETWEEN 1 AND 20000)
            );
            CREATE INDEX ticket_comments_ticket_time_idx ON ticket_comments (organization_id, ticket_id, created_at);
            CREATE TABLE attachments (
                id uuid PRIMARY KEY,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                owner_type varchar(32) NOT NULL,
                owner_id uuid NOT NULL,
                storage_key varchar(512) NOT NULL,
                filename varchar(255) NOT NULL,
                content_type varchar(160) NOT NULL,
                size_bytes bigint NOT NULL,
                checksum_sha256 varchar(64) NOT NULL,
                uploaded_by uuid NOT NULL REFERENCES users(id),
                created_at timestamptz NOT NULL DEFAULT now(),
                UNIQUE (organization_id, storage_key),
                CHECK (owner_type IN ('ticket', 'ticket_comment')),
                CHECK (size_bytes BETWEEN 1 AND 104857600),
                CHECK (checksum_sha256 ~ '^[0-9a-f]{64}$')
            );
            CREATE INDEX attachments_owner_idx ON attachments (organization_id, owner_type, owner_id);
            CREATE TABLE notifications (
                id uuid PRIMARY KEY,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                recipient_user_id uuid NOT NULL REFERENCES users(id),
                notification_type varchar(120) NOT NULL,
                payload jsonb NOT NULL DEFAULT '{}'::jsonb,
                read_at timestamptz,
                created_at timestamptz NOT NULL DEFAULT now()
            );
            CREATE INDEX notifications_recipient_idx
                ON notifications (organization_id, recipient_user_id, created_at DESC);

            INSERT INTO permissions (id, key, description) VALUES
                ('10000000-0000-7000-8000-000000000021', 'ticket.ticket.read', 'Read organization tickets'),
                ('10000000-0000-7000-8000-000000000022', 'ticket.ticket.create', 'Create support tickets'),
                ('10000000-0000-7000-8000-000000000023', 'ticket.comment.create', 'Comment on support tickets'),
                ('10000000-0000-7000-8000-000000000024', 'ticket.ticket.manage', 'Assign and transition tickets'),
                ('10000000-0000-7000-8000-000000000025', 'ticket.sla.manage', 'Manage ticket SLA policies');
            INSERT INTO role_permissions (role_id, permission_id, organization_id)
            SELECT role.id, permission.id, role.organization_id
            FROM roles role
            JOIN permissions permission ON permission.key LIKE 'ticket.%'
            WHERE role.key IN ('owner', 'admin')
               OR (role.key = 'member' AND permission.key IN (
                    'ticket.ticket.read', 'ticket.ticket.create', 'ticket.comment.create'
               ))
               OR (role.key = 'viewer' AND permission.key = 'ticket.ticket.read');

            ALTER TABLE ticket_counters ENABLE ROW LEVEL SECURITY;
            ALTER TABLE ticket_counters FORCE ROW LEVEL SECURITY;
            ALTER TABLE sla_policies ENABLE ROW LEVEL SECURITY;
            ALTER TABLE sla_policies FORCE ROW LEVEL SECURITY;
            ALTER TABLE tickets ENABLE ROW LEVEL SECURITY;
            ALTER TABLE tickets FORCE ROW LEVEL SECURITY;
            ALTER TABLE ticket_comments ENABLE ROW LEVEL SECURITY;
            ALTER TABLE ticket_comments FORCE ROW LEVEL SECURITY;
            ALTER TABLE attachments ENABLE ROW LEVEL SECURITY;
            ALTER TABLE attachments FORCE ROW LEVEL SECURITY;
            ALTER TABLE notifications ENABLE ROW LEVEL SECURITY;
            ALTER TABLE notifications FORCE ROW LEVEL SECURITY;
            CREATE POLICY ticket_counters_tenant_isolation ON ticket_counters
                USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY sla_policies_tenant_isolation ON sla_policies
                USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY tickets_tenant_isolation ON tickets
                USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY ticket_comments_tenant_isolation ON ticket_comments
                USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY attachments_tenant_isolation ON attachments
                USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY notifications_tenant_isolation ON notifications
                USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
        ").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r"
            DELETE FROM role_permissions WHERE permission_id IN (
                SELECT id FROM permissions WHERE key LIKE 'ticket.%'
            );
            DELETE FROM permissions WHERE key LIKE 'ticket.%';
            DROP TABLE IF EXISTS notifications;
            DROP TABLE IF EXISTS attachments;
            DROP TABLE IF EXISTS ticket_comments;
            DROP TABLE IF EXISTS tickets;
            DROP TABLE IF EXISTS sla_policies;
            DROP TABLE IF EXISTS ticket_counters;
        ",
            )
            .await?;
        Ok(())
    }
}
