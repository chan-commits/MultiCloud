use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub user_id: Uuid,
    pub organization_id: Option<Uuid>,
    pub refresh_token_hash: Vec<u8>,
    pub expires_at: TimeDateTimeWithTimeZone,
    pub revoked_at: Option<TimeDateTimeWithTimeZone>,
    pub ip_address: Option<IpNetwork>,
    pub user_agent: Option<String>,
    pub created_at: TimeDateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
