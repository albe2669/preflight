//! Linear pull stub and issue upsert helper.
use std::sync::Arc;

use crate::entity::linear_issue;
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::entity::prelude::DateTimeWithTimeZone;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    QueryFilter, Set,
};

use crate::cursor;
use crate::error::{LinearError, Result};
/// Re-export so callers construct via `linear::sync::LinearFilter`.
pub use crate::filters::LinearFilter;

#[async_trait]
pub trait LinearSync: Send + Sync {
    async fn pull(&self) -> Result<sync_state::entity::sync_state::Model>;
}

/// Configuration for the Linear sync provider.
#[derive(Clone, Debug, Default)]
pub struct LinearOptions {
    pub token: String,
    pub filters: Vec<LinearFilter>,
}

/// Concrete implementation — private outside the crate.
pub(crate) struct LinearSyncImpl {
    db: DatabaseConnection,
    opts: LinearOptions,
    client: Arc<dyn crate::client::LinearApiClient>,
}

/// Constructor — returns the trait so callers cannot depend on the concrete type.
pub fn new(
    db: DatabaseConnection,
    client: impl crate::client::LinearApiClient + 'static,
    opts: LinearOptions,
) -> impl LinearSync {
    LinearSyncImpl {
        db,
        opts,
        client: Arc::new(client),
    }
}

#[async_trait]
impl LinearSync for LinearSyncImpl {
    /// Pull issues from Linear.
    ///
    /// When `token` is empty the call is a no-op (cursor marked "never").
    /// When filters are empty, returns ok without fetching.
    /// Otherwise paginates through all issues, buffers them, and upserts
    /// all at once. On mid-page error, returns PartialResults with no
    /// database changes.
    async fn pull(&self) -> Result<sync_state::entity::sync_state::Model> {
        let db = self.db.clone();
        let token = self.opts.token.clone();

        if token.is_empty() {
            cursor::put(
                &db,
                "linear",
                None,
                "never",
                Some("no token configured".into()),
            )
            .await?;
        } else {
            let filter = crate::filters::compile_linear_filter(&self.opts.filters);
            tracing::debug!(filter = %filter, "compiled linear sync filter");

            // If filter is null (no filters configured), skip fetch
            if filter.is_null() {
                cursor::put(&db, "linear", None, "ok", None).await?;
            } else {
                // Buffer all issues across pages, then upsert all at once.
                // A first-page failure surfaces the actual error; a mid-page
                // failure (after a page succeeded) returns PartialResults
                // and commits nothing.
                let result = self.fetch_all_pages(&filter).await;
                match result {
                    Ok(all_issues) => {
                        for issue in &all_issues {
                            upsert_issue(&db, issue).await?;
                        }
                        cursor::put(&db, "linear", None, "ok", None).await?;
                    }
                    Err(LinearError::PartialResults) => {
                        cursor::put(&db, "linear", None, "error", None).await?;
                        return Err(LinearError::PartialResults);
                    }
                    Err(err) => {
                        cursor::put(&db, "linear", None, "error", Some(err.to_string())).await?;
                        return Err(err);
                    }
                }
            }
        }

        sync_state::entity::sync_state::Entity::find_by_id("linear")
            .one(&db)
            .await?
            .ok_or_else(|| LinearError::NotFound)
    }
}

/// Record for upserting a Linear issue.
#[derive(Clone, Debug)]
pub struct IssueRecord {
    pub linear_id: String,
    pub identifier: String,
    pub title: String,
    pub description: Option<String>,
    pub url: String,
    pub state_name: String,
    pub state_type: String,
    pub priority: Option<i64>,
    pub team_key: Option<String>,
    pub assignee_name: Option<String>,
    pub assigned_to_me: bool,
    pub remote_created_at: Option<DateTimeWithTimeZone>,
    pub remote_updated_at: Option<DateTimeWithTimeZone>,
}

/// Idempotently upsert a Linear issue row.
///
/// The identity key is `linear_id`.  If the row already exists the mutable
/// fields and `synced_at` are updated; otherwise a new row is inserted.
pub async fn upsert_issue(
    db: &DatabaseConnection,
    rec: &IssueRecord,
) -> Result<linear_issue::Model> {
    let synced = Utc::now().into();

    let existing = linear_issue::Entity::find()
        .filter(linear_issue::Column::LinearId.eq(&rec.linear_id))
        .one(db)
        .await?;

    match existing {
        Some(model) => {
            let mut am = model.into_active_model();
            am.identifier = Set(rec.identifier.to_string());
            am.title = Set(rec.title.to_string());
            am.description = Set(rec.description.as_deref().map(|s| s.to_string()));
            am.url = Set(rec.url.to_string());
            am.state_name = Set(rec.state_name.to_string());
            am.state_type = Set(rec.state_type.to_string());
            am.priority = Set(rec.priority);
            am.team_key = Set(rec.team_key.as_deref().map(|s| s.to_string()));
            am.assignee_name = Set(rec.assignee_name.as_deref().map(|s| s.to_string()));
            am.assigned_to_me = Set(rec.assigned_to_me);
            am.remote_updated_at = Set(rec.remote_updated_at);
            am.synced_at = Set(synced);
            Ok(am.update(db).await?)
        }
        None => {
            let am = linear_issue::ActiveModel {
                id: ActiveValue::not_set(),
                linear_id: Set(rec.linear_id.to_string()),
                identifier: Set(rec.identifier.to_string()),
                title: Set(rec.title.to_string()),
                description: Set(rec.description.as_deref().map(|s| s.to_string())),
                url: Set(rec.url.to_string()),
                state_name: Set(rec.state_name.to_string()),
                state_type: Set(rec.state_type.to_string()),
                priority: Set(rec.priority),
                team_key: Set(rec.team_key.as_deref().map(|s| s.to_string())),
                assignee_name: Set(rec.assignee_name.as_deref().map(|s| s.to_string())),
                assigned_to_me: Set(rec.assigned_to_me),
                remote_created_at: Set(rec.remote_created_at),
                remote_updated_at: Set(rec.remote_updated_at),
                synced_at: Set(synced),
                dismissed_at: Set(None),
            };
            Ok(am.insert(db).await?)
        }
    }
}

impl LinearSyncImpl {
    /// Fetch all pages of issues, buffering them in memory.
    ///
    /// A first-page failure returns the actual error. A mid-page failure
    /// (after at least one page succeeded) returns `PartialResults`.
    async fn fetch_all_pages(&self, filter: &serde_json::Value) -> Result<Vec<IssueRecord>> {
        let mut all_issues: Vec<IssueRecord> = Vec::new();
        let mut after: Option<String> = None;
        let mut pages_fetched = 0u32;

        loop {
            let page = match self.client.issues_page(filter, after.as_deref()).await {
                Ok(p) => p,
                Err(e) => {
                    return if pages_fetched > 0 {
                        Err(LinearError::PartialResults)
                    } else {
                        Err(e)
                    };
                }
            };
            pages_fetched += 1;
            all_issues.extend(page.issues);

            if page.has_next_page {
                after = Some(
                    page.end_cursor
                        .ok_or(LinearError::PaginationCursorInvalid)?,
                );
            } else {
                break;
            }
        }

        Ok(all_issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{LinearApiClient, LinearPage};
    use migration::MigratorTrait;
    use sea_orm::PaginatorTrait;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeClient {
        pages: parking_lot::Mutex<Vec<std::result::Result<LinearPage, LinearError>>>,
        call_count: AtomicUsize,
    }

    impl FakeClient {
        fn new(pages: Vec<std::result::Result<LinearPage, LinearError>>) -> Self {
            Self {
                pages: parking_lot::Mutex::new(pages),
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl LinearApiClient for FakeClient {
        async fn issues_page(
            &self,
            _filter: &serde_json::Value,
            _after: Option<&str>,
        ) -> Result<LinearPage> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let mut pages = self.pages.lock();
            pages
                .pop()
                .unwrap_or_else(|| Err(LinearError::Remote("no more pages".into())))
        }
    }

    fn make_page(issue_id: &str, end_cursor: Option<&str>, has_next_page: bool) -> LinearPage {
        LinearPage {
            issues: vec![IssueRecord {
                linear_id: issue_id.to_string(),
                identifier: format!("ENG-{issue_id}"),
                title: format!("Issue {issue_id}"),
                description: None,
                url: format!("https://linear.test/{issue_id}"),
                state_name: "Todo".into(),
                state_type: "triage".into(),
                priority: None,
                team_key: None,
                assignee_name: None,
                assigned_to_me: false,
                remote_created_at: None,
                remote_updated_at: None,
            }],
            end_cursor: end_cursor.map(str::to_string),
            has_next_page,
        }
    }

    async fn setup_db() -> DatabaseConnection {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("connect memory db");
        migration::Migrator::up(&db, None)
            .await
            .expect("migrations");
        db
    }

    fn build_svc(
        db: DatabaseConnection,
        fake: Arc<FakeClient>,
        opts: LinearOptions,
    ) -> LinearSyncImpl {
        LinearSyncImpl {
            db,
            opts,
            client: fake,
        }
    }

    #[tokio::test]
    async fn test_pull_two_pages_upserts_all_issues() {
        let db = setup_db().await;
        let fake = Arc::new(FakeClient::new(vec![
            Ok(make_page("issue-2", None, false)),
            Ok(make_page("issue-1", Some("X"), true)),
        ]));
        let svc = build_svc(
            db.clone(),
            fake,
            LinearOptions {
                token: "fake-token".into(),
                filters: vec![LinearFilter {
                    team: Some("ENG".into()),
                    ..Default::default()
                }],
            },
        );

        let result = svc.pull().await.expect("pull should succeed");
        assert_eq!(result.last_status, "ok");

        let count = linear_issue::Entity::find().count(&db).await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_pull_mid_page_error_returns_partial_results() {
        let db = setup_db().await;
        let fake = Arc::new(FakeClient::new(vec![
            Err(LinearError::Remote("server error".into())),
            Ok(make_page("issue-1", Some("X"), true)),
        ]));
        let svc = build_svc(
            db.clone(),
            fake,
            LinearOptions {
                token: "fake-token".into(),
                filters: vec![LinearFilter {
                    team: Some("ENG".into()),
                    ..Default::default()
                }],
            },
        );

        let result = svc.pull().await;
        assert!(matches!(result, Err(LinearError::PartialResults)));

        let count = linear_issue::Entity::find().count(&db).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_pull_single_page_upserts_and_stops() {
        let db = setup_db().await;
        let fake = Arc::new(FakeClient::new(vec![Ok(make_page("issue-1", None, false))]));
        let svc = build_svc(
            db.clone(),
            fake.clone(),
            LinearOptions {
                token: "fake-token".into(),
                filters: vec![LinearFilter {
                    team: Some("ENG".into()),
                    ..Default::default()
                }],
            },
        );

        let result = svc.pull().await.expect("pull should succeed");
        assert_eq!(result.last_status, "ok");

        let count = linear_issue::Entity::find().count(&db).await.unwrap();
        assert_eq!(count, 1);

        assert_eq!(fake.call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_pull_no_token_marks_never() {
        let db = setup_db().await;
        let fake = Arc::new(FakeClient::new(vec![]));
        let svc = build_svc(db.clone(), fake, LinearOptions::default());

        let result = svc.pull().await.expect("pull should succeed");
        assert_eq!(result.last_status, "never");
        assert_eq!(result.last_error, Some("no token configured".into()));
    }

    #[tokio::test]
    async fn test_pull_empty_filters_returns_ok_no_requests() {
        let db = setup_db().await;
        let fake = Arc::new(FakeClient::new(vec![]));
        let svc = build_svc(
            db.clone(),
            fake.clone(),
            LinearOptions {
                token: "fake-token".into(),
                filters: vec![],
            },
        );

        let result = svc.pull().await.expect("pull should succeed");
        assert_eq!(result.last_status, "ok");

        assert_eq!(fake.call_count.load(Ordering::SeqCst), 0);
    }
}
