//! GitHub GraphQL API client abstraction and response parsing.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use remote_sync::page::Page;
use remote_sync::sync::RemoteApiClient;
use serde_json::Value;

use crate::entity::enums::PullRequestState;
use crate::error::{GithubError, Result};
use crate::filters::FetchedPr;

/// Maximum characters of a request/response body kept in an error log line.
const BODY_LOG_LIMIT: usize = 4000;

/// Truncate a body for logging, preserving the tail (where GitHub puts the
/// human-readable error message).
fn truncate_body(s: impl AsRef<str>) -> String {
    let s = s.as_ref();
    if s.len() <= BODY_LOG_LIMIT {
        s.to_string()
    } else {
        format!("…{}", &s[s.len() - BODY_LOG_LIMIT..])
    }
}
/// A single page of search results from the GitHub API.
#[derive(Clone, Debug)]
pub struct GithubPage {
    pub prs: Vec<FetchedPr>,
    pub end_cursor: Option<String>,
    pub has_next_page: bool,
}

/// Trait for fetching pages of pull requests from GitHub.
///
/// The trait speaks domain types only — no `reqwest::Response` or
/// `serde_json::Value` in the exported signature.
#[async_trait]
pub trait GithubApiClient: Send + Sync {
    async fn search_page(&self, query: &str, after: Option<&str>) -> Result<GithubPage>;
}

/// Parse a GitHub GraphQL search response into a typed page.
///
/// Returns `SchemaMismatch` when the JSON shape is not what we expect.
pub(crate) fn parse_github_page(json: Value) -> Result<GithubPage> {
    let data = json
        .get("data")
        .and_then(|v| v.get("search"))
        .ok_or_else(|| GithubError::SchemaMismatch("missing data.search".into()))?;

    let page_info = data
        .get("pageInfo")
        .ok_or_else(|| GithubError::SchemaMismatch("missing search.pageInfo".into()))?;

    let end_cursor = page_info
        .get("endCursor")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let has_next_page = page_info
        .get("hasNextPage")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let nodes = data
        .get("nodes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| GithubError::SchemaMismatch("missing search.nodes".into()))?;

    let mut prs = Vec::with_capacity(nodes.len());
    for node in nodes {
        let number = node
            .get("number")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| GithubError::SchemaMismatch("missing node.number".into()))?;

        let title = node
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GithubError::SchemaMismatch("missing node.title".into()))?
            .to_string();

        let url = node
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GithubError::SchemaMismatch("missing node.url".into()))?
            .to_string();

        let state_str = node
            .get("state")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GithubError::SchemaMismatch("missing node.state".into()))?;

        match state_str {
            "OPEN" | "CLOSED" | "MERGED" => {}
            _ => {
                return Err(GithubError::SchemaMismatch(format!(
                    "unknown state: {state_str}"
                )));
            }
        }

        let is_draft = node
            .get("isDraft")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // A draft PR is OPEN on GitHub but tracked as a distinct state here.
        let state = if is_draft {
            "draft".to_string()
        } else {
            state_str.to_string()
        };

        let name_with_owner = node
            .get("repository")
            .and_then(|r| r.get("nameWithOwner"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                GithubError::SchemaMismatch("missing repository.nameWithOwner".into())
            })?;
        let mut parts = name_with_owner.split('/');
        let repo_owner = parts
            .next()
            .ok_or_else(|| GithubError::SchemaMismatch("no owner in nameWithOwner".into()))?
            .to_string();
        let repo_name = parts
            .next()
            .ok_or_else(|| GithubError::SchemaMismatch("no repo in nameWithOwner".into()))?
            .to_string();

        let author_login = node
            .get("author")
            .and_then(|a| a.get("login"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let remote_created_at = node
            .get("createdAt")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            });

        let remote_updated_at = node
            .get("updatedAt")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            });

        let changes_requested = node
            .get("reviewDecision")
            .and_then(|v| v.as_str())
            .map(|s| s == "CHANGES_REQUESTED")
            .unwrap_or(false);

        let merge_conflicts = node
            .get("mergeable")
            .and_then(|v| v.as_str())
            .map(|s| s == "CONFLICTING")
            .unwrap_or(false);

        // ponytail: Copilot comment detection needs a review-author query we
        // do not run yet; always false until that is added.
        let copilot_comments = false;

        prs.push(FetchedPr {
            repo_owner,
            repo_name,
            number,
            title,
            url,
            author_login,
            state,
            is_draft,
            review_requested: false,
            requested_reviewer_teams: Vec::new(),
            authored_by_me: false,
            remote_created_at,
            remote_updated_at,
            changes_requested,
            copilot_comments,
            merge_conflicts,
        });
    }

    Ok(GithubPage {
        prs,
        end_cursor,
        has_next_page,
    })
}

/// Map a `FetchedPr` to a `PrRecord` for upsert.
pub(crate) fn fetched_to_record(f: &FetchedPr) -> crate::sync::PrRecord {
    crate::sync::PrRecord {
        provider: "github".into(),
        owner: f.repo_owner.clone(),
        repo: f.repo_name.clone(),
        number: f.number,
        title: f.title.clone(),
        url: f.url.clone(),
        author: f.author_login.clone(),
        state: if f.is_draft {
            PullRequestState::Draft
        } else {
            match f.state.to_lowercase().as_str() {
                "open" => PullRequestState::Open,
                "closed" => PullRequestState::Closed,
                "merged" => PullRequestState::Merged,
                _ => PullRequestState::Open,
            }
        },
        review_requested: f.review_requested,
        authored_by_me: f.authored_by_me,
        remote_created_at: f.remote_created_at,
        remote_updated_at: f.remote_updated_at,
        changes_requested: f.changes_requested,
        copilot_comments: f.copilot_comments,
        merge_conflicts: f.merge_conflicts,
    }
}

/// Concrete HTTP client backed by `reqwest`.
pub(crate) struct GithubApiClientImpl {
    client: reqwest::Client,
    token: String,
    base_url: String,
}

#[async_trait]
impl GithubApiClient for GithubApiClientImpl {
    async fn search_page(&self, query: &str, after: Option<&str>) -> Result<GithubPage> {
        use crate::error::map_response_error;

        let start = std::time::Instant::now();

        let graphql_query = r#"
            query($query: String!, $first: Int!, $after: String) {
                search(query: $query, first: $first, after: $after, type: ISSUE_ADVANCED) {
                    pageInfo {
                        endCursor
                        hasNextPage
                    }
                    nodes {
                        ... on PullRequest {
                            number
                            title
                            url
                            state
                            isDraft
                            reviewDecision
                            mergeable
                            author { login }
                            createdAt
                            updatedAt
                            repository { nameWithOwner }
                        }
                    }
                }
            }
        "#;

        let request_body = serde_json::json!({
            "query": graphql_query,
            "variables": {
                "query": query,
                "first": 30,
                "after": after,
            }
        });

        let response = self
            .client
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .json(&request_body)
            .send()
            .await;

        let elapsed_ms = start.elapsed().as_millis() as u64;

        // Map transport errors (connection refused, timeout, etc.) to a
        // loggable error before returning.
        let response = match response {
            Ok(r) => r,
            Err(e) => {
                let err_str = e.to_string();
                tracing::error!(
                    provider = "github",
                    operation = "search_page",
                    endpoint = %self.base_url,
                    status = 0u16,
                    elapsed_ms = elapsed_ms,
                    error = %err_str,
                    "outbound request"
                );
                return Err(GithubError::Remote(err_str));
            }
        };

        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let retry_after = headers
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .map(std::time::Duration::from_secs);
        let text = response
            .text()
            .await
            .map_err(|e| GithubError::Remote(e.to_string()))?;

        // Check for HTTP-level errors first
        let http_err = map_response_error(status, retry_after, vec![]);
        if let Err(e) = http_err {
            let request_body =
                truncate_body(serde_json::to_string(&request_body).unwrap_or_default());
            if let GithubError::RateLimited { retry_after } = &e {
                tracing::warn!(
                    provider = "github",
                    operation = "search_page",
                    endpoint = %self.base_url,
                    status = status,
                    elapsed_ms = elapsed_ms,
                    retry_after_ms = retry_after.map(|d| d.as_millis() as u64),
                    error = %e,
                    request_body = %request_body,
                    response_body = %truncate_body(&text),
                    "outbound request"
                );
            } else {
                tracing::error!(
                    provider = "github",
                    operation = "search_page",
                    endpoint = %self.base_url,
                    status = status,
                    elapsed_ms = elapsed_ms,
                    error = %e,
                    request_body = %request_body,
                    response_body = %truncate_body(&text),
                    "outbound request"
                );
            }
            return Err(e);
        }

        let json: Value =
            serde_json::from_str(&text).map_err(|e| GithubError::SchemaMismatch(e.to_string()))?;

        // Extract and check for GraphQL errors (2xx body errors)
        let graphql_errors: Vec<String> = json["errors"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| e["message"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let gql_err = map_response_error(status, None, graphql_errors);
        if let Err(e) = gql_err {
            let request_body =
                truncate_body(serde_json::to_string(&request_body).unwrap_or_default());
            if let GithubError::RateLimited { retry_after } = &e {
                tracing::warn!(
                    provider = "github",
                    operation = "search_page",
                    endpoint = %self.base_url,
                    status = status,
                    elapsed_ms = elapsed_ms,
                    retry_after_ms = retry_after.map(|d| d.as_millis() as u64),
                    error = %e,
                    request_body = %request_body,
                    response_body = %truncate_body(&text),
                    "outbound request"
                );
            } else {
                tracing::error!(
                    provider = "github",
                    operation = "search_page",
                    endpoint = %self.base_url,
                    status = status,
                    elapsed_ms = elapsed_ms,
                    error = %e,
                    request_body = %request_body,
                    response_body = %truncate_body(&text),
                    "outbound request"
                );
            }
            return Err(e);
        }

        let page = parse_github_page(json)?;
        let items = page.prs.len();

        tracing::info!(
            provider = "github",
            operation = "search_page",
            endpoint = %self.base_url,
            status = status,
            elapsed_ms = elapsed_ms,
            items = items,
            has_next_page = page.has_next_page,
            cursor = page.end_cursor.as_deref(),
            "outbound request"
        );

        Ok(page)
    }
}

/// Construct a new GitHub API client.
pub fn new_client(token: String, base_url: String) -> impl GithubApiClient {
    GithubApiClientImpl {
        client: reqwest::Client::builder()
            .user_agent("preflight")
            .build()
            .expect("build reqwest client"),
        token,
        base_url,
    }
}

/// Newtype adapter carrying the client behind an `Arc<dyn GithubApiClient>`.
///
/// `GithubSyncImpl` holds `Arc<dyn GithubApiClient>`; wrapping it in this
/// local newtype lets the shared [`SyncLoop`] (generic over
/// [`RemoteApiClient`]) drive pagination. The provider's `GithubPage` maps
/// onto the shared [`Page`] here.
#[derive(Clone)]
pub(crate) struct GithubClientAdapter(pub(crate) Arc<dyn GithubApiClient>);

#[async_trait]
impl RemoteApiClient for GithubClientAdapter {
    type Item = FetchedPr;
    type Error = GithubError;
    type Params = String;

    async fn fetch_page(
        &self,
        params: &Self::Params,
        after: Option<&str>,
    ) -> std::result::Result<Page<Self::Item>, Self::Error> {
        let page = self.0.search_page(params, after).await?;
        Ok(Page {
            items: page.prs,
            end_cursor: page.end_cursor,
            has_next_page: page.has_next_page,
        })
    }
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use parking_lot::Mutex;
    use proptest::prelude::*;

    fn sample_response(page_size: usize, cursor: Option<&str>, has_next: bool) -> Value {
        let nodes: Vec<Value> = (0..page_size)
            .map(|i| {
                serde_json::json!({
                    "number": 40 + i,
                    "title": format!("Fix #{i}"),
                    "url": format!("https://github.com/owner/repo/pull/{}", 40 + i),
                    "state": "OPEN",
                    "author": { "login": "alice" },
                    "createdAt": "2024-01-01T00:00:00Z",
                    "updatedAt": "2024-01-02T00:00:00Z",
                    "repository": { "nameWithOwner": "owner/repo" }
                })
            })
            .collect();

        serde_json::json!({
            "data": {
                "search": {
                    "pageInfo": {
                        "endCursor": cursor,
                        "hasNextPage": has_next
                    },
                    "nodes": nodes
                }
            }
        })
    }

    #[test]
    fn test_parse_github_page_single_result() {
        let json = sample_response(1, Some("Y2Vy"), true);
        let page = parse_github_page(json).unwrap();

        assert_eq!(page.prs.len(), 1);
        let pr = &page.prs[0];
        assert_eq!(pr.number, 40);
        assert_eq!(pr.title, "Fix #0");
        assert_eq!(pr.repo_owner, "owner");
        assert_eq!(pr.repo_name, "repo");
        assert_eq!(pr.author_login, Some("alice".into()));
        assert_eq!(page.end_cursor.as_deref(), Some("Y2Vy"));
        assert!(page.has_next_page);
        assert!(pr.remote_created_at.is_some());
        assert!(pr.remote_updated_at.is_some());
    }

    #[test]
    fn test_parse_github_page_empty_nodes() {
        let json = sample_response(0, None, false);
        let page = parse_github_page(json).unwrap();

        assert!(page.prs.is_empty());
        assert!(page.end_cursor.is_none());
        assert!(!page.has_next_page);
    }

    #[test]
    fn test_parse_github_page_final_page() {
        let json = sample_response(2, None, false);
        let page = parse_github_page(json).unwrap();

        assert_eq!(page.prs.len(), 2);
        assert!(!page.has_next_page);
    }

    #[test]
    fn test_parse_github_page_missing_data_returns_schema_mismatch() {
        let json = serde_json::json!({});
        let err = parse_github_page(json).unwrap_err();
        assert!(matches!(err, GithubError::SchemaMismatch(_)));
    }

    #[test]
    fn test_parse_github_page_null_author_is_ok() {
        let json = serde_json::json!({
            "data": {
                "search": {
                    "pageInfo": { "endCursor": null, "hasNextPage": false },
                    "nodes": [{
                        "number": 1,
                        "title": "PR",
                        "url": "https://example.com",
                        "state": "CLOSED",
                        "author": null,
                        "createdAt": "2024-01-01T00:00:00Z",
                        "updatedAt": "2024-01-02T00:00:00Z",
                        "repository": { "nameWithOwner": "a/b" }
                    }]
                }
            }
        });
        let page = parse_github_page(json).unwrap();
        assert_eq!(page.prs[0].author_login, None);
    }

    #[test]
    fn test_parse_github_page_states() {
        for (state_str, expected) in [
            ("OPEN", PullRequestState::Open),
            ("CLOSED", PullRequestState::Closed),
            ("MERGED", PullRequestState::Merged),
        ] {
            let json = serde_json::json!({
                "data": {
                    "search": {
                        "pageInfo": { "endCursor": null, "hasNextPage": false },
                        "nodes": [{
                            "number": 1, "title": "t", "url": "https://x",
                            "state": state_str, "author": null,
                            "createdAt": "2024-01-01T00:00:00Z",
                            "updatedAt": "2024-01-01T00:00:00Z",
                            "repository": { "nameWithOwner": "a/b" }
                        }]
                    }
                }
            });
            // parse_github_page doesn't produce PullRequestState directly,
            // but fetched_to_record does
            let page = parse_github_page(json).unwrap();
            let rec = fetched_to_record(&page.prs[0]);
            assert_eq!(rec.state, expected, "state {}", state_str);
        }
    }

    #[test]
    fn test_parse_github_page_unknown_state_returns_schema_mismatch() {
        let json = serde_json::json!({
            "data": {
                "search": {
                    "pageInfo": { "endCursor": null, "hasNextPage": false },
                    "nodes": [{
                        "number": 1, "title": "t", "url": "https://x",
                        "state": "UNKNOWN", "author": null,
                        "createdAt": "2024-01-01T00:00:00Z",
                        "updatedAt": "2024-01-01T00:00:00Z",
                        "repository": { "nameWithOwner": "a/b" }
                    }]
                }
            }
        });
        let err = parse_github_page(json).unwrap_err();
        assert!(matches!(err, GithubError::SchemaMismatch(_)));
    }

    fn pr_node(
        state: &str,
        is_draft: bool,
        review_decision: Option<&str>,
        mergeable: Option<&str>,
    ) -> Value {
        serde_json::json!({
            "number": 1,
            "title": "t",
            "url": "https://x",
            "state": state,
            "isDraft": is_draft,
            "reviewDecision": review_decision,
            "mergeable": mergeable,
            "author": null,
            "createdAt": "2024-01-01T00:00:00Z",
            "updatedAt": "2024-01-01T00:00:00Z",
            "repository": { "nameWithOwner": "a/b" }
        })
    }

    #[test]
    fn test_parse_github_page_changes_requested_true() {
        let json = serde_json::json!({
            "data": { "search": {
                "pageInfo": { "endCursor": null, "hasNextPage": false },
                "nodes": [pr_node("OPEN", false, Some("CHANGES_REQUESTED"), Some("MERGEABLE"))]
            }}
        });
        let page = parse_github_page(json).unwrap();
        assert!(page.prs[0].changes_requested);
    }

    #[test]
    fn test_parse_github_page_changes_requested_false_when_approved() {
        let json = serde_json::json!({
            "data": { "search": {
                "pageInfo": { "endCursor": null, "hasNextPage": false },
                "nodes": [pr_node("OPEN", false, Some("APPROVED"), Some("MERGEABLE"))]
            }}
        });
        let page = parse_github_page(json).unwrap();
        assert!(!page.prs[0].changes_requested);
    }

    #[test]
    fn test_parse_github_page_merge_conflicts_true() {
        let json = serde_json::json!({
            "data": { "search": {
                "pageInfo": { "endCursor": null, "hasNextPage": false },
                "nodes": [pr_node("OPEN", false, None, Some("CONFLICTING"))]
            }}
        });
        let page = parse_github_page(json).unwrap();
        assert!(page.prs[0].merge_conflicts);
    }

    #[test]
    fn test_parse_github_page_merge_conflicts_false_when_mergeable() {
        let json = serde_json::json!({
            "data": { "search": {
                "pageInfo": { "endCursor": null, "hasNextPage": false },
                "nodes": [pr_node("OPEN", false, None, Some("MERGEABLE"))]
            }}
        });
        let page = parse_github_page(json).unwrap();
        assert!(!page.prs[0].merge_conflicts);
    }

    #[test]
    fn test_parse_github_page_draft_state_overrides_open() {
        let json = serde_json::json!({
            "data": { "search": {
                "pageInfo": { "endCursor": null, "hasNextPage": false },
                "nodes": [pr_node("OPEN", true, None, None)]
            }}
        });
        let page = parse_github_page(json).unwrap();
        assert_eq!(page.prs[0].state, "draft");
        assert!(page.prs[0].is_draft);
        let rec = fetched_to_record(&page.prs[0]);
        assert_eq!(rec.state, PullRequestState::Draft);
    }

    #[test]
    fn test_parse_github_page_copilot_comments_always_false() {
        let json = serde_json::json!({
            "data": { "search": {
                "pageInfo": { "endCursor": null, "hasNextPage": false },
                "nodes": [pr_node("OPEN", false, None, None)]
            }}
        });
        let page = parse_github_page(json).unwrap();
        assert!(!page.prs[0].copilot_comments);
    }

    #[test]
    fn test_parse_github_page_null_review_decision_and_mergeable() {
        let json = serde_json::json!({
            "data": { "search": {
                "pageInfo": { "endCursor": null, "hasNextPage": false },
                "nodes": [pr_node("OPEN", false, None, None)]
            }}
        });
        let page = parse_github_page(json).unwrap();
        assert!(!page.prs[0].changes_requested);
        assert!(!page.prs[0].merge_conflicts);
    }

    #[test]
    fn test_fetched_to_record_maps_correctly() {
        let fetched = FetchedPr {
            repo_owner: "org".into(),
            repo_name: "app".into(),
            number: 5,
            title: "Fix".into(),
            url: "https://github.com/org/app/pull/5".into(),
            author_login: Some("bob".into()),
            state: "open".into(),
            is_draft: true,
            review_requested: true,
            requested_reviewer_teams: vec![],
            authored_by_me: true,
            remote_created_at: None,
            remote_updated_at: None,
            changes_requested: true,
            copilot_comments: false,
            merge_conflicts: true,
        };
        let rec = fetched_to_record(&fetched);
        assert_eq!(rec.provider, "github");
        assert_eq!(rec.owner, "org");
        assert_eq!(rec.repo, "app");
        assert_eq!(rec.number, 5);
        assert_eq!(rec.title, "Fix");
        assert_eq!(rec.author, Some("bob".into()));
        assert!(rec.authored_by_me);
        assert!(rec.review_requested);
        assert_eq!(rec.state, PullRequestState::Draft);
        assert!(rec.changes_requested);
        assert!(!rec.copilot_comments);
        assert!(rec.merge_conflicts);
    }

    // parse_github_page only panics if it has an unreachable code path; all
    // error branches return SchemaMismatch. Exercise arbitrary JSON.
    proptest! {
        #[test]
        fn parse_github_page_never_panics(raw in ".*") {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
                let _ = parse_github_page(val);
            }
        }
    }

    // ── outbound-request logging tests ──────────────────────────────

    pub(crate) struct LogCaptureWriter {
        inner: Arc<Mutex<Vec<u8>>>,
    }

    impl std::io::Write for LogCaptureWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let mut guard = self.inner.lock();
            guard.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Clone for LogCaptureWriter {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }

    pub(crate) struct LogCaptureMakeWriter {
        inner: Arc<Mutex<Vec<u8>>>,
    }

    impl LogCaptureMakeWriter {
        pub(crate) fn new(inner: Arc<Mutex<Vec<u8>>>) -> Self {
            Self { inner }
        }
    }

    impl Default for LogCaptureMakeWriter {
        fn default() -> Self {
            Self {
                inner: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl Clone for LogCaptureMakeWriter {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
            }
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogCaptureMakeWriter {
        type Writer = LogCaptureWriter;
        fn make_writer(&'a self) -> Self::Writer {
            LogCaptureWriter {
                inner: self.inner.clone(),
            }
        }
    }

    fn install_capture(buffer: Arc<Mutex<Vec<u8>>>) -> tracing::subscriber::DefaultGuard {
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing_subscriber::filter::LevelFilter::TRACE)
            .with_writer(LogCaptureMakeWriter::new(buffer))
            .with_ansi(false)
            .finish();
        tracing::subscriber::set_default(subscriber)
    }

    #[tokio::test]
    async fn test_outbound_request_log_has_stats_and_no_body() {
        let response_body = serde_json::json!({
            "data": {
                "search": {
                    "pageInfo": { "endCursor": "cur1", "hasNextPage": false },
                    "nodes": [
                        {
                            "number": 40,
                            "title": "Fix",
                            "url": "https://github.com/owner/repo/pull/40",
                            "state": "OPEN",
                            "author": { "login": "alice" },
                            "createdAt": "2024-01-01T00:00:00Z",
                            "updatedAt": "2024-01-02T00:00:00Z",
                            "repository": { "nameWithOwner": "owner/repo" }
                        }
                    ]
                }
            }
        });

        let server = httptest::Server::run();
        server.expect(
            httptest::Expectation::matching(httptest::matchers::any())
                .respond_with(httptest::responders::json_encoded(response_body)),
        );
        let url = server.url("/graphql").to_string();

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let _guard = install_capture(buffer.clone());

        let client = new_client("token".to_string(), url.clone());
        let _result = client.search_page("repo:owner/repo", None).await;

        let bytes = buffer.lock();
        let log_text = String::from_utf8_lossy(&bytes);

        assert!(
            log_text.contains("\"message\":\"outbound request\""),
            "expected outbound request log message, got: {log_text}"
        );
        assert!(
            log_text.contains("\"provider\":\"github\""),
            "expected provider=github, got: {log_text}"
        );
        assert!(
            log_text.contains("\"operation\":\"search_page\""),
            "expected operation=search_page, got: {log_text}"
        );
        assert!(
            log_text.contains("\"endpoint\""),
            "expected endpoint field, got: {log_text}"
        );
        assert!(
            log_text.contains("\"status\":200"),
            "expected status=200, got: {log_text}"
        );
        assert!(
            log_text.contains("\"elapsed_ms\""),
            "expected elapsed_ms field, got: {log_text}"
        );
        assert!(
            log_text.contains("\"items\""),
            "expected items field, got: {log_text}"
        );
        // Ensure request/response body is NOT logged
        assert!(
            !log_text.contains("query") && !log_text.contains("GraphQL"),
            "body should not be logged, got: {log_text}"
        );
    }

    #[tokio::test]
    async fn test_error_log_dumps_request_and_response_bodies() {
        let server = httptest::Server::run();
        server.expect(
            httptest::Expectation::matching(httptest::matchers::request::method_path("POST", "/"))
                .times(1)
                .respond_with(
                    httptest::responders::status_code(403)
                        .body("Request forbidden: missing User-Agent"),
                ),
        );
        let url = server.url("/").to_string();

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let _guard = install_capture(buffer.clone());

        let client = new_client("token".to_string(), url.clone());
        let _result = client.search_page("repo:owner/repo", None).await;

        let bytes = buffer.lock();
        let log_text = String::from_utf8_lossy(&bytes);

        // The HTTP-level 403 error log must carry both bodies so the real
        // upstream message (e.g. missing User-Agent) is not lost.
        assert!(
            log_text.contains("\"status\":403"),
            "expected status=403, got: {log_text}"
        );
        assert!(
            log_text.contains("repo:owner/repo"),
            "expected request body in log, got: {log_text}"
        );
        assert!(
            log_text.contains("Request forbidden: missing User-Agent"),
            "expected response body in log, got: {log_text}"
        );
    }
}
