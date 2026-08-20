#![cfg(feature = "integration")]

mod common;
use common::*;

use async_graphql::Value;
use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use links::entity::enums::LinkRelation;
use sea_orm::prelude::DateTimeWithTimeZone;
use std::sync::Arc;
use todo_domain::entity::enums::TodoStatus;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Construct a `DateTimeWithTimeZone` from `Utc::now()`.
fn now_tz() -> DateTimeWithTimeZone {
    Utc::now().into()
}

/// Construct a minimal `todo` Model for stub return values.
fn stub_todo(id: i64, title: &str) -> todo_domain::entity::todo::Model {
    let ts = now_tz();
    todo_domain::entity::todo::Model {
        id,
        title: title.to_string(),
        description: None,
        status: TodoStatus::Todo,
        blocked_reason: None,
        sort_key: 0,
        created_at: ts,
        updated_at: ts,
        started_at: None,
        closed_at: None,
    }
}

/// Construct a minimal `todo_day_plan` Model for stub return values.
fn stub_day_plan(id: i64, todo_id: i64) -> todo_domain::entity::todo_day_plan::Model {
    todo_domain::entity::todo_day_plan::Model {
        id,
        plan_date: NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
        todo_id,
        position: 0,
        carried_over: false,
        added_at: now_tz(),
        removed_at: None,
    }
}

/// Construct a minimal `sync_state` Model for stub return values.
fn stub_sync_state(source: &str) -> sync_state::entity::sync_state::Model {
    sync_state::entity::sync_state::Model {
        source: source.to_string(),
        cursor: None,
        last_synced_at: Some(now_tz()),
        last_status: "ok".to_string(),
        last_error: None,
    }
}

/// Build a GraphQL schema with the given mocked service Arcs.
async fn build_schema(
    db: sea_orm::DatabaseConnection,
    todo: Arc<dyn todo_domain::TodoService>,
    day_plan: Arc<dyn todo_domain::DayPlanService>,
    link: Arc<dyn links::LinkService>,
    review: Arc<dyn todo_domain::ReviewService>,
    github: Arc<dyn github::GithubSync>,
    linear: Arc<dyn linear::LinearSync>,
) -> async_graphql::dynamic::Schema {
    graphql::schema_builder(
        db,
        todo,
        day_plan,
        link,
        review,
        github,
        linear,
        todo_domain::Clock::new(chrono_tz::America::Los_Angeles, 4),
        None,
        None,
    )
    .finish()
    .expect("schema build failed")
}

/// Execute a GraphQL query / mutation and return the response.
async fn execute(schema: &async_graphql::dynamic::Schema, query: &str) -> async_graphql::Response {
    schema.execute(query).await
}

// ---------------------------------------------------------------------------
// Stub services — return fixed data
// ---------------------------------------------------------------------------

struct StubTodoService {
    todo: todo_domain::entity::todo::Model,
}

#[async_trait]
impl todo_domain::TodoService for StubTodoService {
    async fn get(&self, _id: i64) -> todo_domain::Result<todo_domain::entity::todo::Model> {
        Ok(self.todo.clone())
    }
    async fn create(
        &self,
        title: String,
        description: Option<String>,
    ) -> todo_domain::Result<todo_domain::entity::todo::Model> {
        let mut t = self.todo.clone();
        t.title = title;
        t.description = description;
        Ok(t)
    }
    async fn update(
        &self,
        _id: i64,
        title: Option<String>,
        description: Option<String>,
    ) -> todo_domain::Result<todo_domain::entity::todo::Model> {
        let mut t = self.todo.clone();
        if let Some(title) = title {
            t.title = title;
        }
        t.description = description;
        Ok(t)
    }
    async fn set_status(
        &self,
        _id: i64,
        status: TodoStatus,
        blocked_reason: Option<String>,
    ) -> todo_domain::Result<todo_domain::entity::todo::Model> {
        let mut t = self.todo.clone();
        t.status = status;
        t.blocked_reason = blocked_reason;
        Ok(t)
    }
    async fn all(&self) -> todo_domain::Result<Vec<todo_domain::entity::todo::Model>> {
        Ok(vec![self.todo.clone()])
    }
    async fn find_by_title(
        &self,
        _title: &str,
    ) -> todo_domain::Result<Option<todo_domain::entity::todo::Model>> {
        Ok(Some(self.todo.clone()))
    }
}

struct StubDayPlanService {
    plan: todo_domain::entity::todo_day_plan::Model,
}

#[async_trait]
impl todo_domain::DayPlanService for StubDayPlanService {
    async fn plan(
        &self,
        _todo_id: i64,
        _date: NaiveDate,
    ) -> todo_domain::Result<todo_domain::entity::todo_day_plan::Model> {
        Ok(self.plan.clone())
    }
    async fn plan_today(
        &self,
        _todo_id: i64,
    ) -> todo_domain::Result<todo_domain::entity::todo_day_plan::Model> {
        Ok(self.plan.clone())
    }
    async fn unplan_today(&self, _todo_id: i64) -> todo_domain::Result<()> {
        Ok(())
    }
    async fn reorder(
        &self,
        _date: NaiveDate,
        _todo_ids: &[i64],
    ) -> todo_domain::Result<Vec<todo_domain::entity::todo_day_plan::Model>> {
        Ok(vec![self.plan.clone()])
    }
    async fn carry_over(
        &self,
        _from: NaiveDate,
        _to: NaiveDate,
    ) -> todo_domain::Result<Vec<todo_domain::entity::todo_day_plan::Model>> {
        Ok(vec![self.plan.clone()])
    }
    async fn list_for_date(
        &self,
        _date: NaiveDate,
    ) -> todo_domain::Result<Vec<todo_domain::entity::todo_day_plan::Model>> {
        Ok(vec![self.plan.clone()])
    }
}

struct StubReviewService {
    review: todo_domain::DailyReview,
}

#[async_trait]
impl todo_domain::ReviewService for StubReviewService {
    async fn daily(&self, _date: NaiveDate) -> todo_domain::Result<todo_domain::DailyReview> {
        Ok(self.review.clone())
    }
}

struct StubLinkService {
    todo: todo_domain::entity::todo::Model,
    pr: github::entity::pull_request::Model,
}

#[async_trait]
impl links::LinkService for StubLinkService {
    async fn todo_from_pr(
        &self,
        _pr_id: i64,
        _plan_today: bool,
    ) -> links::Result<todo_domain::entity::todo::Model> {
        Ok(self.todo.clone())
    }
    async fn link_pr(
        &self,
        _todo_id: i64,
        _pr_id: i64,
        _relation: LinkRelation,
    ) -> links::Result<todo_domain::entity::todo::Model> {
        Ok(self.todo.clone())
    }
    async fn dismiss_pr(&self, _pr_id: i64) -> links::Result<github::entity::pull_request::Model> {
        Ok(self.pr.clone())
    }
    async fn todo_from_linear(
        &self,
        _linear_issue_id: i64,
        _plan_today: bool,
    ) -> links::Result<todo_domain::entity::todo::Model> {
        Ok(self.todo.clone())
    }
    async fn link_linear(
        &self,
        _todo_id: i64,
        _linear_issue_id: i64,
    ) -> links::Result<todo_domain::entity::todo::Model> {
        Ok(self.todo.clone())
    }
    async fn add_tag(
        &self,
        _todo_id: i64,
        _slug: &str,
    ) -> links::Result<todo_domain::entity::todo::Model> {
        Ok(self.todo.clone())
    }
    async fn remove_tag(
        &self,
        _todo_id: i64,
        _slug: &str,
    ) -> links::Result<todo_domain::entity::todo::Model> {
        Ok(self.todo.clone())
    }
    async fn unlink_pr(
        &self,
        _todo_id: i64,
        _pr_id: i64,
    ) -> links::Result<todo_domain::entity::todo::Model> {
        Ok(self.todo.clone())
    }
    async fn unlink_linear(
        &self,
        _todo_id: i64,
        _linear_issue_id: i64,
    ) -> links::Result<todo_domain::entity::todo::Model> {
        Ok(self.todo.clone())
    }
}

struct StubGithubSync {
    state: sync_state::entity::sync_state::Model,
}

#[async_trait]
impl github::GithubSync for StubGithubSync {
    async fn pull(&self) -> github::Result<sync_state::entity::sync_state::Model> {
        Ok(self.state.clone())
    }
}

struct StubLinearSync {
    state: sync_state::entity::sync_state::Model,
}

#[async_trait]
impl linear::LinearSync for StubLinearSync {
    async fn pull(&self) -> linear::Result<sync_state::entity::sync_state::Model> {
        Ok(self.state.clone())
    }
}

// ---------------------------------------------------------------------------
// Failing services — always return an error
// ---------------------------------------------------------------------------

struct FailingTodoService;

#[async_trait]
impl todo_domain::TodoService for FailingTodoService {
    async fn get(&self, _id: i64) -> todo_domain::Result<todo_domain::entity::todo::Model> {
        Err(todo_domain::Error::NotFound("todo".into()))
    }
    async fn create(
        &self,
        _title: String,
        _description: Option<String>,
    ) -> todo_domain::Result<todo_domain::entity::todo::Model> {
        Err(todo_domain::Error::NotFound("todo".into()))
    }
    async fn update(
        &self,
        _id: i64,
        _title: Option<String>,
        _description: Option<String>,
    ) -> todo_domain::Result<todo_domain::entity::todo::Model> {
        Err(todo_domain::Error::NotFound("todo".into()))
    }
    async fn set_status(
        &self,
        _id: i64,
        _status: TodoStatus,
        _blocked_reason: Option<String>,
    ) -> todo_domain::Result<todo_domain::entity::todo::Model> {
        Err(todo_domain::Error::NotFound("todo".into()))
    }
    async fn all(&self) -> todo_domain::Result<Vec<todo_domain::entity::todo::Model>> {
        Err(todo_domain::Error::NotFound("todo".into()))
    }
    async fn find_by_title(
        &self,
        _title: &str,
    ) -> todo_domain::Result<Option<todo_domain::entity::todo::Model>> {
        Err(todo_domain::Error::NotFound("todo".into()))
    }
}

struct FailingDayPlanService;

#[async_trait]
impl todo_domain::DayPlanService for FailingDayPlanService {
    async fn plan(
        &self,
        _todo_id: i64,
        _date: NaiveDate,
    ) -> todo_domain::Result<todo_domain::entity::todo_day_plan::Model> {
        Err(todo_domain::Error::NotFound("plan".into()))
    }
    async fn plan_today(
        &self,
        _todo_id: i64,
    ) -> todo_domain::Result<todo_domain::entity::todo_day_plan::Model> {
        Err(todo_domain::Error::NotFound("plan".into()))
    }
    async fn unplan_today(&self, _todo_id: i64) -> todo_domain::Result<()> {
        Err(todo_domain::Error::NotFound("plan".into()))
    }
    async fn reorder(
        &self,
        _date: NaiveDate,
        _todo_ids: &[i64],
    ) -> todo_domain::Result<Vec<todo_domain::entity::todo_day_plan::Model>> {
        Err(todo_domain::Error::NotFound("plan".into()))
    }
    async fn carry_over(
        &self,
        _from: NaiveDate,
        _to: NaiveDate,
    ) -> todo_domain::Result<Vec<todo_domain::entity::todo_day_plan::Model>> {
        Err(todo_domain::Error::NotFound("plan".into()))
    }
    async fn list_for_date(
        &self,
        _date: NaiveDate,
    ) -> todo_domain::Result<Vec<todo_domain::entity::todo_day_plan::Model>> {
        Err(todo_domain::Error::NotFound("plan".into()))
    }
}

struct FailingReviewService;

#[async_trait]
impl todo_domain::ReviewService for FailingReviewService {
    async fn daily(&self, _date: NaiveDate) -> todo_domain::Result<todo_domain::DailyReview> {
        Err(todo_domain::Error::NotFound("review".into()))
    }
}

struct FailingLinkService;

#[async_trait]
impl links::LinkService for FailingLinkService {
    async fn todo_from_pr(
        &self,
        _pr_id: i64,
        _plan_today: bool,
    ) -> links::Result<todo_domain::entity::todo::Model> {
        Err(links::LinkError::NotFound("link".into()))
    }
    async fn link_pr(
        &self,
        _todo_id: i64,
        _pr_id: i64,
        _relation: LinkRelation,
    ) -> links::Result<todo_domain::entity::todo::Model> {
        Err(links::LinkError::NotFound("link".into()))
    }
    async fn dismiss_pr(&self, _pr_id: i64) -> links::Result<github::entity::pull_request::Model> {
        Err(links::LinkError::NotFound("link".into()))
    }
    async fn todo_from_linear(
        &self,
        _linear_issue_id: i64,
        _plan_today: bool,
    ) -> links::Result<todo_domain::entity::todo::Model> {
        Err(links::LinkError::NotFound("link".into()))
    }
    async fn link_linear(
        &self,
        _todo_id: i64,
        _linear_issue_id: i64,
    ) -> links::Result<todo_domain::entity::todo::Model> {
        Err(links::LinkError::NotFound("link".into()))
    }
    async fn add_tag(
        &self,
        _todo_id: i64,
        _slug: &str,
    ) -> links::Result<todo_domain::entity::todo::Model> {
        Err(links::LinkError::NotFound("link".into()))
    }
    async fn remove_tag(
        &self,
        _todo_id: i64,
        _slug: &str,
    ) -> links::Result<todo_domain::entity::todo::Model> {
        Err(links::LinkError::NotFound("link".into()))
    }
    async fn unlink_pr(
        &self,
        _todo_id: i64,
        _pr_id: i64,
    ) -> links::Result<todo_domain::entity::todo::Model> {
        Err(links::LinkError::NotFound("link".into()))
    }
    async fn unlink_linear(
        &self,
        _todo_id: i64,
        _linear_issue_id: i64,
    ) -> links::Result<todo_domain::entity::todo::Model> {
        Err(links::LinkError::NotFound("link".into()))
    }
}

struct FailingGithubSync;

#[async_trait]
impl github::GithubSync for FailingGithubSync {
    async fn pull(&self) -> github::Result<sync_state::entity::sync_state::Model> {
        Err(github::GithubError::NotFound)
    }
}

struct FailingLinearSync;

#[async_trait]
impl linear::LinearSync for FailingLinearSync {
    async fn pull(&self) -> linear::Result<sync_state::entity::sync_state::Model> {
        Err(linear::LinearError::NotFound)
    }
}

// ---------------------------------------------------------------------------
// Helpers for creating full stub sets
// ---------------------------------------------------------------------------

fn stub_services() -> (
    Arc<dyn todo_domain::TodoService>,
    Arc<dyn todo_domain::DayPlanService>,
    Arc<dyn links::LinkService>,
    Arc<dyn todo_domain::ReviewService>,
    Arc<dyn github::GithubSync>,
    Arc<dyn linear::LinearSync>,
) {
    let todo = stub_todo(1, "Stub Task");
    let plan = stub_day_plan(1, 1);
    let pr_ts = now_tz();
    let pr = github::entity::pull_request::Model {
        id: 1,
        provider: "github".to_string(),
        owner: "test".to_string(),
        repo: "test".to_string(),
        number: 1,
        title: "Test PR".to_string(),
        url: "https://github.com/test/test/pull/1".to_string(),
        author: Some("test".to_string()),
        state: github::entity::enums::PullRequestState::Open,
        review_requested: false,
        authored_by_me: false,
        remote_created_at: None,
        remote_updated_at: None,
        synced_at: pr_ts,
        dismissed_at: None,
    };
    let review = todo_domain::DailyReview {
        date: NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
        planned: vec![todo.clone()],
        touched: vec![],
        completed: vec![],
        carried_over: vec![],
    };

    let link_todo = todo.clone();
    (
        Arc::new(StubTodoService { todo }),
        Arc::new(StubDayPlanService { plan }),
        Arc::new(StubLinkService {
            todo: link_todo,
            pr,
        }),
        Arc::new(StubReviewService { review }),
        Arc::new(StubGithubSync {
            state: stub_sync_state("github"),
        }),
        Arc::new(StubLinearSync {
            state: stub_sync_state("linear"),
        }),
    )
}

fn failing_services() -> (
    Arc<dyn todo_domain::TodoService>,
    Arc<dyn todo_domain::DayPlanService>,
    Arc<dyn links::LinkService>,
    Arc<dyn todo_domain::ReviewService>,
    Arc<dyn github::GithubSync>,
    Arc<dyn linear::LinearSync>,
) {
    (
        Arc::new(FailingTodoService),
        Arc::new(FailingDayPlanService),
        Arc::new(FailingLinkService),
        Arc::new(FailingReviewService),
        Arc::new(FailingGithubSync),
        Arc::new(FailingLinearSync),
    )
}

// ---------------------------------------------------------------------------
// Extract JSON from response data
// ---------------------------------------------------------------------------

fn response_json(resp: &async_graphql::Response) -> serde_json::Value {
    match &resp.data {
        Value::Object(map) => serde_json::to_value(map).unwrap_or(serde_json::Value::Null),
        other => serde_json::to_value(other).unwrap_or(serde_json::Value::Null),
    }
}

// ===========================================================================
// Tests — custom query: dailyReview
// ===========================================================================

#[tokio::test]
async fn test_daily_review_returns_review_with_planned_todos() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"{ dailyReview(date: "2026-08-01") { date planned { id title } } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["dailyReview"]["date"],
        serde_json::json!("2026-08-01"),
        "date should match"
    );
    assert_eq!(
        data["dailyReview"]["planned"][0]["id"],
        serde_json::json!(1),
        "planned todo id should be 1"
    );
    assert_eq!(
        data["dailyReview"]["planned"][0]["title"],
        serde_json::json!("Stub Task"),
        "planned todo title should match"
    );
}

#[tokio::test]
async fn test_daily_review_empty_arrays_for_unplanned_fields() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"{ dailyReview(date: "2026-08-01") { touched { id } completed { id } carriedOver { id } } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert!(
        data["dailyReview"]["touched"].is_array(),
        "touched should be an array"
    );
    assert!(
        data["dailyReview"]["touched"]
            .as_array()
            .map_or(true, |a| a.is_empty()),
        "touched should be empty"
    );
}

#[tokio::test]
async fn test_clock_returns_configured_timezone_and_day_start() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"{ clock { logicalDate timezone dayStartHour } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    // The test schema uses America/Los_Angeles, day_start_hour 4 (build_schema).
    assert_eq!(
        data["clock"]["timezone"],
        serde_json::json!("America/Los_Angeles")
    );
    assert_eq!(data["clock"]["dayStartHour"], serde_json::json!(4));
    // logicalDate is a string of the form YYYY-MM-DD.
    let date = data["clock"]["logicalDate"]
        .as_str()
        .expect("logicalDate is a string");
    assert!(
        regex_like_is_date(date),
        "logicalDate should be YYYY-MM-DD, got {date}"
    );
}

/// Trivial YYYY-MM-DD shape check (no regex crate in dev-deps).
fn regex_like_is_date(s: &str) -> bool {
    s.len() == 10
        && s.as_bytes()[4] == b'-'
        && s.as_bytes()[7] == b'-'
        && s[..4].chars().all(|c| c.is_ascii_digit())
        && s[5..7].chars().all(|c| c.is_ascii_digit())
        && s[8..10].chars().all(|c| c.is_ascii_digit())
}

#[tokio::test]
async fn test_daily_review_error_returns_graphql_error() {
    let db = setup_db().await;
    let (todo, day_plan, link, _review, github, linear) = failing_services();
    // Replace review with failing
    let review = Arc::new(FailingReviewService) as Arc<dyn todo_domain::ReviewService>;
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(&schema, r#"{ dailyReview(date: "2026-08-01") { date } }"#).await;

    assert!(
        !resp.errors.is_empty(),
        "expected an error when review service fails"
    );
}

// ===========================================================================
// Tests — custom mutation: createTodo
// ===========================================================================

#[tokio::test]
async fn test_create_todo_returns_created_todo() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { createTodo(title: "Test Task") { id title } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["createTodo"]["id"],
        serde_json::json!(1),
        "created todo id should match stub"
    );
}

#[tokio::test]
async fn test_create_todo_with_description() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { createTodo(title: "Test Task", description: "A description") { id title description } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["createTodo"]["description"],
        serde_json::json!("A description"),
        "description should be passed through"
    );
}

#[tokio::test]
async fn test_create_todo_error_returns_graphql_error() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = failing_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { createTodo(title: "Test Task") { id } }"#,
    )
    .await;

    assert!(
        !resp.errors.is_empty(),
        "expected an error when todo service fails"
    );
    // The error message should contain the domain error text
    let err_msg = &resp.errors[0].message;
    assert!(
        err_msg.contains("not found"),
        "error message should contain 'not found', got: {err_msg}"
    );
}

// ===========================================================================
// Tests — custom mutation: updateTodo
// ===========================================================================

#[tokio::test]
async fn test_update_todo_returns_updated_todo() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { updateTodo(id: 1, title: "Updated") { id title } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["updateTodo"]["title"],
        serde_json::json!("Updated"),
        "title should be updated"
    );
}

#[tokio::test]
async fn test_update_todo_error_returns_graphql_error() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = failing_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { updateTodo(id: 1, title: "Updated") { id } }"#,
    )
    .await;

    assert!(
        !resp.errors.is_empty(),
        "expected an error when todo service fails"
    );
}

// ===========================================================================
// Tests — custom mutation: setTodoStatus
// ===========================================================================

#[tokio::test]
async fn test_set_todo_status_returns_updated_todo() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { setTodoStatus(id: 1, status: done) { id status } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["setTodoStatus"]["status"],
        serde_json::json!("done"),
        "status should be done"
    );
}

#[tokio::test]
async fn test_set_todo_status_blocked_with_reason() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { setTodoStatus(id: 1, status: blocked, blockedReason: "Waiting on API") { id status blockedReason } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["setTodoStatus"]["status"],
        serde_json::json!("blocked"),
        "status should be blocked"
    );
    assert_eq!(
        data["setTodoStatus"]["blockedReason"],
        serde_json::json!("Waiting on API"),
        "blockedReason should be set"
    );
}

#[tokio::test]
async fn test_set_todo_status_error_returns_graphql_error() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = failing_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { setTodoStatus(id: 1, status: done) { id } }"#,
    )
    .await;

    assert!(
        !resp.errors.is_empty(),
        "expected an error when todo service fails"
    );
}

// ===========================================================================
// Tests — custom mutation: planForToday
// ===========================================================================

#[tokio::test]
async fn test_plan_for_today_returns_day_plan() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { planForToday(todoId: 1) { id todoId position } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["planForToday"]["id"],
        serde_json::json!(1),
        "day plan id should match"
    );
}

#[tokio::test]
async fn test_plan_for_today_error_returns_graphql_error() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = failing_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(&schema, r#"mutation { planForToday(todoId: 1) { id } }"#).await;

    assert!(
        !resp.errors.is_empty(),
        "expected an error when day plan service fails"
    );
}

// ===========================================================================
// Tests — custom mutation: unplanForToday
// ===========================================================================

#[tokio::test]
async fn test_unplan_for_today_returns_true() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(&schema, r#"mutation { unplanForToday(todoId: 1) }"#).await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["unplanForToday"],
        serde_json::json!(true),
        "unplanForToday should return true"
    );
}

#[tokio::test]
async fn test_unplan_for_today_error_returns_graphql_error() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = failing_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(&schema, r#"mutation { unplanForToday(todoId: 1) }"#).await;

    assert!(
        !resp.errors.is_empty(),
        "expected an error when day plan service fails"
    );
}

// ===========================================================================
// Tests — custom mutation: reorderDayPlan
// ===========================================================================

#[tokio::test]
async fn test_reorder_day_plan_returns_plans() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { reorderDayPlan(date: "2026-08-01", todoIds: [1, 2]) { id position } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert!(
        data["reorderDayPlan"].is_array(),
        "reorderDayPlan should return an array"
    );
}

#[tokio::test]
async fn test_reorder_day_plan_error_returns_graphql_error() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = failing_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { reorderDayPlan(date: "2026-08-01", todoIds: [1]) { id } }"#,
    )
    .await;

    assert!(
        !resp.errors.is_empty(),
        "expected an error when day plan service fails"
    );
}

// ===========================================================================
// Tests — custom mutation: carryOverUnfinished
// ===========================================================================

#[tokio::test]
async fn test_carry_over_unfinished_returns_plans() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { carryOverUnfinished(from: "2026-08-01", to: "2026-08-02") { id carriedOver } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert!(
        data["carryOverUnfinished"].is_array(),
        "carryOverUnfinished should return an array"
    );
}

// ===========================================================================
// Tests — custom mutation: addTag / removeTag
// ===========================================================================

#[tokio::test]
async fn test_add_tag_returns_todo() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { addTag(todoId: 1, slug: "urgent") { id title } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["addTag"]["id"],
        serde_json::json!(1),
        "addTag should return the todo"
    );
}

#[tokio::test]
async fn test_remove_tag_returns_todo() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { removeTag(todoId: 1, slug: "urgent") { id title } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["removeTag"]["id"],
        serde_json::json!(1),
        "removeTag should return the todo"
    );
}

#[tokio::test]
async fn test_add_tag_error_returns_graphql_error() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = failing_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { addTag(todoId: 1, slug: "urgent") { id } }"#,
    )
    .await;

    assert!(
        !resp.errors.is_empty(),
        "expected an error when link service fails"
    );
}

// ===========================================================================
// Tests — custom mutation: syncGithub
// ===========================================================================

#[tokio::test]
async fn test_sync_github_returns_sync_state() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(&schema, r#"mutation { syncGithub { source lastStatus } }"#).await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["syncGithub"]["source"],
        serde_json::json!("github"),
        "source should be github"
    );
}

#[tokio::test]
async fn test_sync_github_error_returns_graphql_error() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = failing_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(&schema, r#"mutation { syncGithub { source } }"#).await;

    assert!(
        !resp.errors.is_empty(),
        "expected an error when github sync fails"
    );
}

// ===========================================================================
// Tests — custom mutation: syncLinear
// ===========================================================================

#[tokio::test]
async fn test_sync_linear_returns_sync_state() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(&schema, r#"mutation { syncLinear { source lastStatus } }"#).await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["syncLinear"]["source"],
        serde_json::json!("linear"),
        "source should be linear"
    );
}

#[tokio::test]
async fn test_sync_linear_error_returns_graphql_error() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = failing_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(&schema, r#"mutation { syncLinear { source } }"#).await;

    assert!(
        !resp.errors.is_empty(),
        "expected an error when linear sync fails"
    );
}

// ===========================================================================
// Tests — custom mutation: todoFromPullRequest
// ===========================================================================

#[tokio::test]
async fn test_todo_from_pull_request_returns_todo() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { todoFromPullRequest(pullRequestId: 1, planToday: true) { id title } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["todoFromPullRequest"]["id"],
        serde_json::json!(1),
        "should return the created todo"
    );
}

// ===========================================================================
// Tests — custom mutation: linkPullRequest
// ===========================================================================

#[tokio::test]
async fn test_link_pull_request_returns_todo() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { linkPullRequest(todoId: 1, pullRequestId: 1, relation: reviews) { id title } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["linkPullRequest"]["id"],
        serde_json::json!(1),
        "should return the linked todo"
    );
}

// ===========================================================================
// Tests — custom mutation: dismissPullRequest
// ===========================================================================

#[tokio::test]
async fn test_dismiss_pull_request_returns_pr() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { dismissPullRequest(id: 1) { id title } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["dismissPullRequest"]["id"],
        serde_json::json!(1),
        "should return the dismissed PR"
    );
}

// ===========================================================================
// Tests — custom mutation: todoFromLinearIssue
// ===========================================================================

#[tokio::test]
async fn test_todo_from_linear_issue_returns_todo() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { todoFromLinearIssue(linearIssueId: 1, planToday: false) { id title } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["todoFromLinearIssue"]["id"],
        serde_json::json!(1),
        "should return the created todo"
    );
}

// ===========================================================================
// Tests — custom mutation: linkLinearIssue
// ===========================================================================

#[tokio::test]
async fn test_link_linear_issue_returns_todo() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { linkLinearIssue(todoId: 1, linearIssueId: 1) { id title } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["linkLinearIssue"]["id"],
        serde_json::json!(1),
        "should return the linked todo"
    );
}
// ===========================================================================
// Tests — custom mutation: unlinkPullRequest
// ===========================================================================

#[tokio::test]
async fn test_unlink_pull_request_returns_todo() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { unlinkPullRequest(todoId: 1, pullRequestId: 1) { id title } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["unlinkPullRequest"]["id"],
        serde_json::json!(1),
        "should return the unlinked todo"
    );
}

// ===========================================================================
// Tests — custom mutation: unlinkLinearIssue
// ===========================================================================

#[tokio::test]
async fn test_unlink_linear_issue_returns_todo() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"mutation { unlinkLinearIssue(todoId: 1, linearIssueId: 1) { id title } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    assert_eq!(
        data["unlinkLinearIssue"]["id"],
        serde_json::json!(1),
        "should return the unlinked todo"
    );
}

// ===========================================================================
// Tests — Error mapping: domain errors become GraphQL errors
// ===========================================================================

#[tokio::test]
async fn test_error_mapping_domain_error_becomes_graphql_error() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = failing_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(&schema, r#"mutation { createTodo(title: "Test") { id } }"#).await;

    assert!(
        !resp.errors.is_empty(),
        "domain errors must produce GraphQL errors"
    );
    let err = &resp.errors[0];
    assert!(
        !err.message.is_empty(),
        "GraphQL error message must not be empty"
    );
}

// ===========================================================================
// Tests — Schema introspection: types are registered
// ===========================================================================

#[tokio::test]
async fn test_schema_introspection_has_todo_type() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(&schema, r#"{ __schema { types { name } } }"#).await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    let types: Vec<&str> = data["__schema"]["types"]
        .as_array()
        .expect("types should be an array")
        .iter()
        .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
        .collect();
    assert!(
        types.iter().any(|t| t.contains("Todo")),
        "Todo type should be registered in schema, found: {:?}",
        types
    );
}

#[tokio::test]
async fn test_schema_introspection_has_mutation_type() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"{ __schema { mutationType { name fields { name } } } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    let fields: Vec<&str> = data["__schema"]["mutationType"]["fields"]
        .as_array()
        .expect("fields should be an array")
        .iter()
        .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
        .collect();

    assert!(
        fields.contains(&"createTodo"),
        "createTodo mutation should be registered, found: {:?}",
        fields
    );
    assert!(
        fields.contains(&"planForToday"),
        "planForToday mutation should be registered, found: {:?}",
        fields
    );
    assert!(
        fields.contains(&"syncGithub"),
        "syncGithub mutation should be registered, found: {:?}",
        fields
    );
    assert!(
        fields.contains(&"syncLinear"),
        "syncLinear mutation should be registered, found: {:?}",
        fields
    );
}

#[tokio::test]
async fn test_schema_introspection_has_query_type() {
    let db = setup_db().await;
    let (todo, day_plan, link, review, github, linear) = stub_services();
    let schema = build_schema(db, todo, day_plan, link, review, github, linear).await;

    let resp = execute(
        &schema,
        r#"{ __schema { queryType { name fields { name } } } }"#,
    )
    .await;

    assert!(resp.errors.is_empty(), "errors: {:#?}", resp.errors);
    let data = response_json(&resp);
    let fields: Vec<&str> = data["__schema"]["queryType"]["fields"]
        .as_array()
        .expect("fields should be an array")
        .iter()
        .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
        .collect();

    assert!(
        fields.contains(&"dailyReview"),
        "dailyReview query should be registered, found: {:?}",
        fields
    );
}
