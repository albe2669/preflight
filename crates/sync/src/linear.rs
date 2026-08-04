//! Linear pull stub and issue upsert helper.

use entity::linear_issue;
use preflight_core::Result;
use preflight_core::clock::now_tz;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    QueryFilter, Set,
};

use crate::cursor;

/// Pull issues from Linear.
///
/// When `token` is empty the call is a no-op (cursor marked "never").
/// Otherwise this is a stub — the real HTTP body is marked `TODO(network)`.
pub async fn pull_linear(
    db: &DatabaseConnection,
    token: &str,
    _team_keys: &[String],
) -> Result<()> {
    if token.is_empty() {
        cursor::put(
            db,
            "linear",
            None,
            "never",
            Some("no token configured".into()),
        )
        .await?;
        return Ok(());
    }

    // TODO(network): query Linear GraphQL API.
    //
    // The real implementation will:
    //   1. Build a GraphQL query for the requested team keys.
    //   2. Paginate results.
    //   3. Call `upsert_issue` for each issue.
    //   4. Update the cursor with the last seen value.

    cursor::put(db, "linear", None, "ok", None).await?;
    Ok(())
}

/// Idempotently upsert a Linear issue row.
///
/// The identity key is `linear_id`.  If the row already exists the mutable
/// fields and `synced_at` are updated; otherwise a new row is inserted.
pub async fn upsert_issue(
    db: &DatabaseConnection,
    linear_id: &str,
    identifier: &str,
    title: &str,
    description: Option<&str>,
    url: &str,
    state_name: &str,
    state_type: &str,
    priority: Option<i64>,
    team_key: Option<&str>,
    assignee_name: Option<&str>,
    assigned_to_me: bool,
) -> Result<linear_issue::Model> {
    let synced = now_tz();

    let existing = linear_issue::Entity::find()
        .filter(linear_issue::Column::LinearId.eq(linear_id))
        .one(db)
        .await?;

    match existing {
        Some(model) => {
            let mut am = model.into_active_model();
            am.identifier = Set(identifier.to_string());
            am.title = Set(title.to_string());
            am.description = Set(description.map(|s| s.to_string()));
            am.url = Set(url.to_string());
            am.state_name = Set(state_name.to_string());
            am.state_type = Set(state_type.to_string());
            am.priority = Set(priority);
            am.team_key = Set(team_key.map(|s| s.to_string()));
            am.assignee_name = Set(assignee_name.map(|s| s.to_string()));
            am.assigned_to_me = Set(assigned_to_me);
            am.synced_at = Set(synced);
            am.update(db).await.map_err(preflight_core::Error::from)
        }
        None => {
            let am = linear_issue::ActiveModel {
                id: ActiveValue::not_set(),
                linear_id: Set(linear_id.to_string()),
                identifier: Set(identifier.to_string()),
                title: Set(title.to_string()),
                description: Set(description.map(|s| s.to_string())),
                url: Set(url.to_string()),
                state_name: Set(state_name.to_string()),
                state_type: Set(state_type.to_string()),
                priority: Set(priority),
                team_key: Set(team_key.map(|s| s.to_string())),
                assignee_name: Set(assignee_name.map(|s| s.to_string())),
                assigned_to_me: Set(assigned_to_me),
                remote_created_at: Set(None),
                remote_updated_at: Set(None),
                synced_at: Set(synced),
                dismissed_at: Set(None),
            };
            am.insert(db).await.map_err(preflight_core::Error::from)
        }
    }
}
