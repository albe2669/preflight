use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_195600_create_todo_tag"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("todo_tag")
                    .if_not_exists()
                    .col(integer("todo_id").not_null())
                    .col(integer("tag_id").not_null())
                    .col(
                        timestamp("added_at")
                            .not_null()
                            .default(Expr::cust("strftime('%Y-%m-%dT%H:%M:%fZ','now')")),
                    )
                    .primary_key(Index::create().col("todo_id").col("tag_id"))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_tag_todo")
                            .from("todo_tag", "todo_id")
                            .to("todo", "id")
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_tag_tag")
                            .from("todo_tag", "tag_id")
                            .to("tag", "id")
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_todo_tag_by_tag")
                    .table("todo_tag")
                    .col("tag_id")
                    .col("todo_id")
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("todo_tag").to_owned())
            .await
    }
}
