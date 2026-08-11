# Rust development guidelines

This document defines the architecture, coding, and testing conventions for this codebase. Every rule is intended to be followed literally by both humans and AI agents. When a rule here conflicts with general Rust folklore, this document wins.

## Core principles

1. **One crate per domain.** Each domain (`todo`, `github`, `linear`, ...) is its own crate under `crates/`. A domain crate owns its entities, its enums, its traits, its errors, its sync/puller logic, and its API client. It is self-contained: a different project could depend on `crates/github` and get pull-request sync without pulling in `todo` or `linear`.
2. **Domain crates never depend on each other.** `todo` does not know about `github` or `linear`. Cross-domain relationships — join tables, `Related` impls, link mutations — live in a composition crate (`links`), which is the only place allowed to depend on two or more domain crates at once. This is what makes domain crates reusable: no incoming domain edges.
3. **Producer-side traits.** Every domain crate that is consumed by another layer exports traits that are the *only* thing other crates may depend on. Concrete implementation types stay private; only the constructor is public.
4. **TDD, always.** No production code without a failing test first. Unit tests are fast and mock-based, integration tests use a real SQLite database.
5. **Explicit over magic.** Manual dependency injection, SeaORM queries, axum routing. No DI containers, no hidden state, no globals.
6. **The compiler is the safety net.** Prefer designs where mistakes fail `cargo build`, then designs where mistakes fail `cargo test`, and only then designs that rely on review.

## Repository layout

```
.
├── Cargo.toml                 # workspace, members listed here
├── crates/
│   ├── todo/                  # the todo domain — reusable
│   │   └── src/
│   │       ├── lib.rs          # module declarations + public re-exports
│   │       ├── entity/         # sea-orm entities this domain owns
│   │       │   ├── mod.rs       #   todo, todo_event, todo_day_plan, tag, todo_tag
│   │       │   ├── todo.rs
│   │       │   ├── todo_event.rs
│   │       │   ├── todo_day_plan.rs
│   │       │   ├── tag.rs
│   │       │   ├── todo_tag.rs
│   │       │   └── enums.rs     # TodoStatus, EventKind, EventActor
│   │       ├── clock.rs        # logical-date clock abstraction
│   │       ├── events.rs       # append-only event writer
│   │       ├── todo_service.rs # create / edit / set_status
│   │       ├── day_plan.rs     # plan, unplan, reorder, carry_over
│   │       ├── review.rs       # daily review queries
│   │       └── error.rs        # TodoError
│   ├── github/                # the github domain — reusable
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── entity/         # pull_request entity
│   │       │   ├── pull_request.rs
│   │       │   └── enums.rs     # PullRequestState
│   │       ├── sync.rs         # one-way PR puller + upsert
│   │       ├── client.rs        # GitHub API client (reqwest)
│   │       ├── cursor.rs        # pagination / ETag cursor
│   │       └── error.rs        # GithubError
│   ├── linear/                # the linear domain — reusable
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── entity/         # linear_issue entity
│   │       │   ├── linear_issue.rs
│   │       │   └── enums.rs     # (Linear state types if typed)
│   │       ├── sync.rs         # one-way issue puller + upsert
│   │       ├── client.rs        # Linear API client (reqwest)
│   │       ├── cursor.rs
│   │       └── error.rs        # LinearError
│   ├── links/                 # cross-domain composition — NOT reusable, app-specific
│   │   └── src/
│   │       ├── lib.rs          # re-exports link entities + Related impls
│   │       ├── entity/         # join tables that bridge domains
│   │       │   ├── todo_pull_request.rs   # FK -> todo + FK -> pull_request
│   │       │   ├── todo_linear_issue.rs   # FK -> todo + FK -> linear_issue
│   │       │   └── enums.rs                 # LinkRelation
│   │       ├── link_service.rs # link/unlink todo <-> PR / Linear issue
│   │       └── convert.rs      # promote a PR/issue into a todo
│   ├── migration/             # sea-orm-migrate (all migrations, single Migrator)
│   │   └── src/
│   │       ├── lib.rs          # Migrator struct + migration list
│   │       ├── idens.rs        # shared Iden enums for table/column names
│   │       ├── main.rs         # CLI entrypoint (sea-orm-cli)
│   │       └── m20260101_NNNNNN_name.rs
│   ├── graphql/               # seaography roots + hand-written mutations
│   │   └── src/
│   │       ├── lib.rs          # schema builder
│   │       ├── query_root.rs   # seaography-generated roots + custom queries
│   │       ├── mutation/       # hand-written, intent-shaped
│   │       └── types.rs        # input types, custom output objects
│   └── server/                # axum server, wiring, lifecycle
│       └── src/
│           ├── main.rs  db.rs  config.rs  routes.rs
├── config/
│   └── default.toml
├── devenv.nix
├── flake.nix                 # Nix flake: packages.preflight + homeManagerModules.preflight
├── nix/
│   ├── package.nix           # naersk build of crates/server -> preflight binary
│   ├── toml.nix              # renders home-manager settings -> config.toml
│   ├── modules/
│   │   └── home-manager.nix  # programs.preflight module
│   └── tests/
│       └── eval-module.nix   # standalone module evaluation + render smoke test
└── GUIDELINES.md
```

Rules:

* **A domain crate owns its entities.** There is no single `entity` crate spanning all domains. Each domain crate has an `entity/` submodule with the SeaORM models for the tables it owns. This is what makes a domain crate portable: its schema travels with it.
* **Join tables that bridge two domains live in `links`, never in a domain crate.** `todo_pull_request` references both `todo` and `pull_request`; it belongs in `links` because neither `todo` nor `github` should depend on the other.
* Generated code (sea-orm entities, seaography output) is committed to the repo and regenerated with the devenv tasks. Never edit it by hand.
* `server/src/main.rs` contains wiring and lifecycle only — no business logic, no GraphQL handlers.
* Keep `crates/` flat. Do not introduce grouping directories. A new concern is a new crate.

### Nix packaging

* The flake (`flake.nix`) exposes `packages.<system>.preflight` (the
  deployable, named `default` too) and `homeManagerModules.preflight` plus an
  `overlays.default` so the package is available as `pkgs.preflight`. The
  home-manager module's `package` option defaults to `pkgs.preflight`.
* `nix/package.nix` builds the workspace via naersk with `-p server -p tui`
  and renames the `server` and `tui` binaries to `preflight` and `pftui`. It
  links the system `sqlite` and `openssl` libs (they are not bundled). Build
  inputs are real package derivations passed from the flake — never bare
  name strings.
* `nix/toml.nix` is the config serializer. The app's `SyncConfig` is
  `deny_unknown_fields` and every required field must be present, so the
  renderer emits exactly the known keys and always includes
  `github_token` / `linear_token` (empty when unset) while omitting `null`
  optionals (`*_token_path`). Nix options are camelCase; the renderer
  converts them to the snake_case TOML keys the Rust structs expect. **Keep
  this file in lockstep with `crates/server/src/config.rs`.**
* `nix/modules/home-manager.nix` is the `programs.preflight` module. It
  defaults `settings.database.path` to an absolute state path
  (`~/.local/state/preflight/db.sqlite`) and `settings.server` to
  `127.0.0.1:8000`, and launches the binaries with the `CONFIG` /
  `PREFLIGHT_GRAPHQL_ENDPOINT` env vars (a `preflight` server launcher and a
  `pftui` client launcher). With `installService` (default `true`) it also
  runs the server as `systemd.user.services.preflight` on Linux or
  `launchd.agents.preflight` on Darwin. A supplied `configFile` takes
  precedence over `settings`-rendered content (no merge).
* `Cargo.lock` is committed: naersk requires it to be present in the source
  tree for reproducible builds.

### Why domain crates, not a monolithic `core`

A single `core` crate that owns todo logic, day-plan logic, event writing, and link/convert logic forces every consumer to pull in the whole graph. If a second project wants GitHub PR sync without the todo system, it can't. Splitting by domain means:

* `crates/github` is reusable in any project that needs to pull and store PRs.
* `crates/todo` is reusable in any project that needs a todo system with tags, day plans, and an event log.
* `crates/links` is the only crate that ties them together, and it stays in *this* app — a different app composes its own `links`.

The dependency graph is:

```
todo    github    linear         ← domain crates, no edges between them
  \      /          /
   links              ← composition: depends on todo + github + linear
    |
  graphql             ← depends on domain traits + links
    |
  server              ← depends on everything, wires it
```

No arrow points from a domain crate to another domain crate. That invariant is what "reusable" means here.

## Crate design: domains and traits

### Documentation

Every crate should have a `README.md` in its top-level describing the exposed functionalities and providing an overview of the internal and external flows.

### The domain crate contract

Every domain crate follows the same shape. Using `crates/github` as the template:

```rust
// crates/github/src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GithubError {
    #[error("pull request not found: {owner}/{repo}#{number}")]
    NotFound { owner: String, repo: String, number: i64 },
    #[error("rate limited, retry after {retry_after:?}")]
    RateLimited { retry_after: Duration },
    #[error(transparent)]
    Db(#[from] sea_orm::DbErr),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}
```

```rust
// crates/github/src/sync.rs
use sea_orm::DatabaseConnection;

/// The trait consumed by the GraphQL layer and the links crate.
/// A domain crate's public API — the only names other crates may reference — is:
/// the trait, the error type, the entity models, the enums, and the constructor
/// (constructor calls belong in server/main only).
pub trait PullRequestSync: Send + Sync {
    async fn pull(&self) -> Result<(), GithubError>;
    async fn dismiss(&self, pr_id: i64) -> Result<(), GithubError>;
}
```

```rust
// crates/github/src/sync_impl.rs
/// The real implementation. Private; only the constructor is public.
pub(crate) struct PullRequestSyncImpl {
    db: DatabaseConnection,
    client: GithubClient,
    opts: Options,
}

/// Called from server/main only.
/// Returns the trait so callers can never depend on the concrete type.
pub fn new(db: DatabaseConnection, client: GithubClient, opts: Options) -> impl PullRequestSync {
    PullRequestSyncImpl { db, client, opts }
}
```

Rules:

* **At least one trait per consumed domain crate**, named for the role (`PullRequestSync`, `TodoService`, `LinearSync`), not `Interface` or `ITodo`.
* Trait methods accept `&self` (or `&mut self`) and return `Result`. Async methods are the norm.
* Traits speak **domain types** — the domain crate's own entity `Model` types and enums — never driver types. `sea_orm::QueryResult`, raw `sqlx` rows, or `reqwest::Response` must not appear in any exported signature.
* Keep traits as small as the current callers need. Add methods when a real caller needs them (test-first), not speculatively.
* If a domain crate grows a second distinct responsibility, split it into a second trait within the crate (or a second crate) rather than widening the interface.
* The following files, if present, serve specific purposes:
  * `error.rs`: sentinel errors that the crate can emit, intended for consumption by other crates.
  * `lib.rs`: crate root — module declarations and re-exports of public traits/types only.
  * `entity/mod.rs`: the SeaORM entity module(s) this domain owns, plus `seaography::register_entity_modules!` if the domain is served via GraphQL.
  * `entity/enums.rs`: hand-maintained `ActiveEnum` types for this domain's columns.
  * `client.rs`: the HTTP API client for a remote-sync domain (`github`, `linear`).
  * `cursor.rs`: pagination/cursor logic for a remote-sync domain.

### The composition crate: `links`

`links` is the only crate allowed to depend on more than one domain crate. It owns:

* **Join-table entities** — `todo_pull_request`, `todo_linear_issue` — whose foreign keys point into two different domain crates' tables.
* **Cross-domain `Related` impls** — `impl Related<github::entity::PullRequest> for todo::entity::Todo`, etc. These cannot live in either domain crate because they would create a domain-to-domain edge.
* **Link/convert services** — `link_todo_to_pr`, `unlink`, `convert_pr_to_todo` — which call into domain traits from both sides.

```rust
// crates/links/src/lib.rs
pub mod entity;
pub mod link_service;
pub mod convert;

// Join-table entity bridging todo and github.
// FKs into both domain crates' tables; lives here, not in either domain.
```

Rules:

* `links` depends on `todo`, `github`, and `linear` through their public APIs (traits, entities, errors) only.
* `links` never re-exports domain crate internals. It exposes its own `LinkService` trait and its own `LinkError`.
* `links` is app-specific. A different project that reuses `github` and `todo` writes its own composition crate (or none).

### Domain-to-domain dependencies (forbidden)

A domain crate **must not** depend on another domain crate. Concretely:

* `crates/todo` must not appear in `crates/github/Cargo.toml` (or vice versa).
* No `impl Related<github::entity::Entity> for todo::entity::Entity` inside `todo` — that impl belongs in `links`.
* No join table with FKs into two domain crates inside any domain crate.

If a domain crate seems to need data from another domain, the resolution is always one of:

1. **Pass the data as a function argument.** The caller (server or links) fetches it from the other domain and hands it over.
2. **Move the operation into `links`.** If the operation inherently touches two domains, it is a composition operation, not a domain operation.
3. **Introduce a shared type in a tiny dependency-free crate** (rare — only for genuine shared vocabulary, not for coupling two domains). For example a logger or observability.

Cargo's compiler forbids import cycles, so the domain graph is a DAG by construction — but the stronger rule is that domain crates have **zero** edges between them, period.

### Consuming a domain crate

```rust
// crates/graphql/src/mutation/todo.rs
use todo::TodoService;

pub struct TodoMutations {
    service: Arc<dyn TodoService>,
}

impl TodoMutations {
    pub fn new(service: Arc<dyn TodoService>) -> Self {
        TodoMutations { service }
    }
}
```

Rules:

* Dependencies are trait-typed struct fields, set once by the constructor. No setters, no static variables, no `lazy_static` for domain state.
* A crate never reaches around a trait (e.g., downcasting to the concrete implementation via `Any`).

### Constructors and the builder pattern

Constructor signatures follow a fixed shape: **required things are positional, optional things are via a typed options struct with `Default`.**

```rust
pub fn new(db: DatabaseConnection, client: GithubClient, opts: Options) -> impl PullRequestSync { ... }
```

* Required dependencies are positional arguments; forgetting one must be a compile error, not a runtime panic. Required dependencies are never optional.
* For optional behavior, prefer typed config structs over a long parameter list. If a constructor grows beyond 4 parameters, group the optional ones into an `Options` struct with `#[derive(Default)]`.
* **Never use the builder pattern** (`b.set_timeout(x).set_retries(y).build()`). Builders allow half-built objects, hide required fields, and read poorly in Rust. Use a `Default`-able options struct passed as the last argument.

```rust
// crates/github/src/lib.rs
#[derive(Clone, Debug)]
pub struct Options {
    pub timeout: Duration,
    pub max_retries: u32,
}
impl Default for Options {
    fn default() -> Self {
        Self { timeout: Duration::from_secs(30), max_retries: 3 }
    }
}
pub fn new(db: DatabaseConnection, client: GithubClient, opts: Options) -> impl PullRequestSync { ... }
```

## Dependency wiring

All wiring happens by hand in `server/src/main.rs`.

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    tracing_subscriber::fmt().init();

    let db = db::connect(&std::env::var("DATABASE_URL")?).await?;
    migration::Migrator::up(&db, None).await?;

    let clock = todo::Clock::new();

    // Domain crates — constructed independently, no cross-domain edges.
    let todo_svc = todo::todo_service::new(db.clone(), clock.clone());
    let github_sync = github::new(db.clone(), github_client, github::Options::default());
    let linear_sync = linear::new(db.clone(), linear_client, linear::Options::default());

    // Composition — links depends on all three domains.
    let link_svc = links::new(db.clone(), todo_svc.clone(), github_sync.clone(), linear_sync.clone());

    let schema = graphql::build_schema(db, todo_svc, github_sync, linear_sync, link_svc)?;
    let app = routes::router(schema);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

Rules:

* `main` is thin: load config, connect db, run migrations, build domain services, build composition, build schema, build server, run. Errors bubble up through `Result`; only `main` decides the exit code.
* Domain services are constructed **before** the composition layer. The composition layer receives them as trait-typed constructor parameters.
* Graceful shutdown via axum's built-in `serve` + signal handling.

## Error handling

* Each crate defines **sentinel errors** for the failure modes callers must distinguish: `TodoError::NotFound`, `TodoError::Conflict`, `GithubError::RateLimited`, `LinearError::ContentFiltered`, ...
* Errors are defined using `thiserror::Error`. Every error variant has a `#[error("...")]` message.
* Check with `matches!` or `if let`. Never compare `Display` strings.
* Wrap on the way up, always with context: `map_err(|e| TodoError::Db(e))` or `?` via `#[from]`. Never `unwrap` swallowing a cause, never bare `return Err(e)` across a crate boundary if you can add information.
* Only the GraphQL layer translates errors into response fields/HTTP status. Domain and composition crates know nothing about GraphQL. Errors are unwrapped and mapped to their respective GraphQL error extensions, with the most external error taking precedence.
* `DbErr` is an infrastructure detail; it gets wrapped into the crate's own error type at the boundary, never leaked in a public signature.

## The database layer

### Schema ownership

* Migrations live in `crates/migration`. They are the **single source of truth** for schema. Domain crate entities are generated from the migrated schema and committed.
* **Entity ownership follows domain ownership.** Tables owned by a domain crate have their entities generated into that crate's `src/entity/` directory. Join tables owned by `links` have their entities in `crates/links/src/entity/`.

  Regeneration generates per-domain:
  ```sh
  sea-orm-cli generate entity -o crates/todo/src/entity --with-serde both \
      --model-extra-derives 'async_graphql::SimpleObject' --seaography
  sea-orm-cli generate entity -o crates/github/src/entity ...
  sea-orm-cli generate entity -o crates/links/src/entity ...
  ```

* Never edit generated entity files. To change the schema, write a migration, apply it, regenerate entities.

### Migrations

Every migration is a file `mYYYYMMDD_NNNNNN_snake_case_name.rs` with an `up` and a `down`:

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

* **Every migration has a working `down`.** `down` must reverse `up` — drop what `up` created, never half-reverse.
* **No `CURRENT_TIMESTAMP` defaults.** SQLite renders them as `"YYYY-MM-DD HH:MM:SS"` with no offset, which will not parse back into `DateTimeWithTimeZone`. The application sets every timestamp explicitly.
* All timestamp columns use `timestamp_with_time_zone()`. All date columns use `date()`.
* **CHECK constraints only on closed, stable domains.** `todo.status` and `todo_event.actor` are closed. `todo_event.kind` and `pull_request.state` are *not* — SQLite cannot drop or alter a CHECK, so anything you expect to grow must not have one. The `ActiveEnum` in the domain crate is the guard for open domains.
* **Shared `Iden`s** live in `crates/migration/src/idens.rs`. The SeaORM docs suggest re-declaring per-migration to freeze against renames; for a greenfield schema where four migrations share `Todo::Id`, one shared module is less noisy. Rule if you ever rename: copy the *old* `Iden` into the migration that used it, then rename in `idens.rs`.
* Foreign keys name themselves (`fk_<table>_<ref>`) and specify `on_delete`. Use `Cascade` for join tables and child rows that are owned by the parent.
* Indexes are named `idx_<table>_<cols>` (non-unique) or `ux_<table>_<cols>` (unique). Every foreign-key column and every query filter column gets an index.
* Register every migration in `lib.rs`'s `Migrator::migrations()` vector.
* Tables use `if_not_exists()` defensively. Indexes too.
* `down` drops tables in reverse dependency order (children before parents).
* **Migrations are centralized** in `crates/migration` even though entities are per-domain. The schema is one coherent database; splitting migrations per crate adds indirection without benefit. If a domain crate is extracted to a standalone library later, its migrations can be exported as a vector and composed into a host's `Migrator`.

### Entities

* Generated by `sea-orm-cli generate entity` into the owning crate's `src/entity/` directory, with `--with-serde both --model-extra-derives 'async_graphql::SimpleObject' --seaography`.
* Each domain crate's `entity/mod.rs` includes a `seaography::register_entity_modules!([...])` listing its entity modules. The `links` crate does the same for join-table entities.
* Enums (`entity/enums.rs`) are **hand-maintained** on top of the generated output. The generation command preserves manual enum additions; verify after regenerating.
* Domain crate entities are a shared dependency of the composition and GraphQL layers. They contain only data models — no logic, no traits, no constructors.
* **Cross-domain `Related` impls live in `links`, not in domain crates.** The generated entity for `todo` will not reference `pull_request`; the `impl Related` that bridges them is hand-written in `crates/links`.

### The event log

`todo_event` is an append-only log owned by the `todo` domain. Every mutation in `todo` writes at least one event row via `events.rs`. The event records `kind`, `field`, `old_value`, `new_value`, `actor` (`user` / `sync` / `system`), `occurred_at`, and `logical_date`.

Rules:

* Never update or delete an event row. Append only.
* `logical_date` is denormalized from `occurred_at` via `Clock::logical_date` so "what did I do yesterday" is an index scan, never a timezone calculation in SQL.
* The background sync (github/linear) must not make an untouched todo look worked on — the `actor` CHECK (`user`/`sync`/`system`) and the `idx_todo_event_day` index (date, actor) keep this queryable and auditable.

### Sync state

`sync_state` has one row per source (`linear`, `github`). The sync code upserts its own row on first run. Migrations create schema only; they do not seed data.

## Testing strategy

### Three tiers

| Tier | Files | Feature/attribute | Infrastructure | Speed target | Command |
| -- | -- | -- | -- | -- | -- |
| Unit | `tests` module in-file or `tests/` dir | none | mocks | whole suite < 10 s | `cargo test` |
| Integration | `*_integration.rs` or `#[cfg(test)]` with real db | real SQLite | temp file db | < 1 min | `cargo test --features integration` |
| E2E | `tests/e2e/` | full stack | real db + HTTP | < 2 min | `cargo test --features e2e` |

Because integration and e2e tests carry feature gates, plain `cargo test` runs **units only** and must always be fast. This is the inner loop, keep it clean and fast.

### The TDD loop (mandatory)

Every change follows this loop:

1. **Red.** Write (or extend) a test that fails for the right reason. For a bugfix, the test must reproduce the bug. Run it and confirm the failure message is the one you expect.
2. **Green.** Write the minimum production code to pass. Run `cargo test`.
3. **Refactor.** Clean up with the tests green. Run `cargo clippy` and `cargo test` again.
4. If the change touches schema: write the migration, apply it, regenerate entities, then fix every compile error the regeneration surfaces.

Agents: never write production code and its tests in the same step "to save time". Write the test, show it failing, then implement. Do not write tests that merely execute code without asserting behavior. A test with no meaningful assertion is worse than no test.

### Unit test style

* One test function per behavior, named `test_<unit>_<condition>_<expectation>`: `test_set_status_rejects_unknown_status`, not `test_case_2`.
* Use `pretty_assertions` for diff output on `assert_eq!` of structs.
* Structure tests according to the Arrange/Act/Assert pattern.
* Never `tokio::time::sleep` in unit tests. Inject clocks (`Clock` trait or `fn now() -> DateTimeWithTimeZone` field).
* Use `#[tokio::test]` for async tests; `#[tokio::test(flavor = "current_thread")]` when isolation matters.
* Tests are hermetic: no shared mutable state, no reliance on test ordering.
* **Domain crate unit tests mock their own dependencies, not other domain crates.** If a domain crate's unit test would need another domain crate's real infrastructure, the dependency boundary is wrong — the operation belongs in `links`.

```rust
#[tokio::test]
async fn test_create_todo_rejects_empty_title() {
    let db = setup::memory_db().await;
    let svc = todo::todo_service::new(db, Clock::fixed());
    let err = svc.create("".to_string()).await.unwrap_err();
    assert!(matches!(err, todo::TodoError::Validation(_)));
}
```

### Fuzz tests

Use `proptest` or `cargo-fuzz` for every function that parses, decodes, validates, or transforms **untrusted input**: GraphQL input validation, sync cursor parsing, URL/key sanitizers, pagination cursor decoding, and any hand-written parser. Pure business logic with trusted inputs does not need fuzzing.

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

* Fuzz targets live in `tests/` or a `fuzz/` crate. Seed cases run automatically as ordinary tests during `cargo test`, so seeds are part of the fast unit tier for free.
* Fuzz targets must be pure: no mocks, no network, no filesystem. If the function under fuzz needs a dependency, extract the parse/validate logic into a pure function first.
* Assert properties: the code never panics, it produces errors instead of garbage, invariants hold on accepted input, and encode/parse succeeds.
* When a fuzzer finds a crasher, commit the failing input as a regression seed. Fix via the normal TDD loop, the crasher is your failing test.

## Mocks

Mocks are hand-written trait implementations in test modules. There is no mock generator; Rust traits are cheap to stub.

```rust
#[cfg(test)]
mod fake {
    use todo::TodoService;
    pub struct FailingTodoService;
    impl TodoService for FailingTodoService {
        async fn create(&self, _: String) -> Result<todo::entity::todo::Model, todo::TodoError> {
            Err(todo::TodoError::Conflict)
        }
    }
}
```

Rules:

* One fake per trait, defined in the *consuming* crate's test module, not the producing crate.
* Use `mockall` only when a trait has many methods and the test needs per-call assertion. Prefer hand-written fakes for simple cases — they read better and carry no dependency.
* Mocks are for unit tests only. Integration tests use real dependencies; e2e tests must not use mocks.
* Do not mock entity `Model` types — they are plain data structs. Use them directly.
* Mock domain traits, not functions inside a crate. Mocks are for cross-crate boundaries.

### Mocking transactions correctly

If a service method wraps multiple writes in a transaction, the unit test must assert that a failure mid-transaction rolls back. Use a fake `DatabaseConnection` (or a trait abstracting the transaction) that records commits/rollbacks. Never assert commit/rollback mechanics in unit tests against a real db — that is the db layer's job, verified in its integration tests.

## Integration tests

Integration tests exercise a single crate against a real SQLite database (temp file or `:memory:`). Each crate that needs db access owns a test setup helper:

```rust
// crates/todo/tests/common.rs
pub async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    migration::Migrator::up(&db, None).await.unwrap();
    db
}
```

Rules:

* Use `sqlite::memory:` or a `tempfile`-backed db. Never touch the developer's `db.sqlite`.
* Migrations always run inside the test setup so integration tests validate the real, migrated schema.
* The `github` and `linear` crates' integration tests run against an `httptest::Server` that speaks the provider's wire format (verifying request encoding, auth headers, retry/backoff, error mapping). Tests that hit the real provider API are allowed only behind an explicit feature (`--features live-api`) and are never part of CI. This is 1. due to cost, 2. because of the non-deterministic nature of remote APIs and 3. because they're generally slow and have caused issues in the past.
* Integration tests use real dependencies for the crate under test and should use mocks for unrelated crates.

## E2E tests

E2E tests treat the system as a black box: a real SQLite db is created, migrations run, the server is started, and tests speak HTTP/GraphQL.

* Location: `tests/e2e/`, behind `#[cfg(feature = "e2e")]`, package `e2e`.
* Setup creates a temp db, runs migrations, starts the axum server on a random port, and constructs the full dependency graph exactly like `main.rs` does.
* Tests cover user-visible flows end to end (create → fetch → list → delete), and the error contract. They do not re-test edge cases already covered by unit tests.
* No assertions against database rows — assert only through the GraphQL/HTTP API, like a client would.

## The GraphQL layer (`crates/graphql`)

* The query surface is generated by `seaography-cli` into `crates/graphql`. The generated `query_root.rs` and `lib.rs` register entity modules from all domain and links crates and build the schema. Regenerate with `devenv tasks run server:generate-server`.
* **Mutations are hand-written** in `crates/graphql/src/mutation/`. They are intent-shaped (`planTodoForToday`, not `insertIntoTodoDayPlan`), validate input, delegate to domain/link traits, and return domain types.
* Custom queries that seaography can't express live in `query_root.rs` alongside the generated roots.
* Input types and custom output objects live in `types.rs`.
* The schema owns depth/complexity limits (`DEPTH_LIMIT`, `COMPLEXITY_LIMIT` env vars) for query safety.
* Handler unit tests use `async_graphql::Schema::execute` with mocked services and assert the response shape and errors.

### Regenerating the GraphQL server

```sh
devenv tasks run server:generate-entities   # migrate + generate entity (per domain)
devenv tasks run server:generate-server     # seaography-cli generate
```

The generate-server task depends on generate-entities, which depends on migrate. Run them in order by running the last one, or run the chain explicitly.

## Config

Environment variables and `config/default.toml`. Each crate that needs config owns its own `Config` struct, loaded in `server/main.rs`.

```rust
// crates/server/src/config.rs
#[derive(Deserialize)]
pub struct Config {
    pub database_url: String,
    pub day_start_hour: u32,
    pub timezone: String,
    pub github_token: Option<String>,
    pub linear_token: Option<String>,
}
```

Rules:

* Config keys are **prefixed with the crate/domain name** (`DATABASE_URL`, `GITHUB_TOKEN`, `LINEAR_TOKEN`, `SERVER_PORT`) so ownership is greppable and collisions are impossible. A crate never reads another crate's variables.
* `main` is the only place config is loaded (once per crate config, fail-fast with the crate name in the error).
* Cross-field validation beyond what the config format expresses lives in the crate's constructor, which returns an error for invalid config.
* No secrets in `config/default.toml`. Secrets come from env vars only, never get default values, and never get logged.
* Every secret has a **`_path` variant**: a `foo`/`foo_path` pair where `foo_path` points at a secrets file (sops-nix style). When set, the file's trimmed contents win over the inline value. This keeps secrets out of config and lets sops-nix manage them without code changes.

## Logging

* `tracing` macros are the only logging mechanism. Crates use the process subscriber given to them — it is initialized once in `main`. **No crate may initialize its own `tracing_subscriber`.**
* No `println!`, `eprintln!`, or `dbg!` in library crates.
* Structured fields, stable keys: `tracing::info!(todo_id = id, "todo created")`.
* Levels: `DEBUG` for diagnostics, `INFO` for lifecycle and notable events, `WARN` for degraded-but-handled, `ERROR` only where the error is finally handled.
* Error logs will trigger an alert, so use sparingly.
* Always use span context (`#[tracing::instrument]`) so request-scoped attributes are included automatically.

### Configuration

The `[logging]` section in `config/default.toml` controls logging:

* `level` — default directive string (default `"info"`). Overridden by `PREFLIGHT_LOG_LEVEL`. When `RUST_LOG` is set, `EnvFilter` uses the config level as a default and lets `RUST_LOG` directives take precedence.
* `directory` — directory for the day-rolling JSON log file (default `"logs"`). Overridden by `PREFLIGHT_LOG_DIR`.

### Outbound request logging

Every outbound HTTP request from a domain client must log an event with stable fields: `provider`, `operation`, `endpoint`, `status`, `elapsed_ms`, and page or item counts. Log `INFO` on success, `WARN`/`ERROR` on error or rate-limit paths. Include the error message on failure. **Never log request or response bodies.**

## Tooling, lint, and devenv

Lint findings should be fixed, not suppressed. `#[allow(...)]` requires a specific lint name and a justification comment.

```ini
# devenv tasks
server:migrate             # sea-orm-cli migrate
server:generate-entities   # migrate + sea-orm-cli generate entity (per domain)
server:generate-server     # seaography-cli generate
```

```sh
cargo fmt                          # format
cargo clippy -- -D warnings        # lint, warnings are errors
cargo test                         # unit tests (fast)
cargo test --features integration  # integration tests
cargo test --features e2e          # e2e tests
devenv tasks run server:migrate
```

Git hooks (via devenv): `rustfmt` on commit, `clippy` on commit.

CI runs `fmt --check`, `clippy`, `cargo test` (units), `cargo test --features integration`, `cargo test --features e2e`, in that order.

## Conformity checklist

Before finishing any task, verify:

- [ ] Started from a failing test (unit for logic, integration for infra behavior).
- [ ] `cargo test` (units) passes and still runs fast.
- [ ] Schema changed?: migration written with `up` **and** `down`, entities regenerated into the owning crate's `src/entity/`, all compile errors fixed.
- [ ] New crate failure mode?: sentinel error added to the crate's `error.rs`, mapped at the GraphQL boundary, covered by a handler test.
- [ ] No concrete silo types, driver types, or SDK types leaked across a crate boundary.
- [ ] New cross-domain relationship?: join table and `Related` impls live in `links`, not in a domain crate. Domain crates have zero edges to other domain crates.
- [ ] Crate depends on another crate?: via its trait, injected in `main`, mocked in unit tests. Domain crates never depend on other domain crates.
- [ ] New config value?: added to the owning crate's `Config` struct with the crate prefix, loaded in `main`.
- [ ] No new static state or `lazy_static` for domain state.
- [ ] Errors wrapped with context via `thiserror` `#[from]` or `map_err`.
- [ ] `cargo clippy` clean.
- [ ] Touched schema?: `down` migration reverses `up`, integration test green against migrated schema, no `CURRENT_TIMESTAMP` defaults.
- [ ] New or changed parsing/validation of untrusted input?: fuzz/proptest target exists with seeds for valid, empty, and tricky inputs.
- [ ] Fuzzer found a crasher?: regression seed committed, fix done via the TDD loop.
- [ ] Add/update documentation.
- [ ] Never write code that panics outside of `main` (and `main` should return `Result`, not panic).

Hard "never" list: never edit generated entity or seaography code. Never use mocks in E2E tests. Never construct one domain inside another — domain dependencies arrive as trait-typed constructor parameters, wired in `main`. Never reference another crate's concrete implementation types. Never put logic in `main`. Never `sleep` in a unit test. Never compare error `Display` strings. Never hit a real remote API outside `--features live-api` tests. Never use the builder pattern — use a `Default`-able options struct. Never make a required dependency optional. Never use `CURRENT_TIMESTAMP` as a column default. Never add a CHECK constraint to a domain you expect to grow. Never edit or delete an event row — append only. Never add a dependency from one domain crate to another domain crate.
