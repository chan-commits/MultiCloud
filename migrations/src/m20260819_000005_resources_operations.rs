use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    #[allow(clippy::too_many_lines)]
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(r"
            ALTER TABLE provider_credentials
                ADD COLUMN risk_level varchar(32) NOT NULL DEFAULT 'restricted',
                ADD COLUMN masked_identifier varchar(320),
                ADD COLUMN schema_version integer NOT NULL DEFAULT 1,
                ADD CONSTRAINT provider_credentials_risk_check CHECK (risk_level IN ('restricted', 'high'));
            ALTER TABLE operations ADD COLUMN next_attempt_at timestamptz NOT NULL DEFAULT now();
            CREATE INDEX operations_ready_idx ON operations (next_attempt_at, created_at)
                WHERE status = 'queued';

            CREATE TABLE resources (
                id uuid PRIMARY KEY,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                resource_type varchar(64) NOT NULL,
                name varchar(255) NOT NULL,
                lifecycle varchar(32) NOT NULL DEFAULT 'unknown',
                region varchar(120),
                attributes jsonb NOT NULL DEFAULT '{}'::jsonb,
                created_at timestamptz NOT NULL DEFAULT now(),
                updated_at timestamptz NOT NULL DEFAULT now(),
                CONSTRAINT resources_lifecycle_check CHECK (lifecycle IN (
                    'provisioning', 'active', 'stopped', 'deleting', 'deleted', 'error', 'unknown'
                ))
            );
            CREATE INDEX resources_tenant_type_idx ON resources (organization_id, resource_type, lifecycle);

            CREATE TABLE external_resource_mappings (
                id uuid PRIMARY KEY,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                provider_account_id uuid NOT NULL REFERENCES provider_accounts(id),
                resource_id uuid NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
                external_type varchar(120) NOT NULL,
                external_id varchar(512) NOT NULL,
                client_reference varchar(255),
                first_seen_at timestamptz NOT NULL DEFAULT now(),
                last_seen_at timestamptz NOT NULL DEFAULT now(),
                missing_since timestamptz,
                CONSTRAINT external_resource_identity_uq UNIQUE (
                    provider_account_id, external_type, external_id
                )
            );
            CREATE INDEX external_mapping_resource_idx ON external_resource_mappings (organization_id, resource_id);
            CREATE UNIQUE INDEX external_mapping_client_reference_uq
                ON external_resource_mappings (provider_account_id, client_reference)
                WHERE client_reference IS NOT NULL;

            CREATE TABLE resource_desired_states (
                id uuid PRIMARY KEY,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                resource_id uuid NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
                version bigint NOT NULL,
                managed_fields jsonb NOT NULL,
                state jsonb NOT NULL,
                state_hash char(64) NOT NULL,
                created_by uuid NOT NULL REFERENCES users(id),
                created_at timestamptz NOT NULL DEFAULT now(),
                CONSTRAINT resource_desired_version_uq UNIQUE (resource_id, version)
            );
            CREATE INDEX resource_desired_latest_idx ON resource_desired_states (organization_id, resource_id, version DESC);

            CREATE TABLE resource_observed_states (
                id uuid PRIMARY KEY,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                resource_id uuid NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
                mapping_id uuid NOT NULL REFERENCES external_resource_mappings(id) ON DELETE CASCADE,
                version bigint NOT NULL,
                state jsonb NOT NULL,
                state_hash char(64) NOT NULL,
                is_latest boolean NOT NULL DEFAULT true,
                observed_at timestamptz NOT NULL,
                created_at timestamptz NOT NULL DEFAULT now(),
                CONSTRAINT resource_observed_version_uq UNIQUE (resource_id, version)
            );
            CREATE UNIQUE INDEX resource_observed_one_latest_uq ON resource_observed_states (resource_id) WHERE is_latest;

            CREATE TABLE resource_metadata (
                id uuid PRIMARY KEY,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                resource_id uuid NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
                source varchar(120) NOT NULL,
                namespace varchar(120) NOT NULL,
                metadata jsonb NOT NULL,
                observed_at timestamptz NOT NULL,
                CONSTRAINT resource_metadata_source_uq UNIQUE (resource_id, source, namespace)
            );

            CREATE TABLE resource_drifts (
                id uuid PRIMARY KEY,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                resource_id uuid NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
                desired_state_id uuid NOT NULL REFERENCES resource_desired_states(id),
                observed_state_id uuid NOT NULL REFERENCES resource_observed_states(id),
                fingerprint char(64) NOT NULL,
                status varchar(32) NOT NULL,
                differences jsonb NOT NULL,
                detected_at timestamptz NOT NULL DEFAULT now(),
                resolved_at timestamptz,
                CONSTRAINT resource_drifts_status_check CHECK (status IN ('drifted', 'ignored', 'resolved'))
            );
            CREATE UNIQUE INDEX resource_drift_active_uq
                ON resource_drifts (resource_id, fingerprint) WHERE resolved_at IS NULL;

            CREATE TABLE reconciliation_tasks (
                id uuid PRIMARY KEY,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                resource_id uuid NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
                drift_id uuid NOT NULL REFERENCES resource_drifts(id),
                drift_fingerprint char(64) NOT NULL,
                desired_version bigint NOT NULL,
                policy varchar(32) NOT NULL,
                status varchar(32) NOT NULL DEFAULT 'pending',
                operation_id uuid REFERENCES operations(id),
                attempt_count integer NOT NULL DEFAULT 0,
                not_before timestamptz NOT NULL DEFAULT now(),
                approved_by uuid REFERENCES users(id),
                approved_at timestamptz,
                created_at timestamptz NOT NULL DEFAULT now(),
                completed_at timestamptz,
                CONSTRAINT reconciliation_policy_check CHECK (policy IN ('observe_only', 'manual_approval', 'automatic')),
                CONSTRAINT reconciliation_status_check CHECK (status IN ('pending', 'approved', 'running', 'succeeded', 'failed', 'cancelled'))
            );
            CREATE UNIQUE INDEX reconciliation_active_drift_uq
                ON reconciliation_tasks (resource_id, drift_fingerprint)
                WHERE status IN ('pending', 'approved', 'running');

            CREATE TABLE provider_operation_requests (
                operation_id uuid PRIMARY KEY REFERENCES operations(id) ON DELETE CASCADE,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                provider_account_id uuid NOT NULL REFERENCES provider_accounts(id),
                action varchar(120) NOT NULL,
                resource_type varchar(120) NOT NULL,
                external_id varchar(512),
                parameters jsonb NOT NULL DEFAULT '{}'::jsonb,
                idempotency_key varchar(255) NOT NULL,
                created_at timestamptz NOT NULL DEFAULT now(),
                CONSTRAINT provider_request_idempotency_uq UNIQUE (provider_account_id, idempotency_key)
            );

            CREATE TABLE provider_operation_attempts (
                id uuid PRIMARY KEY,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                operation_id uuid NOT NULL REFERENCES operations(id) ON DELETE CASCADE,
                provider_account_id uuid NOT NULL REFERENCES provider_accounts(id),
                attempt_number integer NOT NULL,
                status varchar(32) NOT NULL DEFAULT 'running',
                lease_owner varchar(160),
                lease_expires_at timestamptz,
                provider_request_id varchar(255),
                masked_request jsonb NOT NULL DEFAULT '{}'::jsonb,
                masked_result jsonb,
                error_category varchar(64),
                error_code varchar(160),
                retryable boolean,
                retry_after timestamptz,
                started_at timestamptz NOT NULL DEFAULT now(),
                completed_at timestamptz,
                CONSTRAINT provider_attempt_number_uq UNIQUE (operation_id, attempt_number),
                CONSTRAINT provider_attempt_status_check CHECK (status IN ('running', 'succeeded', 'failed', 'timed_out'))
            );
            CREATE INDEX provider_attempt_claim_idx ON provider_operation_attempts (organization_id, status, lease_expires_at);

            CREATE TABLE provider_sync_cursors (
                id uuid PRIMARY KEY,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                provider_account_id uuid NOT NULL REFERENCES provider_accounts(id),
                resource_type varchar(120) NOT NULL,
                cursor text,
                status varchar(32) NOT NULL DEFAULT 'idle',
                last_error_code varchar(160),
                last_synced_at timestamptz,
                updated_at timestamptz NOT NULL DEFAULT now(),
                CONSTRAINT provider_sync_cursor_uq UNIQUE (provider_account_id, resource_type),
                CONSTRAINT provider_sync_status_check CHECK (status IN ('idle', 'running', 'failed'))
            );

            CREATE TABLE assets (
                id uuid PRIMARY KEY,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                asset_type varchar(64) NOT NULL,
                name varchar(255) NOT NULL,
                lifecycle varchar(32) NOT NULL DEFAULT 'active',
                tags jsonb NOT NULL DEFAULT '[]'::jsonb,
                metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
                created_at timestamptz NOT NULL DEFAULT now(),
                updated_at timestamptz NOT NULL DEFAULT now()
            );
            CREATE INDEX assets_tenant_type_idx ON assets (organization_id, asset_type, lifecycle);

            CREATE TABLE asset_resources (
                id uuid PRIMARY KEY,
                organization_id uuid NOT NULL REFERENCES organizations(id),
                asset_id uuid NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
                resource_id uuid NOT NULL REFERENCES resources(id) ON DELETE CASCADE,
                purpose varchar(120) NOT NULL,
                created_at timestamptz NOT NULL DEFAULT now(),
                CONSTRAINT asset_resource_uq UNIQUE (asset_id, resource_id, purpose)
            );

            INSERT INTO permissions (id, key, description) VALUES
                ('10000000-0000-7000-8000-000000000014', 'resource.resource.read', 'Read canonical resources and state'),
                ('10000000-0000-7000-8000-000000000015', 'resource.resource.manage', 'Manage resource desired state and lifecycle'),
                ('10000000-0000-7000-8000-000000000016', 'resource.sync.execute', 'Execute provider inventory synchronization'),
                ('10000000-0000-7000-8000-000000000017', 'resource.reconciliation.manage', 'Approve and manage reconciliation tasks');

            INSERT INTO role_permissions (role_id, permission_id, organization_id)
            SELECT role.id, permission.id, role.organization_id FROM roles role
            JOIN permissions permission ON permission.key IN (
                'resource.resource.read', 'resource.resource.manage', 'resource.sync.execute', 'resource.reconciliation.manage'
            ) WHERE role.key IN ('owner', 'admin')
            UNION ALL
            SELECT role.id, permission.id, role.organization_id FROM roles role
            JOIN permissions permission ON permission.key = 'resource.resource.read'
            WHERE role.key IN ('member', 'viewer');

            ALTER TABLE resources ENABLE ROW LEVEL SECURITY; ALTER TABLE resources FORCE ROW LEVEL SECURITY;
            ALTER TABLE external_resource_mappings ENABLE ROW LEVEL SECURITY; ALTER TABLE external_resource_mappings FORCE ROW LEVEL SECURITY;
            ALTER TABLE resource_desired_states ENABLE ROW LEVEL SECURITY; ALTER TABLE resource_desired_states FORCE ROW LEVEL SECURITY;
            ALTER TABLE resource_observed_states ENABLE ROW LEVEL SECURITY; ALTER TABLE resource_observed_states FORCE ROW LEVEL SECURITY;
            ALTER TABLE resource_metadata ENABLE ROW LEVEL SECURITY; ALTER TABLE resource_metadata FORCE ROW LEVEL SECURITY;
            ALTER TABLE resource_drifts ENABLE ROW LEVEL SECURITY; ALTER TABLE resource_drifts FORCE ROW LEVEL SECURITY;
            ALTER TABLE reconciliation_tasks ENABLE ROW LEVEL SECURITY; ALTER TABLE reconciliation_tasks FORCE ROW LEVEL SECURITY;
            ALTER TABLE provider_operation_attempts ENABLE ROW LEVEL SECURITY; ALTER TABLE provider_operation_attempts FORCE ROW LEVEL SECURITY;
            ALTER TABLE provider_operation_requests ENABLE ROW LEVEL SECURITY; ALTER TABLE provider_operation_requests FORCE ROW LEVEL SECURITY;
            ALTER TABLE provider_sync_cursors ENABLE ROW LEVEL SECURITY; ALTER TABLE provider_sync_cursors FORCE ROW LEVEL SECURITY;
            ALTER TABLE assets ENABLE ROW LEVEL SECURITY; ALTER TABLE assets FORCE ROW LEVEL SECURITY;
            ALTER TABLE asset_resources ENABLE ROW LEVEL SECURITY; ALTER TABLE asset_resources FORCE ROW LEVEL SECURITY;

            CREATE POLICY resources_tenant ON resources USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY mappings_tenant ON external_resource_mappings USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY desired_states_tenant ON resource_desired_states USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY observed_states_tenant ON resource_observed_states USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY resource_metadata_tenant ON resource_metadata USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY resource_drifts_tenant ON resource_drifts USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY reconciliation_tenant ON reconciliation_tasks USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY provider_attempts_tenant ON provider_operation_attempts USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY provider_requests_tenant ON provider_operation_requests USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY provider_sync_tenant ON provider_sync_cursors USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY assets_tenant ON assets USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
            CREATE POLICY asset_resources_tenant ON asset_resources USING (organization_id = app.current_organization_id()) WITH CHECK (organization_id = app.current_organization_id());
        ").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r"
            DELETE FROM role_permissions WHERE permission_id IN (
                SELECT id FROM permissions WHERE key LIKE 'resource.%'
            );
            DELETE FROM permissions WHERE key LIKE 'resource.%';
            DROP TABLE IF EXISTS asset_resources;
            DROP TABLE IF EXISTS assets;
            DROP TABLE IF EXISTS provider_sync_cursors;
            DROP TABLE IF EXISTS provider_operation_attempts;
            DROP TABLE IF EXISTS provider_operation_requests;
            DROP TABLE IF EXISTS reconciliation_tasks;
            DROP TABLE IF EXISTS resource_drifts;
            DROP TABLE IF EXISTS resource_metadata;
            DROP TABLE IF EXISTS resource_observed_states;
            DROP TABLE IF EXISTS resource_desired_states;
            DROP TABLE IF EXISTS external_resource_mappings;
            DROP TABLE IF EXISTS resources;
            ALTER TABLE provider_credentials
                DROP CONSTRAINT IF EXISTS provider_credentials_risk_check,
                DROP COLUMN IF EXISTS schema_version,
                DROP COLUMN IF EXISTS masked_identifier,
                DROP COLUMN IF EXISTS risk_level;
            ALTER TABLE operations DROP COLUMN IF EXISTS next_attempt_at;
        ",
            )
            .await?;
        Ok(())
    }
}
