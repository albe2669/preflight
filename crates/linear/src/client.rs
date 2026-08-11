//! Linear API client trait and response types.

use std::sync::Arc;

use remote_sync::page::Page;
use remote_sync::sync::RemoteApiClient;

use async_trait::async_trait;

use crate::error::{LinearError, Result};
use crate::sync::IssueRecord;

/// A page of issues returned by the Linear API.
#[derive(Clone, Debug)]
pub struct LinearPage {
    pub issues: Vec<IssueRecord>,
    pub end_cursor: Option<String>,
    pub has_next_page: bool,
}

/// Trait for the Linear API client.
///
/// Other crates depend on this trait, never the concrete implementation.
#[async_trait]
pub trait LinearApiClient: Send + Sync {
    /// Fetch a page of issues from Linear.
    async fn issues_page(
        &self,
        filter: &serde_json::Value,
        after: Option<&str>,
    ) -> Result<LinearPage>;
}

/// Parse a Linear GraphQL `issues` response into a [`LinearPage`].
///
/// The JSON should be the full response body from the GraphQL endpoint,
/// with `data.issues.nodes` containing the issue objects and
/// `data.issues.pageInfo` containing pagination info.
pub(crate) fn parse_linear_page(json: serde_json::Value) -> Result<LinearPage> {
    let data = json
        .as_object()
        .and_then(|o| o.get("data"))
        .and_then(|v| v.as_object())
        .ok_or(LinearError::SchemaMismatch)?;

    let issues_obj = data
        .get("issues")
        .and_then(|v| v.as_object())
        .ok_or(LinearError::SchemaMismatch)?;

    // Parse pageInfo
    let (end_cursor, has_next_page) = issues_obj
        .get("pageInfo")
        .and_then(|v| v.as_object())
        .map(|pi| {
            let end_cursor = pi
                .get("endCursor")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let has_next_page = pi
                .get("hasNextPage")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            (end_cursor, has_next_page)
        })
        .unwrap_or((None, false));

    // Parse nodes
    let empty: Vec<serde_json::Value> = Vec::new();
    let nodes = issues_obj
        .get("nodes")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);

    let issues: Vec<IssueRecord> = nodes.iter().filter_map(parse_issue_node).collect();

    Ok(LinearPage {
        issues,
        end_cursor,
        has_next_page,
    })
}

fn parse_issue_node(node: &serde_json::Value) -> Option<IssueRecord> {
    let obj = node.as_object()?;

    let linear_id = obj.get("id")?.as_str()?.to_string();
    let identifier = obj.get("identifier")?.as_str()?.to_string();
    let title = obj.get("title")?.as_str()?.to_string();
    let description = obj
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let url = obj.get("url")?.as_str()?.to_string();

    let state_name = obj
        .get("state")
        .and_then(|s| s.as_object())
        .and_then(|s| s.get("name")?.as_str())
        .unwrap_or("")
        .to_string();

    let state_type = obj
        .get("state")
        .and_then(|s| s.as_object())
        .and_then(|s| s.get("type")?.as_str())
        .unwrap_or("")
        .to_string();

    let priority = obj.get("priority").and_then(|v| v.as_i64());
    let team_key = obj
        .get("team")
        .and_then(|t| t.as_object())
        .and_then(|t| t.get("key")?.as_str())
        .map(|s| s.to_string());

    let assignee_name = obj
        .get("assignee")
        .and_then(|a| a.as_object())
        .and_then(|a| a.get("name")?.as_str())
        .map(|s| s.to_string());

    // assigned_to_me defaults false — the sync doesn't know "me" without an identity query
    let assigned_to_me = false;

    let remote_created_at = obj
        .get("createdAt")
        .and_then(|v| v.as_str())
        .and_then(|s| sea_orm::entity::prelude::DateTimeWithTimeZone::parse_from_rfc3339(s).ok());

    let remote_updated_at = obj
        .get("updatedAt")
        .and_then(|v| v.as_str())
        .and_then(|s| sea_orm::entity::prelude::DateTimeWithTimeZone::parse_from_rfc3339(s).ok());

    Some(IssueRecord {
        linear_id,
        identifier,
        title,
        description,
        url,
        state_name,
        state_type,
        priority,
        team_key,
        assignee_name,
        assigned_to_me,
        remote_created_at,
        remote_updated_at,
    })
}

/// Concrete Linear API client — private outside the crate.
#[derive(Clone)]
pub(crate) struct LinearApiClientImpl {
    client: reqwest::Client,
    token: String,
    base_url: String,
}

/// Construct a new Linear API client.
pub fn new_client(token: String, base_url: String) -> impl LinearApiClient {
    LinearApiClientImpl {
        client: reqwest::Client::new(),
        token,
        base_url,
    }
}

#[async_trait]
impl LinearApiClient for LinearApiClientImpl {
    async fn issues_page(
        &self,
        filter: &serde_json::Value,
        after: Option<&str>,
    ) -> Result<LinearPage> {
        let start = std::time::Instant::now();

        let mut variables = serde_json::Map::new();
        variables.insert("filter".into(), filter.clone());
        if let Some(cursor) = after {
            variables.insert(
                "after".into(),
                serde_json::Value::String(cursor.to_string()),
            );
        } else {
            variables.insert("after".into(), serde_json::Value::Null);
        }

        let body = serde_json::json!({
            "query": self.query(),
            "variables": variables,
        });

        let response = self
            .client
            .post(&self.base_url)
            .header("Authorization", &self.token)
            .header("Content-Type", "application/json")
            .json(&body)
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
                    provider = "linear",
                    operation = "issues_page",
                    endpoint = %self.base_url,
                    status = 0u16,
                    elapsed_ms = elapsed_ms,
                    error = %err_str,
                    "outbound request"
                );
                return Err(LinearError::Remote(err_str));
            }
        };

        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .map(std::time::Duration::from_secs);

        // Check HTTP-level errors first (401, 429, 5xx) — the body may not
        // be JSON on these paths.
        let http_err = crate::error::map_response_error(status, retry_after, vec![]);

        let body_text = response.text().await.unwrap_or_default();

        // If HTTP-level error, log it and return before parsing body.
        if let Err(e) = http_err {
            if let LinearError::RateLimited { retry_after } = &e {
                tracing::warn!(
                    provider = "linear",
                    operation = "issues_page",
                    endpoint = %self.base_url,
                    status = status,
                    elapsed_ms = elapsed_ms,
                    retry_after_ms = retry_after.map(|d| d.as_millis() as u64),
                    error = %e,
                    "outbound request"
                );
            } else {
                tracing::error!(
                    provider = "linear",
                    operation = "issues_page",
                    endpoint = %self.base_url,
                    status = status,
                    elapsed_ms = elapsed_ms,
                    error = %e,
                    "outbound request"
                );
            }
            return Err(e);
        }

        let json: serde_json::Value =
            serde_json::from_str(&body_text).map_err(|e| LinearError::Remote(e.to_string()))?;

        // Extract GraphQL errors if present
        let graphql_errors: Vec<String> = json
            .get("errors")
            .and_then(|e| e.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        e.get("message")
                            .and_then(|m| m.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();

        let gql_err = crate::error::map_response_error(status, retry_after, graphql_errors.clone());

        if let Err(e) = gql_err {
            if let LinearError::RateLimited { retry_after } = &e {
                tracing::warn!(
                    provider = "linear",
                    operation = "issues_page",
                    endpoint = %self.base_url,
                    status = status,
                    elapsed_ms = elapsed_ms,
                    retry_after_ms = retry_after.map(|d| d.as_millis() as u64),
                    error = %e,
                    "outbound request"
                );
            } else {
                tracing::error!(
                    provider = "linear",
                    operation = "issues_page",
                    endpoint = %self.base_url,
                    status = status,
                    elapsed_ms = elapsed_ms,
                    error = %e,
                    "outbound request"
                );
            }
            return Err(e);
        }

        let page = parse_linear_page(json)?;
        let items = page.issues.len();

        tracing::info!(
            provider = "linear",
            operation = "issues_page",
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

impl LinearApiClientImpl {
    fn query(&self) -> String {
        r#"
        query Issues($filter: IssueFilter, $after: String) {
          issues(filter: $filter, first: 100, after: $after) {
            pageInfo {
              endCursor
              hasNextPage
            }
            nodes {
              id
              identifier
              title
              description
              url
              state {
                name
                type
              }
              priority
              team {
                key
              }
              assignee {
                name
              }
              createdAt
              updatedAt
            }
          }
        }
        "#
        .to_string()
    }
}

/// Newtype adapter carrying the client behind an `Arc<dyn LinearApiClient>`.
///
/// `new` receives `impl LinearApiClient`; boxing it as a trait object and
/// wrapping in this local newtype lets the shared [`SyncLoop`] (generic over
/// [`RemoteApiClient`]) drive it. The provider's `LinearPage` maps onto the
/// shared [`Page`] here.
#[derive(Clone)]
pub(crate) struct LinearClientAdapter(pub(crate) Arc<dyn LinearApiClient>);

#[async_trait]
impl RemoteApiClient for LinearClientAdapter {
    type Item = IssueRecord;
    type Error = LinearError;
    type Params = serde_json::Value;

    async fn fetch_page(
        &self,
        params: &Self::Params,
        after: Option<&str>,
    ) -> Result<Page<Self::Item>, Self::Error> {
        let page = self.0.issues_page(params, after).await?;
        Ok(Page {
            items: page.issues,
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

    fn sample_response() -> serde_json::Value {
        serde_json::json!({
            "data": {
                "issues": {
                    "pageInfo": { "endCursor": "xyz", "hasNextPage": true },
                    "nodes": [
                        {
                            "id": "issue-uuid-1",
                            "identifier": "ENG-123",
                            "title": "Fix bug",
                            "description": "details",
                            "url": "https://linear.app/issue/ENG-123",
                            "state": { "name": "In Progress", "type": "started" },
                            "priority": 2,
                            "team": { "key": "ENG" },
                            "assignee": { "name": "alice" },
                            "createdAt": "2024-01-01T00:00:00Z",
                            "updatedAt": "2024-01-02T00:00:00Z"
                        }
                    ]
                }
            }
        })
    }

    #[test]
    fn test_parse_linear_page_maps_issue_fields() {
        let page = parse_linear_page(sample_response()).unwrap();
        assert_eq!(page.issues.len(), 1);
        let issue = &page.issues[0];
        assert_eq!(issue.linear_id, "issue-uuid-1");
        assert_eq!(issue.identifier, "ENG-123");
        assert_eq!(issue.title, "Fix bug");
        assert_eq!(issue.description, Some("details".into()));
        assert_eq!(issue.url, "https://linear.app/issue/ENG-123");
        assert_eq!(issue.state_name, "In Progress");
        assert_eq!(issue.state_type, "started");
        assert_eq!(issue.priority, Some(2));
        assert_eq!(issue.team_key, Some("ENG".into()));
        assert_eq!(issue.assignee_name, Some("alice".into()));
        assert!(!issue.assigned_to_me);
        assert!(issue.remote_created_at.is_some());
        assert!(issue.remote_updated_at.is_some());
    }

    #[test]
    fn test_parse_linear_page_pagination() {
        let page = parse_linear_page(sample_response()).unwrap();
        assert_eq!(page.end_cursor, Some("xyz".into()));
        assert!(page.has_next_page);
    }

    #[test]
    fn test_parse_linear_page_null_description() {
        let json = serde_json::json!({
            "data": {
                "issues": {
                    "pageInfo": { "endCursor": null, "hasNextPage": false },
                    "nodes": [
                        {
                            "id": "issue-null-desc",
                            "identifier": "ENG-0",
                            "title": "No desc",
                            "description": null,
                            "url": "https://linear.app/issue/ENG-0",
                            "state": { "name": "Todo", "type": "triage" },
                            "priority": null,
                            "team": null,
                            "assignee": null,
                            "createdAt": null,
                            "updatedAt": null
                        }
                    ]
                }
            }
        });
        let page = parse_linear_page(json).unwrap();
        let issue = &page.issues[0];
        assert_eq!(issue.description, None);
        assert_eq!(issue.priority, None);
        assert_eq!(issue.team_key, None);
        assert_eq!(issue.assignee_name, None);
        assert_eq!(issue.remote_created_at, None);
        assert_eq!(issue.remote_updated_at, None);
        assert!(!page.has_next_page);
    }

    #[test]
    fn test_parse_linear_page_empty_nodes() {
        let json = serde_json::json!({
            "data": {
                "issues": {
                    "pageInfo": { "endCursor": null, "hasNextPage": false },
                    "nodes": []
                }
            }
        });
        let page = parse_linear_page(json).unwrap();
        assert!(page.issues.is_empty());
        assert!(!page.has_next_page);
    }

    #[test]
    fn test_parse_linear_page_missing_data_returns_schema_mismatch() {
        let json = serde_json::json!({"not_data": {}});
        let result = parse_linear_page(json);
        assert!(matches!(result, Err(LinearError::SchemaMismatch)));
    }

    #[test]
    fn test_parse_linear_page_missing_issues_returns_schema_mismatch() {
        let json = serde_json::json!({"data": {}});
        let result = parse_linear_page(json);
        assert!(matches!(result, Err(LinearError::SchemaMismatch)));
    }

    #[test]
    fn test_parse_linear_page_invalid_json() {
        let json = serde_json::json!("not an object");
        let result = parse_linear_page(json);
        assert!(matches!(result, Err(LinearError::SchemaMismatch)));
    }

    proptest! {
        #[test]
        fn parse_linear_page_never_panics(raw in ".*") {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
                let _ = parse_linear_page(val);
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
                "issues": {
                    "pageInfo": { "endCursor": "cur1", "hasNextPage": false },
                    "nodes": [
                        {
                            "id": "test-id",
                            "identifier": "ENG-1",
                            "title": "Hello",
                            "description": null,
                            "url": "https://linear.app/ENG-1",
                            "state": { "name": "Todo", "type": "triage" },
                            "priority": null,
                            "team": null,
                            "assignee": null,
                            "createdAt": null,
                            "updatedAt": null
                        }
                    ]
                }
            }
        });

        // Spawn an httptest server that returns a valid Linear issues response
        let server = httptest::Server::run();
        server.expect(
            httptest::Expectation::matching(httptest::matchers::any())
                .respond_with(httptest::responders::json_encoded(response_body)),
        );
        let url = server.url("/graphql").to_string();

        // Capture log output
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let _guard = install_capture(buffer.clone());

        // Call the client
        let client = new_client("Bearer test".to_string(), url.clone());
        let filter = serde_json::json!({});
        let _result = client.issues_page(&filter, None).await;

        let bytes = buffer.lock();
        let log_text = String::from_utf8_lossy(&bytes);

        // Assert outbound request event fields are present
        assert!(
            log_text.contains("\"message\":\"outbound request\""),
            "expected outbound request log message, got: {log_text}"
        );
        assert!(
            log_text.contains("\"provider\":\"linear\""),
            "expected provider=linear, got: {log_text}"
        );
        assert!(
            log_text.contains("\"operation\":\"issues_page\""),
            "expected operation=issues_page, got: {log_text}"
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
}
