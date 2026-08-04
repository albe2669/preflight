use sea_orm_migration::prelude::*;

use crate::idens::{PullRequest, Todo, TodoPullRequest};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PullRequest::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PullRequest::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PullRequest::Provider)
                            .string_len(16)
                            .not_null()
                            .default("github"),
                    )
                    .col(
                        ColumnDef::new(PullRequest::Owner)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(ColumnDef::new(PullRequest::Repo).string_len(128).not_null())
                    .col(ColumnDef::new(PullRequest::Number).integer().not_null())
                    .col(ColumnDef::new(PullRequest::Title).text().not_null())
                    .col(ColumnDef::new(PullRequest::Url).text().not_null())
                    .col(ColumnDef::new(PullRequest::Author).string_len(128).null())
                    // No CHECK: this mirrors a remote system, and remote systems
                    // add states without asking your local database first.
                    .col(ColumnDef::new(PullRequest::State).string_len(16).not_null())
                    .col(
                        ColumnDef::new(PullRequest::ReviewRequested)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(PullRequest::AuthoredByMe)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(PullRequest::RemoteCreatedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(PullRequest::RemoteUpdatedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(PullRequest::SyncedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    // Clear the inbox without creating a todo.
                    .col(
                        ColumnDef::new(PullRequest::DismissedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Makes ingest an idempotent upsert -- this is the ON CONFLICT target.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ux_pull_request_identity")
                    .table(PullRequest::Table)
                    .col(PullRequest::Provider)
                    .col(PullRequest::Owner)
                    .col(PullRequest::Repo)
                    .col(PullRequest::Number)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // The review inbox: open, not dismissed, review requested.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_pull_request_inbox")
                    .table(PullRequest::Table)
                    .col(PullRequest::DismissedAt)
                    .col(PullRequest::State)
                    .col(PullRequest::ReviewRequested)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TodoPullRequest::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(TodoPullRequest::TodoId).integer().not_null())
                    .col(
                        ColumnDef::new(TodoPullRequest::PullRequestId)
                            .integer()
                            .not_null(),
                    )
                    // Closed, stable domain -> CHECK is safe here.
                    .col(
                        ColumnDef::new(TodoPullRequest::Relation)
                            .string_len(16)
                            .not_null()
                            .default("reviews")
                            .check(Expr::col(TodoPullRequest::Relation).is_in([
                                "reviews",
                                "implements",
                                "references",
                            ])),
                    )
                    .col(
                        ColumnDef::new(TodoPullRequest::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(TodoPullRequest::TodoId)
                            .col(TodoPullRequest::PullRequestId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_pull_request_todo")
                            .from(TodoPullRequest::Table, TodoPullRequest::TodoId)
                            .to(Todo::Table, Todo::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_pull_request_pr")
                            .from(TodoPullRequest::Table, TodoPullRequest::PullRequestId)
                            .to(PullRequest::Table, PullRequest::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_todo_pull_request_pr")
                    .table(TodoPullRequest::Table)
                    .col(TodoPullRequest::PullRequestId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TodoPullRequest::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(PullRequest::Table).to_owned())
            .await
    }
}
