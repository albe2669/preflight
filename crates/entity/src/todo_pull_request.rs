use crate::sea_orm_active_enums::LinkRelation;
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
#[sea_orm(table_name = "todo_pull_request")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub todo_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub pull_request_id: i64,
    pub relation: LinkRelation,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::pull_request::Entity",
        from = "Column::PullRequestId",
        to = "super::pull_request::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    PullRequest,
    #[sea_orm(
        belongs_to = "super::todo::Entity",
        from = "Column::TodoId",
        to = "super::todo::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Todo,
}

impl Related<super::pull_request::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PullRequest.def()
    }
}

impl Related<super::todo::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Todo.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelatedEntity)]
pub enum RelatedEntity {
    #[sea_orm(entity = "super::pull_request::Entity")]
    PullRequest,
    #[sea_orm(entity = "super::todo::Entity")]
    Todo,
}
