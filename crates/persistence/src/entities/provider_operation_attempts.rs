use sea_orm::entity::prelude::*;
use serde_json::Value;
use time::OffsetDateTime;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "provider_operation_attempts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub organization_id: Uuid,
    pub operation_id: Uuid,
    pub provider_account_id: Uuid,
    pub attempt_number: i32,
    pub status: String,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<OffsetDateTime>,
    pub provider_request_id: Option<String>,
    pub masked_request: Value,
    pub masked_result: Option<Value>,
    pub error_category: Option<String>,
    pub error_code: Option<String>,
    pub retryable: Option<bool>,
    pub retry_after: Option<OffsetDateTime>,
    pub started_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
