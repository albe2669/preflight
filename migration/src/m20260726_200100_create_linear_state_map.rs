use sea_orm_migration::{
    prelude::*,
    schema::*,
    sea_orm::{Statement, TransactionTrait},
};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_200100_create_linear_state_map"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("linear_state_map")
                    .if_not_exists()
                    .col(
                        enumeration(
                            "state_type",
                            "state_type",
                            vec!["triage", "backlog", "unstarted", "started", "canceled"],
                        )
                        .primary_key()
                        .not_null(),
                    )
                    .col(string("status").not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_linear_state_map_status")
                            .from("linear_state_map", "status")
                            .to("todo_status", "code")
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        let mappings = vec![
            ("triage", "todo"),
            ("backlog", "todo"),
            ("unstarted", "todo"),
            ("started", "started"),
            ("canceled", "cancelled"),
        ];

        let db = manager.get_connection();
        let ts = db.begin().await?;
        for (state_type, status) in mappings {
            ts.execute_raw(Statement::from_sql_and_values(
                manager.get_database_backend(),
                r#"
                    INSERT INTO linear_state_map (state_type, status)
                    VALUES (?, ?)
                "#,
                [state_type.into(), status.into()],
            ))
            .await?;
        }
        ts.commit().await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table("linear_state_map").to_owned())
            .await
    }
}
