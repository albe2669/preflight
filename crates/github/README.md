# github

The GitHub sync provider: fetches pull requests over the GitHub GraphQL
API and makes them available to the app.

Sync is a **full refresh**: every `pull()` starts from page one and
pagination runs to the end. Pages are buffered and committed atomically —
if a later page fails, nothing is written and `PartialResults` is
returned.

## Exposed API

- `new_client(token, base_url) -> impl GithubApiClient` — the reqwest-backed
  client. `base_url` defaults to GitHub's API in `main`; override to point at
  an integration-test server.
- `GithubApiClient::search_page(q, after) -> GithubPage` — one page of
  `FetchedPr`s (owner, repo, number, title, url, author, state, draft,
  authored-by-me, review-requested, remote timestamps) plus pagination info.
- `GithubSync` trait — `pull() -> Result<sync_state::Model>`: the pull loop.
  Concrete impl is `pub(crate)`; callers get `pub fn new(db, client, opts)
  -> impl GithubSync`.
- `GithubOptions { token, filters, exclude_drafts_unless_authored_by_me }`.

## Error taxonomy (`GithubError`)

- `Unauthorized` — 401.
- `RateLimited { retry_after }` — 429 (parsed from the `Retry-After` header).
- `Remote(String)` — 5xx or GraphQL `errors` in the response body. Surfaced
  directly, never retried internally.
- `SchemaMismatch(String)` — response didn't match the expected shape.
- `PartialResults` — a mid-pagination page failed after at least one page was
  already fetched; no rows are committed.
- `Db`, `NotFound`, `PaginationCursorInvalid`, `ContentFiltered` for local
  failures.

A first-page failure surfaces the actual error type above; only failures
after the first page map to `PartialResults`.

## Post-fetch filtering

`filters.rs` compiles `<GithubFilter>` rules into a GraphQL `search` query
(`ISSUE_ADVANCED`) and provides `apply_draft_policy` (drop drafts unless
authored by me) which runs on fetched records before upsert.

## Integration tests

`tests/github_integration.rs` runs under `--features integration`: it points
the real reqwest client at `httptest::Server` fixtures speaking the GitHub
GraphQL wire format, backed by a real SQLite database. It covers request
encoding, single/multi-page cursor pagination, error mapping, and
atomicity.
