use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_200000_create_linear_issue"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("linear_issue")
                    .if_not_exists()
                    .col(string("id").primary_key().not_null())
                    .col(string("identifier").not_null().unique_key())
                    .col(integer_null("todo_id").unique_key())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_linear_issue_todo")
                            .from("linear_issue", "todo_id")
                            .to("todo", "id")
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .col(string("title").not_null())
                    .col(string("description").not_null().default(""))
                    .col(string("state_name").not_null())
                    .col(
                        enumeration(
                            "state_type",
                            "state_type",
                            vec!["triage", "backlog", "unstarted", "started", "canceled"],
                        )
                        .not_null(),
                    )
                    .col(integer_null("priority"))
                    .col(string_null("assignee_id"))
                    .col(string("url").not_null())
                    .col(timestamp("remote_created_at").not_null())
                    .col(timestamp("remote_updated_at").not_null())
                    .col(timestamp_null("archived_at"))
                    .col(timestamp_null("unassigned_at"))
                    .col(text_null("raw_payload"))
                    .check(Expr::cust("raw_payload IS NULL OR json_valid(raw_payload)"))
                    .col(timestamp("synced_at").not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_linear_remote_updated")
                    .table("linear_issue")
                    .col("remote_updated_at")
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_linear_assignee")
                    .table("linear_issue")
                    .col("assignee_id")
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("linear_issue").to_owned())
            .await
    }
}
