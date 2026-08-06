use thiserror::Error;

pub type Result<T> = std::result::Result<T, GithubError>;

#[derive(Debug, Error)]
pub enum GithubError {
    #[error("database error: {0}")]
    Db(#[from] sea_orm::DbErr),
    #[error("sync state not found for github")]
    NotFound,
}
