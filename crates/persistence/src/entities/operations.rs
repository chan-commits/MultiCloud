use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub organization_id: Uuid,
    pub operation_type: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub requested_by: Uuid,
    pub idempotency_key: String,
    pub status: String,
    pub progress: i16,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub started_at: Option<TimeDateTimeWithTimeZone>,
    pub completed_at: Option<TimeDateTimeWithTimeZone>,
    pub created_at: TimeDateTimeWithTimeZone,
    pub updated_at: TimeDateTimeWithTimeZone,
    pub next_attempt_at: TimeDateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
