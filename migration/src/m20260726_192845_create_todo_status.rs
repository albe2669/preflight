use sea_orm_migration::{
    prelude::*,
    schema::*,
    sea_orm::{Statement, TransactionTrait},
};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_192845_create_todo_status"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("todo_status")
                    .if_not_exists()
                    .col(string("code").primary_key().not_null())
                    .col(string("label").not_null())
                    .col(boolean("is_open").not_null())
                    .col(boolean("is_terminal").not_null())
                    .col(integer("sort_order").unsigned().not_null())
                    .to_owned(),
            )
            .await?;

        let states = vec![
            ("todo", "To do", true, false, 1),
            ("started", "Started", true, false, 2),
            ("blocked", "Blocked", true, false, 3),
            ("done", "Done", false, false, 4),
            ("cancelled", "Cancelled", false, true, 5),
        ];

        let db = manager.get_connection();
        let ts = db.begin().await?;
        for (code, label, is_open, is_terminal, sort_order) in states {
            ts.execute_raw(Statement::from_sql_and_values(
                manager.get_database_backend(),
                r#"
                    INSERT INTO todo_status (code, label, is_open, is_terminal, sort_order)
                    VALUES (?, ?, ?, ?, ?)
                "#,
                [
                    code.into(),
                    label.into(),
                    is_open.into(),
                    is_terminal.into(),
                    sort_order.into(),
                ],
            ))
            .await?;
        }
        ts.commit().await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("todo_status").to_owned())
            .await
    }
}
