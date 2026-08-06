#![cfg(feature = "integration")]

mod common;
use common::*;

use chrono::NaiveDate;
use pretty_assertions::assert_eq;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use todo_domain::entity::enums::TodoStatus;

use todo_domain::entity::enums::EventKind;
use todo_domain::{day_plan::DayPlanService, todo_service::TodoService};

#[tokio::test]
async fn test_plan_creates_row_at_tail_position() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let plan_svc = todo_domain::day_plan::new(db, clock);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 8, 4).unwrap();

    let plan = plan_svc.plan(todo.id, date).await.unwrap();

    assert_eq!(plan.position, 0);
    assert_eq!(plan.plan_date, date);
    assert_eq!(plan.todo_id, todo.id);
}

#[tokio::test]
async fn test_plan_twice_reuses_row_clears_removed_at() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let plan_svc = todo_domain::day_plan::new(db, clock);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 8, 4).unwrap();

    let plan1 = plan_svc.plan(todo.id, date).await.unwrap();
    // Plan again — should reuse the row
    let plan2 = plan_svc.plan(todo.id, date).await.unwrap();

    assert_eq!(plan2.id, plan1.id, "should reuse the same row");
    assert_eq!(plan2.removed_at, None, "removed_at should be cleared");
}

#[tokio::test]
async fn test_unplan_sets_removed_at() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let plan_svc = todo_domain::day_plan::new(db.clone(), clock);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();
    // Plan for "today" according to the clock
    let _ = plan_svc.plan_today(todo.id).await.unwrap();

    // Unplan
    plan_svc.unplan_today(todo.id).await.unwrap();

    // The row should still exist but with removed_at set
    let plan = todo_domain::entity::todo_day_plan::Entity::find()
        .filter(todo_domain::entity::todo_day_plan::Column::TodoId.eq(todo.id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(plan.removed_at.is_some());
}

#[tokio::test]
async fn test_reorder_rewrites_positions() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let plan_svc = todo_domain::day_plan::new(db, clock);

    let t1 = todo_svc.create("A".to_string(), None).await.unwrap();
    let t2 = todo_svc.create("B".to_string(), None).await.unwrap();
    let t3 = todo_svc.create("C".to_string(), None).await.unwrap();

    let date = NaiveDate::from_ymd_opt(2026, 8, 4).unwrap();
    plan_svc.plan(t1.id, date).await.unwrap(); // pos 0
    plan_svc.plan(t2.id, date).await.unwrap(); // pos 1
    plan_svc.plan(t3.id, date).await.unwrap(); // pos 2

    // Reorder to [t3, t1, t2]
    let result = plan_svc
        .reorder(date, &[t3.id, t1.id, t2.id])
        .await
        .unwrap();

    assert_eq!(result[0].todo_id, t3.id);
    assert_eq!(result[0].position, 0);
    assert_eq!(result[1].todo_id, t1.id);
    assert_eq!(result[1].position, 1);
    assert_eq!(result[2].todo_id, t2.id);
    assert_eq!(result[2].position, 2);
}

#[tokio::test]
async fn test_carry_over_copies_unfinished_only() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let plan_svc = todo_domain::day_plan::new(db, clock);

    let t1 = todo_svc
        .create("Unfinished".to_string(), None)
        .await
        .unwrap();
    let t2 = todo_svc.create("Done".to_string(), None).await.unwrap();

    let date_a = NaiveDate::from_ymd_opt(2026, 8, 4).unwrap();
    let date_b = NaiveDate::from_ymd_opt(2026, 8, 5).unwrap();

    plan_svc.plan(t1.id, date_a).await.unwrap();
    plan_svc.plan(t2.id, date_a).await.unwrap();

    // Mark t2 as Done
    todo_svc
        .set_status(t2.id, TodoStatus::Done, None)
        .await
        .unwrap();

    let carried = plan_svc.carry_over(date_a, date_b).await.unwrap();

    // Only the unfinished one should be carried
    assert_eq!(carried.len(), 1);
    assert_eq!(carried[0].todo_id, t1.id);
    assert!(carried[0].carried_over);
}

#[tokio::test]
async fn test_list_for_date_excludes_removed() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let plan_svc = todo_domain::day_plan::new(db.clone(), clock.clone());

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();

    // Plan for today
    plan_svc.plan_today(todo.id).await.unwrap();
    // Unplan
    plan_svc.unplan_today(todo.id).await.unwrap();

    // list_for_date for today should be empty
    let today = clock.now_logical();
    let list = plan_svc.list_for_date(today).await.unwrap();

    assert!(list.is_empty());
}

#[tokio::test]
async fn test_plan_nonexistent_todo_returns_not_found() {
    let db = setup_db().await;
    let clock = test_clock();
    let _todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let plan_svc = todo_domain::day_plan::new(db, clock);

    let date = NaiveDate::from_ymd_opt(2026, 8, 4).unwrap();
    let result = plan_svc.plan(99999, date).await;

    assert!(matches!(
        result,
        Err(todo_domain::error::Error::NotFound(_))
    ));
}

#[tokio::test]
async fn test_reorder_skips_ids_not_planned() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let plan_svc = todo_domain::day_plan::new(db, clock);

    let t1 = todo_svc.create("A".to_string(), None).await.unwrap();
    let t2 = todo_svc.create("B".to_string(), None).await.unwrap();
    let t3 = todo_svc.create("C".to_string(), None).await.unwrap();

    let date = NaiveDate::from_ymd_opt(2026, 8, 4).unwrap();
    plan_svc.plan(t1.id, date).await.unwrap();
    plan_svc.plan(t2.id, date).await.unwrap();
    plan_svc.plan(t3.id, date).await.unwrap();

    // Reorder with t2, t3, and a nonexistent id (999) — 999 should be skipped
    let result = plan_svc.reorder(date, &[t2.id, t3.id, 999]).await.unwrap();

    assert_eq!(
        result.len(),
        2,
        "should return only the two planned entries"
    );
    assert_eq!(result[0].todo_id, t2.id);
    assert_eq!(result[0].position, 0);
    assert_eq!(result[1].todo_id, t3.id);
    assert_eq!(result[1].position, 1);
}

#[tokio::test]
async fn test_carry_over_skips_done_and_cancelled() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let plan_svc = todo_domain::day_plan::new(db, clock);

    let t1 = todo_svc.create("Done".to_string(), None).await.unwrap();
    let t2 = todo_svc
        .create("Cancelled".to_string(), None)
        .await
        .unwrap();
    let t3 = todo_svc.create("Todo".to_string(), None).await.unwrap();

    let date_a = NaiveDate::from_ymd_opt(2026, 8, 4).unwrap();
    let date_b = NaiveDate::from_ymd_opt(2026, 8, 5).unwrap();

    plan_svc.plan(t1.id, date_a).await.unwrap();
    plan_svc.plan(t2.id, date_a).await.unwrap();
    plan_svc.plan(t3.id, date_a).await.unwrap();

    // Mark t1 as Done, t2 as Cancelled; t3 stays Todo
    todo_svc
        .set_status(t1.id, TodoStatus::Done, None)
        .await
        .unwrap();
    todo_svc
        .set_status(t2.id, TodoStatus::Cancelled, None)
        .await
        .unwrap();

    let carried = plan_svc.carry_over(date_a, date_b).await.unwrap();

    assert_eq!(carried.len(), 1);
    assert_eq!(carried[0].todo_id, t3.id);
    assert!(carried[0].carried_over);
}

#[tokio::test]
async fn test_carry_over_to_date_with_existing_plan_appends() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let plan_svc = todo_domain::day_plan::new(db, clock);

    let t1 = todo_svc.create("A".to_string(), None).await.unwrap();
    let t2 = todo_svc.create("B".to_string(), None).await.unwrap();

    let date_a = NaiveDate::from_ymd_opt(2026, 8, 4).unwrap();
    let date_b = NaiveDate::from_ymd_opt(2026, 8, 5).unwrap();

    // Plan t1 for date_b (already there)
    plan_svc.plan(t1.id, date_b).await.unwrap();
    // Plan t2 for date_a (will be carried over)
    plan_svc.plan(t2.id, date_a).await.unwrap();

    let _carried = plan_svc.carry_over(date_a, date_b).await.unwrap();

    let list = plan_svc.list_for_date(date_b).await.unwrap();

    assert_eq!(list.len(), 2);
    assert_eq!(list[0].todo_id, t1.id);
    assert_eq!(list[1].todo_id, t2.id);
}

#[tokio::test]
async fn test_unplan_nonexistent_row_is_noop() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let plan_svc = todo_domain::day_plan::new(db, clock);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();
    // Do NOT plan it

    // unplan_today should succeed even though there is no plan row for today
    let result = plan_svc.unplan_today(todo.id).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_unplan_already_unplanned_is_noop() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let plan_svc = todo_domain::day_plan::new(db.clone(), clock);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();

    plan_svc.plan_today(todo.id).await.unwrap();
    plan_svc.unplan_today(todo.id).await.unwrap();
    // Unplan again — should be idempotent
    plan_svc.unplan_today(todo.id).await.unwrap();

    // There should be exactly 1 Unplanned event, not 2
    let events = todo_domain::entity::todo_event::Entity::find()
        .filter(todo_domain::entity::todo_event::Column::TodoId.eq(todo.id))
        .filter(todo_domain::entity::todo_event::Column::Kind.eq(EventKind::Unplanned))
        .all(&db)
        .await
        .unwrap();

    assert_eq!(events.len(), 1, "should have exactly one Unplanned event");
}
