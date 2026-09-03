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
                ALTER TABLE platform_settings
                    ADD COLUMN log_level text NOT NULL DEFAULT 'info',
                    ADD CONSTRAINT platform_settings_log_level_check
                        CHECK (log_level IN ('error', 'warn', 'info', 'debug', 'trace'));
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
                ALTER TABLE platform_settings
                    DROP CONSTRAINT IF EXISTS platform_settings_log_level_check,
                    DROP COLUMN IF EXISTS log_level;
                ",
            )
            .await?;
        Ok(())
    }
}
