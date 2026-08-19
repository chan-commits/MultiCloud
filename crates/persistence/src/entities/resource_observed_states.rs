use sea_orm::entity::prelude::*;
use serde_json::Value;
use time::OffsetDateTime;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "resource_observed_states")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub organization_id: Uuid,
    pub resource_id: Uuid,
    pub mapping_id: Uuid,
    pub version: i64,
    pub state: Value,
    pub state_hash: String,
    pub is_latest: bool,
    pub observed_at: OffsetDateTime,
    pub created_at: OffsetDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
