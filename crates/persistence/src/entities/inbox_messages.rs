use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "inbox_messages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub organization_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub consumer: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub message_id: Uuid,
    pub processed_at: TimeDateTimeWithTimeZone,
    pub result: Option<Json>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
