use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_200500_create_todo_fts"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            r#"
                CREATE VIRTUAL TABLE todo_fts USING fts5(
                    title,
                    description,
                    content = 'todo',
                    content_rowid = 'id',
                    tokenize = 'unicode61 remove_diacritics 2'
                );
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
                CREATE TRIGGER trg_todo_fts_insert AFTER INSERT ON todo
                BEGIN
                    INSERT INTO todo_fts (rowid, title, description)
                    VALUES (NEW.id, NEW.title, NEW.description);
                END;
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
                CREATE TRIGGER trg_todo_fts_delete AFTER DELETE ON todo
                BEGIN
                    INSERT INTO todo_fts (todo_fts, rowid, title, description)
                    VALUES ('delete', OLD.id, OLD.title, OLD.description);
                END;
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
                CREATE TRIGGER trg_todo_fts_update AFTER UPDATE OF title, description ON todo
                BEGIN
                    INSERT INTO todo_fts (todo_fts, rowid, title, description)
                    VALUES ('delete', OLD.id, OLD.title, OLD.description);
                    INSERT INTO todo_fts (rowid, title, description)
                    VALUES (NEW.id, NEW.title, NEW.description);
                END;
            "#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for trigger in [
            "trg_todo_fts_update",
            "trg_todo_fts_delete",
            "trg_todo_fts_insert",
        ] {
            db.execute_unprepared(&format!("DROP TRIGGER IF EXISTS {trigger};"))
                .await?;
        }
        db.execute_unprepared("DROP TABLE IF EXISTS todo_fts;")
            .await?;
        Ok(())
    }
}
