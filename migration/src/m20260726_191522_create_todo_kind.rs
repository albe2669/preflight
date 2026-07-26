use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_191522_create_todo_kind"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("todo_kind")
                    .if_not_exists()
                    .col(string("code").primary_key().not_null())
                    .col(string("label").not_null())
                    .col(integer("sort_order").unsigned().not_null())
                    .to_owned(),
            )
            .await?;

        let db = manager.get_connection();
        db.execute_unprepared(
            r#"
                INSERT INTO todo_kind (code, label, sort_order) VALUES
                    ('task',      'Task',                1),
                    ('pr_review', 'Pull request review', 2);
            "#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("todo_kind").to_owned())
            .await
    }
}
