
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
#[sea_orm(table_name = "todo_tag")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub todo_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub tag_id: i64,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::tag::Entity",
        from = "Column::TagId",
        to = "super::tag::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Tag,
    #[sea_orm(
        belongs_to = "super::todo::Entity",
        from = "Column::TodoId",
        to = "super::todo::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Todo,
}

impl Related<super::tag::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tag.def()
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
    #[sea_orm(entity = "super::tag::Entity")]
    Tag,
    #[sea_orm(entity = "super::todo::Entity")]
    Todo,
}
