use sea_orm_migration::prelude::*;

use crate::idens::PullRequest;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PullRequest::Table)
                    .add_column(
                        ColumnDef::new(PullRequest::ChangesRequested)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(PullRequest::Table)
                    .add_column(
                        ColumnDef::new(PullRequest::CopilotComments)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(PullRequest::Table)
                    .add_column(
                        ColumnDef::new(PullRequest::MergeConflicts)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PullRequest::Table)
                    .drop_column(PullRequest::MergeConflicts)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(PullRequest::Table)
                    .drop_column(PullRequest::CopilotComments)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(PullRequest::Table)
                    .drop_column(PullRequest::ChangesRequested)
                    .to_owned(),
            )
            .await
    }
}
