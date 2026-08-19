pub use sea_orm_migration::prelude::*;

mod m20260819_000001_identity_organization;
mod m20260819_000002_rbac;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260819_000001_identity_organization::Migration),
            Box::new(m20260819_000002_rbac::Migration),
        ]
    }
}
