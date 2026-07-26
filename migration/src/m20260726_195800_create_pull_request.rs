use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_195800_create_pull_request"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("pull_request")
                    .if_not_exists()
                    .col(integer("id").primary_key().not_null())
                    .col(
                        enumeration(
                            "provider",
                            "provider",
                            vec!["github", "gitlab", "bitbucket"],
                        )
                        .not_null()
                        .default("github"),
                    )
                    .col(string("repository").not_null())
                    .col(integer("number").not_null())
                    .check(Expr::cust("number > 0"))
                    .col(string("url").not_null())
                    .col(string("title").not_null().default(""))
                    .col(string_null("author"))
                    .col(
                        enumeration("state", "state", vec!["open", "draft", "merged", "closed"])
                            .not_null()
                            .default("open"),
                    )
                    .col(timestamp_null("remote_updated_at"))
                    .col(timestamp_null("fetched_at"))
                    .col(timestamp_null("state_changed_at"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_pull_request_natural_key")
                    .table("pull_request")
                    .col("provider")
                    .col("repository")
                    .col("number")
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("pull_request").to_owned())
            .await
    }
}
