use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_200300_create_todo_event"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("todo_event")
                    .if_not_exists()
                    // AUTOINCREMENT is deliberate here: several clients resume from
                    // `since: <last id seen>`, so ids must never be reused after a
                    // cascade delete, unlike the plain INTEGER PRIMARY KEY elsewhere.
                    .col(pk_auto("id").not_null())
                    .col(integer("todo_id").not_null())
                    .col(
                        timestamp("occurred_at")
                            .not_null()
                            .default(Expr::cust("strftime('%Y-%m-%dT%H:%M:%fZ','now')")),
                    )
                    .col(date("occurred_on").not_null())
                    .check(Expr::cust("occurred_on IS date(occurred_on)"))
                    .col(
                        enumeration(
                            "actor",
                            "actor",
                            vec!["user", "system", "linear_sync", "github_sync"],
                        )
                        .not_null()
                        .default("user"),
                    )
                    .col(string("event_type").not_null())
                    .col(string_null("field"))
                    .col(string_null("old_value"))
                    .col(string_null("new_value"))
                    .col(text_null("payload"))
                    .check(Expr::cust("payload IS NULL OR json_valid(payload)"))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_event_todo")
                            .from("todo_event", "todo_id")
                            .to("todo", "id")
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_event_event_type")
                            .from("todo_event", "event_type")
                            .to("todo_event_type", "code")
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_event_by_todo")
                    .table("todo_event")
                    .col("todo_id")
                    .col(("occurred_at", IndexOrder::Desc))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_event_by_day")
                    .table("todo_event")
                    .col("occurred_on")
                    .col("todo_id")
                    .to_owned(),
            )
            .await?;

        let db = manager.get_connection();

        db.execute_unprepared(
            r#"
                CREATE TRIGGER trg_todo_event_no_update BEFORE UPDATE ON todo_event
                BEGIN
                    SELECT RAISE(ABORT, 'todo_event is append-only');
                END;
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
                CREATE TRIGGER trg_todo_event_no_delete BEFORE DELETE ON todo_event
                WHEN (SELECT 1 FROM todo WHERE id = OLD.todo_id) IS NOT NULL
                BEGIN
                    SELECT RAISE(ABORT, 'todo_event is append-only; archive the todo instead');
                END;
            "#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP TRIGGER IF EXISTS trg_todo_event_no_delete;")
            .await?;
        db.execute_unprepared("DROP TRIGGER IF EXISTS trg_todo_event_no_update;")
            .await?;
        manager
            .drop_table(Table::drop().table("todo_event").to_owned())
            .await
    }
}
