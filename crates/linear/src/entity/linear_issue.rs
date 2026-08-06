use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "linear_issue")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub linear_id: String,
    #[sea_orm(unique)]
    pub identifier: String,
    #[sea_orm(column_type = "Text")]
    pub title: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub url: String,
    pub state_name: String,
    pub state_type: String,
    pub priority: Option<i64>,
    pub team_key: Option<String>,
    pub assignee_name: Option<String>,
    pub assigned_to_me: bool,
    pub remote_created_at: Option<DateTimeWithTimeZone>,
    pub remote_updated_at: Option<DateTimeWithTimeZone>,
    pub synced_at: DateTimeWithTimeZone,
    pub dismissed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelatedEntity)]
pub enum RelatedEntity {}
