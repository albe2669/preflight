use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_195900_create_todo_pull_request"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("todo_pull_request")
                    .if_not_exists()
                    .col(integer("todo_id").not_null())
                    .col(integer("pull_request_id").not_null())
                    .col(
                        enumeration("relation", "relation", vec!["implements", "reviews"])
                            .not_null(),
                    )
                    .col(
                        timestamp("linked_at")
                            .not_null()
                            .default(Expr::cust("strftime('%Y-%m-%dT%H:%M:%fZ','now')")),
                    )
                    .primary_key(Index::create().col("todo_id").col("pull_request_id"))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_pull_request_todo")
                            .from("todo_pull_request", "todo_id")
                            .to("todo", "id")
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_pull_request_pull_request")
                            .from("todo_pull_request", "pull_request_id")
                            .to("pull_request", "id")
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tpr_by_pr")
                    .table("todo_pull_request")
                    .col("pull_request_id")
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_tpr_one_review_target")
                    .table("todo_pull_request")
                    .col("todo_id")
                    .unique()
                    .and_where(Expr::col("relation").eq("reviews"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("todo_pull_request").to_owned())
            .await
    }
}
