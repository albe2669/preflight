use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_195500_create_tag"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("tag")
                    .if_not_exists()
                    .col(integer("id").primary_key().not_null())
                    .col(
                        string("name")
                            .not_null()
                            .unique_key()
                            .extra("COLLATE NOCASE"),
                    )
                    .check(Expr::cust("name IS trim(name) AND length(name) > 0"))
                    .col(string_null("color"))
                    .check(Expr::cust(
                        "color IS NULL OR color GLOB '#[0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F]'",
                    ))
                    .col(
                        timestamp("created_at")
                            .not_null()
                            .default(Expr::cust("strftime('%Y-%m-%dT%H:%M:%fZ','now')")),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO tag (name) VALUES ('dev'), ('review'), ('ops'), ('admin');",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("tag").to_owned())
            .await
    }
}
