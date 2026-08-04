//! Errors for the core crate.

use sea_orm::DbErr;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] DbErr),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("conflict: {0}")]
    Conflict(String),
}

/// `transaction` returns `TransactionError<E>` where `E` is the callback's
/// error. Our callbacks return [`Error`], so the transaction result is
/// `TransactionError<Error>`; flatten it so service fns can `?`-propagate.
/// A connection-level failure (`TransactionError::Connection`) carries a
/// `DbErr` which maps through `Error::Db`.
impl From<sea_orm::TransactionError<Error>> for Error {
    fn from(e: sea_orm::TransactionError<Error>) -> Self {
        match e {
            sea_orm::TransactionError::Connection(db) => Error::Db(db),
            sea_orm::TransactionError::Transaction(err) => err,
        }
    }
}
