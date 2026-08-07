//! A page of provider items plus pagination metadata.
//!
//! [`Page`] is the single shape every provider returns from its
//! `fetch_page`, so the shared `SyncLoop` can drive pagination without
//! knowing the provider's concrete item or transport types.

/// One page of remote items and the pagination cursor that follows it.
///
/// - `items` are the fetched records for this page.
/// - `end_cursor` is the cursor to send on the next request.
/// - `has_next_page` tells whether more pages remain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub end_cursor: Option<String>,
    pub has_next_page: bool,
}

impl<T> Page<T> {
    /// An empty, terminal page (`has_next_page == false`).
    pub fn empty() -> Self {
        Page {
            items: Vec::new(),
            end_cursor: None,
            has_next_page: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_has_no_items_and_terminates() {
        let p = Page::<i32>::empty();
        assert!(p.items.is_empty());
        assert_eq!(p.end_cursor, None);
        assert!(!p.has_next_page);
    }

    #[test]
    fn page_carries_items_and_cursor() {
        let p = Page {
            items: vec![1, 2, 3],
            end_cursor: Some("abc".into()),
            has_next_page: true,
        };
        assert_eq!(p.items, vec![1, 2, 3]);
        assert_eq!(p.end_cursor, Some("abc".into()));
        assert!(p.has_next_page);
    }

    #[test]
    fn partial_eq_compares_all_fields() {
        let a = Page {
            items: vec!["x".to_string()],
            end_cursor: Some("c1".into()),
            has_next_page: true,
        };
        let b = Page {
            items: vec!["x".to_string()],
            end_cursor: Some("c1".into()),
            has_next_page: true,
        };
        let c = Page {
            items: vec!["x".to_string()],
            end_cursor: Some("c1".into()),
            has_next_page: false,
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
