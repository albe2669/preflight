//! GitHub pull stub and PR upsert helper.

use crate::entity::enums::PullRequestState;
use crate::entity::pull_request;
use crate::filters::compile_github_query;
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    QueryFilter, Set,
};
use sync_state::entity::sync_state;

use crate::cursor;
use crate::error::{GithubError, Result};

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

/// Concrete implementation — private outside the crate.
pub(crate) struct GithubSyncImpl {
    db: DatabaseConnection,
    opts: GithubOptions,
}

/// Constructor — returns the trait so callers cannot depend on the concrete type.
pub fn new(db: DatabaseConnection, opts: GithubOptions) -> impl GithubSync {
    GithubSyncImpl { db, opts }
}

#[async_trait]
impl GithubSync for GithubSyncImpl {
    /// Pull open pull requests from GitHub.
    ///
    /// When `token` is empty the call is a no-op (cursor marked "never").
    /// Otherwise this is a stub — the real HTTP body is marked `TODO(network)`.
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
        } else {
            let query = compile_github_query(&self.opts.filters);
            tracing::debug!(query = %query, "compiled github sync query");

            // TODO(network): query GitHub search API with the compiled query and upsert PRs.
            cursor::put(&db, "github", None, "ok", None).await?;
        }

        sync_state::Entity::find_by_id("github")
            .one(&db)
            .await?
            .ok_or_else(|| GithubError::NotFound)
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
                remote_created_at: Set(None),
                remote_updated_at: Set(None),
                synced_at: Set(synced),
                dismissed_at: Set(None),
            };
            Ok(am.insert(db).await?)
        }
    }
}
