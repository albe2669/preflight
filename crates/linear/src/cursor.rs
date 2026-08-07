//! `sync_state` upsert helpers for the linear source.
//!
//! This is a thin adapter over the shared [`remote_sync::cursor`], mapping
//! its [`CursorError`] onto [`LinearError::Db`] so callers keep returning
//! the crate's own error type.

use remote_sync::cursor::CursorError;
use sea_orm::DatabaseConnection;

use crate::error::{LinearError, Result};

fn map_err(e: CursorError) -> LinearError {
    LinearError::Db(e.into())
}

/// Read the cursor for a source, if any.
pub async fn get(db: &DatabaseConnection, source: &str) -> Result<Option<String>> {
    remote_sync::cursor::get(db, source).await.map_err(map_err)
}

/// Upsert the cursor row for a source.
pub async fn put(
    db: &DatabaseConnection,
    source: &str,
    cursor: Option<String>,
    status: &str,
    error: Option<String>,
) -> Result<()> {
    remote_sync::cursor::put(db, source, cursor, status, error)
        .await
        .map_err(map_err)
}
