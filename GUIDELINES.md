# Rust development guidelines

This document defines the architecture, coding, and testing conventions for
this codebase. Every rule is intended to be followed literally by both humans
and AI agents. When a rule here conflicts with general Rust folklore, this
document wins.

## Core principles

1. **One crate per domain.** Each domain (`todo`, `github`, `linear`) is its
   own crate under `crates/`. A domain crate owns its entities, its enums,
   its traits, its errors, and its sync/client logic. It is self-contained: a
   different project could depend on `crates/github` and get pull-request
   sync without pulling in `todo` or `linear`.
2. **Domain crates never depend on each other.** `todo` does not know about
   `github` or `linear`. Cross-domain relationships — join tables, `Related`
   impls, link/convert operations — live in the `links` composition crate,
   which is the only place allowed to depend on two or more domain crates at
   once. This is what makes domain crates reusable: no incoming domain edges.
3. **Producer-side traits.** Every crate consumed across a boundary exports
   traits that are the *only* thing other crates may depend on. Concrete
   implementations are `pub(crate)`; the only public constructor is a
   module-level `pub fn new(...) -> impl Trait`. Callers can never name the
   concrete type.
4. **TDD, always.** No production code without a failing test first. Unit
   tests are fast and mock-based; integration tests use a real SQLite
   database.
5. **Explicit over magic.** Manual dependency injection, SeaORM queries,
   axum routing. No DI containers, no hidden state, no globals.
6. **The compiler is the safety net.** Prefer designs where mistakes fail
   `cargo build`, then designs where mistakes fail `cargo test`, and only
   then designs that rely on review.

## Repository layout

```
.
├── Cargo.toml                 # [workspace] — members, shared deps
├── crates/
│   ├── todo/                   # todo domain — reusable (lib name: todo_domain)
│   ├── github/                 # github domain — reusable
│   ├── linear/                 # linear domain — reusable
│   ├── links/                 # cross-domain composition — app-specific
│   ├── remote-sync/            # shared sync plumbing (pagination, error mapping)
│   ├── sync-state/             # sync_state entity shared by providers
│   ├── migration/             # sea-orm-migrate (all migrations, single Migrator)
│   ├── graphql/                # seaography roots + hand-written mutations
│   ├── server/                 # axum server, wiring, lifecycle
│   └── tui/                    # terminal client (pftui)
├── frontend/                  # web client (Vite/React)
├── raycast/                   # Raycast extension client
├── api/schema.graphql         # generated GraphQL schema (committed)
├── config/default.toml        # app config (no secrets)
├── devenv.nix                 # devenv tasks + git hooks
├── flake.nix                  # Nix: packages.preflight + homeManagerModules.preflight
└── nix/                       # packaging, config serializer, home-manager module
```

Rules:

* `publish = false` for every crate. Keep `crates/` flat — no grouping
  directories. A new concern is a new top-level crate.
* **A domain crate owns its entities.** There is no single `entity` crate
  spanning all domains. Each domain crate has an `entity/` submodule with the
  SeaORM models for the tables it owns. `sync-state` is a tiny shared
  dependency-free crate that owns the `sync_state` table both providers upsert.
* **Join tables that bridge two domains live in `links`, never in a domain
  crate.** `todo_pull_request` references both `todo` and `pull_request`; it
  belongs in `links` because neither `todo` nor `github` should depend on the
  other.
* Generated code (sea-orm entities, seaography output) is committed and
  regenerated with `devenv tasks run gen:entities` / `server:generate-server`.
  Never edit it by hand.
* `crates/server/src/main.rs` contains wiring and lifecycle only — no
  business logic, no GraphQL handlers, no SQL.
* `crates/tui` is a binary crate (terminal client) that talks to the server
  over GraphQL. `crates/server` is the only binary that constructs the
  dependency graph.

### Dependency graph

```
todo    github    linear         ← domain crates, no edges between them
  \      /          /
   links              ← composition: depends on todo + github + linear
    |
  graphql             ← depends on domain traits + links
    |
  server              ← depends on everything, wires it
```

`remote-sync` and `sync-state` sit beneath the providers: `github` and
`linear` depend on them, they never depend on a domain crate. No arrow points
from a domain crate to another domain crate. That invariant is what
"reusable" means here.

## Crate design: domains and traits

### Documentation

Every crate should have a `README.md` at its top-level describing the exposed
functionality and the internal/external flows.

### The domain crate contract

Every domain crate follows the same shape. Using `crates/github` as the
template:

```rust
// crates/github/src/error.rs
use thiserror::Error;

pub type Result<T> = std::result::Result<T, GithubError>;

#[derive(Debug, Error)]
pub enum GithubError {
    #[error("database error: {0}")]
    Db(#[from] sea_orm::DbErr),
    #[error("sync state not found for github")]
    NotFound,
    #[error("rate limited, retry after {retry_after:?}")]
    RateLimited { retry_after: Option<Duration> },
    #[error("unauthorized")]
    Unauthorized,
    #[error("remote error: {0}")]
    Remote(String),
    #[error("pagination cursor invalid")]
    PaginationCursorInvalid,
    #[error("partial results returned")]
    PartialResults,
    #[error("content filtered out")]
    ContentFiltered,
    #[error("schema mismatch: {0}")]
    SchemaMismatch(String),
}
```

```rust
// crates/github/src/sync.rs
use sea_orm::DatabaseConnection;

/// The trait consumed by the GraphQL layer and the links crate.
/// A domain crate's public API — the only names other crates may reference —
/// is: the trait(s), the error type, the entity models, the enums, and the
/// constructor (constructor calls belong in server/main only).
#[async_trait]
pub trait GithubSync: Send + Sync {
    async fn pull(&self) -> Result<sync_state::Model>;
}

/// Configuration for the GitHub sync provider.
#[derive(Clone, Debug, Default)]
pub struct GithubOptions {
    pub token: String,
    pub filters: Vec<crate::filters::GithubFilter>,
    pub exclude_drafts_unless_authored_by_me: bool,
}

/// Concrete implementation — private outside the crate.
pub(crate) struct GithubSyncImpl {
    db: DatabaseConnection,
    client: GithubClientAdapter,
    opts: GithubOptions,
}

/// Constructor — returns the trait so callers cannot depend on the concrete type.
pub fn new(
    db: DatabaseConnection,
    client: Arc<dyn GithubApiClient>,
    opts: GithubOptions,
) -> impl GithubSync {
    GithubSyncImpl { db, client, opts }
}
```

Rules:

* **At least one trait per consumed crate**, named for the role
  (`GithubSync`, `TodoService`, `DayPlanService`, `LinkService`,
  `GithubApiClient`), not `ITodo` or `Interface`.
* Traits that cross a crate boundary are `Send + Sync` and use
  `#[async_trait]` so `Arc<dyn Trait>` works. Native `async fn` in traits is
  acceptable only for traits that are never used as `dyn` (the repo currently
  uses `#![allow(async_fn_in_trait)]` in `todo` for non-`dyn` traits; keep
  that local).
* Traits speak **domain types** — the crate's own entity `Model` types and
  enums — never driver types. `sea_orm::DbErr`, `QueryResult`, or
  `reqwest::Response` must not appear in any exported signature. Wrap them
  in types owned by the crate.
* Keep traits as small as the current callers need. Add methods when a real
  caller needs them (test-first), not speculatively.
* If a crate grows a second distinct responsibility, split it into a second
  trait within the crate (or a second crate) rather than widening the
  interface.
* Concrete impl types are `pub(crate)`; all fields private; the only way to
  build one is the public `new` constructor. They may be `pub` only when a
  binary or test harness must name them — never in a library's public API.
* File layout inside a crate (no file is mandatory; if present it serves a
  specific purpose):
  * `lib.rs`: crate docs, module declarations, `pub use` re-exports of
    public traits/types only.
  * `error.rs`: sentinel errors the crate can emit, for consumption and
    handling by other crates.
  * `entity/mod.rs`: the SeaORM entity module(s) this crate owns, plus
    `seaography::register_entity_modules!` when served via GraphQL.
  * `entity/enums.rs`: hand-maintained `ActiveEnum` types.
  * `client.rs`: the HTTP API client for a remote-sync domain.
  * `cursor.rs`: pagination/cursor logic for a remote-sync domain.
  * `sync.rs`: the pull/upsert loop and the sync trait for a remote-sync
    domain.
* No `traits.rs` dumping ground. A trait lives in the module that owns its
  implementation — the client trait next to the client.

### The composition crate: `links`

`links` is the only crate allowed to depend on more than one domain crate.
It owns:

* **Join-table entities** — `todo_pull_request`, `todo_linear_issue` — whose
  foreign keys point into two different domain crates' tables.
* **Cross-domain `Related` impls** — `impl Related<github::entity::PullRequest>
  for todo::entity::Todo`. These cannot live in either domain crate because
  they would create a domain-to-domain edge.
* **Link/convert services** — `todo_from_pr`, `link_pr`, `unlink_pr`,
  `add_tag`, `remove_tag`, `todo_from_linear` — which call into domain
  entities and the `DayPlanService` trait from the `todo` side.

Rules:

* `links` depends on `todo`, `github`, and `linear` through their public
  APIs (traits, entities, errors) only.
* `links` never re-exports domain crate internals. It exposes its own
  `LinkService` trait and its own `LinkError`.
* `links` is app-specific. A different project that reuses `github` and
  `todo` writes its own composition crate (or none).

### The shared plumbing crates

* `remote-sync` carries no domain models and no HTTP client bindings. It is
  the pagination loop (`SyncLoop` over `RemoteApiClient`) and the
  HTTP→sentinel error mapping (`RemoteError<E>`, `map_response_error`)
  parameterized over the provider's own error type. Domain crates depend on
  it; it never depends on them.
* `sync-state` owns only the `sync_state` entity (one row per source). Both
  providers upsert their own row on first run. It is a dependency-free leaf
  crate.

### Domain-to-domain dependencies (forbidden)

A domain crate **must not** depend on another domain crate. Concretely:

* `crates/todo` must not appear in `crates/github/Cargo.toml` (or vice versa).
* No `impl Related<github::entity::Entity> for todo::entity::Entity` inside
  `todo` — that impl belongs in `links`.
* No join table with FKs into two domain crates inside any domain crate.

If a domain crate seems to need data from another domain, the resolution is
always one of:

1. **Pass the data as a function argument.** The caller (server or links)
   fetches it from the other domain and hands it over.
2. **Move the operation into `links`.** If the operation inherently touches
   two domains, it is a composition operation, not a domain operation.
3. **Introduce a tiny dependency-free shared crate** (like `sync-state`) —
   rare, only for genuine shared vocabulary, never to couple two domains.

Cargo forbids import cycles, so the graph is a DAG by construction — but the
stronger rule is that domain crates have **zero** edges between them, period.

### Consuming a domain crate

```rust
// crates/graphql/src/query_root.rs
pub fn schema_builder(
    database: DatabaseConnection,
    todo: Arc<dyn todo_domain::TodoService>,
    day_plan: Arc<dyn todo_domain::DayPlanService>,
    link: Arc<dyn links::LinkService>,
    review: Arc<dyn todo_domain::ReviewService>,
    github: Arc<dyn github::GithubSync>,
    linear: Arc<dyn linear::LinearSync>,
    clock: todo_domain::Clock,
    depth: Option<usize>,
    complexity: Option<usize>,
) -> SchemaBuilder { ... }
```

Rules:

* Dependencies are trait-typed struct fields, set once by the constructor.
  No setters, no `static`s, no `OnceLock` singletons for domain state.
* A crate never reaches around a trait (no downcasting to the concrete impl
  via `Any`).

### Constructors and options

Constructor signatures follow a fixed shape: **required things are positional,
optional things live in an `Options` struct with a `Default` impl.**

```rust
pub fn new(
    db: DatabaseConnection,
    client: Arc<dyn GithubApiClient>,
    opts: GithubOptions,
) -> impl GithubSync { ... }
```

* Required dependencies are positional arguments; forgetting one must be a
  compile error, not a runtime panic. Required dependencies are never
  optional.
* Optional behavior goes in an `Options` struct (`GithubOptions`,
  `LinearOptions`). It is `pub` with `pub` fields and a hand-written or
  derived `Default` — the defaults are documentation. `Type::new(db, deps)`
  with no options must produce a fully usable value.
* Options that can be invalid are validated inside the constructor, which
  returns `Err`. Validation never panics.
* **Never use the builder pattern** (`b.set_timeout(x).build()?` where
  `build()` may fail because a required field was never set). Builders push
  required-field errors from compile time to run time. A builder is
  acceptable only when every field it sets is optional — at which point
  `Options` is simpler anyway.
* Test-only seams (clocks) are regular constructor arguments, not
  `#[cfg(test)]` setters.

### `dyn` versus generics

* **Default: `Arc<dyn Trait>`** for dependencies held in service structs and
  passed to schema builders. Keeps signatures short, wiring readable, and
  avoids monomorphisation blowup.
* **Generics** when the trait is not object safe (e.g. `RemoteApiClient`
  has associated types and is driven by `SyncLoop<C>`) or when the type
  parameter is already there for other reasons.
* Never reach around a trait — no downcasting, no `Any`.

## Dependency wiring

All wiring happens by hand in `crates/server/src/main.rs`.

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = config::Config::load()?;

    // Logging: PREFLIGHT_LOG_LEVEL / PREFLIGHT_LOG_DIR override config.
    let (log_level, level_source) = match std::env::var("PREFLIGHT_LOG_LEVEL") { ... };
    let _logging_guard = logging::init(level_filter, log_dir.as_ref())?;

    let clock = cfg.clock()?;
    let db = db::connect(&cfg.database.path).await?;
    db::migrate(&db).await?;

    // Domain services — constructed independently, no cross-domain edges.
    let todo_svc     = Arc::new(todo_domain::todo_service::new(db.clone(), clock.clone()));
    let day_plan_svc = Arc::new(todo_domain::day_plan::new(db.clone(), clock.clone()));
    let review_svc   = Arc::new(todo_domain::review::new(db.clone(), clock.clone()));
    // Composition — links depends on todo + day_plan.
    let link_svc = Arc::new(links::new(db.clone(), clock.clone(), day_plan_svc.clone()));
    // Providers — each gets its client + options.
    let github_sync = Arc::new(github::sync::new(db.clone(), github_client, github_opts));
    let linear_sync = Arc::new(linear::sync::new(db.clone(), linear_client, linear_opts));

    let schema = graphql::schema_builder(db, todo_svc, day_plan_svc, link_svc,
        review_svc, github_sync, linear_sync, clock, depth, complexity).finish()?;
    let app = routes::router(schema, &cfg.server.allowed_origins(), cfg.server.frontend_dist.as_deref());
    axum::serve(listener, app).await?;
    Ok(())
}
```

Rules:

* `main` is thin: load config, init logging, connect db, run migrations,
  build domain services, build composition, build providers, build schema,
  build router, run. Errors bubble up through `anyhow::Result`; only `main`
  decides the exit code. `main` returns `Result`, never panics.
* `anyhow` is used in `main` (and `build.rs`, tests) only. It is banned from
  library crates' public APIs.
* Domain services are constructed **before** the composition layer. The
  composition layer receives them as trait-typed constructor parameters.
* Token resolution (`config::resolve_token`) honors the `_path` variant:
  when a `*_token_path` file is set, its trimmed contents win over the
  inline `*_token`. This keeps secrets out of config and lets sops-nix
  manage them without code changes.
* Graceful shutdown is axum's built-in `serve` + signal handling.

## Error handling

* Each crate defines **sentinel errors** for the failure modes callers must
  distinguish: `TodoError::NotFound`, `GithubError::RateLimited`,
  `LinearError::ContentFiltered`, `LinkError::AlreadyLinked`, ...
* Errors are defined with `thiserror::Error`. Every variant has a
  `#[error("...")]` message.
* Variants carry **structured fields**, not formatted free-form strings
  (`RateLimited { retry_after: Option<Duration> }`). The `#[error(...)]`
  renders them for humans; the fields serve logs and mapping.
* Check with `matches!` or `if let`. Never compare `Display` strings.
* Wrap on the way up, always with context: `map_err(|e| TodoError::Db(e))`
  or `?` via `#[from]`. Never leak `DbErr` across a crate boundary.
* `remote_sync::RemoteError<E>` wraps a provider's own error `E` and is
  converted into the provider's error via a `From` impl
  (`GithubError::from(RemoteError<GithubError>)`), mapping shared sentinels
  onto provider-specific variants.
* Only the GraphQL layer translates errors into response fields/HTTP status.
  Domain and composition crates know nothing about GraphQL/HTTP. Mapping
  walks the source chain; the outermost mapped error wins.
* `anyhow` is allowed in `main`, `db.rs`, `logging.rs` (server binary), and
  tests. It is banned from library crates' public APIs.

## The database layer

### Schema ownership

* Migrations live in `crates/migration`. They are the **single source of
  truth** for schema. Domain crate entities are generated from the migrated
  schema and committed.
* **Entity ownership follows domain ownership.** Tables owned by a domain
  crate have their entities generated into that crate's `src/entity/`. Join
  tables owned by `links` have their entities in `crates/links/src/entity/`.
  `sync_state` lives in `crates/sync-state/src/entity/`.
* Regeneration is per-domain via the `gen:entities` devenv task:
  ```sh
  devenv tasks run gen:entities     # migrate + generate entity per domain
  devenv tasks run server:generate-server  # seaography-cli generate
  ```
  The generate task overwrites hand-maintained enum typing — re-apply the
  `ActiveEnum` column types and `entity/enums.rs` afterward.
* Never edit generated entity files. To change the schema, write a
  migration, apply it, regenerate entities.

### Migrations

Every migration is a file `mYYYYMMDD_NNNNNN_snake_case_name.rs` with an `up`
**and** a `down`:

```rust
// crates/migration/src/m20260101_000001_create_todo.rs
use sea_orm_migration::prelude::*;
use crate::idens::Todo;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> { ... }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> { ... }
}
```

Rules:

* **Every migration has a working `down`.** `down` reverses `up` exactly;
  drop in reverse dependency order (children before parents).
* **No `CURRENT_TIMESTAMP` defaults.** SQLite renders them as
  `"YYYY-MM-DD HH:MM:SS"` with no offset, which will not parse back into
  `DateTimeWithTimeZone`. The application sets every timestamp explicitly.
* All timestamp columns use `timestamp_with_time_zone()`. All date columns
  use `date()`.
* **CHECK constraints only on closed, stable domains.** `todo.status` and
  `todo_event.actor` are closed. `todo_event.kind` and `pull_request.state`
  are *not* — SQLite cannot drop or alter a CHECK, so anything you expect to
  grow must not have one. The `ActiveEnum` in the domain crate is the guard
  for open domains; unknown values surface as a SeaORM error on read.
* **Shared `Iden`s** live in `crates/migration/src/idens.rs`. One shared
  module is less noisy than re-declaring per migration. Rule if you ever
  rename: copy the *old* `Iden` into the migration that used it, then rename
  in `idens.rs`.
* Foreign keys name themselves (`fk_<table>_<ref>`) and specify `on_delete`.
  Use `Cascade` for join tables and child rows owned by the parent.
* Indexes are named `idx_<table>_<cols>` (non-unique) or `ux_<table>_<cols>`
  (unique). Every foreign-key column and every query filter column gets an
  index.
* Register every migration in `lib.rs`'s `Migrator::migrations()` vector.
* Tables and indexes use `if_not_exists()` defensively.
* **Migrations are centralized** in `crates/migration` even though entities
  are per-domain. The schema is one coherent database. If a domain crate is
  extracted to a standalone library later, its migrations can be exported as
  a vector and composed into a host's `Migrator`.

### Entities

* Generated by `sea-orm-cli generate entity` into the owning crate's
  `src/entity/`, with `--with-serde both --model-extra-derives
  'async_graphql::SimpleObject' --seaography`.
* Each domain crate's `entity/mod.rs` registers its entity modules with
  seaography. `links` does the same for join-table entities.
* Enums (`entity/enums.rs`) are **hand-maintained** on top of the generated
  output. Each variant's `#[serde(rename = ...)]` matches its
  `#[sea_orm(string_value = ...)]` so JSON round-trips use the same token the
  database stores. Hand-written `CustomInputType`/`CustomOutputType` impls
  bridge each enum to Seaography and must live in the defining crate (orphan
  rule).
* Domain crate entities are a shared dependency of the composition and
  GraphQL layers. They contain only data models — no logic, no traits, no
  constructors.
* **Cross-domain `Related` impls live in `links`, not in domain crates.** The
  generated entity for `todo` will not reference `pull_request`; the
  `impl Related` that bridges them is hand-written in `crates/links`.

### The event log

`todo_event` is an append-only log owned by the `todo` domain. Every
mutation in `todo` writes at least one event row via `EventWriter::append`
on the same transaction as the mutation. The event records `kind`, `field`,
`old_value`, `new_value`, `actor` (`user` / `sync` / `system`),
`occurred_at`, and `logical_date`.

Rules:

* Never update or delete an event row. Append only.
* `logical_date` is denormalized from `occurred_at` via
  `Clock::logical_date` so "what did I do yesterday" is an index scan, never
  a timezone calculation in SQL.
* The background sync (github/linear) must not make an untouched todo look
  worked on — the `actor` CHECK (`user`/`sync`/`system`) and the
  `idx_todo_event_day` index (date, actor) keep this queryable and auditable.

### Sync state

`sync_state` has one row per source (`linear`, `github`). The sync code
upserts its own row on first run. Migrations create schema only; they do not
seed data.

## Testing strategy

### Three tiers

| Tier | Files | Feature | Infrastructure | Speed target | Command |
| -- | -- | -- | -- | -- | -- |
| Unit | `#[cfg(test)] mod tests` in-file | none | mocks/fakes | whole suite < 10 s | `cargo test` |
| Integration | `crates/*/tests/*_integration.rs` | `--features integration` | real SQLite (temp/`:memory:`); providers against `httptest` | < 1 min | `cargo test --features integration` |
| E2E | `tests/e2e/` (package `e2e`) | `--features e2e` | full stack + HTTP/GraphQL | < 2 min | `cargo test --features e2e` |

Tiers are gated by Cargo features. Plain `cargo test` runs **units only** and
must always be fast. This is the inner loop — keep it clean.

```toml
# crates/<silo>/Cargo.toml
[features]
default = []
integration = []
e2e = []
live-api = []
```

Every gated test file starts with `#![cfg(feature = "integration")]` (or
`e2e`). The `live-api` feature gates tests that hit a real provider API and
is never part of CI.

### The TDD loop (mandatory)

Every change follows this loop:

1. **Red.** Write (or extend) a test that fails for the right reason. For a
   bugfix, the test must reproduce the bug. Run it and confirm the failure
   message is the one you expect.
2. **Green.** Write the minimum production code to pass. Run `cargo test`.
3. **Refactor.** Clean up with the tests green. Run `cargo clippy -- -D
   warnings` and `cargo test` again.
4. If the change touches schema: write the migration, apply it, regenerate
   entities, then fix every compile error regeneration surfaces.
5. If the change touches a trait: recompile and fix every call site and
   fake.

Agents: never write production code and its tests in the same step "to save
time". Write the test, show it failing, then implement. Do not write tests
that merely execute code without asserting behavior. A test with no
meaningful assertion is worse than no test.

### Unit test style

* One test function per behavior, named `test_<unit>_<condition>_<expectation>`:
  `test_set_status_rejects_unknown_status`, not `test_case_2`.
* Use `pretty_assertions` for diff output on `assert_eq!` of structs.
* Structure tests as Arrange / Act / Assert, with blank lines between.
* Never `tokio::time::sleep` in unit tests. Inject the `Clock`
  (`Clock::new(tz, day_start_hour)` with a fixed timezone, or a fixed `now`
  helper); use `tokio::time::pause()` and `advance()` when you need time to
  move.
* Use `#[tokio::test]` for async tests; `#[tokio::test(flavor =
  "current_thread")]` when isolation matters.
* Tests are hermetic: no shared mutable state, no reliance on test ordering.
* **Domain crate unit tests mock their own dependencies, not other domain
  crates.** If a domain crate's unit test would need another domain crate's
  real infrastructure, the dependency boundary is wrong — the operation
  belongs in `links`.

```rust
#[tokio::test]
async fn test_create_todo_rejects_empty_title() {
    let db = setup::memory_db().await;
    let svc = todo_domain::todo_service::new(db, Clock::new(tz, 4));

    let err = svc.create(String::new(), None).await.unwrap_err();

    assert!(matches!(err, todo_domain::Error::Invalid(_)));
}
```

### Property and fuzz tests

Use `proptest` (fast, runs in the unit tier) and `cargo-fuzz` (deep, nightly)
for every function that parses, decodes, validates, or transforms
**untrusted input**: GraphQL input validation, sync cursor parsing, URL/key
sanitizers, pagination cursor decoding, and any hand-written parser. Pure
business logic with trusted inputs does not need fuzzing.

```rust
proptest! {
    #[test]
    fn parse_cursor_never_panics(raw in ".*") {
        let _ = github::cursor::parse_cursor(&raw); // must never panic
    }

    #[test]
    fn cursor_round_trips(offset in 0i64..1_000_000) {
        let c = github::cursor::Cursor { offset };
        let encoded = c.encode();
        let parsed = github::cursor::parse_cursor(&encoded).unwrap();
        prop_assert_eq!(c, parsed);
    }
}
```

Rules:

* Fuzz targets live in `tests/` or a `fuzz/` crate. Seed cases run as
  ordinary tests during `cargo test`, so seeds are part of the fast tier.
* Fuzz targets must be pure: no mocks, no network, no filesystem. If the
  function under fuzz needs a dependency, extract the parse/validate logic
  into a pure function first.
* Assert properties: the code never panics, it produces errors instead of
  garbage, invariants hold on accepted input, and encode/parse round-trips.
* When a fuzzer finds a crasher, commit the failing input as a regression
  seed. Fix via the normal TDD loop — the crasher is your failing test.

## Mocks

Mocks are hand-written trait implementations in test modules. There is no
mock generator; Rust traits are cheap to stub.

```rust
#[cfg(test)]
mod fake {
    use todo_domain::TodoService;
    pub struct FailingTodoService;
    impl TodoService for FailingTodoService {
        async fn create(&self, _: String, _: Option<String>)
            -> Result<todo_domain::entity::todo::Model, todo_domain::Error> {
            Err(todo_domain::Error::Conflict("duplicate".into()))
        }
        // ...
    }
}
```

Rules:
* Mocks are for unit tests only. Integration tests use real dependencies for
  the crate under test; e2e tests must not use mocks.
* Mock domain traits, not functions inside a crate. Mocks are for
  cross-crate boundaries.

### Modelling transactions

`todo_service` wraps each mutation in a SeaORM transaction and appends an
event row on the same transaction. Unit tests assert behavior through the
public API (e.g. a duplicate create returns `Conflict` and no event row was
committed). Do not assert transaction mechanics (isolation, actual
atomicity) in unit tests — that is the db layer's job, verified in
integration tests.

## Integration tests

Integration tests exercise a single crate against a real SQLite database
(temp file or `:memory:`). Each crate that needs db access owns a test setup
helper:

```rust
// crates/todo/tests/common/mod.rs
pub async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    migration::Migrator::up(&db, None).await.unwrap();
    db
}
```

Rules:

* Use `sqlite::memory:` or a `tempfile`-backed db. Never touch the
  developer's `db.sqlite`.
* Migrations always run inside test setup so integration tests validate the
  real, migrated schema.
* The `github` and `linear` crates' integration tests run against an
  `httptest::Server` speaking the provider's wire format (verifying request
  encoding, auth headers, error mapping). Tests that hit the real provider
  API are allowed only behind the `live-api` feature and are never part of
  CI — because of cost, non-deterministic output, and past flakiness.
* Integration tests use real dependencies for the crate under test and
  fakes for unrelated crates.

## E2E tests

E2E tests treat the system as a black box: a real SQLite db is created,
migrations run, the server is started, and tests speak HTTP/GraphQL.

* Location: `tests/e2e/`, its own workspace member, gated by the `e2e`
  feature.
* Setup creates a temp db, runs migrations, starts the axum server on a
  random port, and constructs the full dependency graph exactly like
  `main.rs` does — the harness constructs the `Config` structs directly
  instead of reading env vars.
* Tests cover user-visible flows end to end (create → fetch → list → delete)
  and the error contract. They do not re-test edge cases already covered by
  unit tests.
* No assertions against database rows — assert only through the GraphQL/HTTP
  API, like a client would.
* No mocks in e2e tests.

## The GraphQL layer (`crates/graphql`)

* The query surface is generated by `seaography-cli` into `crates/graphql`.
  The generated `query_root.rs` and `lib.rs` register entity modules from
  all domain, links, and sync-state crates and build the schema. Regenerate
  with `devenv tasks run server:generate-server`.
* **Mutations are hand-written** in `crates/graphql/src/mutation/`. They are
  intent-shaped (`planTodoForToday`, `convertPrToTodo`, `syncNow`), validate
  input, delegate to domain/link traits, and return domain types.
* Custom queries that seaography can't express live in `query.rs` alongside
  the generated roots. Input types and custom output objects live in
  `types.rs`.
* The schema owns depth/complexity limits (`depth_limit`, `complexity_limit`
  config keys) for query safety.
* Handler unit tests use `async_graphql::dynamic::Schema::execute` with
  mocked services and assert the response shape and errors.

### Regenerating the GraphQL server

```sh
devenv tasks run gen:entities        # migrate + generate entity per domain
devenv tasks run server:generate-server  # seaography-cli generate
```

`server:generate-server` depends on `gen:entities`, which depends on
`db:migrate`. Run the last one to run the chain, or run them explicitly.

## Config

Environment variables and `config/default.toml`. The `server` crate owns the
`Config` struct, loaded in `server/main.rs` via `Config::load()` (resolves
the XDG config path: `$CONFIG` env var → `$XDG_CONFIG_HOME/preflight/config.toml`
→ `$HOME/.config/preflight/config.toml`).

```rust
// crates/server/src/config.rs
#[derive(Debug, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub clock: ClockConfig,
    pub logging: LoggingConfig,
    pub sync: SyncConfig,
    pub server: ServerConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncConfig {
    pub github_token: String,
    pub github_token_path: Option<String>,
    pub linear_token: String,
    pub linear_token_path: Option<String>,
    pub github: GithubSyncConfig,
    pub linear: LinearSyncConfig,
}
```

Rules:

* Config keys are **prefixed with the crate/domain name** (`DATABASE_URL`,
  `SERVER_PORT`, `GITHUB_TOKEN`, `LINEAR_TOKEN`) so ownership is greppable
  and collisions are impossible. A crate never reads another crate's
  variables.
* `main` is the only place config is loaded, once, failing fast with the
  crate name in the error.
* Cross-field validation beyond what the config format expresses lives in
  the owning crate's constructor, which returns an error for invalid config.
* No secrets in `config/default.toml`. Secrets come from env vars or
  `*_path` files only, never get default values, and never get logged.
* Every secret has a **`_path` variant**: `github_token` /
  `github_token_path`, `linear_token` / `linear_token_path`. When the path
  is set, the file's trimmed contents win over the inline value. This keeps
  secrets out of config and lets sops-nix manage them without code changes.
* `SyncConfig` is `#[serde(deny_unknown_fields)]`, so unknown keys are a
  build/runtime bug, not a warning.

## Logging

* `tracing` macros are the only logging mechanism. The process subscriber
  is initialized **once** in `main` via `logging::init`. **No crate may
  initialize its own `tracing_subscriber` or install its own layer.**
* No `println!`, `eprintln!`, or `dbg!` in library crates.
* Structured fields, stable keys: `tracing::info!(todo_id = id, "todo
  created")`. Fields, not formatted strings.
* Levels: `DEBUG` for diagnostics, `INFO` for lifecycle and notable events,
  `WARN` for degraded-but-handled, `ERROR` only where the error is finally
  handled (in practice the GraphQL layer and the binary).
* Error logs will trigger alerts, so use sparingly. A handled 404 is not an
  error.
* Always use span context (`#[tracing::instrument]`) so request-scoped
  attributes are included automatically.

### Configuration

The `[logging]` section in `config/default.toml` controls logging:

* `level` — default directive string (default `"info"`). Overridden by
  `PREFLIGHT_LOG_LEVEL`. When `RUST_LOG` is set, `EnvFilter` uses the config
  level as a default and lets `RUST_LOG` directives take precedence.
* `directory` — directory for the day-rolling JSON log file (default
  `"logs"`). Overridden by `PREFLIGHT_LOG_DIR`.

### Outbound request logging

Every outbound HTTP request from a domain client logs an event with stable
fields: `provider`, `operation`, `endpoint`, `status`, `elapsed_ms`, and
page/item counts. Log `INFO` on success, `WARN`/`ERROR` on error or
rate-limit paths. Include the error message on failure. **Never log request
or response bodies** (truncate to a bounded limit when a body must appear
in an error message).

## Nix packaging (flake + home-manager)

* The flake exposes `packages.<system>.preflight` (deployable, also
  `default`), `homeManagerModules.preflight`, and `overlays.default`
  (`pkgs.preflight`). `nix/` holds the packaging: `nix/package.nix`
  (naersk build), `nix/toml.nix` (config serializer), and
  `nix/modules/home-manager.nix` (`programs.preflight`).
* `nix/package.nix` builds `-p server -p tui` via naersk and renames the
  binaries to `preflight` and `pftui`. It links the system `sqlite` and
  `openssl` libs (not bundled). Build inputs are real package derivations
  passed from the flake — never bare name strings.
* `nix/toml.nix` is the config serializer. The app's `SyncConfig` is
  `deny_unknown_fields`, so the renderer emits exactly the known keys and
  always includes `github_token` / `linear_token` (empty when unset) while
  omitting `null` optionals (`*_token_path`). Nix options are camelCase; the
  renderer converts them to the snake_case TOML keys the Rust structs
  expect. **Keep this file in lockstep with `crates/server/src/config.rs`.**
* `nix/modules/home-manager.nix` is the `programs.preflight` module. It
  defaults `settings.database.path` to an absolute state path
  (`~/.local/state/preflight/db.sqlite`) and `settings.server` to
  `127.0.0.1:8000`, and launches the binaries with the `CONFIG` /
  `PREFLIGHT_GRAPHQL_ENDPOINT` env vars. With `installService` (default
  `true`) it runs the server as `systemd.user.services.preflight` on Linux
  or `launchd.agents.preflight` on Darwin. A supplied `configFile` takes
  precedence over `settings`-rendered content (no merge).
* `Cargo.lock` is committed: naersk requires it in the source tree for
  reproducible builds.
* Config schema changes that add/remove a required option or rename a TOML
  key must update `nix/toml.nix` and `nix/modules/home-manager.nix` in the
  same change. Package dependency changes (Rust crates linking system libs)
  may add `buildInputs`/`nativeBuildInputs` in `nix/package.nix`.

## Tooling, lint, and devenv

Lint findings are fixed, not suppressed. An `#[allow(...)]` requires the
specific lint name and a justification; blanket crate-level `allow` is
forbidden.

```sh
cargo fmt                          # format
cargo clippy -- -D warnings        # lint, warnings are errors
cargo test                         # unit tests (fast)
cargo test --features integration  # integration tests
cargo test --features e2e          # e2e tests

devenv tasks run db:migrate         # apply migrations
devenv tasks run gen:entities       # regenerate entities per domain
devenv tasks run server:generate-server  # regenerate seaography
```

devenv git hooks (via `devenv.nix`): `rustfmt` on commit, `clippy` on
commit.
CI runs `fmt --check`, `clippy`, `cargo test` (units), `cargo test
--features integration`, `cargo test --features e2e`, in that order.

`Cargo.lock` is committed and CI builds with `--locked`. Dependencies are
declared once in `[workspace.dependencies]` and referenced as
`{ workspace = true }`.

## Conformity checklist

Before finishing any task, verify:

- [ ] Started from a failing test (unit for logic, integration for infra
      behavior).
- [ ] `cargo test` (units) passes and still runs fast.
- [ ] Schema changed? Migration written with `up` **and** `down`, entities
      regenerated into the owning crate's `src/entity/`, hand-maintained
      `entity/enums.rs` re-applied, all compile errors fixed.
- [ ] New crate failure mode? Sentinel error added to the crate's
      `error.rs`, mapped at the GraphQL boundary, covered by a handler test.
- [ ] No concrete impl, driver, or SDK types leaked across a crate boundary.
- [ ] New cross-domain relationship? Join table and `Related` impls live in
      `links`, not in a domain crate. Domain crates have zero edges to
      other domain crates.
- [ ] Crate depends on another? Via its trait, injected in `main`, mocked
      in unit tests. Domain crates never depend on other domain crates.
- [ ] New config value? Added to the owning crate's `Config` with the crate
      prefix, loaded in `main`. For `[logging]`, the section has
      `level`/`directory` defaults (info/logs) and env overrides
      `PREFLIGHT_LOG_LEVEL`/`PREFLIGHT_LOG_DIR`.
- [ ] Config schema changed? `nix/toml.nix` and `nix/modules/home-manager.nix`
      updated and in lockstep with `crates/server/src/config.rs`.
- [ ] Nix change builds: `nix flake check` and `nix build .#preflight` pass;
      `nix/tests/eval-module.nix` still evaluates.
- [ ] No new `static`, `OnceLock` singleton, or `lazy_static` for domain
      state.
- [ ] Errors carry structured fields and a `#[source]` chain; no string
      comparison anywhere.
- [ ] `cargo clippy -- -D warnings` clean, including pedantic warnings and
      formatting.
- [ ] Touched schema? `down` migration reverses `up`, integration test
      green against the migrated schema, no `CURRENT_TIMESTAMP` defaults.
- [ ] New or changed parsing/validation of untrusted input? A `proptest`
      in the fast tier and a fuzz target with seeds for valid, empty, and
      tricky inputs.
- [ ] Fuzzer found a crasher? Regression seed committed, fix done via the
      TDD loop.
- [ ] Documentation added/updated.
- [ ] No `unwrap`, `expect`, `panic!`, `todo!`, or slice indexing outside
      `main` and tests. No panics outside `main` (and `main` returns
      `Result`, not panic).

## Hard "never" list

Never edit generated entity or seaography code. Never use mocks in e2e
tests. Never construct one domain inside another — domain dependencies arrive
as trait-typed constructor parameters, wired in `main`. Never reference
another crate's concrete implementation types or private modules. Never put
logic in `main` beyond wiring. Never `sleep` in a unit test. Never compare
error `Display` strings. Never hit a real remote API outside `--features
live-api` tests. Never use the builder pattern — use a `Default`-able
options struct. Never make a required dependency optional.
Never use `CURRENT_TIMESTAMP` as a column default. Never add a CHECK
constraint to a domain you expect to grow. Never edit or delete an event row
— append only.

Never add a dependency from one domain crate to another domain crate. Never
initialize a `tracing_subscriber` or install a logging layer outside `main`.
Never leak `DbErr` across a crate boundary. Never write code that panics
outside `main` (and `main` returns `Result`, not panic).
