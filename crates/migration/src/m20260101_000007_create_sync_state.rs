use sea_orm_migration::prelude::*;

use crate::idens::SyncState;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SyncState::Table)
                    .if_not_exists()
                    // Natural PK: "linear", "github". One row per source.
                    // Not seeded here -- migrations create schema, the sync
                    // code upserts its own row on first run.
                    .col(
                        ColumnDef::new(SyncState::Source)
                            .string_len(16)
                            .not_null()
                            .primary_key(),
                    )
                    // Linear: an updatedAt watermark. GitHub: an ETag or
                    // search cursor. Opaque to everything but its own client.
                    .col(ColumnDef::new(SyncState::Cursor).text().null())
                    .col(
                        ColumnDef::new(SyncState::LastSyncedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(SyncState::LastStatus)
                            .string_len(16)
                            .not_null()
                            .default("never")
                            .check(
                                Expr::col(SyncState::LastStatus)
                                    .is_in(["never", "ok", "error"]),
                            ),
                    )
                    .col(ColumnDef::new(SyncState::LastError).text().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SyncState::Table).to_owned())
            .await
    }
}
