//! HTTP↔sentinel error mapping shared by the sync providers.
//!
//! [`RemoteError`] carries the sentinel variants every provider surfaces
//! (`RateLimited`, `Unauthorized`, `Remote`, …) while wrapping the
//! provider's own error type in `Provider(E)`. [`map_response_error`] maps
//! raw HTTP status / `Retry-After` / GraphQL errors onto those sentinels,
//! keeping this crate free of any concrete HTTP client type.

use std::time::Duration;

use thiserror::Error;

/// Sentinel error shared across sync providers, wrapping the provider's own
/// `E` for anything this crate cannot classify.
#[derive(Debug, Error)]
pub enum RemoteError<E> {
    /// The remote demanded a retry, optionally with a `Retry-After` duration.
    #[error("rate limited, retry after {retry_after:?}")]
    RateLimited { retry_after: Option<Duration> },
    /// The remote returned 401.
    #[error("unauthorized")]
    Unauthorized,
    /// A remote-side failure that has no more specific sentinel.
    #[error("remote error: {0}")]
    Remote(String),
    /// Pagination carried a cursor this crate cannot handle.
    #[error("invalid pagination cursor")]
    PaginationCursorInvalid,
    /// Some pages were fetched before a mid-stream failure.
    #[error("partial results")]
    PartialResults,
    /// The response was filtered/truncated by the remote, not by us.
    #[error("content filtered")]
    ContentFiltered,
    /// The response did not match the expected schema.
    #[error("schema mismatch: {0}")]
    SchemaMismatch(String),
    /// Anything the provider itself raised.
    #[error("provider error: {0}")]
    Provider(#[from] E),
}

/// Map an HTTP response onto a [`RemoteError`] sentinel, as `Result<(), E>`.
///
/// GraphQL errors are handled before status dispatch, matching the existing
/// provider behaviour. 2xx → `Ok`; 401 → `Unauthorized`; 429 →
/// `RateLimited`; 403 and every other non-2xx → `Remote`.
pub fn map_response_error<E: std::fmt::Display>(
    status: u16,
    retry_after: Option<Duration>,
    graphql_errors: &[String],
) -> Result<(), RemoteError<E>> {
    if graphql_errors.is_empty() {
        match status {
            200..=299 => Ok(()),
            401 => Err(RemoteError::Unauthorized),
            429 => Err(RemoteError::RateLimited { retry_after }),
            _ => Err(RemoteError::Remote(format!("HTTP {status}"))),
        }
    } else {
        Err(RemoteError::Remote(graphql_errors.join("; ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestError;

    impl std::fmt::Display for TestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "test error")
        }
    }

    type R = Result<(), RemoteError<TestError>>;

    fn assert_sentinel(r: R, pred: impl Fn(&RemoteError<TestError>) -> bool) {
        match r {
            Ok(()) => panic!("expected error, got Ok"),
            Err(e) => assert!(pred(&e), "unexpected error: {e}"),
        }
    }

    #[test]
    fn ok_on_2xx() {
        assert!(map_response_error::<TestError>(200, None, &[]).is_ok());
        assert!(map_response_error::<TestError>(299, None, &[]).is_ok());
    }

    #[test]
    fn unauthorized_on_401() {
        assert_sentinel(map_response_error(401, None, &[]), |e| {
            matches!(e, RemoteError::Unauthorized)
        });
    }

    #[test]
    fn rate_limited_on_429_with_retry_after() {
        assert_sentinel(
            map_response_error(429, Some(Duration::from_secs(60)), &[]),
            |e| {
                matches!(
                    e,
                    RemoteError::RateLimited {
                        retry_after: Some(_)
                    }
                )
            },
        );
    }

    #[test]
    fn rate_limited_on_429_without_retry_after() {
        assert_sentinel(map_response_error(429, None, &[]), |e| {
            matches!(e, RemoteError::RateLimited { retry_after: None })
        });
    }

    #[test]
    fn remote_on_403() {
        assert_sentinel(
            map_response_error(403, None, &[]),
            |e| matches!(e, RemoteError::Remote(m) if m == "HTTP 403"),
        );
    }

    #[test]
    fn remote_on_5xx() {
        assert_sentinel(
            map_response_error(500, None, &[]),
            |e| matches!(e, RemoteError::Remote(m) if m == "HTTP 500"),
        );
    }

    #[test]
    fn remote_on_other_status() {
        assert_sentinel(map_response_error(418, None, &[]), |e| {
            matches!(e, RemoteError::Remote(_))
        });
    }

    #[test]
    fn graphql_errors_preempt_status() {
        let errs = vec!["boom".to_string(), "oops".to_string()];
        assert_sentinel(
            map_response_error(200, None, &errs),
            |e| matches!(e, RemoteError::Remote(m) if m == "boom; oops"),
        );
    }

    #[test]
    fn empty_graphql_errors_are_ignored() {
        assert!(map_response_error::<TestError>(200, None, &[]).is_ok());
    }
}
