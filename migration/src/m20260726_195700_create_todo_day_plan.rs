use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_195700_create_todo_day_plan"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("todo_day_plan")
                    .if_not_exists()
                    .col(date("plan_date").not_null())
                    .check(Expr::cust("plan_date IS date(plan_date)"))
                    .col(integer("todo_id").not_null())
                    .col(
                        timestamp("added_at")
                            .not_null()
                            .default(Expr::cust("strftime('%Y-%m-%dT%H:%M:%fZ','now')")),
                    )
                    .col(timestamp_null("removed_at"))
                    .col(
                        enumeration(
                            "origin",
                            "origin",
                            vec!["manual", "carry_over", "linear_sync"],
                        )
                        .not_null()
                        .default("manual"),
                    )
                    .col(timestamp_null("day_start_utc"))
                    .col(string_null("status_at_plan_time"))
                    .primary_key(Index::create().col("plan_date").col("todo_id"))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_day_plan_todo")
                            .from("todo_day_plan", "todo_id")
                            .to("todo", "id")
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_day_plan_status")
                            .from("todo_day_plan", "status_at_plan_time")
                            .to("todo_status", "code")
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_day_plan_by_todo")
                    .table("todo_day_plan")
                    .col("todo_id")
                    .col(("plan_date", IndexOrder::Desc))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("todo_day_plan").to_owned())
            .await
    }
}
