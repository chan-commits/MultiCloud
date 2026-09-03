use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "tickets")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub organization_id: Uuid,
    pub number: i64,
    pub subject: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub requester_id: Uuid,
    pub assigned_to: Option<Uuid>,
    pub sla_policy_id: Option<Uuid>,
    pub response_due_at: Option<TimeDateTimeWithTimeZone>,
    pub resolution_due_at: Option<TimeDateTimeWithTimeZone>,
    pub first_responded_at: Option<TimeDateTimeWithTimeZone>,
    pub resolved_at: Option<TimeDateTimeWithTimeZone>,
    pub version: i32,
    pub created_at: TimeDateTimeWithTimeZone,
    pub updated_at: TimeDateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
