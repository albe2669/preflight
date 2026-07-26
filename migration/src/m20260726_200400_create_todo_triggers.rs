use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_200400_create_todo_triggers"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            r#"
                CREATE TRIGGER trg_todo_touch AFTER UPDATE ON todo
                FOR EACH ROW WHEN NEW.updated_at IS OLD.updated_at
                BEGIN
                    UPDATE todo
                       SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                     WHERE id = NEW.id;
                END;
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
                CREATE TRIGGER trg_todo_status_derived AFTER UPDATE OF status ON todo
                FOR EACH ROW WHEN NEW.status IS NOT OLD.status
                BEGIN
                    UPDATE todo
                       SET status_changed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                           started_at = COALESCE(started_at,
                                                 CASE WHEN NEW.status = 'started'
                                                      THEN strftime('%Y-%m-%dT%H:%M:%fZ','now') END),
                           closed_at  = CASE WHEN (SELECT is_terminal FROM todo_status
                                                    WHERE code = NEW.status) = 1
                                             THEN strftime('%Y-%m-%dT%H:%M:%fZ','now') END
                     WHERE id = NEW.id;
                END;
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
                CREATE TRIGGER trg_todo_status_transition BEFORE UPDATE OF status ON todo
                FOR EACH ROW WHEN NEW.status IS NOT OLD.status
                    AND NOT EXISTS (SELECT 1 FROM todo_status_transition
                                     WHERE from_status = OLD.status AND to_status = NEW.status)
                BEGIN
                    SELECT RAISE(ABORT, 'illegal status transition');
                END;
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
                CREATE TRIGGER trg_day_plan_anchor AFTER INSERT ON todo_day_plan
                FOR EACH ROW WHEN NEW.day_start_utc IS NULL
                BEGIN
                    UPDATE todo_day_plan
                       SET day_start_utc = strftime(
                               '%Y-%m-%dT%H:%M:%fZ',
                               NEW.plan_date,
                               '+' || (SELECT day_start_offset_minutes FROM app_setting) || ' minutes',
                               'utc')
                     WHERE plan_date = NEW.plan_date AND todo_id = NEW.todo_id;
                END;
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
                CREATE TRIGGER trg_day_plan_anchor_frozen BEFORE UPDATE OF day_start_utc ON todo_day_plan
                FOR EACH ROW WHEN OLD.day_start_utc IS NOT NULL AND NEW.day_start_utc IS NOT OLD.day_start_utc
                BEGIN
                    SELECT RAISE(ABORT, 'day_start_utc is derived and immutable');
                END;
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
                CREATE TRIGGER trg_pr_state_change AFTER UPDATE OF state ON pull_request
                FOR EACH ROW WHEN NEW.state IS NOT OLD.state
                BEGIN
                    UPDATE pull_request
                       SET state_changed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                     WHERE id = NEW.id;

                    INSERT INTO todo_event (todo_id, occurred_on, actor, event_type,
                                            field, old_value, new_value, payload)
                    SELECT l.todo_id,
                           date('now', 'localtime',
                                '-' || (SELECT day_start_offset_minutes FROM app_setting) || ' minutes'),
                           'github_sync',
                           'pr_state_changed',
                           'state',
                           OLD.state,
                           NEW.state,
                           json_object('pull_request_id', NEW.id,
                                       'repository',      NEW.repository,
                                       'number',          NEW.number,
                                       'relation',        l.relation)
                      FROM todo_pull_request l
                     WHERE l.pull_request_id = NEW.id;
                END;
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
                CREATE TRIGGER trg_tpr_review_kind BEFORE INSERT ON todo_pull_request
                FOR EACH ROW WHEN NEW.relation = 'reviews'
                    AND (SELECT kind FROM todo WHERE id = NEW.todo_id) IS NOT 'pr_review'
                BEGIN
                    SELECT RAISE(ABORT, 'only a pr_review todo may review a pull request');
                END;
            "#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for trigger in [
            "trg_tpr_review_kind",
            "trg_pr_state_change",
            "trg_day_plan_anchor_frozen",
            "trg_day_plan_anchor",
            "trg_todo_status_transition",
            "trg_todo_status_derived",
            "trg_todo_touch",
        ] {
            db.execute_unprepared(&format!("DROP TRIGGER IF EXISTS {trigger};"))
                .await?;
        }
        Ok(())
    }
}
