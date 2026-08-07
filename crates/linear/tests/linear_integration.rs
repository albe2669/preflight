#![cfg(feature = "integration")]

mod common;
use common::*;
use pretty_assertions::assert_eq;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter};

use linear::LinearSync;

fn make_issue(linear_id: &str, identifier: &str, title: &str) -> linear::IssueRecord {
    linear::IssueRecord {
        linear_id: linear_id.into(),
        identifier: identifier.into(),
        title: title.into(),
        description: None,
        url: format!("https://linear.test/{linear_id}"),
        state_name: "Todo".into(),
        state_type: "triage".into(),
        priority: None,
        team_key: None,
        assignee_name: None,
        assigned_to_me: false,
        remote_created_at: None,
        remote_updated_at: None,
    }
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
        remote_created_at: None,
        remote_updated_at: None,
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
async fn test_upsert_issue_inserts_with_timestamps() {
    let db = setup_db().await;

    let created = chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .into();
    let updated = chrono::DateTime::parse_from_rfc3339("2024-01-02T00:00:00Z")
        .unwrap()
        .into();

    let rec = linear::IssueRecord {
        linear_id: "issue-ts-1".into(),
        identifier: "PRJ-TS-1".into(),
        title: "Timestamp Issue".into(),
        description: None,
        url: "https://linear.test/issue-ts-1".into(),
        state_name: "Todo".into(),
        state_type: "triage".into(),
        priority: None,
        team_key: None,
        assignee_name: None,
        assigned_to_me: false,
        remote_created_at: Some(created),
        remote_updated_at: Some(updated),
    };

    let model = linear::sync::upsert_issue(&db, &rec).await.unwrap();

    assert_eq!(
        model.remote_created_at,
        Some(created),
        "remote_created_at should be set on insert"
    );
    assert_eq!(
        model.remote_updated_at,
        Some(updated),
        "remote_updated_at should be set on insert"
    );
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
        remote_created_at: None,
        remote_updated_at: None,
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
        remote_created_at: None,
        remote_updated_at: None,
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
        remote_created_at: None,
        remote_updated_at: None,
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
        remote_created_at: None,
        remote_updated_at: None,
    };
    let updated = linear::sync::upsert_issue(&db, &rec2).await.unwrap();

    assert_eq!(updated.title, "Updated");
    assert!(
        updated.dismissed_at.is_some(),
        "dismissed_at should be preserved after upsert"
    );
}

#[tokio::test]
async fn test_upsert_issue_update_preserves_created_refreshes_updated() {
    let db = setup_db().await;

    let created = chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .into();
    let updated1 = chrono::DateTime::parse_from_rfc3339("2024-01-02T00:00:00Z")
        .unwrap()
        .into();

    // Insert with timestamps
    let rec1 = linear::IssueRecord {
        linear_id: "issue-ts-update".into(),
        identifier: "PRJ-TS-U".into(),
        title: "Original".into(),
        description: None,
        url: "https://linear.test/issue-ts-update".into(),
        state_name: "Todo".into(),
        state_type: "triage".into(),
        priority: None,
        team_key: None,
        assignee_name: None,
        assigned_to_me: false,
        remote_created_at: Some(created),
        remote_updated_at: Some(updated1),
    };
    let first = linear::sync::upsert_issue(&db, &rec1).await.unwrap();
    assert_eq!(first.remote_created_at, Some(created));
    assert_eq!(first.remote_updated_at, Some(updated1));

    // Update with a new updated_at
    let updated2 = chrono::DateTime::parse_from_rfc3339("2024-01-03T00:00:00Z")
        .unwrap()
        .into();
    let rec2 = linear::IssueRecord {
        linear_id: "issue-ts-update".into(),
        identifier: "PRJ-TS-U".into(),
        title: "Updated".into(),
        description: None,
        url: "https://linear.test/issue-ts-update".into(),
        state_name: "InProgress".into(),
        state_type: "started".into(),
        priority: None,
        team_key: None,
        assignee_name: None,
        assigned_to_me: false,
        remote_created_at: Some(created),
        remote_updated_at: Some(updated2),
    };
    let second = linear::sync::upsert_issue(&db, &rec2).await.unwrap();

    // remote_created_at preserved
    assert_eq!(
        second.remote_created_at,
        Some(created),
        "remote_created_at should be preserved on update"
    );
    // remote_updated_at refreshed
    assert_eq!(
        second.remote_updated_at,
        Some(updated2),
        "remote_updated_at should be refreshed on update"
    );
    // dismissed_at stays None (was never set)
    assert!(second.dismissed_at.is_none());
}

#[tokio::test]
async fn test_pull_no_token_marks_never() {
    let db = setup_db().await;

    let svc = linear::sync::new(
        db.clone(),
        linear::new_client(String::new(), "http://unused".into()),
        linear::LinearOptions::default(),
    );
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
        linear::new_client("fake-token".into(), "http://unused".into()),
        linear::LinearOptions {
            token: "fake-token".into(),
            filters: vec![],
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
        linear::new_client("fake".into(), "http://unused".into()),
        linear::LinearOptions {
            token: "fake-token".into(),
            filters: vec![],
        },
    );

    // Verify the return type is sync_state::entity::sync_state::Model
    let result: sync_state::entity::sync_state::Model = svc.pull().await.unwrap();

    assert_eq!(result.source, "linear");
    assert!(result.last_synced_at.is_some());
}
// ---------------------------------------------------------------------------
// httptest-based integration tests — real reqwest client against in-process
// httptest::Server speaking Linear's GraphQL wire format.
// ---------------------------------------------------------------------------

use httptest::{Expectation, Server, matchers::*, responders::*};
use linear::{Actor, LinearFilter, LinearOptions, compile_linear_filter};
use sea_orm::PaginatorTrait;

/// Build a minimal Linear issue GraphQL node JSON value.
fn issue_node(id: &str, identifier: &str, title: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "identifier": identifier,
        "title": title,
        "description": "",
        "url": format!("https://linear.app/test/issue/{id}"),
        "state": { "name": "Todo", "type": "triage" },
        "priority": 0,
        "team": { "key": "ENG" },
        "assignee": { "name": "Alice" },
        "createdAt": "2024-06-01T00:00:00Z",
        "updatedAt": "2024-06-02T00:00:00Z",
    })
}

/// Build a full GraphQL response JSON for the `issues` query.
fn graphql_page_response(
    nodes: Vec<serde_json::Value>,
    end_cursor: Option<&str>,
    has_next_page: bool,
) -> serde_json::Value {
    serde_json::json!({
        "data": {
            "issues": {
                "pageInfo": {
                    "endCursor": end_cursor,
                    "hasNextPage": has_next_page,
                },
                "nodes": nodes,
            }
        }
    })
}

/// Count rows in the `linear_issue` table.
async fn count_issues(db: &sea_orm::DatabaseConnection) -> i64 {
    linear::entity::linear_issue::Entity::find()
        .count(db)
        .await
        .unwrap() as i64
}

// ---------------------------------------------------------------------------
// Task 10.1 — Single-page fetch
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_single_page_fetch() {
    let db = setup_db().await;

    let nodes = vec![
        issue_node("issue-1", "ENG-1", "First Issue"),
        issue_node("issue-2", "ENG-2", "Second Issue"),
    ];
    let body = graphql_page_response(nodes, None, false);

    let server = Server::run();
    server.expect(Expectation::matching(any()).respond_with(json_encoded(body)));

    let filter = LinearFilter {
        team: Some("ENG".into()),
        ..Default::default()
    };
    let opts = LinearOptions {
        token: "test-token".into(),
        filters: vec![filter],
    };

    let svc = linear::sync::new(
        db.clone(),
        linear::new_client("test-token".into(), server.url_str("/")),
        opts,
    );

    let result = svc.pull().await.unwrap();
    assert_eq!(result.source, "linear");
    assert_eq!(result.last_status, "ok");
    assert_eq!(result.last_error, None);

    // Two issues upserted
    assert_eq!(count_issues(&db).await, 2);

    // Verify remote timestamps populated
    let rows = linear::entity::linear_issue::Entity::find()
        .all(&db)
        .await
        .unwrap();
    for row in &rows {
        assert!(
            row.remote_created_at.is_some(),
            "remote_created_at should be populated"
        );
        assert!(
            row.remote_updated_at.is_some(),
            "remote_updated_at should be populated"
        );
    }
    assert_eq!(rows[0].linear_id, "issue-1");
    assert_eq!(rows[1].linear_id, "issue-2");
}

// ---------------------------------------------------------------------------
// Task 10.2 — Multi-page test
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_multi_page_fetch() {
    let db = setup_db().await;

    let page1_body = graphql_page_response(
        vec![issue_node("issue-1", "ENG-1", "Page1 Issue")],
        Some("cursor-X"),
        true,
    );
    let page2_body = graphql_page_response(
        vec![
            issue_node("issue-2", "ENG-2", "Page2 Issue A"),
            issue_node("issue-3", "ENG-3", "Page2 Issue B"),
        ],
        None,
        false,
    );
    let server = Server::run();

    // Second request carries after: "cursor-X" — registered first so checked first
    // (reverse order matching in httptest).
    server.expect(
        Expectation::matching(all_of![
            request::method("POST"),
            request::body(matches(r#""after": ?"cursor-X""#)),
        ])
        .respond_with(json_encoded(page2_body)),
    );

    // First request carries after: null.
    server.expect(
        Expectation::matching(all_of![
            request::method("POST"),
            request::body(matches(r#""after": ?null"#)),
        ])
        .respond_with(json_encoded(page1_body)),
    );

    let filter = LinearFilter {
        team: Some("ENG".into()),
        ..Default::default()
    };
    let opts = LinearOptions {
        token: "test-token".into(),
        filters: vec![filter],
    };

    let svc = linear::sync::new(
        db.clone(),
        linear::new_client("test-token".into(), server.url_str("/")),
        opts,
    );

    let result = svc.pull().await.unwrap();
    assert_eq!(result.last_status, "ok");

    // All 3 nodes upserted.
    assert_eq!(count_issues(&db).await, 3);
}

// ---------------------------------------------------------------------------
// Task 10.3 — Error-mapping tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_error_401_unauthorized() {
    let db = setup_db().await;
    let server = Server::run();
    server.expect(Expectation::matching(any()).respond_with(status_code(401)));

    let filter = LinearFilter {
        team: Some("ENG".into()),
        ..Default::default()
    };
    let opts = LinearOptions {
        token: "bad-token".into(),
        filters: vec![filter],
    };

    let svc = linear::sync::new(
        db.clone(),
        linear::new_client("bad-token".into(), server.url_str("/")),
        opts,
    );

    let err = svc.pull().await.unwrap_err();
    assert!(
        matches!(err, linear::LinearError::Unauthorized),
        "401 should return Unauthorized"
    );
}

#[tokio::test]
async fn test_error_429_with_retry_after() {
    let db = setup_db().await;
    let server = Server::run();
    server.expect(
        Expectation::matching(any())
            .respond_with(status_code(429).append_header("Retry-After", "60")),
    );

    let filter = LinearFilter {
        team: Some("ENG".into()),
        ..Default::default()
    };
    let opts = LinearOptions {
        token: "test-token".into(),
        filters: vec![filter],
    };

    let svc = linear::sync::new(
        db.clone(),
        linear::new_client("test-token".into(), server.url_str("/")),
        opts,
    );

    let err = svc.pull().await.unwrap_err();
    assert!(
        matches!(
            err,
            linear::LinearError::RateLimited {
                retry_after: Some(_)
            }
        ),
        "429 with Retry-After should return RateLimited with retry_after"
    );
}

#[tokio::test]
async fn test_error_429_without_retry_after() {
    let db = setup_db().await;
    let server = Server::run();
    server.expect(Expectation::matching(any()).respond_with(status_code(429)));

    let filter = LinearFilter {
        team: Some("ENG".into()),
        ..Default::default()
    };
    let opts = LinearOptions {
        token: "test-token".into(),
        filters: vec![filter],
    };

    let svc = linear::sync::new(
        db.clone(),
        linear::new_client("test-token".into(), server.url_str("/")),
        opts,
    );

    let err = svc.pull().await.unwrap_err();
    assert!(
        matches!(err, linear::LinearError::RateLimited { retry_after: None }),
        "429 without Retry-After should return RateLimited with None"
    );
}

#[tokio::test]
async fn test_error_500_remote() {
    let db = setup_db().await;
    let server = Server::run();
    server.expect(Expectation::matching(any()).respond_with(status_code(500)));

    let filter = LinearFilter {
        team: Some("ENG".into()),
        ..Default::default()
    };
    let opts = LinearOptions {
        token: "test-token".into(),
        filters: vec![filter],
    };

    let svc = linear::sync::new(
        db.clone(),
        linear::new_client("test-token".into(), server.url_str("/")),
        opts,
    );

    let err = svc.pull().await.unwrap_err();
    assert!(
        matches!(err, linear::LinearError::Remote(_)),
        "500 should return Remote"
    );
}

#[tokio::test]
async fn test_error_graphql_errors_in_2xx() {
    let db = setup_db().await;
    let server = Server::run();
    let body = serde_json::json!({
        "errors": [
            { "message": "permission denied" },
            { "message": "field not found" }
        ],
        "data": null
    });
    server.expect(Expectation::matching(any()).respond_with(json_encoded(body)));

    let filter = LinearFilter {
        team: Some("ENG".into()),
        ..Default::default()
    };
    let opts = LinearOptions {
        token: "test-token".into(),
        filters: vec![filter],
    };

    let svc = linear::sync::new(
        db.clone(),
        linear::new_client("test-token".into(), server.url_str("/")),
        opts,
    );

    let err = svc.pull().await.unwrap_err();
    assert!(
        matches!(err, linear::LinearError::Remote(_)),
        "GraphQL errors should return Remote"
    );
}

#[tokio::test]
async fn test_error_malformed_2xx_schema_mismatch() {
    let db = setup_db().await;
    let server = Server::run();
    // Valid JSON but not the expected GraphQL shape — no "data.issues"
    let body = serde_json::json!({
        "something": "else"
    });
    server.expect(Expectation::matching(any()).respond_with(json_encoded(body)));

    let filter = LinearFilter {
        team: Some("ENG".into()),
        ..Default::default()
    };
    let opts = LinearOptions {
        token: "test-token".into(),
        filters: vec![filter],
    };

    let svc = linear::sync::new(
        db.clone(),
        linear::new_client("test-token".into(), server.url_str("/")),
        opts,
    );

    let err = svc.pull().await.unwrap_err();
    assert!(
        matches!(err, linear::LinearError::SchemaMismatch),
        "malformed 2xx should return SchemaMismatch"
    );
}

// ---------------------------------------------------------------------------
// Task 10.4 — Atomicity test: page 1 ok, page 2 fails → zero rows committed
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_atomicity_mid_page_failure() {
    let db = setup_db().await;

    let page1_body = graphql_page_response(
        vec![issue_node("issue-1", "ENG-1", "Page1 Issue")],
        Some("cursor-Y"),
        true,
    );

    let server = Server::run();

    // Second request (after: "cursor-Y") → 500 — registered first for reverse-match.
    server.expect(
        Expectation::matching(all_of![
            request::method("POST"),
            request::body(matches(r#""after": ?"cursor-Y""#)),
        ])
        .times(1..)
        .respond_with(status_code(500)),
    );

    // First request → after: null → page 1 with cursor.
    server.expect(
        Expectation::matching(all_of![
            request::method("POST"),
            request::body(matches(r#""after": ?null"#)),
        ])
        .respond_with(json_encoded(page1_body)),
    );

    let filter = LinearFilter {
        team: Some("ENG".into()),
        ..Default::default()
    };
    let opts = LinearOptions {
        token: "test-token".into(),
        filters: vec![filter],
    };
    let svc = linear::sync::new(
        db.clone(),
        linear::new_client("test-token".into(), server.url_str("/")),
        opts,
    );
    let err = svc.pull().await.unwrap_err();
    assert!(
        matches!(err, linear::LinearError::PartialResults),
        "should return PartialResults"
    );

    // Zero rows committed — atomicity guaranteed by buffering.
    assert_eq!(
        count_issues(&db).await,
        0,
        "no rows should be committed on mid-page failure"
    );
}

// ---------------------------------------------------------------------------
// Task 10.5 — Request-encoding assertion
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_request_encoding() {
    let db = setup_db().await;

    let response_body = graphql_page_response(vec![], None, false);

    let server = Server::run();

    // Build the expected compiled filter JSON for our filter.
    let filter = LinearFilter {
        team: Some("ENG".into()),
        assignee: Some(Actor::Me),
        ..Default::default()
    };
    let compiled_filter = compile_linear_filter(&[filter.clone()]);

    // We capture request details via a custom Matcher to inspect headers
    // and body, then assert in the test after pull() returns.
    use std::fmt;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct CaptureState {
        auth: Arc<Mutex<Option<String>>>,
        ct: Arc<Mutex<Option<String>>>,
        body: Arc<Mutex<Option<String>>>,
    }

    struct CaptureMatcher {
        state: CaptureState,
    }

    impl CaptureMatcher {
        fn new(
            auth: Arc<Mutex<Option<String>>>,
            ct: Arc<Mutex<Option<String>>>,
            body: Arc<Mutex<Option<String>>>,
        ) -> Self {
            Self {
                state: CaptureState { auth, ct, body },
            }
        }
    }

    impl fmt::Debug for CaptureMatcher {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "CaptureMatcher")
        }
    }

    impl<B: AsRef<[u8]>> httptest::matchers::Matcher<http::Request<B>> for CaptureMatcher {
        fn matches(
            &mut self,
            input: &http::Request<B>,
            _: &mut httptest::matchers::ExecutionContext,
        ) -> bool {
            if let Some(auth) = input.headers().get("Authorization") {
                *self.state.auth.lock().unwrap() = auth.to_str().ok().map(|s| s.to_string());
            }
            if let Some(ct) = input.headers().get("Content-Type") {
                *self.state.ct.lock().unwrap() = ct.to_str().ok().map(|s| s.to_string());
            }
            let body_bytes = input.body().as_ref();
            *self.state.body.lock().unwrap() = String::from_utf8(body_bytes.to_vec()).ok();
            true
        }

        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "CaptureMatcher")
        }
    }

    let captured_auth = Arc::new(Mutex::new(None));
    let captured_ct = Arc::new(Mutex::new(None));
    let captured_body = Arc::new(Mutex::new(None));

    let matcher = CaptureMatcher::new(
        captured_auth.clone(),
        captured_ct.clone(),
        captured_body.clone(),
    );

    server.expect(Expectation::matching(matcher).respond_with(json_encoded(response_body)));

    let opts = LinearOptions {
        token: "test-token".into(),
        filters: vec![filter],
    };

    let svc = linear::sync::new(
        db.clone(),
        linear::new_client("test-token".into(), server.url_str("/")),
        opts,
    );

    let result = svc.pull().await.unwrap();
    assert_eq!(result.last_status, "ok");

    // Assert Authorization header is raw token (not "Bearer ...")
    let auth = captured_auth.lock().unwrap().clone().expect("auth header");
    assert_eq!(
        auth, "test-token",
        "Authorization should be raw token, not Bearer"
    );
    assert!(
        !auth.starts_with("Bearer "),
        "Authorization should NOT be Bearer-prefixed"
    );

    // Assert Content-Type is application/json
    let ct = captured_ct
        .lock()
        .unwrap()
        .clone()
        .expect("content-type header");
    assert_eq!(
        ct, "application/json",
        "Content-Type should be application/json"
    );

    // Assert body contains the compiled filter in variables.filter
    let body = captured_body.lock().unwrap().clone().expect("request body");
    let body_json: serde_json::Value =
        serde_json::from_str(&body).expect("body should be valid JSON");
    let sent_filter = body_json
        .get("variables")
        .and_then(|v| v.get("filter"))
        .expect("body should have variables.filter");
    assert_eq!(
        sent_filter, &compiled_filter,
        "variables.filter should match the compiled filter"
    );
}
