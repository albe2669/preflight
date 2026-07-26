use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260726_200600_create_views"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            r#"
                CREATE VIEW v_todo AS
                SELECT t.*,
                       s.label      AS status_label,
                       s.is_open    AS is_open,
                       s.is_terminal AS is_terminal,
                       (SELECT group_concat(g.name, ',')
                          FROM todo_tag tt JOIN tag g ON g.id = tt.tag_id
                         WHERE tt.todo_id = t.id ORDER BY g.name) AS tags,
                       li.identifier AS linear_identifier,
                       li.url        AS linear_url
                  FROM todo t
                  JOIN todo_status s ON s.code = t.status
                  LEFT JOIN linear_issue li ON li.todo_id = t.id;
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
                CREATE VIEW v_day_plan AS
                SELECT p.plan_date,
                       p.day_start_utc,
                       p.origin,
                       p.added_at,
                       p.removed_at,
                       p.status_at_plan_time,
                       t.*
                  FROM todo_day_plan p
                  JOIN v_todo t ON t.id = p.todo_id;
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
                CREATE VIEW v_worked_on AS
                SELECT e.occurred_on          AS work_date,
                       e.todo_id              AS todo_id,
                       min(e.occurred_at)     AS first_activity_at,
                       max(e.occurred_at)     AS last_activity_at,
                       count(*)               AS activity_count
                  FROM todo_event e
                  JOIN todo_event_type et ON et.code = e.event_type
                 WHERE et.counts_as_work = 1
                   AND e.actor <> 'linear_sync'
                 GROUP BY e.occurred_on, e.todo_id;
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
                CREATE VIEW v_day_review AS
                SELECT d.day,
                       t.*,
                       d.was_planned,
                       d.was_worked_on
                  FROM (
                        SELECT plan_date AS day, todo_id, 1 AS was_planned, 0 AS was_worked_on
                          FROM todo_day_plan
                        UNION
                        SELECT work_date, todo_id, 0, 1
                          FROM v_worked_on
                       ) AS d
                  JOIN v_todo t ON t.id = d.todo_id;
            "#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for view in ["v_day_review", "v_worked_on", "v_day_plan", "v_todo"] {
            db.execute_unprepared(&format!("DROP VIEW IF EXISTS {view};"))
                .await?;
        }
        Ok(())
    }
}
