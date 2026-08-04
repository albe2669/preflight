//! Linear pull stub and issue upsert helper.
use async_trait::async_trait;
use chrono::Utc;
use entity::linear_issue;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    QueryFilter, Set,
};

use crate::cursor;
use crate::error::{Result, SyncError};

#[async_trait]
pub trait LinearSync: Send + Sync {
    async fn pull(&self) -> Result<entity::sync_state::Model>;
}

/// Configuration for the Linear sync provider.
#[derive(Clone, Debug, Default)]
pub struct LinearOptions {
    pub token: String,
    pub team_keys: Vec<String>,
}

/// Concrete implementation — private outside the crate.
pub(crate) struct LinearSyncImpl {
    db: DatabaseConnection,
    opts: LinearOptions,
}

/// Constructor — returns the trait so callers cannot depend on the concrete type.
pub fn new(db: DatabaseConnection, opts: LinearOptions) -> impl LinearSync {
    LinearSyncImpl { db, opts }
}

#[async_trait]
impl LinearSync for LinearSyncImpl {
    /// Pull issues from Linear.
    ///
    /// When `token` is empty the call is a no-op (cursor marked "never").
    /// Otherwise this is a stub — the real HTTP body is marked `TODO(network)`.
    async fn pull(&self) -> Result<entity::sync_state::Model> {
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
            // TODO(network): query Linear GraphQL API.
            //
            // The real implementation will:
            //   1. Build a GraphQL query for the requested team keys.
            //   2. Paginate results.
            //   3. Call `upsert_issue` for each issue.
            //   4. Update the cursor with the last seen value.

            cursor::put(&db, "linear", None, "ok", None).await?;
        }

        entity::sync_state::Entity::find_by_id("linear")
            .one(&db)
            .await?
            .ok_or_else(|| SyncError::NotFound("linear".into()))
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
                remote_created_at: Set(None),
                remote_updated_at: Set(None),
                synced_at: Set(synced),
                dismissed_at: Set(None),
            };
            Ok(am.insert(db).await?)
        }
    }
}
