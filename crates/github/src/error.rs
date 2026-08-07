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

/// Map an HTTP status code and optional GraphQL errors to a `GithubError`.
///
/// When `status` is 2xx and `graphql_errors` is empty, the caller should
/// proceed with parsing the response — this function returns `None`.
pub(crate) fn map_response_error(
    status: u16,
    retry_after: Option<Duration>,
    graphql_errors: Vec<String>,
) -> Option<GithubError> {
    // GraphQL errors take priority (they indicate a response body error)
    if !graphql_errors.is_empty() {
        return Some(GithubError::Remote(graphql_errors.join("; ")));
    }

    match status {
        401 => Some(GithubError::Unauthorized),
        429 => Some(GithubError::RateLimited { retry_after }),
        s if s >= 500 => Some(GithubError::Remote(format!("http {s}"))),
        _ => None,
    }
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
        assert!(matches!(err, Some(GithubError::Unauthorized)));
    }

    #[test]
    fn test_map_response_error_429_with_retry_after() {
        let err = map_response_error(429, Some(Duration::from_secs(30)), vec![]);
        assert!(matches!(err, Some(GithubError::RateLimited { .. })));
        if let Some(GithubError::RateLimited { retry_after }) = err {
            assert_eq!(retry_after, Some(Duration::from_secs(30)));
        }
    }

    #[test]
    fn test_map_response_error_429_without_retry_after() {
        let err = map_response_error(429, None, vec![]);
        assert!(matches!(err, Some(GithubError::RateLimited { .. })));
        if let Some(GithubError::RateLimited { retry_after }) = err {
            assert_eq!(retry_after, None);
        }
    }

    #[test]
    fn test_map_response_error_5xx_returns_remote() {
        let err = map_response_error(500, None, vec![]);
        assert!(matches!(err, Some(GithubError::Remote(_))));
    }

    #[test]
    fn test_map_response_error_graphql_errors_returns_remote() {
        let err = map_response_error(200, None, vec!["field not found".into()]);
        assert!(matches!(err, Some(GithubError::Remote(_))));
    }

    #[test]
    fn test_map_response_error_ok_no_errors_returns_none() {
        let err = map_response_error(200, None, vec![]);
        assert!(err.is_none());
    }
}
