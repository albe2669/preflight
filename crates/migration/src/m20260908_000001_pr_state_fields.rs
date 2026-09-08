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
                        ColumnDef::new(PullRequest::Approved)
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
                        ColumnDef::new(PullRequest::ActionsFailing)
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
                    .drop_column(PullRequest::ActionsFailing)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(PullRequest::Table)
                    .drop_column(PullRequest::Approved)
                    .to_owned(),
            )
            .await
    }
}
