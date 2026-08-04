//! Sync-specific errors.

use thiserror::Error;

/// Errors emitted by the sync crate.
#[derive(Debug, Error)]
pub enum SyncError {
    #[error("database error: {0}")]
    Db(#[from] sea_orm::DbErr),

    #[error("sync state not found for {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, SyncError>;
