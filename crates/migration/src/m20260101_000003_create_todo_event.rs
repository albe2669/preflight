use sea_orm_migration::prelude::*;

use crate::idens::{Todo, TodoEvent};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TodoEvent::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TodoEvent::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TodoEvent::TodoId).integer().not_null())
                    // Deliberately NOT constrained by a CHECK. SQLite cannot
                    // drop or alter a CHECK constraint, so anything you expect
                    // to grow must not have one -- and event kinds are the
                    // single most likely thing in this schema to grow. The
                    // EventKind ActiveEnum in the entity crate is the guard.
                    .col(ColumnDef::new(TodoEvent::Kind).string_len(32).not_null())
                    .col(ColumnDef::new(TodoEvent::Field).string_len(32).null())
                    .col(ColumnDef::new(TodoEvent::OldValue).text().null())
                    .col(ColumnDef::new(TodoEvent::NewValue).text().null())
                    .col(ColumnDef::new(TodoEvent::Payload).json().null())
                    // This one IS closed and stable, and it is load-bearing:
                    // it keeps a background Linear sync from making an
                    // untouched todo look like it was worked on.
                    .col(
                        ColumnDef::new(TodoEvent::Actor)
                            .string_len(16)
                            .not_null()
                            .default("user")
                            .check(Expr::col(TodoEvent::Actor).is_in(["user", "sync", "system"])),
                    )
                    .col(
                        ColumnDef::new(TodoEvent::OccurredAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    // Denormalized from occurred_at via Clock::logical_date so
                    // "what did I do yesterday" is an index scan and never a
                    // timezone calculation in SQL.
                    .col(ColumnDef::new(TodoEvent::LogicalDate).date().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_event_todo")
                            .from(TodoEvent::Table, TodoEvent::TodoId)
                            .to(Todo::Table, Todo::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Timeline for one todo.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_todo_event_todo_time")
                    .table(TodoEvent::Table)
                    .col(TodoEvent::TodoId)
                    .col(TodoEvent::OccurredAt)
                    .to_owned(),
            )
            .await?;

        // "Everything I touched on date D." Actor is second so the index also
        // serves a plain date-range scan.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_todo_event_day")
                    .table(TodoEvent::Table)
                    .col(TodoEvent::LogicalDate)
                    .col(TodoEvent::Actor)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TodoEvent::Table).to_owned())
            .await
    }
}
