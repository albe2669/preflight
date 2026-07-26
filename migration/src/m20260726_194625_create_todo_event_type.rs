use sea_orm_migration::{
    prelude::*,
    schema::*,
    sea_orm::{Statement, TransactionTrait},
};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_194625_create_todo_event_type"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("todo_event_type")
                    .if_not_exists()
                    .col(string("code").primary_key().not_null())
                    .col(string("label").not_null())
                    .col(boolean("counts_as_work").not_null())
                    .to_owned(),
            )
            .await?;

        let event_types = vec![
            ("created", "Created", false),
            ("title_changed", "Title edited", true),
            ("description_changed", "Description edited", true),
            ("status_changed", "Status changed", true),
            ("priority_changed", "Priority changed", false),
            ("due_date_changed", "Due date changed", false),
            ("tag_added", "Tag added", false),
            ("tag_removed", "Tag removed", false),
            ("planned_for_day", "Set to doing today", false),
            ("unplanned_from_day", "Removed from today", false),
            ("pull_request_linked", "Pull request linked", true),
            ("pull_request_unlinked", "Pull request unlinked", false),
            ("note_added", "Note added", true),
            ("synced_from_linear", "Synced from Linear", false),
            ("unassigned_in_linear", "Unassigned in Linear", false),
            ("pr_state_changed", "Pull request state changed", false),
            ("archived", "Archived", false),
            ("restored", "Restored", false),
        ];

        let db = manager.get_connection();
        let ts = db.begin().await?;
        for (code, label, counts_as_work) in event_types {
            ts.execute_raw(Statement::from_sql_and_values(
                manager.get_database_backend(),
                r#"
                    INSERT INTO todo_event_type (code, label, counts_as_work)
                    VALUES (?, ?, ?)
                "#,
                [code.into(), label.into(), counts_as_work.into()],
            ))
            .await?;
        }
        ts.commit().await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("todo_event_type").to_owned())
            .await
    }
}
