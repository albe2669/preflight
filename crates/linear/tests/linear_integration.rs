#![cfg(feature = "integration")]

mod common;
use common::*;
use pretty_assertions::assert_eq;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter};

use linear::LinearSync;

#[tokio::test]
async fn test_cursor_put_then_get() {
    let db = setup_db().await;

    // Put a cursor, then get it back
    linear::cursor::put(&db, "linear", Some("abc123".into()), "ok", None)
        .await
        .unwrap();
    let cursor = linear::cursor::get(&db, "linear").await.unwrap();
    assert_eq!(cursor, Some("abc123".into()));

    // Put with None cursor, get returns None
    linear::cursor::put(&db, "linear", None, "ok", None)
        .await
        .unwrap();
    let cursor = linear::cursor::get(&db, "linear").await.unwrap();
    assert_eq!(cursor, None);
}

#[tokio::test]
async fn test_cursor_put_updates_existing() {
    let db = setup_db().await;

    // First put
    linear::cursor::put(&db, "linear", Some("first".into()), "ok", None)
        .await
        .unwrap();
    let cursor = linear::cursor::get(&db, "linear").await.unwrap();
    assert_eq!(cursor, Some("first".into()));

    // Second put should overwrite
    linear::cursor::put(&db, "linear", Some("second".into()), "ok", None)
        .await
        .unwrap();
    let cursor = linear::cursor::get(&db, "linear").await.unwrap();
    assert_eq!(cursor, Some("second".into()));
}

#[tokio::test]
async fn test_cursor_get_returns_none_for_unknown_source() {
    let db = setup_db().await;

    let cursor = linear::cursor::get(&db, "nonexistent").await.unwrap();
    assert_eq!(cursor, None);
}

#[tokio::test]
async fn test_cursor_put_stores_status_and_error() {
    let db = setup_db().await;

    linear::cursor::put(&db, "linear", Some("cur".into()), "ok", Some("oops".into()))
        .await
        .unwrap();

    let row = sync_state::entity::sync_state::Entity::find_by_id("linear")
        .one(&db)
        .await
        .unwrap()
        .expect("row should exist");

    assert_eq!(row.cursor, Some("cur".into()));
    assert_eq!(row.last_status, "ok");
    assert_eq!(row.last_error, Some("oops".into()));
    assert!(row.last_synced_at.is_some());
}

#[tokio::test]
async fn test_upsert_issue_inserts_new_row() {
    let db = setup_db().await;

    let rec = linear::IssueRecord {
        linear_id: "issue-1".into(),
        identifier: "PRJ-1".into(),
        title: "First Issue".into(),
        description: Some("A description".into()),
        url: "https://linear.test/issue-1".into(),
        state_name: "Todo".into(),
        state_type: "triage".into(),
        priority: Some(1),
        team_key: Some("ENG".into()),
        assignee_name: Some("Alice".into()),
        assigned_to_me: true,
    };

    let model = linear::sync::upsert_issue(&db, &rec).await.unwrap();

    assert_eq!(model.linear_id, "issue-1");
    assert_eq!(model.identifier, "PRJ-1");
    assert_eq!(model.title, "First Issue");
    assert_eq!(model.description, Some("A description".into()));
    assert_eq!(model.url, "https://linear.test/issue-1");
    assert_eq!(model.state_name, "Todo");
    assert_eq!(model.state_type, "triage");
    assert_eq!(model.priority, Some(1));
    assert_eq!(model.team_key, Some("ENG".into()));
    assert_eq!(model.assignee_name, Some("Alice".into()));
    assert!(model.synced_at.to_string().len() > 0);
}

#[tokio::test]
async fn test_upsert_issue_updates_existing_row() {
    let db = setup_db().await;

    // First insert
    let rec1 = linear::IssueRecord {
        linear_id: "issue-2".into(),
        identifier: "PRJ-2".into(),
        title: "Original Title".into(),
        description: None,
        url: "https://linear.test/issue-2".into(),
        state_name: "Todo".into(),
        state_type: "triage".into(),
        priority: None,
        team_key: None,
        assignee_name: None,
        assigned_to_me: false,
    };
    let first = linear::sync::upsert_issue(&db, &rec1).await.unwrap();
    let first_id = first.id;

    // Second upsert with same linear_id, different title/state
    let rec2 = linear::IssueRecord {
        linear_id: "issue-2".into(),
        identifier: "PRJ-2".into(),
        title: "Updated Title".into(),
        description: Some("New desc".into()),
        url: "https://linear.test/issue-2".into(),
        state_name: "InProgress".into(),
        state_type: "triage".into(),
        priority: Some(2),
        team_key: Some("ENG".into()),
        assignee_name: Some("Bob".into()),
        assigned_to_me: true,
    };
    let second = linear::sync::upsert_issue(&db, &rec2).await.unwrap();

    // Same database row (same primary key id), updated fields
    assert_eq!(second.id, first_id, "should be the same row");
    assert_eq!(second.title, "Updated Title");
    assert_eq!(second.state_name, "InProgress");
    assert_eq!(second.description, Some("New desc".into()));
    assert_eq!(second.priority, Some(2));
    assert_eq!(second.team_key, Some("ENG".into()));
    assert_eq!(second.assignee_name, Some("Bob".into()));
    assert!(second.assigned_to_me);
}

#[tokio::test]
async fn test_upsert_issue_preserves_dismissed_at_on_update() {
    let db = setup_db().await;

    // Insert issue
    let rec = linear::IssueRecord {
        linear_id: "issue-3".into(),
        identifier: "PRJ-3".into(),
        title: "Original".into(),
        description: None,
        url: "https://linear.test/issue-3".into(),
        state_name: "Todo".into(),
        state_type: "triage".into(),
        priority: None,
        team_key: None,
        assignee_name: None,
        assigned_to_me: false,
    };
    linear::sync::upsert_issue(&db, &rec).await.unwrap();

    // Set dismissed_at manually via ActiveModel
    let model = linear::entity::linear_issue::Entity::find()
        .filter(linear::entity::linear_issue::Column::LinearId.eq("issue-3"))
        .one(&db)
        .await
        .unwrap()
        .unwrap();

    let mut am = model.into_active_model();
    am.dismissed_at = sea_orm::ActiveValue::Set(Some(chrono::Utc::now().into()));
    am.update(&db).await.unwrap();

    // Verify dismissed_at is set
    let check = linear::entity::linear_issue::Entity::find()
        .filter(linear::entity::linear_issue::Column::LinearId.eq("issue-3"))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(
        check.dismissed_at.is_some(),
        "dismissed_at should be set before upsert"
    );

    // Upsert with changed title — should preserve dismissed_at
    let rec2 = linear::IssueRecord {
        linear_id: "issue-3".into(),
        identifier: "PRJ-3".into(),
        title: "Updated".into(),
        description: None,
        url: "https://linear.test/issue-3".into(),
        state_name: "Done".into(),
        state_type: "triage".into(),
        priority: None,
        team_key: None,
        assignee_name: None,
        assigned_to_me: false,
    };
    let updated = linear::sync::upsert_issue(&db, &rec2).await.unwrap();

    assert_eq!(updated.title, "Updated");
    assert!(
        updated.dismissed_at.is_some(),
        "dismissed_at should be preserved after upsert"
    );
}

#[tokio::test]
async fn test_pull_no_token_marks_never() {
    let db = setup_db().await;

    let svc = linear::sync::new(db.clone(), linear::LinearOptions::default());
    let result = svc.pull().await.unwrap();

    assert_eq!(result.source, "linear");
    assert_eq!(result.last_status, "never");
    assert_eq!(result.last_error, Some("no token configured".into()));
}

#[tokio::test]
async fn test_pull_with_token_marks_ok() {
    let db = setup_db().await;

    let svc = linear::sync::new(
        db.clone(),
        linear::LinearOptions {
            token: "fake-token".into(),
            team_keys: vec![],
        },
    );
    let result = svc.pull().await.unwrap();

    assert_eq!(result.source, "linear");
    assert_eq!(result.last_status, "ok");
    assert_eq!(result.last_error, None);
}

#[tokio::test]
async fn test_pull_returns_sync_state_model() {
    let db = setup_db().await;

    let svc = linear::sync::new(
        db.clone(),
        linear::LinearOptions {
            token: "fake-token".into(),
            team_keys: vec!["ENG".into()],
        },
    );

    // Verify the return type is sync_state::entity::sync_state::Model
    let result: sync_state::entity::sync_state::Model = svc.pull().await.unwrap();

    assert_eq!(result.source, "linear");
    assert!(result.last_synced_at.is_some());
}
