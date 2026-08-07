use std::time::Duration;

use thiserror::Error;

pub type Result<T, E = LinearError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum LinearError {
    #[error("database error: {0}")]
    Db(#[from] sea_orm::DbErr),
    #[error("sync state not found for linear")]
    NotFound,
    #[error("rate limited, retry after {retry_after:?}")]
    RateLimited { retry_after: Option<Duration> },
    #[error("unauthorized")]
    Unauthorized,
    #[error("remote error: {0}")]
    Remote(String),
    #[error("pagination cursor invalid")]
    PaginationCursorInvalid,
    #[error("partial results")]
    PartialResults,
    #[error("content filtered")]
    ContentFiltered,
    #[error("schema mismatch")]
    SchemaMismatch,
}
impl From<reqwest::Error> for LinearError {
    fn from(e: reqwest::Error) -> Self {
        LinearError::Remote(e.to_string())
    }
}

impl From<remote_sync::error::RemoteError<LinearError>> for LinearError {
    fn from(e: remote_sync::error::RemoteError<LinearError>) -> Self {
        match e {
            remote_sync::error::RemoteError::RateLimited { retry_after } => {
                LinearError::RateLimited { retry_after }
            }
            remote_sync::error::RemoteError::Unauthorized => LinearError::Unauthorized,
            remote_sync::error::RemoteError::Remote(msg) => LinearError::Remote(msg),
            remote_sync::error::RemoteError::PaginationCursorInvalid => {
                LinearError::PaginationCursorInvalid
            }
            remote_sync::error::RemoteError::PartialResults => LinearError::PartialResults,
            remote_sync::error::RemoteError::ContentFiltered => LinearError::ContentFiltered,
            remote_sync::error::RemoteError::SchemaMismatch(_) => LinearError::SchemaMismatch,
            remote_sync::error::RemoteError::Provider(e) => e,
        }
    }
}

/// Map an HTTP response status and optional GraphQL errors to a [`LinearError`].
///
/// Returns `Ok(())` when the status is 2xx and there are no GraphQL errors.
/// Delegates to the shared [`remote_sync::error::map_response_error`] and
/// translates the sentinel onto [`LinearError`].
pub(crate) fn map_response_error(
    status: u16,
    retry_after: Option<Duration>,
    graphql_errors: Vec<String>,
) -> std::result::Result<(), LinearError> {
    remote_sync::error::map_response_error(status, retry_after, &graphql_errors)
        .map_err(LinearError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_rate_limited_has_message() {
        let err = LinearError::RateLimited {
            retry_after: Some(Duration::from_secs(30)),
        };
        assert!(format!("{err}").contains("rate"));
    }

    #[test]
    fn test_error_unauthorized_has_message() {
        let err = LinearError::Unauthorized;
        assert!(format!("{err}").contains("unauthorized"));
    }

    #[test]
    fn test_error_remote_has_message() {
        let err = LinearError::Remote("connection refused".into());
        assert!(format!("{err}").contains("connection refused"));
    }

    #[test]
    fn test_error_pagination_cursor_invalid_has_message() {
        let err = LinearError::PaginationCursorInvalid;
        assert!(format!("{err}").contains("cursor"));
    }

    #[test]
    fn test_error_partial_results_has_message() {
        let err = LinearError::PartialResults;
        assert!(format!("{err}").contains("partial"));
    }

    #[test]
    fn test_error_content_filtered_has_message() {
        let err = LinearError::ContentFiltered;
        assert!(format!("{err}").contains("filtered"));
    }

    #[test]
    fn test_error_schema_mismatch_has_message() {
        let err = LinearError::SchemaMismatch;
        assert!(format!("{err}").contains("schema"));
    }

    #[test]
    fn test_error_matches_rate_limited() {
        let err = LinearError::RateLimited {
            retry_after: Some(Duration::from_secs(10)),
        };
        assert!(matches!(err, LinearError::RateLimited { .. }));
    }

    #[test]
    fn test_error_matches_unauthorized() {
        let err = LinearError::Unauthorized;
        assert!(matches!(err, LinearError::Unauthorized));
    }

    #[test]
    fn test_error_matches_remote() {
        let err = LinearError::Remote("oops".into());
        assert!(matches!(err, LinearError::Remote(_)));
    }

    #[test]
    fn test_map_response_error_401_returns_unauthorized() {
        let result = map_response_error(401, None, vec![]);
        assert!(matches!(result, Err(LinearError::Unauthorized)));
    }

    #[test]
    fn test_map_response_error_429_with_retry_after() {
        let result = map_response_error(429, Some(Duration::from_secs(30)), vec![]);
        assert!(matches!(
            result,
            Err(LinearError::RateLimited {
                retry_after: Some(_)
            })
        ));
    }

    #[test]
    fn test_map_response_error_429_without_retry_after() {
        let result = map_response_error(429, None, vec![]);
        assert!(matches!(
            result,
            Err(LinearError::RateLimited { retry_after: None })
        ));
    }

    #[test]
    fn test_map_response_error_5xx_returns_remote() {
        let result = map_response_error(500, None, vec![]);
        assert!(matches!(result, Err(LinearError::Remote(_))));
    }

    #[test]
    fn test_map_response_error_graphql_errors_returns_remote() {
        let result = map_response_error(200, None, vec!["field not found".into()]);
        assert!(matches!(result, Err(LinearError::Remote(_))));
    }

    #[test]
    fn test_map_response_error_200_no_errors_returns_ok() {
        let result = map_response_error(200, None, vec![]);
        assert!(matches!(result, Ok(())));
    }
}
