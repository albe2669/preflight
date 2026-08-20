//! GitHub pull loop and PR upsert helper.

use std::sync::Arc;

use crate::client::{GithubApiClient, GithubClientAdapter, fetched_to_record};
use crate::entity::enums::PullRequestState;
use crate::entity::pull_request;
use crate::filters::{apply_draft_policy, compile_github_query};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    QueryFilter, Set,
};
use sync_state::entity::sync_state;

use crate::cursor;
use crate::error::{GithubError, Result};
use remote_sync::sync::SyncLoop;

#[async_trait]
pub trait GithubSync: Send + Sync {
    async fn pull(&self) -> Result<sync_state::Model>;
}

/// Configuration for the GitHub sync provider.
#[derive(Clone, Debug, Default)]
pub struct GithubOptions {
    pub token: String,
    pub filters: Vec<crate::filters::GithubFilter>,
    pub exclude_drafts_unless_authored_by_me: bool,
}

pub use crate::filters::GithubFilter;

/// Concrete implementation — private outside the crate. The client is
/// carried as a cloneable adapter so the shared [`SyncLoop`] can drive it.
pub(crate) struct GithubSyncImpl {
    db: DatabaseConnection,
    client: GithubClientAdapter,
    opts: GithubOptions,
}

/// Constructor — returns the trait so callers cannot depend on the concrete type.
pub fn new(
    db: DatabaseConnection,
    client: Arc<dyn GithubApiClient>,
    opts: GithubOptions,
) -> impl GithubSync {
    GithubSyncImpl {
        db,
        client: GithubClientAdapter(client),
        opts,
    }
}

#[async_trait]
impl GithubSync for GithubSyncImpl {
    async fn pull(&self) -> Result<sync_state::Model> {
        let db = self.db.clone();
        let token = self.opts.token.clone();

        if token.is_empty() {
            cursor::put(
                &db,
                "github",
                None,
                "never",
                Some("no token configured".into()),
            )
            .await?;
            return sync_state::Entity::find_by_id("github")
                .one(&db)
                .await?
                .ok_or_else(|| GithubError::NotFound);
        }

        let query = compile_github_query(&self.opts.filters);
        tracing::debug!(query = %query, "compiled github sync query");

        // Empty filters → no-op, mark ok
        if query.is_empty() {
            cursor::put(&db, "github", None, "ok", None).await?;
            return sync_state::Entity::find_by_id("github")
                .one(&db)
                .await?
                .ok_or_else(|| GithubError::NotFound);
        }

        tracing::info!(
            provider = "github",
            filter_summary = %query,
            "sync loop start"
        );

        // Buffer all PRs across pages before upserting.
        // A first-page failure surfaces the actual error (Unauthorized,
        // RateLimited, Remote, SchemaMismatch). A mid-page failure (after
        // at least one page succeeded) returns PartialResults — the
        // buffered batch is discarded without committing.
        let result = SyncLoop::new(self.client.clone()).run(&query).await;
        match result {
            Ok(sync_result) => {
                // Post-fetch filters
                let filtered = apply_draft_policy(
                    sync_result.items,
                    &self.opts.filters,
                    self.opts.exclude_drafts_unless_authored_by_me,
                );

                let items = filtered.len();

                // Upsert each PR
                for fetched in &filtered {
                    let rec = fetched_to_record(fetched);
                    upsert_pr(&db, &rec).await?;
                }

                tracing::info!(
                    provider = "github",
                    items = items,
                    cursor = sync_result.end_cursor.as_deref(),
                    "sync loop end"
                );
                cursor::put(&db, "github", sync_result.end_cursor, "ok", None).await?;
            }
            Err(remote_err) => {
                let err: GithubError = remote_err.into();
                tracing::error!(
                    provider = "github",
                    error = %err,
                    "sync loop end"
                );
                cursor::put(&db, "github", None, "error", Some(err.to_string())).await?;
                return if let GithubError::PartialResults = err {
                    Err(GithubError::PartialResults)
                } else {
                    Err(err)
                };
            }
        }

        let state = sync_state::Entity::find_by_id("github")
            .one(&db)
            .await?
            .ok_or_else(|| GithubError::NotFound)?;
        tracing::info!(
            provider = "github",
            status = %state.last_status,
            error = state.last_error.as_deref(),
            cursor = state.cursor.as_deref(),
            "sync state saved"
        );
        Ok(state)
    }
}

/// Record for upserting a GitHub pull request.
#[derive(Clone, Debug)]
pub struct PrRecord {
    pub provider: String,
    pub owner: String,
    pub repo: String,
    pub number: i64,
    pub title: String,
    pub url: String,
    pub author: Option<String>,
    pub state: PullRequestState,
    pub review_requested: bool,
    pub authored_by_me: bool,
    pub remote_created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub remote_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub changes_requested: bool,
    pub copilot_comments: bool,
    pub merge_conflicts: bool,
}

/// Idempotently upsert a GitHub pull request row.
///
/// The identity key is `(provider, owner, repo, number)`.  If the row
/// already exists the mutable fields and `synced_at` are updated;
/// otherwise a new row is inserted.
pub async fn upsert_pr(db: &DatabaseConnection, rec: &PrRecord) -> Result<pull_request::Model> {
    let synced = Utc::now().into();

    let existing = pull_request::Entity::find()
        .filter(pull_request::Column::Provider.eq(&rec.provider))
        .filter(pull_request::Column::Owner.eq(&rec.owner))
        .filter(pull_request::Column::Repo.eq(&rec.repo))
        .filter(pull_request::Column::Number.eq(rec.number))
        .one(db)
        .await?;

    match existing {
        Some(model) => {
            let mut am = model.into_active_model();
            am.title = Set(rec.title.to_string());
            am.url = Set(rec.url.to_string());
            am.author = Set(rec.author.as_deref().map(|s| s.to_string()));
            am.state = Set(rec.state.clone());
            am.review_requested = Set(rec.review_requested);
            am.authored_by_me = Set(rec.authored_by_me);
            am.changes_requested = Set(rec.changes_requested);
            am.copilot_comments = Set(rec.copilot_comments);
            am.merge_conflicts = Set(rec.merge_conflicts);
            am.remote_updated_at = Set(rec.remote_updated_at.map(|dt| dt.into()));
            am.synced_at = Set(synced);
            Ok(am.update(db).await?)
        }
        None => {
            let am = pull_request::ActiveModel {
                id: ActiveValue::not_set(),
                provider: Set(rec.provider.to_string()),
                owner: Set(rec.owner.to_string()),
                repo: Set(rec.repo.to_string()),
                number: Set(rec.number),
                title: Set(rec.title.to_string()),
                url: Set(rec.url.to_string()),
                author: Set(rec.author.as_deref().map(|s| s.to_string())),
                state: Set(rec.state.clone()),
                review_requested: Set(rec.review_requested),
                authored_by_me: Set(rec.authored_by_me),
                changes_requested: Set(rec.changes_requested),
                copilot_comments: Set(rec.copilot_comments),
                merge_conflicts: Set(rec.merge_conflicts),
                remote_created_at: Set(rec.remote_created_at.map(|dt| dt.into())),
                remote_updated_at: Set(rec.remote_updated_at.map(|dt| dt.into())),
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
    use crate::client::{GithubApiClient, GithubPage};
    use crate::filters::FetchedPr;
    use migration::MigratorTrait;
    use parking_lot::Mutex;
    use sea_orm::{EntityTrait, PaginatorTrait};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeClient {
        pages: Arc<Mutex<Vec<GithubPage>>>,
        call_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl GithubApiClient for FakeClient {
        async fn search_page(
            &self,
            _query: &str,
            _after: Option<&str>,
        ) -> crate::error::Result<GithubPage> {
            let idx = self.call_count.fetch_add(1, Ordering::Relaxed);
            let pages = self.pages.lock();
            Ok(pages[idx].clone())
        }
    }

    struct FailingClient {
        pages_before_fail: Arc<Mutex<Vec<GithubPage>>>,
        call_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl GithubApiClient for FailingClient {
        async fn search_page(
            &self,
            _query: &str,
            _after: Option<&str>,
        ) -> crate::error::Result<GithubPage> {
            let idx = self.call_count.fetch_add(1, Ordering::Relaxed);
            let pages = self.pages_before_fail.lock();
            if idx < pages.len() {
                Ok(pages[idx].clone())
            } else {
                Err(GithubError::Remote("boom".into()))
            }
        }
    }

    fn fetched_pr(number: i64, is_draft: bool, authored_by_me: bool) -> FetchedPr {
        FetchedPr {
            repo_owner: "org".into(),
            repo_name: "repo".into(),
            number,
            title: format!("PR #{number}"),
            url: format!("https://github.com/org/repo/pull/{number}"),
            author_login: Some("alice".into()),
            state: "open".into(),
            is_draft,
            review_requested: false,
            requested_reviewer_teams: vec![],
            authored_by_me,
            remote_created_at: None,
            remote_updated_at: None,
            changes_requested: false,
            copilot_comments: false,
            merge_conflicts: false,
        }
    }

    async fn setup_db() -> DatabaseConnection {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        migration::Migrator::up(&db, None).await.unwrap();
        db
    }

    // Task 6.3 — single page → ok
    #[tokio::test]
    async fn test_pull_single_page_ok() {
        let db = setup_db().await;

        let page = GithubPage {
            prs: vec![fetched_pr(42, false, false)],
            end_cursor: None,
            has_next_page: false,
        };
        let client = FakeClient {
            pages: Arc::new(Mutex::new(vec![page])),
            call_count: Arc::new(AtomicUsize::new(0)),
        };

        let sync = new(
            db.clone(),
            Arc::new(client),
            GithubOptions {
                token: "tok".into(),
                filters: vec![GithubFilter {
                    repo: Some("org/repo".into()),
                    ..Default::default()
                }],
                exclude_drafts_unless_authored_by_me: false,
            },
        );
        let state = sync.pull().await.unwrap();

        assert_eq!(state.source, "github");
        assert_eq!(state.last_status, "ok");

        let count = pull_request::Entity::find().count(&db).await.unwrap();
        assert_eq!(count, 1);
    }

    // Task 6.1 — two pages → ok
    #[tokio::test]
    async fn test_pull_two_pages_ok() {
        let db = setup_db().await;

        let client = FakeClient {
            pages: Arc::new(Mutex::new(vec![
                GithubPage {
                    prs: vec![fetched_pr(1, false, false)],
                    end_cursor: Some("X".into()),
                    has_next_page: true,
                },
                GithubPage {
                    prs: vec![fetched_pr(2, false, false)],
                    end_cursor: None,
                    has_next_page: false,
                },
            ])),
            call_count: Arc::new(AtomicUsize::new(0)),
        };

        let sync = new(
            db.clone(),
            Arc::new(client),
            GithubOptions {
                token: "tok".into(),
                filters: vec![GithubFilter {
                    repo: Some("org/repo".into()),
                    ..Default::default()
                }],
                exclude_drafts_unless_authored_by_me: false,
            },
        );
        let state = sync.pull().await.unwrap();

        assert_eq!(state.last_status, "ok");

        let count = pull_request::Entity::find().count(&db).await.unwrap();
        assert_eq!(count, 2);
    }

    // Task 6.2 — mid-page error → PartialResults, no commit
    #[tokio::test]
    async fn test_pull_mid_page_error_partial_results_no_commit() {
        let db = setup_db().await;

        let client = FailingClient {
            pages_before_fail: Arc::new(Mutex::new(vec![GithubPage {
                prs: vec![fetched_pr(1, false, false)],
                end_cursor: Some("X".into()),
                has_next_page: true,
            }])),
            call_count: Arc::new(AtomicUsize::new(0)),
        };

        let sync = new(
            db.clone(),
            Arc::new(client),
            GithubOptions {
                token: "tok".into(),
                filters: vec![GithubFilter {
                    repo: Some("org/repo".into()),
                    ..Default::default()
                }],
                exclude_drafts_unless_authored_by_me: false,
            },
        );
        let result = sync.pull().await;

        assert!(matches!(result, Err(GithubError::PartialResults)));

        let count = pull_request::Entity::find().count(&db).await.unwrap();
        assert_eq!(count, 0);
    }

    // Task 6.5a — no-token → never
    #[tokio::test]
    async fn test_pull_no_token_marks_never() {
        let db = setup_db().await;

        let client = FakeClient {
            pages: Arc::new(Mutex::new(vec![])),
            call_count: Arc::new(AtomicUsize::new(0)),
        };

        let sync = new(
            db.clone(),
            Arc::new(client),
            GithubOptions {
                token: String::new(),
                filters: vec![],
                exclude_drafts_unless_authored_by_me: false,
            },
        );
        let state = sync.pull().await.unwrap();

        assert_eq!(state.source, "github");
        assert_eq!(state.last_status, "never");
        assert_eq!(state.last_error, Some("no token configured".into()));
    }

    // Task 6.5b — empty filters → ok, no request
    #[tokio::test]
    async fn test_pull_empty_filters_ok_no_request() {
        let db = setup_db().await;

        let call_count = Arc::new(AtomicUsize::new(0));
        let client = FakeClient {
            pages: Arc::new(Mutex::new(vec![])),
            call_count: call_count.clone(),
        };

        let sync = new(
            db.clone(),
            Arc::new(client),
            GithubOptions {
                token: "tok".into(),
                filters: vec![],
                exclude_drafts_unless_authored_by_me: false,
            },
        );
        let state = sync.pull().await.unwrap();

        assert_eq!(state.source, "github");
        assert_eq!(state.last_status, "ok");
        assert_eq!(call_count.load(Ordering::Relaxed), 0);
    }

    // Task 6.4 — draft policy filter
    #[tokio::test]
    async fn test_pull_draft_policy_filters_out_others_drafts() {
        let db = setup_db().await;

        let page = GithubPage {
            prs: vec![
                fetched_pr(1, false, false), // non-draft, keep
                fetched_pr(2, true, true),   // draft by me, keep
                fetched_pr(3, true, false),  // draft by other, drop
            ],
            end_cursor: None,
            has_next_page: false,
        };

        let client = FakeClient {
            pages: Arc::new(Mutex::new(vec![page])),
            call_count: Arc::new(AtomicUsize::new(0)),
        };

        let sync = new(
            db.clone(),
            Arc::new(client),
            GithubOptions {
                token: "tok".into(),
                filters: vec![GithubFilter {
                    repo: Some("org/repo".into()),
                    ..Default::default()
                }],
                exclude_drafts_unless_authored_by_me: true,
            },
        );
        let state = sync.pull().await.unwrap();

        assert_eq!(state.last_status, "ok");
        let count = pull_request::Entity::find().count(&db).await.unwrap();
        assert_eq!(count, 2);
    }

    // ── sync lifecycle logging tests ────────────────────────────────

    /// A capture subscriber installed once as the process default so events
    /// emitted from tokio worker threads (e.g. DB awaits inside `pull`) are
    /// captured regardless of which thread they land on. Tests clear the
    /// shared buffer before running and inspect it after.
    fn ensure_global_capture() -> Arc<Mutex<Vec<u8>>> {
        use std::sync::{LazyLock, OnceLock};
        static BUFFER: LazyLock<Arc<Mutex<Vec<u8>>>> =
            LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));
        static INSTALLED: OnceLock<()> = OnceLock::new();

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

        let page = GithubPage {
            prs: vec![fetched_pr(42, false, false)],
            end_cursor: None,
            has_next_page: false,
        };
        let client = FakeClient {
            pages: Arc::new(Mutex::new(vec![page])),
            call_count: Arc::new(AtomicUsize::new(0)),
        };

        let sync = new(
            db.clone(),
            Arc::new(client),
            GithubOptions {
                token: "tok".into(),
                filters: vec![GithubFilter {
                    repo: Some("org/repo".into()),
                    ..Default::default()
                }],
                exclude_drafts_unless_authored_by_me: false,
            },
        );

        let buffer = ensure_global_capture();
        buffer.lock().clear();

        let result = sync.pull().await.expect("pull should succeed");
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
            log_text.contains("\"provider\":\"github\""),
            "expected provider=github, got: {log_text}"
        );
        assert!(
            log_text.contains("sync loop end"),
            "expected sync loop end event, got: {log_text}"
        );
    }

    #[tokio::test]
    async fn test_upsert_pr_updates_state_on_existing_pr_transition() {
        let db = setup_db().await;

        let rec = PrRecord {
            provider: "github".into(),
            owner: "org".into(),
            repo: "repo".into(),
            number: 1,
            title: "PR #1".into(),
            url: "https://github.com/org/repo/pull/1".into(),
            author: Some("alice".into()),
            state: PullRequestState::Open,
            review_requested: false,
            authored_by_me: false,
            remote_created_at: None,
            remote_updated_at: None,
            changes_requested: false,
            copilot_comments: false,
            merge_conflicts: false,
        };
        upsert_pr(&db, &rec).await.unwrap();

        let updated = PrRecord {
            state: PullRequestState::Merged,
            changes_requested: true,
            merge_conflicts: true,
            ..rec
        };
        let model = upsert_pr(&db, &updated).await.unwrap();

        assert_eq!(model.state, PullRequestState::Merged);
        assert!(model.changes_requested);
        assert!(model.merge_conflicts);
        assert!(!model.copilot_comments);

        let count = pull_request::Entity::find().count(&db).await.unwrap();
        assert_eq!(count, 1);
    }
}
