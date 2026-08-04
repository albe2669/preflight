use sea_orm_migration::prelude::*;

use crate::idens::{LinearIssue, Todo, TodoLinearIssue};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(LinearIssue::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(LinearIssue::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    // The API UUID is the sync key...
                    .col(
                        ColumnDef::new(LinearIssue::LinearId)
                            .string_len(64)
                            .not_null(),
                    )
                    // ...and "ENG-123" is the human key.
                    .col(
                        ColumnDef::new(LinearIssue::Identifier)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(LinearIssue::Title).text().not_null())
                    .col(ColumnDef::new(LinearIssue::Description).text().null())
                    .col(ColumnDef::new(LinearIssue::Url).text().not_null())
                    .col(
                        ColumnDef::new(LinearIssue::StateName)
                            .string_len(64)
                            .not_null(),
                    )
                    // Linear's canonical categories. No CHECK -- remote domain.
                    .col(
                        ColumnDef::new(LinearIssue::StateType)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(ColumnDef::new(LinearIssue::Priority).integer().null())
                    .col(ColumnDef::new(LinearIssue::TeamKey).string_len(16).null())
                    .col(
                        ColumnDef::new(LinearIssue::AssigneeName)
                            .string_len(128)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(LinearIssue::AssignedToMe)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(LinearIssue::RemoteCreatedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    // The incremental sync cursor compares against this.
                    .col(
                        ColumnDef::new(LinearIssue::RemoteUpdatedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(LinearIssue::SyncedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(LinearIssue::DismissedAt)
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
                    .name("ux_linear_issue_linear_id")
                    .table(LinearIssue::Table)
                    .col(LinearIssue::LinearId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ux_linear_issue_identifier")
                    .table(LinearIssue::Table)
                    .col(LinearIssue::Identifier)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_linear_issue_inbox")
                    .table(LinearIssue::Table)
                    .col(LinearIssue::DismissedAt)
                    .col(LinearIssue::StateType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TodoLinearIssue::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(TodoLinearIssue::TodoId).integer().not_null())
                    .col(
                        ColumnDef::new(TodoLinearIssue::LinearIssueId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TodoLinearIssue::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(TodoLinearIssue::TodoId)
                            .col(TodoLinearIssue::LinearIssueId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_linear_issue_todo")
                            .from(TodoLinearIssue::Table, TodoLinearIssue::TodoId)
                            .to(Todo::Table, Todo::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_linear_issue_issue")
                            .from(TodoLinearIssue::Table, TodoLinearIssue::LinearIssueId)
                            .to(LinearIssue::Table, LinearIssue::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // An issue converts to exactly one todo. Drop the `.unique()` if you
        // want separate "implement ENG-123" and "review ENG-123" todos.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ux_todo_linear_issue_issue")
                    .table(TodoLinearIssue::Table)
                    .col(TodoLinearIssue::LinearIssueId)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TodoLinearIssue::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(LinearIssue::Table).to_owned())
            .await
    }
}
