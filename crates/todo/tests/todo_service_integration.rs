#![cfg(feature = "integration")]

mod common;
use common::*;
use pretty_assertions::assert_eq;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use todo_domain::entity::enums::{EventKind, TodoStatus};

use todo_domain::{error::Error, todo_service::TodoService};

#[tokio::test]
async fn test_create_todo_returns_model_with_defaults() {
    let db = setup_db().await;
    let service = todo_domain::todo_service::new(db, test_clock());

    let todo = service.create("Buy milk".to_string(), None).await.unwrap();

    assert!(todo.id > 0);
    assert_eq!(todo.title, "Buy milk");
    assert_eq!(todo.status, TodoStatus::Todo);
    assert_eq!(todo.description, None);
    assert_eq!(todo.started_at, None);
    assert_eq!(todo.closed_at, None);
}

#[tokio::test]
async fn test_create_todo_emits_created_event() {
    let db = setup_db().await;
    let service = todo_domain::todo_service::new(db.clone(), test_clock());

    let todo = service.create("Buy milk".to_string(), None).await.unwrap();

    let events = todo_domain::entity::todo_event::Entity::find()
        .filter(todo_domain::entity::todo_event::Column::TodoId.eq(todo.id))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::Created);
}

#[tokio::test]
async fn test_get_returns_not_found_for_missing_id() {
    let db = setup_db().await;
    let service = todo_domain::todo_service::new(db, test_clock());

    let result = service.get(999).await;

    assert!(matches!(result, Err(Error::NotFound(_))));
}

#[tokio::test]
async fn test_update_changes_title_and_emits_event() {
    let db = setup_db().await;
    let service = todo_domain::todo_service::new(db.clone(), test_clock());

    let todo = service.create("Buy milk".to_string(), None).await.unwrap();
    let updated = service
        .update(todo.id, Some("Buy eggs".to_string()), None)
        .await
        .unwrap();

    assert_eq!(updated.title, "Buy eggs");

    let events = todo_domain::entity::todo_event::Entity::find()
        .filter(todo_domain::entity::todo_event::Column::TodoId.eq(todo.id))
        .filter(todo_domain::entity::todo_event::Column::Kind.eq(EventKind::TitleChanged))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
}

#[tokio::test]
async fn test_set_status_to_started_sets_started_at() {
    let db = setup_db().await;
    let service = todo_domain::todo_service::new(db, test_clock());

    let todo = service.create("Buy milk".to_string(), None).await.unwrap();
    let updated = service
        .set_status(todo.id, TodoStatus::Started, None)
        .await
        .unwrap();

    assert!(updated.started_at.is_some());
    assert_eq!(updated.status, TodoStatus::Started);
}

#[tokio::test]
async fn test_set_status_to_done_sets_closed_at() {
    let db = setup_db().await;
    let service = todo_domain::todo_service::new(db, test_clock());

    let todo = service.create("Buy milk".to_string(), None).await.unwrap();
    let updated = service
        .set_status(todo.id, TodoStatus::Done, None)
        .await
        .unwrap();

    assert!(updated.closed_at.is_some());
    assert_eq!(updated.status, TodoStatus::Done);
}

#[tokio::test]
async fn test_set_status_to_blocked_stores_blocked_reason() {
    let db = setup_db().await;
    let service = todo_domain::todo_service::new(db, test_clock());

    let todo = service.create("Buy milk".to_string(), None).await.unwrap();
    let reason = "Waiting on supplier".to_string();
    let updated = service
        .set_status(todo.id, TodoStatus::Blocked, Some(reason.clone()))
        .await
        .unwrap();

    assert_eq!(updated.blocked_reason, Some(reason));
    assert_eq!(updated.status, TodoStatus::Blocked);
}

#[tokio::test]
async fn test_set_status_blocked_then_unblocked_emits_both_events() {
    let db = setup_db().await;
    let service = todo_domain::todo_service::new(db.clone(), test_clock());

    let todo = service.create("Buy milk".to_string(), None).await.unwrap();

    // Block
    service
        .set_status(todo.id, TodoStatus::Blocked, Some("reason".to_string()))
        .await
        .unwrap();

    // Unblock by setting Done
    service
        .set_status(todo.id, TodoStatus::Done, None)
        .await
        .unwrap();

    let events = todo_domain::entity::todo_event::Entity::find()
        .filter(todo_domain::entity::todo_event::Column::TodoId.eq(todo.id))
        .all(&db)
        .await
        .unwrap();

    let kinds: Vec<&EventKind> = events.iter().map(|e| &e.kind).collect();
    assert!(
        kinds.contains(&&EventKind::Blocked),
        "Blocked event should exist"
    );
    assert!(
        kinds.contains(&&EventKind::Unblocked),
        "Unblocked event should exist"
    );
}
