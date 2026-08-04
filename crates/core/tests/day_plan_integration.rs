#![cfg(feature = "integration")]

mod common;
use common::*;

use chrono::NaiveDate;
use entity::sea_orm_active_enums::TodoStatus;
use pretty_assertions::assert_eq;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use preflight_core::{day_plan::DayPlanService, todo_service::TodoService};

#[tokio::test]
async fn test_plan_creates_row_at_tail_position() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = preflight_core::todo_service::new(db.clone(), clock.clone());
    let plan_svc = preflight_core::day_plan::new(db, clock);

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
    let todo_svc = preflight_core::todo_service::new(db.clone(), clock.clone());
    let plan_svc = preflight_core::day_plan::new(db, clock);

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
    let todo_svc = preflight_core::todo_service::new(db.clone(), clock.clone());
    let plan_svc = preflight_core::day_plan::new(db.clone(), clock);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();
    // Plan for "today" according to the clock
    let _ = plan_svc.plan_today(todo.id).await.unwrap();

    // Unplan
    plan_svc.unplan_today(todo.id).await.unwrap();

    // The row should still exist but with removed_at set
    let plan = entity::todo_day_plan::Entity::find()
        .filter(entity::todo_day_plan::Column::TodoId.eq(todo.id))
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
    let todo_svc = preflight_core::todo_service::new(db.clone(), clock.clone());
    let plan_svc = preflight_core::day_plan::new(db, clock);

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
    let todo_svc = preflight_core::todo_service::new(db.clone(), clock.clone());
    let plan_svc = preflight_core::day_plan::new(db, clock);

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
    let todo_svc = preflight_core::todo_service::new(db.clone(), clock.clone());
    let plan_svc = preflight_core::day_plan::new(db, clock.clone());

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
