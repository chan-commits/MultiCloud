pub use sea_orm_migration::prelude::*;

mod m20260819_000001_identity_organization;
mod m20260819_000002_rbac;
mod m20260819_000003_operations_events;
mod m20260819_000004_provider_foundation;
mod m20260819_000005_resources_operations;
mod m20260819_000006_audit_logs;
mod m20260820_000007_platform_registration;
mod m20260903_000008_runtime_log_level;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260819_000001_identity_organization::Migration),
            Box::new(m20260819_000002_rbac::Migration),
            Box::new(m20260819_000003_operations_events::Migration),
            Box::new(m20260819_000004_provider_foundation::Migration),
            Box::new(m20260819_000005_resources_operations::Migration),
            Box::new(m20260819_000006_audit_logs::Migration),
            Box::new(m20260820_000007_platform_registration::Migration),
            Box::new(m20260903_000008_runtime_log_level::Migration),
        ]
    }
}
