#![cfg(feature = "integration")]

mod common;

use chrono::Utc;
use github::cursor;
use github::entity::enums::PullRequestState;
use github::sync::{GithubOptions, GithubSync, PrRecord, new};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter,
    Set,
};

// ---------------------------------------------------------------------------
// cursor tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cursor_put_then_get() {
    let db = common::setup_db().await;

    // Put a cursor and read it back
    cursor::put(&db, "github", Some("abc123".into()), "ok", None)
        .await
        .unwrap();
    let got = cursor::get(&db, "github").await.unwrap();
    assert_eq!(got, Some("abc123".into()));

    // Put with None cursor — get should return None
    cursor::put(&db, "github", None, "ok", None).await.unwrap();
    let got = cursor::get(&db, "github").await.unwrap();
    assert_eq!(got, None);
}

#[tokio::test]
async fn test_cursor_put_updates_existing() {
    let db = common::setup_db().await;

    cursor::put(&db, "github", Some("first".into()), "ok", None)
        .await
        .unwrap();
    cursor::put(&db, "github", Some("second".into()), "ok", None)
        .await
        .unwrap();

    let got = cursor::get(&db, "github").await.unwrap();
    assert_eq!(got, Some("second".into()));
}

#[tokio::test]
async fn test_cursor_get_returns_none_for_unknown_source() {
    let db = common::setup_db().await;

    let got = cursor::get(&db, "nonexistent").await.unwrap();
    assert_eq!(got, None);
}

#[tokio::test]
async fn test_cursor_put_stores_status_and_error() {
    let db = common::setup_db().await;

    cursor::put(
        &db,
        "github",
        Some("page-5".into()),
        "ok",
        Some("some warning".into()),
    )
    .await
    .unwrap();

    let row = sync_state::entity::sync_state::Entity::find()
        .filter(sync_state::entity::sync_state::Column::Source.eq("github"))
        .one(&db)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(row.last_status, "ok");
    assert_eq!(row.last_error, Some("some warning".into()));
    assert_eq!(row.cursor, Some("page-5".into()));
    assert!(row.last_synced_at.is_some());
}

// ---------------------------------------------------------------------------
// upsert_pr tests
// ---------------------------------------------------------------------------

fn make_pr(provider: &str, owner: &str, repo: &str, number: i64, title: &str) -> PrRecord {
    PrRecord {
        provider: provider.into(),
        owner: owner.into(),
        repo: repo.into(),
        number,
        title: title.into(),
        url: format!("https://github.com/{owner}/{repo}/pull/{number}"),
        author: Some("alice".into()),
        state: PullRequestState::Open,
        review_requested: true,
        authored_by_me: false,
    }
}

#[tokio::test]
async fn test_upsert_pr_inserts_new_row() {
    let db = common::setup_db().await;

    let rec = make_pr("github", "preflight", "preflight", 42, "Fix login");
    let model = github::sync::upsert_pr(&db, &rec).await.unwrap();

    assert_eq!(model.provider, "github");
    assert_eq!(model.owner, "preflight");
    assert_eq!(model.repo, "preflight");
    assert_eq!(model.number, 42);
    assert!(model.synced_at > chrono::DateTime::UNIX_EPOCH);
}

#[tokio::test]
async fn test_upsert_pr_updates_existing_row() {
    let db = common::setup_db().await;

    // Insert
    let rec1 = make_pr("github", "preflight", "preflight", 42, "Original title");
    let inserted = github::sync::upsert_pr(&db, &rec1).await.unwrap();

    // Update with same identity but changed title and state
    let rec2 = make_pr("github", "preflight", "preflight", 42, "Updated title");
    let rec2 = PrRecord {
        state: PullRequestState::Closed,
        ..rec2
    };
    let updated = github::sync::upsert_pr(&db, &rec2).await.unwrap();

    // Same id means it was updated, not a new row
    assert_eq!(updated.id, inserted.id);
    assert_eq!(updated.title, "Updated title");
    assert_eq!(updated.state, PullRequestState::Closed);
}

#[tokio::test]
async fn test_upsert_pr_preserves_dismissed_at_on_update() {
    let db = common::setup_db().await;

    // Insert a PR
    let rec = make_pr("github", "preflight", "preflight", 42, "Fix login");
    let _model = github::sync::upsert_pr(&db, &rec).await.unwrap();

    // Manually set dismissed_at
    let dismissed = Utc::now().into();
    let pr = github::entity::pull_request::Entity::find()
        .filter(github::entity::pull_request::Column::Provider.eq("github"))
        .filter(github::entity::pull_request::Column::Owner.eq("preflight"))
        .filter(github::entity::pull_request::Column::Repo.eq("preflight"))
        .filter(github::entity::pull_request::Column::Number.eq(42))
        .one(&db)
        .await
        .unwrap()
        .unwrap();

    let mut am = pr.into_active_model();
    am.dismissed_at = Set(Some(dismissed));
    am.update(&db).await.unwrap();

    // Upsert again with the same identity — should NOT clear dismissed_at
    let rec2 = make_pr("github", "preflight", "preflight", 42, "Updated title");
    let updated = github::sync::upsert_pr(&db, &rec2).await.unwrap();

    assert_eq!(updated.dismissed_at, Some(dismissed));
}

// ---------------------------------------------------------------------------
// pull stub tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_pull_no_token_marks_never() {
    let db = common::setup_db().await;

    let sync = new(db.clone(), GithubOptions::default());
    let state = sync.pull().await.unwrap();

    assert_eq!(state.source, "github");
    assert_eq!(state.last_status, "never");
    assert_eq!(state.last_error, Some("no token configured".into()));
}

#[tokio::test]
async fn test_pull_with_token_marks_ok() {
    let db = common::setup_db().await;

    let sync = new(
        db.clone(),
        GithubOptions {
            token: "fake-token".into(),
            filters: vec![],
            exclude_drafts_unless_authored_by_me: false,
        },
    );
    let state = sync.pull().await.unwrap();

    assert_eq!(state.source, "github");
    assert_eq!(state.last_status, "ok");
    assert_eq!(state.last_error, None);
}

#[tokio::test]
async fn test_pull_returns_sync_state_model() {
    let db = common::setup_db().await;

    let sync = new(
        db.clone(),
        GithubOptions {
            token: "fake".into(),
            filters: vec![],
            exclude_drafts_unless_authored_by_me: false,
        },
    );
    let state = sync.pull().await.unwrap();

    // The return type is sync_state::Model — verify it's a proper model
    assert_eq!(state.source, "github");
    assert!(state.last_synced_at.is_some());
}
