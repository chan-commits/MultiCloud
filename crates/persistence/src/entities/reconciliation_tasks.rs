use sea_orm::entity::prelude::*;
use time::OffsetDateTime;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "reconciliation_tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub organization_id: Uuid,
    pub resource_id: Uuid,
    pub drift_id: Uuid,
    pub drift_fingerprint: String,
    pub desired_version: i64,
    pub policy: String,
    pub status: String,
    pub operation_id: Option<Uuid>,
    pub attempt_count: i32,
    pub not_before: OffsetDateTime,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
