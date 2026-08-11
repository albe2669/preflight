//! Linear pull stub and issue upsert helper.
use crate::entity::linear_issue;
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::entity::prelude::DateTimeWithTimeZone;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    QueryFilter, Set,
};
use sync_state::entity::sync_state;

use crate::client::LinearClientAdapter;
use crate::cursor;
use crate::error::{LinearError, Result};
/// Re-export so callers construct via `linear::sync::LinearFilter`.
pub use crate::filters::LinearFilter;
use remote_sync::sync::SyncLoop;

#[async_trait]
pub trait LinearSync: Send + Sync {
    async fn pull(&self) -> Result<sync_state::Model>;
}

/// Configuration for the Linear sync provider.
#[derive(Clone, Debug, Default)]
pub struct LinearOptions {
    pub token: String,
    pub filters: Vec<LinearFilter>,
}

/// Concrete implementation — private outside the crate. The client is
/// carried as a cloneable adapter so the shared [`SyncLoop`] can drive it.
pub(crate) struct LinearSyncImpl {
    db: DatabaseConnection,
    opts: LinearOptions,
    client: LinearClientAdapter,
}

/// Constructor — returns the trait so callers cannot depend on the concrete type.
pub fn new(
    db: DatabaseConnection,
    client: impl crate::client::LinearApiClient + 'static,
    opts: LinearOptions,
) -> impl LinearSync {
    let client = LinearClientAdapter(std::sync::Arc::new(client));
    LinearSyncImpl { db, opts, client }
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
    async fn pull(&self) -> Result<sync_state::Model> {
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
                tracing::info!(
                    provider = "linear",
                    filter_summary = %filter,
                    "sync loop start"
                );

                // Buffer all issues across pages, then upsert all at once.
                // A first-page failure surfaces the actual error; a mid-page
                // failure (after a page succeeded) returns PartialResults
                // and commits nothing.
                let result = SyncLoop::new(self.client.clone()).run(&filter).await;
                match result {
                    Ok(sync_result) => {
                        let items = sync_result.items.len();
                        for issue in &sync_result.items {
                            upsert_issue(&db, issue).await?;
                        }
                        tracing::info!(
                            provider = "linear",
                            items = items,
                            cursor = sync_result.end_cursor.as_deref(),
                            "sync loop end"
                        );
                        cursor::put(&db, "linear", None, "ok", None).await?;
                    }
                    Err(remote_err) => {
                        let err: LinearError = remote_err.into();
                        if let LinearError::PartialResults = err {
                            tracing::error!(
                                provider = "linear",
                                error = %err,
                                "sync loop end"
                            );
                            cursor::put(&db, "linear", None, "error", None).await?;
                            return Err(LinearError::PartialResults);
                        }
                        tracing::error!(
                            provider = "linear",
                            error = %err,
                            "sync loop end"
                        );
                        cursor::put(&db, "linear", None, "error", Some(err.to_string())).await?;
                        return Err(err);
                    }
                }
            }
        }

        let state = sync_state::Entity::find_by_id("linear")
            .one(&db)
            .await?
            .ok_or_else(|| LinearError::NotFound)?;
        tracing::info!(
            provider = "linear",
            status = %state.last_status,
            error = state.last_error.as_deref(),
            cursor = state.cursor.as_deref(),
            "sync state saved"
        );
        Ok(state)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{LinearApiClient, LinearPage};
    use migration::MigratorTrait;
    use sea_orm::PaginatorTrait;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Shared state behind the fake client so a clone can assert call counts.
    struct FakeInner {
        pages: parking_lot::Mutex<Vec<std::result::Result<LinearPage, LinearError>>>,
        call_count: AtomicUsize,
    }

    #[derive(Clone)]
    struct FakeClient {
        inner: Arc<FakeInner>,
    }

    impl FakeClient {
        fn new(pages: Vec<std::result::Result<LinearPage, LinearError>>) -> Self {
            Self {
                inner: Arc::new(FakeInner {
                    pages: parking_lot::Mutex::new(pages),
                    call_count: AtomicUsize::new(0),
                }),
            }
        }

        fn call_count(&self) -> usize {
            self.inner.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl LinearApiClient for FakeClient {
        async fn issues_page(
            &self,
            _filter: &serde_json::Value,
            _after: Option<&str>,
        ) -> Result<LinearPage> {
            self.inner.call_count.fetch_add(1, Ordering::SeqCst);
            let mut pages = self.inner.pages.lock();
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
            client: LinearClientAdapter(fake),
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

        assert_eq!(fake.call_count(), 1);
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

        assert_eq!(fake.call_count(), 0);
    }
    // ── sync lifecycle logging tests ────────────────────────────────

    /// A capture subscriber installed once as the process default so events
    /// emitted from tokio worker threads (e.g. DB awaits inside `pull`) are
    /// captured regardless of which thread they land on. Tests clear the
    /// shared buffer before running and inspect it after.
    fn ensure_global_capture() -> Arc<parking_lot::Mutex<Vec<u8>>> {
        use std::sync::{LazyLock, OnceLock};
        static BUFFER: LazyLock<Arc<parking_lot::Mutex<Vec<u8>>>> =
            LazyLock::new(|| Arc::new(parking_lot::Mutex::new(Vec::new())));
        static INSTALLED: OnceLock<()> = OnceLock::new();

        // Install the capture subscriber once as the process default so the
        // events emitted from tokio worker threads inside `pull` are captured.
        INSTALLED.get_or_init(|| {
            let subscriber = tracing_subscriber::fmt()
                .json()
                .with_max_level(tracing_subscriber::filter::LevelFilter::TRACE)
                .with_writer(crate::client::tests::LogCaptureMakeWriter::new(
                    BUFFER.clone(),
                ))
                .with_ansi(false)
                .finish();
            tracing::subscriber::set_global_default(subscriber).expect("install global capture");
        });
        BUFFER.clone()
    }

    #[tokio::test]
    async fn test_sync_lifecycle_logs_loop_start_and_end() {
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

        let buffer = ensure_global_capture();
        buffer.lock().clear();

        let result = svc.pull().await.expect("pull should succeed");
        assert_eq!(result.last_status, "ok");

        let log_text = {
            let bytes = buffer.lock();
            String::from_utf8_lossy(&bytes).to_string()
        };

        // Assert sync lifecycle events are present.
        assert!(
            log_text.contains("sync loop start"),
            "expected sync loop start event, got: {log_text}"
        );
        assert!(
            log_text.contains("\"provider\":\"linear\""),
            "expected provider=linear, got: {log_text}"
        );
        assert!(
            log_text.contains("sync loop end"),
            "expected sync loop end event, got: {log_text}"
        );
    }
}
