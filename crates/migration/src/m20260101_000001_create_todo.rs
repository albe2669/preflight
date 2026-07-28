use sea_orm_migration::prelude::*;

use crate::idens::Todo;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Todo::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Todo::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Todo::Title).text().not_null())
                    .col(ColumnDef::new(Todo::Description).text().null())
                    // Closed, stable domain -> worth a CHECK. See the note in
                    // m20260101_000003 for why `todo_event.kind` does not get one.
                    .col(
                        ColumnDef::new(Todo::Status)
                            .string_len(16)
                            .not_null()
                            .default("todo")
                            .check(Expr::col(Todo::Status).is_in([
                                "todo",
                                "started",
                                "blocked",
                                "done",
                                "cancelled",
                            ])),
                    )
                    .col(ColumnDef::new(Todo::BlockedReason).text().null())
                    .col(
                        ColumnDef::new(Todo::SortKey)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    // No CURRENT_TIMESTAMP defaults anywhere in this schema:
                    // SQLite renders it as "YYYY-MM-DD HH:MM:SS" with no offset,
                    // which will not parse back into DateTimeWithTimeZone.
                    // The application sets every timestamp explicitly.
                    .col(
                        ColumnDef::new(Todo::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Todo::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Todo::StartedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Todo::ClosedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_todo_status_sort")
                    .table(Todo::Table)
                    .col(Todo::Status)
                    .col(Todo::SortKey)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_todo_closed_at")
                    .table(Todo::Table)
                    .col(Todo::ClosedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite drops a table's indexes with the table.
        manager
            .drop_table(Table::drop().table(Todo::Table).to_owned())
            .await
    }
}
