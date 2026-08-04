use crate::sea_orm_active_enums::PullRequestState;
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
#[sea_orm(table_name = "pull_request")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique_key = "ux_pull_request_identity")]
    pub provider: String,
    #[sea_orm(unique_key = "ux_pull_request_identity")]
    pub owner: String,
    #[sea_orm(unique_key = "ux_pull_request_identity")]
    pub repo: String,
    #[sea_orm(unique_key = "ux_pull_request_identity")]
    pub number: i64,
    #[sea_orm(column_type = "Text")]
    pub title: String,
    #[sea_orm(column_type = "Text")]
    pub url: String,
    pub author: Option<String>,
    pub state: PullRequestState,
    pub review_requested: bool,
    pub authored_by_me: bool,
    pub remote_created_at: Option<DateTimeWithTimeZone>,
    pub remote_updated_at: Option<DateTimeWithTimeZone>,
    pub synced_at: DateTimeWithTimeZone,
    pub dismissed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::todo_pull_request::Entity")]
    TodoPullRequest,
}

impl Related<super::todo_pull_request::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TodoPullRequest.def()
    }
}

impl Related<super::todo::Entity> for Entity {
    fn to() -> RelationDef {
        super::todo_pull_request::Relation::Todo.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::todo_pull_request::Relation::PullRequest.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelatedEntity)]
pub enum RelatedEntity {
    #[sea_orm(entity = "super::todo_pull_request::Entity")]
    TodoPullRequest,
    #[sea_orm(entity = "super::todo::Entity")]
    Todo,
}
