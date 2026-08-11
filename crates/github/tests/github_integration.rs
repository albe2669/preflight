#![cfg(feature = "integration")]

mod common;

use chrono::Utc;
use github::client::{GithubApiClient, new_client};
use github::entity::enums::PullRequestState;
use github::sync::{GithubOptions, GithubSync, PrRecord, new};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter,
    Set,
};

use std::sync::Arc;

/// Build a no-op client — never called by these integration tests.
fn noop_client() -> Arc<dyn GithubApiClient> {
    Arc::new(new_client(String::new(), String::new()))
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
        remote_created_at: None,
        remote_updated_at: None,
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
// timestamp tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_upsert_pr_insert_with_timestamps() {
    let db = common::setup_db().await;

    let created = chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let updated = chrono::DateTime::parse_from_rfc3339("2024-01-02T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let rec = PrRecord {
        remote_created_at: Some(created),
        remote_updated_at: Some(updated),
        ..make_pr("github", "preflight", "preflight", 42, "Fix login")
    };
    let model = github::sync::upsert_pr(&db, &rec).await.unwrap();

    assert_eq!(
        model.remote_created_at,
        Some(chrono::DateTime::from(created))
    );
    assert_eq!(
        model.remote_updated_at,
        Some(chrono::DateTime::from(updated))
    );
}

#[tokio::test]
async fn test_upsert_pr_update_refreshes_updated_preserves_created_and_dismissed() {
    let db = common::setup_db().await;

    let created = chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let updated1 = chrono::DateTime::parse_from_rfc3339("2024-01-02T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);

    // Insert with timestamps
    let rec1 = PrRecord {
        remote_created_at: Some(created),
        remote_updated_at: Some(updated1),
        ..make_pr("github", "preflight", "preflight", 42, "Original")
    };
    let inserted = github::sync::upsert_pr(&db, &rec1).await.unwrap();

    // Set dismissed_at
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

    // Update with new remote_updated_at
    let updated2 = chrono::DateTime::parse_from_rfc3339("2024-01-03T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let rec2 = PrRecord {
        title: "Updated".into(),
        remote_updated_at: Some(updated2),
        ..make_pr("github", "preflight", "preflight", 42, "Updated")
    };
    let updated = github::sync::upsert_pr(&db, &rec2).await.unwrap();

    // remote_created_at preserved
    assert_eq!(
        updated.remote_created_at,
        Some(chrono::DateTime::from(created))
    );
    // remote_updated_at refreshed
    assert_eq!(
        updated.remote_updated_at,
        Some(chrono::DateTime::from(updated2))
    );
    // dismissed_at preserved
    assert_eq!(updated.dismissed_at, Some(dismissed));
    // same row updated
    assert_eq!(updated.id, inserted.id);
}

// ---------------------------------------------------------------------------
// pull stub tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_pull_no_token_marks_never() {
    let db = common::setup_db().await;

    let sync = new(db.clone(), noop_client(), GithubOptions::default());
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
        noop_client(),
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
        noop_client(),
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

// ---------------------------------------------------------------------------
// httptest integration tests — real reqwest client against in-process server
// ---------------------------------------------------------------------------

use github::{Author, GithubError, GithubFilter};
use httptest::{Expectation, Server, matchers::*, responders::status_code};
use sea_orm::PaginatorTrait;

fn pr_node(number: i64, title: &str, owner: &str, repo: &str) -> serde_json::Value {
    serde_json::json!({
        "number": number,
        "title": title,
        "url": format!("https://github.com/{owner}/{repo}/pull/{number}"),
        "state": "OPEN",
        "author": { "login": "alice" },
        "createdAt": "2024-06-01T00:00:00Z",
        "updatedAt": "2024-06-02T00:00:00Z",
        "repository": { "nameWithOwner": format!("{owner}/{repo}") }
    })
}

fn make_graphql_response(
    nodes: Vec<serde_json::Value>,
    has_next: bool,
    end_cursor: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "data": {
            "search": {
                "pageInfo": {
                    "endCursor": end_cursor,
                    "hasNextPage": has_next,
                },
                "nodes": nodes,
            }
        }
    })
}

fn filters_with_me_author() -> Vec<GithubFilter> {
    vec![GithubFilter {
        author: Some(Author::Me),
        ..Default::default()
    }]
}

fn make_sync(db: DatabaseConnection, client: Arc<dyn GithubApiClient>) -> impl GithubSync {
    new(
        db,
        client,
        GithubOptions {
            token: "test-token".into(),
            filters: filters_with_me_author(),
            exclude_drafts_unless_authored_by_me: false,
        },
    )
}

/// Task 9.1 — Single-page fetch: server returns one page with 2 PRs, pull() upserts both.
#[tokio::test]
async fn test_single_page_fetch() {
    let db = common::setup_db().await;
    let server = Server::run();

    let nodes = vec![
        pr_node(1, "Fix auth", "preflight", "preflight"),
        pr_node(2, "Add logging", "preflight", "preflight"),
    ];
    let body_str = make_graphql_response(nodes, false, None).to_string();

    server.expect(
        Expectation::matching(request::method_path("POST", "/"))
            .times(1..)
            .respond_with(
                status_code(200)
                    .insert_header("content-type", "application/json")
                    .body(body_str),
            ),
    );

    let client: Arc<dyn GithubApiClient> = Arc::new(new_client(
        "test-token".into(),
        server.url_str("/").to_string(),
    ));
    let sync = make_sync(db.clone(), client);
    let state = sync.pull().await.unwrap();

    assert_eq!(state.last_status, "ok");
    assert_eq!(state.last_error, None);

    let count = github::entity::pull_request::Entity::find()
        .count(&db)
        .await
        .unwrap();
    assert_eq!(count, 2);

    // Verify timestamps populated
    let prs = github::entity::pull_request::Entity::find()
        .all(&db)
        .await
        .unwrap();
    for pr in &prs {
        assert!(pr.remote_created_at.is_some());
        assert!(pr.remote_updated_at.is_some());
    }
}

/// Task 9.2 — Multi-page: two pages, cursor sent on second request.
#[tokio::test]
async fn test_multi_page_fetch() {
    let db = common::setup_db().await;
    let server = Server::run();

    // Page 1 — hasNextPage: true, endCursor: "cursor-1"
    let page1_str = make_graphql_response(
        vec![pr_node(1, "Page1-PR", "preflight", "preflight")],
        true,
        Some("cursor-1"),
    )
    .to_string();

    // Page 2 — hasNextPage: false
    let page2_str = make_graphql_response(
        vec![pr_node(2, "Page2-PR", "preflight", "preflight")],
        false,
        None,
    )
    .to_string();

    // First request: no cursor in body (after is null)
    server.expect(
        Expectation::matching(all_of![
            request::method_path("POST", "/"),
            request::body(matches(r#""after":null"#)),
        ])
        .times(1)
        .respond_with(
            status_code(200)
                .insert_header("content-type", "application/json")
                .body(page1_str),
        ),
    );

    // Second request: cursor "cursor-1" in body
    server.expect(
        Expectation::matching(all_of![
            request::method_path("POST", "/"),
            request::body(matches(r#""after":"cursor-1""#)),
        ])
        .times(1)
        .respond_with(
            status_code(200)
                .insert_header("content-type", "application/json")
                .body(page2_str),
        ),
    );

    let client: Arc<dyn GithubApiClient> = Arc::new(new_client(
        "test-token".into(),
        server.url_str("/").to_string(),
    ));
    let sync = make_sync(db.clone(), client);
    let state = sync.pull().await.unwrap();

    assert_eq!(state.last_status, "ok");
    assert_eq!(state.last_error, None);

    let count = github::entity::pull_request::Entity::find()
        .count(&db)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

/// Task 9.3a — 401 → Unauthorized
#[tokio::test]
async fn test_error_401_unauthorized() {
    let db = common::setup_db().await;
    let server = Server::run();

    server.expect(
        Expectation::matching(request::method_path("POST", "/"))
            .times(1)
            .respond_with(status_code(401)),
    );

    let client: Arc<dyn GithubApiClient> = Arc::new(new_client(
        "test-token".into(),
        server.url_str("/").to_string(),
    ));
    let sync = make_sync(db, client);
    let err = sync.pull().await.unwrap_err();

    assert!(matches!(&err, GithubError::Unauthorized));
}

/// Task 9.3b — 429 with Retry-After header → PartialResults (sync wraps client errors)
/// Task 9.3b — 429 with Retry-After header → RateLimited { Some }
#[tokio::test]
async fn test_error_429_with_retry_after() {
    let db = common::setup_db().await;
    let server = Server::run();

    server.expect(
        Expectation::matching(request::method_path("POST", "/"))
            .times(1)
            .respond_with(status_code(429).insert_header("Retry-After", "60")),
    );

    let client: Arc<dyn GithubApiClient> = Arc::new(new_client(
        "test-token".into(),
        server.url_str("/").to_string(),
    ));
    let sync = make_sync(db, client);
    let err = sync.pull().await.unwrap_err();

    assert!(matches!(
        &err,
        GithubError::RateLimited {
            retry_after: Some(_)
        }
    ));
}
/// Task 9.3c — 429 without Retry-After → RateLimited { None }
#[tokio::test]
async fn test_error_429_without_retry_after() {
    let db = common::setup_db().await;
    let server = Server::run();

    server.expect(
        Expectation::matching(request::method_path("POST", "/"))
            .times(1)
            .respond_with(status_code(429)),
    );

    let client: Arc<dyn GithubApiClient> = Arc::new(new_client(
        "test-token".into(),
        server.url_str("/").to_string(),
    ));
    let sync = make_sync(db, client);
    let err = sync.pull().await.unwrap_err();

    assert!(matches!(
        &err,
        GithubError::RateLimited { retry_after: None }
    ));
}
/// Task 9.3d — 500 → Remote
#[tokio::test]
async fn test_error_500_remote() {
    let db = common::setup_db().await;
    let server = Server::run();

    server.expect(
        Expectation::matching(request::method_path("POST", "/"))
            .times(1)
            .respond_with(status_code(500)),
    );

    let client: Arc<dyn GithubApiClient> = Arc::new(new_client(
        "test-token".into(),
        server.url_str("/").to_string(),
    ));
    let sync = make_sync(db, client);
    let err = sync.pull().await.unwrap_err();

    assert!(matches!(&err, GithubError::Remote(_)));
}

/// Task 9.3e — 2xx with GraphQL errors array → Remote
#[tokio::test]
async fn test_error_graphql_errors() {
    let db = common::setup_db().await;
    let server = Server::run();

    let body_str = serde_json::json!({
        "errors": [{ "message": "rate limit exceeded in GraphQL" }]
    })
    .to_string();

    server.expect(
        Expectation::matching(request::method_path("POST", "/"))
            .times(1)
            .respond_with(
                status_code(200)
                    .insert_header("content-type", "application/json")
                    .body(body_str),
            ),
    );

    let client: Arc<dyn GithubApiClient> = Arc::new(new_client(
        "test-token".into(),
        server.url_str("/").to_string(),
    ));
    let sync = make_sync(db, client);
    let err = sync.pull().await.unwrap_err();

    assert!(matches!(&err, GithubError::Remote(_)));
}

/// Task 9.3f — 2xx body missing data.search → SchemaMismatch
#[tokio::test]
async fn test_error_schema_mismatch() {
    let db = common::setup_db().await;
    let server = Server::run();

    let body_str = serde_json::json!({
        "data": {}
    })
    .to_string();

    server.expect(
        Expectation::matching(request::method_path("POST", "/"))
            .times(1)
            .respond_with(
                status_code(200)
                    .insert_header("content-type", "application/json")
                    .body(body_str),
            ),
    );

    let client: Arc<dyn GithubApiClient> = Arc::new(new_client(
        "test-token".into(),
        server.url_str("/").to_string(),
    ));
    let sync = make_sync(db, client);
    let err = sync.pull().await.unwrap_err();

    assert!(matches!(&err, GithubError::SchemaMismatch(_)));
}

/// Task 9.4 — Atomicity: page 1 ok, page 2 fails → zero rows upserted.
#[tokio::test]
async fn test_atomicity_mid_page_failure() {
    let db = common::setup_db().await;
    let server = Server::run();

    // Page 1 — returns PR, has_next_page: true
    let page1_str = make_graphql_response(
        vec![pr_node(1, "Should-not-appear", "preflight", "preflight")],
        true,
        Some("fail-cursor"),
    )
    .to_string();

    // First request: no cursor (after is null)
    server.expect(
        Expectation::matching(all_of![
            request::method_path("POST", "/"),
            request::body(matches(r#""after":null"#)),
        ])
        .times(1)
        .respond_with(
            status_code(200)
                .insert_header("content-type", "application/json")
                .body(page1_str),
        ),
    );

    // Second request: cursor present, returns 500
    server.expect(
        Expectation::matching(all_of![
            request::method_path("POST", "/"),
            request::body(matches(r#""after":"fail-cursor""#)),
        ])
        .times(1)
        .respond_with(status_code(500)),
    );

    let client: Arc<dyn GithubApiClient> = Arc::new(new_client(
        "test-token".into(),
        server.url_str("/").to_string(),
    ));
    let sync = make_sync(db.clone(), client);
    let err = sync.pull().await.unwrap_err();

    assert!(matches!(&err, GithubError::PartialResults));

    // Zero rows upserted — the buffer was discarded
    let count = github::entity::pull_request::Entity::find()
        .count(&db)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

/// Task 9.5 — Request encoding: verify headers and compiled query.
#[tokio::test]
async fn test_request_encoding() {
    let db = common::setup_db().await;
    let server = Server::run();

    let nodes = vec![pr_node(1, "Verify encoding", "preflight", "preflight")];
    let body_str = make_graphql_response(nodes, false, None).to_string();

    // Expectation validates headers and body content
    server.expect(
        Expectation::matching(all_of![
            request::method_path("POST", "/"),
            request::headers(contains(("authorization", "Bearer test-token"))),
            // GitHub requires a User-Agent on every request (403 otherwise).
            request::headers(contains(("user-agent", "preflight"))),
            request::headers(contains(("accept", "application/vnd.github+json"))),
            // The compiled query for author:@me should appear in the body
            request::body(matches("author:@me")),
        ])
        .times(1)
        .respond_with(
            status_code(200)
                .insert_header("content-type", "application/json")
                .body(body_str),
        ),
    );

    let client: Arc<dyn GithubApiClient> = Arc::new(new_client(
        "test-token".into(),
        server.url_str("/").to_string(),
    ));
    let sync = make_sync(db, client);
    let state = sync.pull().await.unwrap();

    assert_eq!(state.last_status, "ok");
}
