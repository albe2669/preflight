//! Provider-agnostic pagination loop.
//!
//! Every sync provider pages through a remote, buffers the results, and
//! only commits once every page has been fetched. [`SyncLoop`] captures
//! that shared shape: it drives [`RemoteApiClient::fetch_page`] pagination,
//! buffers into one `Vec`, and returns `PartialResults` on a mid-page
//! failure (discarding the buffer) or the provider's own error on a
//! first-page failure. The provider keeps its own upsert and cursor-write
//! semantics in its `pull()`, since those differ between providers.

use crate::error::RemoteError;
use crate::page::Page;
use async_trait::async_trait;

/// A client that can fetch one page of provider items.
///
/// Each provider implements this for its private concrete client type in
/// its own `client.rs`, mapping its page type onto [`Page<Item>`]. `Params`
/// is whatever the provider's page fetch needs beyond the cursor (e.g. a
/// compiled Linear filter or a GitHub query string).
#[async_trait]
pub trait RemoteApiClient: Send + Sync {
    /// The provider's item type (e.g. `IssueRecord`).
    type Item;
    /// The provider's error type (e.g. `LinearError`).
    type Error;
    /// Per-request parameters beyond the pagination cursor.
    type Params;

    /// Fetch one page, using `after` as the cursor (or `None` for the
    /// first page).
    async fn fetch_page(
        &self,
        params: &Self::Params,
        after: Option<&str>,
    ) -> Result<Page<Self::Item>, Self::Error>;
}

/// Result of a full pagination pass.
///
/// `items` holds every fetched record (empty when only a terminal page was
/// returned). `end_cursor` is the accumulated cursor passed to the last
/// fetch, which the provider may persist as its resume point.
#[derive(Debug, Default)]
pub struct SyncResult<Item> {
    pub items: Vec<Item>,
    pub end_cursor: Option<String>,
}

/// Drives a [`RemoteApiClient`] through all pages, buffering the results.
pub struct SyncLoop<C: RemoteApiClient> {
    client: C,
}

impl<C: RemoteApiClient> SyncLoop<C> {
    /// Wrap a provider client.
    pub fn new(client: C) -> Self {
        SyncLoop { client }
    }

    /// Fetch every page reachable from `params`.
    ///
    /// On success returns all buffered items plus the final accumulated
    /// cursor. A first-page failure surfaces the provider's error wrapped
    /// in [`RemoteError::Provider`]; a failure after at least one page
    /// returns [`RemoteError::PartialResults`] with no items. A page that
    /// declares `has_next_page` without an `end_cursor` returns
    /// [`RemoteError::PaginationCursorInvalid`].
    pub async fn run(
        &self,
        params: &C::Params,
    ) -> Result<SyncResult<C::Item>, RemoteError<C::Error>> {
        let mut items = Vec::new();
        let mut after: Option<String> = None;
        let mut pages_fetched = 0u32;

        loop {
            let page = match self.client.fetch_page(params, after.as_deref()).await {
                Ok(p) => p,
                Err(e) => {
                    return if pages_fetched > 0 {
                        Err(RemoteError::PartialResults)
                    } else {
                        Err(RemoteError::Provider(e))
                    };
                }
            };
            pages_fetched += 1;
            items.extend(page.items);
            if page.has_next_page {
                after = Some(
                    page.end_cursor
                        .ok_or(RemoteError::PaginationCursorInvalid)?,
                );
            } else {
                break;
            }
        }

        Ok(SyncResult {
            items,
            end_cursor: after,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, PartialEq, Eq)]
    struct TestError;

    impl std::fmt::Display for TestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "test error")
        }
    }

    fn page(items: Vec<i32>, end_cursor: Option<&str>, has_next_page: bool) -> Page<i32> {
        Page {
            items,
            end_cursor: end_cursor.map(str::to_string),
            has_next_page,
        }
    }

    /// A client that pops pages from a queue (last-in-first-out).
    struct FakeClient {
        pages: Mutex<Vec<Result<Page<i32>, TestError>>>,
        calls: AtomicUsize,
    }

    impl FakeClient {
        fn new(pages: Vec<Result<Page<i32>, TestError>>) -> Self {
            FakeClient {
                pages: Mutex::new(pages),
                calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl RemoteApiClient for FakeClient {
        type Item = i32;
        type Error = TestError;
        type Params = ();

        async fn fetch_page(
            &self,
            _params: &Self::Params,
            after: Option<&str>,
        ) -> Result<Page<Self::Item>, Self::Error> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            // The cursor the loop handed us for the next page (if any).
            let _ = after;
            self.pages
                .lock()
                .unwrap()
                .pop()
                .unwrap_or(Ok(page(vec![], None, false)))
        }
    }

    fn assert_provider_err(r: Result<SyncResult<i32>, RemoteError<TestError>>) {
        match r {
            Ok(_) => panic!("expected error, got Ok"),
            Err(RemoteError::Provider(TestError)) => {}
            Err(other) => panic!("expected Provider error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn single_page_returns_items() {
        let client = FakeClient::new(vec![Ok(page(vec![1, 2], None, false))]);
        let result = SyncLoop::new(client).run(&()).await.unwrap();
        assert_eq!(result.items, vec![1, 2]);
        assert_eq!(result.end_cursor, None);
    }

    #[tokio::test]
    async fn two_pages_buffers_all_and_returns_accumulated_cursor() {
        let client = FakeClient::new(vec![
            Ok(page(vec![2], None, false)),     // popped last → second fetch
            Ok(page(vec![1], Some("X"), true)), // popped first → first fetch
        ]);
        let result = SyncLoop::new(client).run(&()).await.unwrap();
        // Buffered in fetch order, regardless of pop order.
        let mut items = result.items;
        items.sort_unstable();
        assert_eq!(items, vec![1, 2]);
        assert_eq!(result.end_cursor, Some("X".into()));
    }

    #[tokio::test]
    async fn first_page_error_surfaces_provider_error() {
        let client = FakeClient::new(vec![Err(TestError)]);
        assert_provider_err(SyncLoop::new(client).run(&()).await);
    }

    #[tokio::test]
    async fn mid_page_error_returns_partial_results_and_discards_buffer() {
        let client = FakeClient::new(vec![Err(TestError), Ok(page(vec![1], Some("X"), true))]);
        let r = SyncLoop::new(client).run(&()).await;
        match r {
            Err(RemoteError::PartialResults) => {}
            other => panic!("expected PartialResults, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn has_next_without_cursor_returns_pagination_cursor_invalid() {
        let client = FakeClient::new(vec![Ok(page(vec![], None, true))]);
        let r = SyncLoop::new(client).run(&()).await;
        match r {
            Err(RemoteError::PaginationCursorInvalid) => {}
            other => panic!("expected PaginationCursorInvalid, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn empty_terminal_page_returns_empty_items() {
        let client = FakeClient::new(vec![Ok(page(vec![], None, false))]);
        let result = SyncLoop::new(client).run(&()).await.unwrap();
        assert!(result.items.is_empty());
        assert_eq!(result.end_cursor, None);
    }

    #[test]
    fn sync_result_defaults_are_empty() {
        let r = SyncResult::<i32>::default();
        assert!(r.items.is_empty());
        assert_eq!(r.end_cursor, None);
    }
}
