use sea_orm_migration::{
    prelude::*,
    schema::*,
    sea_orm::{Statement, TransactionTrait},
};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_195400_create_todo_status_transition"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("todo_status_transition")
                    .if_not_exists()
                    .col(string("from_status").not_null())
                    .col(string("to_status").not_null())
                    .primary_key(Index::create().col("from_status").col("to_status"))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_status_transition_from")
                            .from("todo_status_transition", "from_status")
                            .to("todo_status", "code")
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_todo_status_transition_to")
                            .from("todo_status_transition", "to_status")
                            .to("todo_status", "code")
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        let transitions = vec![
            ("todo", "started"),
            ("todo", "blocked"),
            ("todo", "cancelled"),
            ("started", "blocked"),
            ("started", "done"),
            ("started", "completed"),
            ("started", "cancelled"),
            ("started", "todo"),
            ("blocked", "started"),
            ("blocked", "todo"),
            ("blocked", "cancelled"),
            ("done", "completed"),
            ("done", "started"),
            ("done", "cancelled"),
            ("completed", "started"),
            ("cancelled", "todo"),
        ];

        let db = manager.get_connection();
        let ts = db.begin().await?;
        for (from_status, to_status) in transitions {
            ts.execute_raw(Statement::from_sql_and_values(
                manager.get_database_backend(),
                r#"
                    INSERT INTO todo_status_transition (from_status, to_status)
                    VALUES (?, ?)
                "#,
                [from_status.into(), to_status.into()],
            ))
            .await?;
        }
        ts.commit().await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("todo_status_transition").to_owned())
            .await
    }
}
