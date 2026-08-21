use crate::entity::enums::TodoStatus;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "todo")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(column_type = "Text")]
    pub title: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    pub status: TodoStatus,
    #[sea_orm(column_type = "Text", nullable)]
    pub blocked_reason: Option<String>,
    pub sort_key: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub started_at: Option<DateTimeWithTimeZone>,
    pub closed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::todo_day_plan::Entity")]
    TodoDayPlan,
    #[sea_orm(has_many = "super::todo_event::Entity")]
    TodoEvent,
    #[sea_orm(has_many = "super::todo_tag::Entity")]
    TodoTag,
}

impl Related<super::todo_day_plan::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TodoDayPlan.def()
    }
}

impl Related<super::todo_event::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TodoEvent.def()
    }
}

impl Related<super::todo_tag::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TodoTag.def()
    }
}

impl Related<super::tag::Entity> for Entity {
    fn to() -> RelationDef {
        super::todo_tag::Relation::Tag.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::todo_tag::Relation::Todo.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelatedEntity)]
pub enum RelatedEntity {
    #[sea_orm(entity = "super::todo_day_plan::Entity")]
    TodoDayPlan,
    #[sea_orm(entity = "super::todo_event::Entity")]
    TodoEvent,
    #[sea_orm(entity = "super::todo_tag::Entity")]
    TodoTag,
    #[sea_orm(entity = "super::tag::Entity")]
    Tag,
}
