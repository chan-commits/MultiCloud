use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "outbox_events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub organization_id: Uuid,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub event_version: i16,
    pub payload: Json,
    pub trace_id: Option<String>,
    pub occurred_at: TimeDateTimeWithTimeZone,
    pub published_at: Option<TimeDateTimeWithTimeZone>,
    pub attempt_count: i32,
    pub next_attempt_at: TimeDateTimeWithTimeZone,
    pub last_error: Option<String>,
    pub dead_lettered_at: Option<TimeDateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
