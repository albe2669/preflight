//! GitHub pull stub and PR upsert helper.

use entity::pull_request;
use entity::sea_orm_active_enums::PullRequestState;
use preflight_core::Result;
use preflight_core::clock::now_tz;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    QueryFilter, Set,
};

use crate::cursor;

/// Pull open pull requests from GitHub.
///
/// When `token` is empty the call is a no-op (cursor marked "never").
/// Otherwise this is a stub — the real HTTP body is marked `TODO(network)`.
pub async fn pull_github(db: &DatabaseConnection, token: &str, _query: &str) -> Result<()> {
    if token.is_empty() {
        cursor::put(
            db,
            "github",
            None,
            "never",
            Some("no token configured".into()),
        )
        .await?;
        return Ok(());
    }

    // TODO(network): query GitHub search API with `query` and upsert PRs.
    //
    // The real implementation will:
    //   1. Build a GitHub search query from `query`.
    //   2. Paginate results.
    //   3. Call `upsert_pr` for each PR.
    //   4. Update the cursor with the last seen value.

    cursor::put(db, "github", None, "ok", None).await?;
    Ok(())
}

/// Idempotently upsert a GitHub pull request row.
///
/// The identity key is `(provider, owner, repo, number)`.  If the row
/// already exists the mutable fields and `synced_at` are updated;
/// otherwise a new row is inserted.
pub async fn upsert_pr(
    db: &DatabaseConnection,
    provider: &str,
    owner: &str,
    repo: &str,
    number: i64,
    title: &str,
    url: &str,
    author: Option<&str>,
    state: PullRequestState,
    review_requested: bool,
    authored_by_me: bool,
) -> Result<pull_request::Model> {
    let synced = now_tz();

    let existing = pull_request::Entity::find()
        .filter(pull_request::Column::Provider.eq(provider))
        .filter(pull_request::Column::Owner.eq(owner))
        .filter(pull_request::Column::Repo.eq(repo))
        .filter(pull_request::Column::Number.eq(number))
        .one(db)
        .await?;

    match existing {
        Some(model) => {
            let mut am = model.into_active_model();
            am.title = Set(title.to_string());
            am.url = Set(url.to_string());
            am.author = Set(author.map(|s| s.to_string()));
            am.state = Set(state);
            am.review_requested = Set(review_requested);
            am.authored_by_me = Set(authored_by_me);
            am.synced_at = Set(synced);
            am.update(db).await.map_err(preflight_core::Error::from)
        }
        None => {
            let am = pull_request::ActiveModel {
                id: ActiveValue::not_set(),
                provider: Set(provider.to_string()),
                owner: Set(owner.to_string()),
                repo: Set(repo.to_string()),
                number: Set(number),
                title: Set(title.to_string()),
                url: Set(url.to_string()),
                author: Set(author.map(|s| s.to_string())),
                state: Set(state),
                review_requested: Set(review_requested),
                authored_by_me: Set(authored_by_me),
                remote_created_at: Set(None),
                remote_updated_at: Set(None),
                synced_at: Set(synced),
                dismissed_at: Set(None),
            };
            am.insert(db).await.map_err(preflight_core::Error::from)
        }
    }
}
