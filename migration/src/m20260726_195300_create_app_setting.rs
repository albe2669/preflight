use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_195300_create_app_setting"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("app_setting")
                    .if_not_exists()
                    .col(integer("id").primary_key().not_null())
                    .check(Expr::cust("id = 1"))
                    .col(integer("day_start_offset_minutes").not_null().default(240))
                    .check(Expr::cust("day_start_offset_minutes BETWEEN 0 AND 719"))
                    .col(string("timezone").not_null().default("system"))
                    .col(date_null("last_rollover_date"))
                    .check(Expr::cust("last_rollover_date IS date(last_rollover_date)"))
                    .col(
                        timestamp("updated_at")
                            .not_null()
                            .default(Expr::cust("strftime('%Y-%m-%dT%H:%M:%fZ','now')")),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared("INSERT INTO app_setting (id) VALUES (1);")
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("app_setting").to_owned())
            .await
    }
}
