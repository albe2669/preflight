# linear

The Linear sync provider: fetches issues over the Linear GraphQL API and
makes them available to the app.

Sync is a **full refresh**: every `pull()` starts from page one and
pagination runs to the end. Pages are buffered and committed atomically —
if a later page fails, nothing is written and `PartialResults` is
returned.

## Exposed API

- `new_client(token, base_url) -> impl LinearApiClient` — the reqwest-backed
  client. `base_url` defaults to Linear's API in `main`; override to point at
  an integration-test server. Requests send `Authorization: <token>` (raw
  token, not Bearer — per Linear's API).
- `LinearApiClient::issues_page(filter, after) -> LinearPage` — one page of
  `IssueRecord`s plus pagination info.
- `LinearSync` trait — `pull() -> Result<sync_state::Model>`: the pull loop.
  Concrete impl is `pub(crate)`; callers get `pub fn new(db, client, opts)
  -> impl LinearSync`.
- `LinearOptions { token, filters }`.

## Error taxonomy (`LinearError`)

- `Unauthorized` — 401.
- `RateLimited { retry_after }` — 429 (parsed from the `Retry-After` header).
- `Remote(String)` — 5xx, GraphQL `errors` in the response body, or a
  transport failure. Surfaced directly, never retried internally.
- `SchemaMismatch` — response didn't match the expected shape.
- `PartialResults` — a mid-pagination page failed after at least one page was
  already fetched; no rows are committed.
- `Db`, `NotFound`, `PaginationCursorInvalid`, `ContentFiltered` for local
  failures.

The client checks the HTTP status before parsing the body, so 401/429 map
correctly even when the body is not JSON. A first-page failure surfaces the
actual error type above; only failures after the first page map to
`PartialResults`.

## Post-fetch filtering

`filters.rs` compiles `<LinearFilter>` rules (team, assignee, …) into the
GraphQL `issues(filter:)` variables. Filtering happens server-side in the
query; records are buffered as fetched.

## Integration tests

`tests/linear_integration.rs` runs under `--features integration`: it points
the real reqwest client at `httptest::Server` fixtures speaking the Linear
GraphQL wire format, backed by a real SQLite database. It covers request
encoding, single/multi-page cursor pagination, error mapping, and
atomicity.
