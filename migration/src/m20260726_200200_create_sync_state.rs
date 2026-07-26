use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_200200_create_sync_state"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("sync_state")
                    .if_not_exists()
                    .col(
                        enumeration("provider", "provider", vec!["linear", "github"])
                            .primary_key()
                            .not_null(),
                    )
                    .col(string_null("cursor"))
                    .col(timestamp_null("last_started_at"))
                    .col(timestamp_null("last_success_at"))
                    .col(string_null("last_error"))
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared("INSERT INTO sync_state (provider) VALUES ('linear');")
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("sync_state").to_owned())
            .await
    }
}
