use thiserror::Error;

pub type Result<T, E = LinearError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum LinearError {
    #[error("database error: {0}")]
    Db(#[from] sea_orm::DbErr),
    #[error("sync state not found for linear")]
    NotFound,
}
