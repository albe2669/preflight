use sea_orm_migration::prelude::*;

use crate::idens::{Tag, Todo, TodoTag};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Tag::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Tag::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    // `slug` is the identity ("dev", "review"); `name` is the
                    // display form. Lower-casing into the slug is what makes
                    // tagging case-insensitive without a COLLATE clause.
                    .col(ColumnDef::new(Tag::Slug).string_len(64).not_null())
                    .col(ColumnDef::new(Tag::Name).text().not_null())
                    .col(ColumnDef::new(Tag::Color).string_len(16).null())
                    .col(
                        ColumnDef::new(Tag::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ux_tag_slug")
                    .table(Tag::Table)
                    .col(Tag::Slug)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TodoTag::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(TodoTag::TodoId).integer().not_null())
                    .col(ColumnDef::new(TodoTag::TagId).integer().not_null())
                    .col(
                        ColumnDef::new(TodoTag::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(Index::create().col(TodoTag::TodoId).col(TodoTag::TagId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_tag_todo")
                            .from(TodoTag::Table, TodoTag::TodoId)
                            .to(Todo::Table, Todo::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_tag_tag")
                            .from(TodoTag::Table, TodoTag::TagId)
                            .to(Tag::Table, Tag::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // The composite PK already indexes (todo_id, tag_id); this covers the
        // other direction, i.e. "every todo tagged `review`".
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_todo_tag_tag")
                    .table(TodoTag::Table)
                    .col(TodoTag::TagId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TodoTag::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Tag::Table).to_owned())
            .await
    }
}
