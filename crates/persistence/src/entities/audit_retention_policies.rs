use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "audit_retention_policies")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub organization_id: Uuid,
    pub retention_days: i32,
    pub export_retention_days: i32,
    pub updated_by: Option<Uuid>,
    pub updated_at: TimeDateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
