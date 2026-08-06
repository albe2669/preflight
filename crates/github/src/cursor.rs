//! `sync_state` upsert helpers.
//!
//! Every pull source keeps a row in `sync_state` keyed by `source`
//! (e.g. "github", "linear").  `get` returns the saved cursor string;
//! `put` writes the current cursor, status, and optional error.

use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use sync_state::entity::sync_state;

use crate::error::Result;

/// Read the cursor for a source, if any.
pub async fn get(db: &DatabaseConnection, source: &str) -> Result<Option<String>> {
    let row = sync_state::Entity::find()
        .filter(sync_state::Column::Source.eq(source))
        .one(db)
        .await?;
    Ok(row.and_then(|r| r.cursor))
}

/// Upsert the cursor row for a source.
pub async fn put(
    db: &DatabaseConnection,
    source: &str,
    cursor: Option<String>,
    status: &str,
    error: Option<String>,
) -> Result<()> {
    let am = sync_state::ActiveModel {
        source: Set(source.to_string()),
        cursor: Set(cursor),
        last_synced_at: Set(Some(Utc::now().into())),
        last_status: Set(status.to_string()),
        last_error: Set(error),
    };

    // If a row already exists, update it; otherwise insert.
    if sync_state::Entity::find()
        .filter(sync_state::Column::Source.eq(source))
        .one(db)
        .await?
        .is_some()
    {
        am.update(db).await?;
    } else {
        am.insert(db).await?;
    }
    Ok(())
}
