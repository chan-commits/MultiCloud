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
                ALTER TABLE users
                    ADD COLUMN is_platform_admin boolean NOT NULL DEFAULT false;

                UPDATE users SET is_platform_admin = true
                WHERE id = COALESCE(
                    (
                        SELECT binding.subject_id
                        FROM role_bindings AS binding
                        JOIN roles AS role ON role.id = binding.role_id
                        JOIN organizations AS organization ON organization.id = binding.organization_id
                        WHERE binding.subject_type = 'user' AND role.key = 'owner'
                        ORDER BY organization.created_at, binding.created_at, binding.id
                        LIMIT 1
                    ),
                    (SELECT id FROM users ORDER BY created_at, id LIMIT 1)
                );

                CREATE TABLE platform_settings (
                    id smallint PRIMARY KEY DEFAULT 1,
                    registration_enabled boolean NOT NULL DEFAULT false,
                    updated_by uuid REFERENCES users(id),
                    updated_at timestamptz NOT NULL DEFAULT now(),
                    CONSTRAINT platform_settings_singleton_check CHECK (id = 1)
                );

                INSERT INTO platform_settings (id, registration_enabled) VALUES (1, false);
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
                DROP TABLE IF EXISTS platform_settings;
                ALTER TABLE users DROP COLUMN IF EXISTS is_platform_admin;
                ",
            )
            .await?;
        Ok(())
    }
}
