
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    DeriveEntityModel,
    Serialize,
    Deserialize,
)]
#[sea_orm(table_name = "sync_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub source: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub cursor: Option<String>,
    pub last_synced_at: Option<DateTimeWithTimeZone>,
    pub last_status: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub last_error: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelatedEntity)]
pub enum RelatedEntity {}
