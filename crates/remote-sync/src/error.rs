//! HTTP↔sentinel error mapping shared by the sync providers.
//!
//! [`RemoteError`] carries the sentinel variants every provider surfaces
//! (`RateLimited`, `Unauthorized`, `Remote`, …) while wrapping the
//! provider's own error type in `Provider(E)`. [`map_response_error`] maps
//! a raw HTTP response onto those sentinels, decoupled from any concrete
//! HTTP client type via the [`ResponseSource`] trait.

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

/// Something `map_response_error` can classify by status, giving the
/// provider control over how `Retry-After` is decoded without this crate
/// depending on a concrete HTTP type.
pub trait ResponseSource {
    fn status(&self) -> u16;
    /// Seconds to wait before retrying, if the remote provided one.
    fn retry_after(&self) -> Option<Duration>;
}

/// Map an HTTP response onto a [`RemoteError`] sentinel, as `Result<(), E>`.
///
/// GraphQL errors in the body are handled before status dispatch, matching
/// the existing provider behaviour. 2xx → `Ok`; 401 → `Unauthorized`;
/// 403/429 → `RateLimited{retry_after}`; everything else → `Remote`.
pub fn map_response_error<S: ResponseSource>(
    source: &S,
    graphql_errors: Option<&[String]>,
) -> Result<(), RemoteError<()>> {
    if let Some(errors) = graphql_errors {
        if let Some(first) = errors.first() {
            return Err(RemoteError::Remote(first.clone()));
        }
    }
    match source.status() {
        200..=299 => Ok(()),
        401 => Err(RemoteError::Unauthorized),
        403 | 429 => Err(RemoteError::RateLimited {
            retry_after: source.retry_after(),
        }),
        _ => Err(RemoteError::Remote(format!("HTTP {}", source.status()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StatusOnly(u16);

    impl ResponseSource for StatusOnly {
        fn status(&self) -> u16 {
            self.0
        }
        fn retry_after(&self) -> Option<Duration> {
            None
        }
    }

    struct WithRetryAfter(u16, Option<Duration>);

    impl ResponseSource for WithRetryAfter {
        fn status(&self) -> u16 {
            self.0
        }
        fn retry_after(&self) -> Option<Duration> {
            self.1
        }
    }

    fn assert_sentinel(r: Result<(), RemoteError<()>>, pred: impl Fn(RemoteError<()>) -> bool) {
        match r {
            Ok(()) => panic!("expected error, got Ok"),
            Err(e) => assert!(pred(e), "unexpected error"),
        }
    }

    #[test]
    fn ok_on_2xx() {
        assert!(map_response_error(&StatusOnly(200), None).is_ok());
        assert!(map_response_error(&StatusOnly(299), None).is_ok());
    }

    #[test]
    fn unauthorized_on_401() {
        assert_sentinel(map_response_error(&StatusOnly(401), None), |e| {
            matches!(e, RemoteError::Unauthorized)
        });
    }

    #[test]
    fn rate_limited_on_429_with_retry_after() {
        assert_sentinel(
            map_response_error(&WithRetryAfter(429, Some(Duration::from_secs(60))), None),
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
        assert_sentinel(map_response_error(&WithRetryAfter(429, None), None), |e| {
            matches!(e, RemoteError::RateLimited { retry_after: None })
        });
    }

    #[test]
    fn rate_limited_on_403() {
        assert_sentinel(
            map_response_error(&WithRetryAfter(403, Some(Duration::from_secs(5))), None),
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
    fn remote_on_5xx() {
        assert_sentinel(map_response_error(&StatusOnly(500), None), |e| {
            matches!(e, RemoteError::Remote(_))
        });
    }

    #[test]
    fn remote_on_other_status() {
        assert_sentinel(map_response_error(&StatusOnly(418), None), |e| {
            matches!(e, RemoteError::Remote(_))
        });
    }

    #[test]
    fn graphql_errors_preempt_status() {
        let errs = vec!["boom".to_string()];
        assert_sentinel(
            map_response_error(&StatusOnly(200), Some(&errs)),
            |e| matches!(e, RemoteError::Remote(m) if m == "boom"),
        );
    }

    #[test]
    fn empty_graphql_errors_are_ignored() {
        assert!(map_response_error(&StatusOnly(200), Some(&[])).is_ok());
    }
}
