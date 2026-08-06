#![cfg(feature = "integration")]

mod common;
use common::*;

use chrono::NaiveDate;
use pretty_assertions::assert_eq;
use todo_domain::entity::enums::TodoStatus;

use sea_orm::ActiveModelTrait;
use todo_domain::entity::enums::EventActor;
use todo_domain::{day_plan::DayPlanService, review::ReviewService, todo_service::TodoService};

#[tokio::test]
async fn test_daily_review_returns_planned_and_touched() {
    let db = setup_db().await;
    let clock = test_clock();
    let today = clock.now_logical();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let plan_svc = todo_domain::day_plan::new(db.clone(), clock.clone());
    let review_svc = todo_domain::review::new(db.clone(), clock);

    let todo = todo_svc.create("Task".to_string(), None).await.unwrap();

    // Plan for today
    plan_svc.plan(todo.id, today).await.unwrap();

    // Change status — creates a user event so it shows up as "touched"
    todo_svc
        .set_status(todo.id, TodoStatus::Started, None)
        .await
        .unwrap();

    let review = review_svc.daily(today).await.unwrap();

    // Should be in planned
    assert_eq!(review.planned.len(), 1);
    assert_eq!(review.planned[0].id, todo.id);

    // Should be in touched (the set_status creates a user event)
    assert_eq!(review.touched.len(), 1);
    assert_eq!(review.touched[0].id, todo.id);
}

#[tokio::test]
async fn test_daily_review_empty_day() {
    let db = setup_db().await;
    let clock = test_clock();
    let review_svc = todo_domain::review::new(db, clock);

    let review = review_svc
        .daily(NaiveDate::from_ymd_opt(2026, 8, 4).unwrap())
        .await
        .unwrap();

    assert_eq!(review.planned.len(), 0);
    assert_eq!(review.touched.len(), 0);
    assert_eq!(review.completed.len(), 0);
    assert_eq!(review.carried_over.len(), 0);
}

#[tokio::test]
async fn test_daily_review_carried_over_appears_in_planned_and_carried() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let plan_svc = todo_domain::day_plan::new(db.clone(), clock.clone());
    let review_svc = todo_domain::review::new(db.clone(), clock);

    let todo = todo_svc
        .create("Carry me over".to_string(), None)
        .await
        .unwrap();

    let date_a = NaiveDate::from_ymd_opt(2026, 8, 3).unwrap();
    let date_b = NaiveDate::from_ymd_opt(2026, 8, 4).unwrap();

    plan_svc.plan(todo.id, date_a).await.unwrap();
    plan_svc.carry_over(date_a, date_b).await.unwrap();

    let review = review_svc.daily(date_b).await.unwrap();

    assert_eq!(review.planned.len(), 1);
    assert_eq!(review.planned[0].id, todo.id);

    assert_eq!(review.carried_over.len(), 1);
    assert_eq!(review.carried_over[0].id, todo.id);
}

#[tokio::test]
async fn test_daily_review_completed_appears_in_touched_and_completed() {
    let db = setup_db().await;
    let clock = test_clock();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let plan_svc = todo_domain::day_plan::new(db.clone(), clock.clone());
    let today = clock.now_logical();

    let review_svc = todo_domain::review::new(db.clone(), clock);
    let todo = todo_svc
        .create("Finish this".to_string(), None)
        .await
        .unwrap();

    plan_svc.plan_today(todo.id).await.unwrap();

    todo_svc
        .set_status(todo.id, TodoStatus::Started, None)
        .await
        .unwrap();
    todo_svc
        .set_status(todo.id, TodoStatus::Done, None)
        .await
        .unwrap();

    let review = review_svc.daily(today).await.unwrap();

    assert_eq!(review.planned.len(), 1);
    assert_eq!(review.planned[0].id, todo.id);

    assert!(!review.touched.is_empty());
    assert!(review.touched.iter().any(|t| t.id == todo.id));

    assert!(!review.completed.is_empty());
    assert!(review.completed.iter().any(|t| t.id == todo.id));
}

#[tokio::test]
async fn test_daily_review_excludes_sync_events() {
    let db = setup_db().await;
    let clock = test_clock();
    let today = clock.now_logical();
    let todo_svc = todo_domain::todo_service::new(db.clone(), clock.clone());
    let plan_svc = todo_domain::day_plan::new(db.clone(), clock.clone());
    let review_svc = todo_domain::review::new(db.clone(), clock);

    // todo1 created via service — has User events
    let todo1 = todo_svc
        .create("User touched".to_string(), None)
        .await
        .unwrap();

    // todo2 inserted directly into DB — no User events at all
    let todo2_id: i64 = {
        let now = todo_domain::clock::now_tz();
        let model = todo_domain::entity::todo::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            title: sea_orm::Set("Sync only".to_string()),
            description: sea_orm::Set(None),
            status: sea_orm::Set(todo_domain::entity::enums::TodoStatus::Todo),
            blocked_reason: sea_orm::Set(None),
            sort_key: sea_orm::Set(0),
            created_at: sea_orm::Set(now),
            updated_at: sea_orm::Set(now),
            started_at: sea_orm::Set(None),
            closed_at: sea_orm::Set(None),
        };
        model.insert(&db).await.unwrap().id
    };

    plan_svc.plan_today(todo1.id).await.unwrap();
    // Insert plan row directly for todo2 so no User event is emitted.
    {
        let plan = todo_domain::entity::todo_day_plan::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            plan_date: sea_orm::Set(today),
            todo_id: sea_orm::Set(todo2_id),
            position: sea_orm::Set(1),
            carried_over: sea_orm::Set(false),
            added_at: sea_orm::Set(todo_domain::clock::now_tz()),
            removed_at: sea_orm::Set(None),
        };
        plan.insert(&db).await.unwrap();
    }

    // set_status on todo1 creates a User event — should appear in touched
    todo_svc
        .set_status(todo1.id, TodoStatus::Started, None)
        .await
        .unwrap();

    // Insert a Sync-actor event directly for todo2
    let sync_event = todo_domain::entity::todo_event::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        todo_id: sea_orm::Set(todo2_id),
        kind: sea_orm::Set(todo_domain::entity::enums::EventKind::Created),
        actor: sea_orm::Set(EventActor::Sync),
        occurred_at: sea_orm::Set(todo_domain::clock::now_tz()),
        logical_date: sea_orm::Set(today),
        field: sea_orm::Set(None),
        old_value: sea_orm::Set(None),
        new_value: sea_orm::Set(None),
        payload: sea_orm::Set(None),
    };
    sync_event.insert(&db).await.unwrap();

    let review = review_svc.daily(today).await.unwrap();

    // Both planned
    assert_eq!(review.planned.len(), 2);
    assert!(review.planned.iter().any(|t| t.id == todo1.id));
    assert!(review.planned.iter().any(|t| t.id == todo2_id));

    // touched contains todo1 (User set_status) but NOT todo2 (Sync-only event)
    assert!(review.touched.iter().any(|t| t.id == todo1.id));
    assert!(!review.touched.iter().any(|t| t.id == todo2_id));
}
