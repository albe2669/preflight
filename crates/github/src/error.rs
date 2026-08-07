use std::time::Duration;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, GithubError>;

#[derive(Debug, Error)]
pub enum GithubError {
    #[error("database error: {0}")]
    Db(#[from] sea_orm::DbErr),
    #[error("sync state not found for github")]
    NotFound,
    #[error("rate limited, retry after {retry_after:?}")]
    RateLimited { retry_after: Option<Duration> },
    #[error("unauthorized")]
    Unauthorized,
    #[error("remote error: {0}")]
    Remote(String),
    #[error("pagination cursor invalid")]
    PaginationCursorInvalid,
    #[error("partial results returned")]
    PartialResults,
    #[error("content filtered out")]
    ContentFiltered,
    #[error("schema mismatch: {0}")]
    SchemaMismatch(String),
}

impl From<remote_sync::error::RemoteError<GithubError>> for GithubError {
    fn from(e: remote_sync::error::RemoteError<GithubError>) -> Self {
        match e {
            remote_sync::error::RemoteError::RateLimited { retry_after } => {
                GithubError::RateLimited { retry_after }
            }
            remote_sync::error::RemoteError::Unauthorized => GithubError::Unauthorized,
            remote_sync::error::RemoteError::Remote(msg) => GithubError::Remote(msg),
            remote_sync::error::RemoteError::PaginationCursorInvalid => {
                GithubError::PaginationCursorInvalid
            }
            remote_sync::error::RemoteError::PartialResults => GithubError::PartialResults,
            remote_sync::error::RemoteError::ContentFiltered => GithubError::ContentFiltered,
            remote_sync::error::RemoteError::SchemaMismatch(msg) => {
                GithubError::SchemaMismatch(msg)
            }
            remote_sync::error::RemoteError::Provider(e) => e,
        }
    }
}

/// Map an HTTP status code and optional GraphQL errors to a `GithubError`.
///
/// When `status` is 2xx and `graphql_errors` is empty, the caller should
/// proceed with parsing the response — this function returns `Ok(())`.
/// Delegates to the shared [`remote_sync::error::map_response_error`] and
/// translates the sentinel onto [`GithubError`].
pub(crate) fn map_response_error(
    status: u16,
    retry_after: Option<Duration>,
    graphql_errors: Vec<String>,
) -> std::result::Result<(), GithubError> {
    remote_sync::error::map_response_error(status, retry_after, &graphql_errors)
        .map_err(GithubError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_variants_exist() {
        let rl = GithubError::RateLimited {
            retry_after: Some(Duration::from_secs(30)),
        };
        assert!(matches!(rl, GithubError::RateLimited { .. }));

        let rl_none = GithubError::RateLimited { retry_after: None };
        assert!(matches!(rl_none, GithubError::RateLimited { .. }));

        let unauth = GithubError::Unauthorized;
        assert!(matches!(unauth, GithubError::Unauthorized));

        let remote = GithubError::Remote("boom".into());
        assert!(matches!(remote, GithubError::Remote(_)));

        let cursor_err = GithubError::PaginationCursorInvalid;
        assert!(matches!(cursor_err, GithubError::PaginationCursorInvalid));

        let partial = GithubError::PartialResults;
        assert!(matches!(partial, GithubError::PartialResults));

        let filtered = GithubError::ContentFiltered;
        assert!(matches!(filtered, GithubError::ContentFiltered));

        let schema = GithubError::SchemaMismatch("bad field".into());
        assert!(matches!(schema, GithubError::SchemaMismatch(_)));
    }

    #[test]
    fn test_map_response_error_401_returns_unauthorized() {
        let err = map_response_error(401, None, vec![]);
        assert!(matches!(err, Err(GithubError::Unauthorized)));
    }

    #[test]
    fn test_map_response_error_429_with_retry_after() {
        let err = map_response_error(429, Some(Duration::from_secs(30)), vec![]);
        assert!(matches!(err, Err(GithubError::RateLimited { .. })));
        if let Err(GithubError::RateLimited { retry_after }) = err {
            assert_eq!(retry_after, Some(Duration::from_secs(30)));
        }
    }

    #[test]
    fn test_map_response_error_429_without_retry_after() {
        let err = map_response_error(429, None, vec![]);
        assert!(matches!(err, Err(GithubError::RateLimited { .. })));
        if let Err(GithubError::RateLimited { retry_after }) = err {
            assert_eq!(retry_after, None);
        }
    }

    #[test]
    fn test_map_response_error_5xx_returns_remote() {
        let err = map_response_error(500, None, vec![]);
        assert!(matches!(err, Err(GithubError::Remote(_))));
    }

    #[test]
    fn test_map_response_error_graphql_errors_returns_remote() {
        let err = map_response_error(200, None, vec!["field not found".into()]);
        assert!(matches!(err, Err(GithubError::Remote(_))));
    }

    #[test]
    fn test_map_response_error_ok_no_errors_returns_ok() {
        let err = map_response_error(200, None, vec![]);
        assert!(matches!(err, Ok(())));
    }
}
