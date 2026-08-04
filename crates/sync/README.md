# sync

One-way remote pullers for GitHub and Linear.

Reads from each provider, upserts local inbox tables (`pull_request`,
`linear_issue`), and persists cursor state in `sync_state`. Tokens may be
empty — `pull` is a no-op then (cursor marked "never") so the server boots
without network.

## Exposed API

Each source is a **producer-side trait**:

- `GithubSync` — `pull()` upserts open PRs and returns the updated `sync_state` row.
- `LinearSync` — `pull()` upserts issues and returns the updated `sync_state` row.

Concrete impls are `pub(crate)`; construct via:

- `github::new(db, GithubOptions { token, query }) -> impl GithubSync`
- `linear::new(db, LinearOptions { token, team_keys }) -> impl LinearSync`

Options structs (`GithubOptions`, `LinearOptions`) are `Default`-able and
cloneable — required fields go in the struct, not a long parameter list
(no builder pattern).

Error type: `SyncError` (thiserror, wraps `DbErr` via `#[from]`). `Result<T>`
is re-exported.

## Internal flow

`cursor.rs` owns the `sync_state` upsert helpers (`get`/`put`). Each puller
checks for an empty token (no-op), otherwise hits the provider API and calls
`upsert_pr` / `upsert_issue` per remote row. Upsert helpers take a `PrRecord`
/ `IssueRecord` struct (not a long arg list) and are idempotent on the
provider identity key.

The real HTTP bodies are stubs marked `TODO(network)`; the no-token path and
cursor persistence are real.

## External flow

`graphql` consumes `Arc<dyn GithubSync>` / `Arc<dyn LinearSync>` injected via
the schema context and wired in `server/main` from the `SyncConfig`. This
crate does **not** depend on `core`.
