#![cfg(feature = "integration")]

mod common;
use common::*;

use chrono::NaiveDate;
use entity::sea_orm_active_enums::TodoStatus;
use pretty_assertions::assert_eq;

use preflight_core::{day_plan::DayPlanService, review::ReviewService, todo_service::TodoService};

#[tokio::test]
async fn test_daily_review_returns_planned_and_touched() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = preflight_core::todo_service::new(db.clone(), clock.clone());
    let plan_svc = preflight_core::day_plan::new(db.clone(), clock.clone());
    let review_svc = preflight_core::review::new(db.clone(), clock);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 8, 4).unwrap();

    // Plan for the date
    plan_svc.plan(todo.id, date).await.unwrap();

    // Change status — creates a user event so it shows up as "touched"
    todo_svc
        .set_status(todo.id, TodoStatus::Started, None)
        .await
        .unwrap();

    let review = review_svc.daily(date).await.unwrap();

    // Should be in planned
    assert_eq!(review.planned.len(), 1);
    assert_eq!(review.planned[0].id, todo.id);

    // Should be in touched (the set_status creates a user event)
    assert_eq!(review.touched.len(), 1);
    assert_eq!(review.touched[0].id, todo.id);
}
