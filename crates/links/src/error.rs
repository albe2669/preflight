//! Errors for the links crate.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, LinkError>;

#[derive(Debug, Error)]
pub enum LinkError {
    #[error("database error: {0}")]
    Db(#[from] sea_orm::DbErr),
    #[error("not found: {0}")]
    NotFound(String),
    #[error(transparent)]
    Todo(#[from] todo::error::Error),
}

/// `transaction` returns `TransactionError<E>` where `E` is the callback's
/// error. Our callbacks return [`LinkError`], so the transaction result is
/// `TransactionError<LinkError>`; flatten it so service fns can `?`-propagate.
/// A connection-level failure (`TransactionError::Connection`) carries a
/// `DbErr` which maps through `LinkError::Db`.
impl From<sea_orm::TransactionError<LinkError>> for LinkError {
    fn from(e: sea_orm::TransactionError<LinkError>) -> Self {
        match e {
            sea_orm::TransactionError::Connection(db) => LinkError::Db(db),
            sea_orm::TransactionError::Transaction(err) => err,
        }
    }
}
