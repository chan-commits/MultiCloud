use sea_orm::entity::prelude::*;
use serde_json::Value;
use time::OffsetDateTime;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "resource_drifts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub organization_id: Uuid,
    pub resource_id: Uuid,
    pub desired_state_id: Uuid,
    pub observed_state_id: Uuid,
    pub fingerprint: String,
    pub status: String,
    pub differences: Value,
    pub detected_at: OffsetDateTime,
    pub resolved_at: Option<OffsetDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
