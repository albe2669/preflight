# Rust development guidelines

This document defines the architecture, coding, and testing conventions for this codebase. Every rule is intended to be followed literally by both humans and AI agents. When a rule here conflicts with general Rust folklore, this document wins.

## Core principles

1. **Siloed crates.** Each piece of infrastructure (database, sync, GraphQL, server, ...) lives in its own crate under `crates/`. A silo owns its config, its errors, its traits, and its domain types. Crates may depend on other crates, but only through the other crate's public API (traits, error types, domain types), received via constructor injection. Everything is wired together only in `main`.
2. **Producer-side traits.** Every silo that is consumed by another crate exports traits that are the *only* thing other crates are allowed to depend on. Concrete implementation types stay private or are touched only in `server`/`main`.
3. **TDD, always.** No production code without a failing test first. Unit tests are fast and mock-based, integration tests use a real SQLite database.
4. **Explicit over magic.** Manual dependency injection, SeaORM queries, axum routing. No DI containers, no hidden state, no globals.
5. **The compiler is the safety net.** Prefer designs where mistakes fail `cargo build`, then designs where mistakes fail `cargo test`, and only then designs that rely on review.
6. **One writer.** `core` is the only crate that mutates domain state. Sync pulls remote data in, GraphQL mutations delegate to `core`. No other crate writes to the database.

## Repository layout

```
.
├── Cargo.toml                 # workspace, members listed here
├── crates/
│   ├── entity/                # sea-orm-cli generated (never edited)
│   │   └── src/
│   │       ├── mod.rs         # module registry + seaography registration
│   │       ├── prelude.rs     # generated, re-exports Entity aliases
│   │       ├── todo.rs  tag.rs  todo_tag.rs  todo_event.rs  ...
│   │       └── sea_orm_active_enums.rs
│   ├── migration/             # sea-orm-migrate
│   │   └── src/
│   │       ├── lib.rs          # Migrator struct + migration list
│   │       ├── idens.rs        # shared Iden enums for table/column names
│   │       ├── main.rs         # CLI entrypoint (sea-orm-cli)
│   │       └── m20260101_NNNNNN_name.rs
│   ├── core/                  # the ONLY crate that writes; business logic
│   │   └── src/
│   │       ├── clock.rs        # logical_date
│   │       ├── events.rs       # append-only event writer
│   │       ├── todo_service.rs # create / edit / set_status
│   │       ├── day_plan.rs     # plan, unplan, reorder, carry_over
│   │       ├── links.rs        # link + convert PR / Linear issue
│   │       ├── review.rs       # daily review queries
│   │       └── error.rs
│   ├── graphql/               # seaography roots + hand-written mutations
│   │   └── src/
│   │       ├── lib.rs          # schema builder
│   │       ├── query_root.rs   # seaography-generated query roots + custom queries
│   │       ├── mutation/       # hand-written, intent-shaped
│   │       │   ├── todo.rs  day_plan.rs  tags.rs  convert.rs  sync.rs
│   │       └── types.rs        # input types, DailyReview object
│   ├── server/                # axum server, wiring, lifecycle
│   │   └── src/
│   │       ├── main.rs  db.rs  config.rs  routes.rs
│   ├── linear/                 # Linear sync client, cursor parsing, rate-limit etc.
│   │   └── src/
│   ├── github/                 # Github sync client, cursor parsing, rate-limit etc.
│   │   └── src/
│   └── <domain>/                 # extra domains
│       └── src/
├── config/
│   └── default.toml            # db path, tz, day_start_hour, tokens
├── devenv.nix                 # dev environment, tasks, git-hooks
└── GUIDELINES.md
```

Rules:

* No `lib.rs` that re-exports the whole crate as a flat namespace. Each module is referenced by path (`core::todo_service`, not `core::TodoService`). The exception is `entity`, which is generated and follows sea-orm-cli's layout.
* Generated code (`entity/`, seaography output) is committed to the repo and regenerated with the devenv tasks. Never edit it by hand.
* `server/src/main.rs` contains wiring and lifecycle only — no business logic, no GraphQL handlers.
* Keep `crates/` flat. Do not introduce grouping directories. A new concern is a new crate.

## Crate design: silos and traits

### Documentation

Every crate should have a `README.md` in its top-level describing the exposed functionalities and providing an overview of the internal and external flows.

### The silo contract

Every crate that is consumed by another follows the same shape. Using this `crates/core` example as the template:

```rust
// crates/core/src/error.rs
use thiserror::Error;

/// Sentinel errors that callers must distinguish.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("todo not found")]
    NotFound,
    #[error("todo title already exists")]
    Conflict,
    #[error(transparent)]
    Db(#[from] sea_orm::DbErr),
}
```

```rust
// crates/core/src/todo_service.rs
use sea_orm::DatabaseConnection;

/// TodoService is the trait consumed by the GraphQL layer.
/// A silo's public API — the only names other crates may reference — is:
/// the trait, the error type, domain types, and the constructor
/// (constructor calls belong in server/main only).
pub trait TodoService: Send + Sync {
    async fn create(&self, title: String) -> Result<entity::todo::Model, CoreError>;
    async fn set_status(&self, id: i64, status: &str) -> Result<(), CoreError>;
}
```

```rust
// crates/core/src/todo_service_impl.rs
use sea_orm::DatabaseConnection;

/// The real implementation. It is private; only the constructor is pub.
pub(crate) struct TodoServiceImpl {
    db: DatabaseConnection,
    clock: Clock,
}

/// Called from server/main only.
/// Constructors return the trait so callers can never depend on the concrete type.
pub fn new(db: DatabaseConnection, clock: Clock) -> impl TodoService {
    TodoServiceImpl { db, clock }
}
```

Rules:

* **At least one trait per consumed silo**, named for the role (`TodoService`, `SyncSource`), not `Interface` or `ITodo`. Referring to it from outside reads as `core::TodoService`, the crate name is part of the name, so no stutter (`core::CoreTodoService` is wrong).
* Trait methods accept `&self` (or `&mut self`) and return `Result`. Async methods are the norm.
* Traits speak **domain types**, never driver types. `sea_orm::QueryResult`, raw `sqlx` rows, or provider SDK types must not appear in any exported signature. The `entity` crate's `Model` types are the shared domain vocabulary.
* Keep traits as small as the current callers need. Add methods when a real caller needs them (test-first), not speculatively.
* If a silo grows a second distinct responsibility, split it into a second trait (or crate) rather than widening the interface.
* Business-logic crates (`core`) export producer-side traits consumed by the GraphQL layer, following the same rules.
* The following files, if present, should serve specific purposes:
  * `error.rs`: sentinel errors that the crate can emit, intended for consumption and handling by other crates.
  * `config.rs`: `Config` struct and the `load_config` function. `load_config` must not panic.
  * `lib.rs`: crate root — module declarations and re-exports of public traits/types only.
  * `clock.rs`: the logical-date clock abstraction.
  * `events.rs`: the append-only event writer used by every mutation.

### Silo-to-silo dependencies

A crate may depend on another crate, under strict conditions:

* Only through the other crate's **public API**: its traits, error types, and domain types. Referencing another crate's private implementation, config internals, or generated code (other than the shared `entity` crate) is forbidden.
* The dependency arrives as a **constructor parameter**, wired in `main`/`server`. A crate never constructs another crate's silo, never reads another crate's config, and never imports another crate's test helpers outside its own unit tests.
* In the depending crate's unit tests, the dependency is a mock. If a crate's unit tests would need another crate's real infrastructure, the dependency boundary is wrong.
* Cargo's compiler forbids import cycles, so the crate graph is a DAG by construction — but keep it shallow. If A needs B and B starts needing A's *data*, pass the data as function arguments instead of inverting the dependency.

### Consuming a silo

```rust
// crates/graphql/src/mutation/todo.rs
use core::TodoService;

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

Constructor signatures follow a fixed shape: **required things are positional, optional things are via a config struct or defaults.**

```rust
pub fn new(db: DatabaseConnection, clock: Clock) -> impl TodoService { ... }
```

* Required dependencies are positional arguments; forgetting one must be a compile error, not a runtime panic. Required dependencies are never optional.
* For optional behavior, prefer typed config structs over a long parameter list. If a constructor grows beyond 4 parameters, group the optional ones into a `Options` struct with `#[derive(Default)]`.
* **Never use the builder pattern** (`b.set_timeout(x).set_retries(y).build()`). Builders allow half-built objects, hide required fields, and read poorly in Rust. Use a `Default`-able options struct passed as the last argument.

```rust
// crates/sync/src/linear.rs
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
pub fn new(db: DatabaseConnection, opts: Options) -> impl LinearSync { ... }
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

    let clock = core::Clock::new();
    let todo_svc = core::todo_service::new(db.clone(), clock.clone());
    let day_plan_svc = core::day_plan::new(db.clone(), clock.clone());
    let sync_runner = sync::new(db.clone());

    let schema = graphql::build_schema(db, todo_svc, day_plan_svc, sync_runner)?;
    let app = routes::router(schema);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

Rules:

* `main` is thin: load config, connect db, run migrations, build silos, build schema, build server, run. Errors bubble up through `Result`; only `main` decides the exit code.
* Graceful shutdown via axum's built-in `serve` + signal handling. Every long-running component takes a cancellation handle where relevant.

## Error handling

* Each crate defines **sentinel errors** for the failure modes callers must distinguish: `CoreError::NotFound`, `CoreError::Conflict`, `SyncError::RateLimited`, ...
* Errors are defined using `thiserror::Error`. Every error variant has a `#[error("...")]` message.
* Check with `matches!` or `if let`. Never compare `Display` strings.
* Wrap on the way up, always with context: `map_err(|e| CoreError::Db(e))` or `?` via `#[from]`. Never `unwrap` swallowing a cause, never bare `return Err(e)` across a crate boundary if you can add information.
* Only the GraphQL layer translates errors into response fields/HTTP status. Core and sync know nothing about GraphQL. Errors are unwrapped and mapped to their respective GraphQL error extensions, with the most external error taking precedence.
* `DbErr` is an infrastructure detail; it gets wrapped into the crate's own error type at the boundary, never leaked in a public signature.

## The database layer

### Schema ownership

* Migrations live in `crates/migration`. They are the **single source of truth** for schema. The `entity` crate is generated from the migrated schema and committed.
* Regenerate entities after schema changes: `devenv tasks run server:generate-entities` (runs migrate then `sea-orm-cli generate entity`).
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
* **CHECK constraints only on closed, stable domains.** `todo.status` and `todo_event.actor` are closed. `todo_event.kind` and `pull_request.state` are *not* — SQLite cannot drop or alter a CHECK, so anything you expect to grow must not have one. The `ActiveEnum` in the entity crate is the guard for open domains.
* **Shared `Iden`s** live in `crates/migration/src/idens.rs`. The SeaORM docs suggest re-declaring per-migration to freeze against renames; for a greenfield schema where four migrations share `Todo::Id`, one shared module is less noisy. Rule if you ever rename: copy the *old* `Iden` into the migration that used it, then rename in `idens.rs`.
* Foreign keys name themselves (`fk_<table>_<ref>`) and specify `on_delete`. Use `Cascade` for join tables and child rows that are owned by the parent.
* Indexes are named `idx_<table>_<cols>` (non-unique) or `ux_<table>_<cols>` (unique). Every foreign-key column and every query filter column gets an index.
* Register every migration in `lib.rs`'s `Migrator::migrations()` vector.
* Tables use `if_not_exists()` defensively. Indexes too.
* `down` drops tables in reverse dependency order (children before parents).

### Entities

* Generated by `sea-orm-cli generate entity -o crates/entity/src --with-serde both --model-extra-derives 'async_graphql::SimpleObject' --seaography`.
* The `mod.rs` file is generated and includes a `seaography::register_entity_modules!([...])` macro call listing every entity module. Keep this in sync when adding/removing tables.
* `prelude.rs` re-exports `Entity as <Name>` aliases. Use these (`entity::prelude::Todo`) in query code, not the raw module path.
* Enums (`sea_orm_active_enums.rs`) are **hand-maintained** on top of the generated output. The generation command preserves manual enum additions; verify after regenerating.
* The `entity` crate is a shared dependency. It contains only data models — no logic, no traits, no constructors.

### The event log

`todo_event` is an append-only log. Every mutation in `core` writes at least one event row via `events.rs`. The event records `kind`, `field`, `old_value`, `new_value`, `actor` (`user` / `sync` / `system`), `occurred_at`, and `logical_date`.

Rules:

* Never update or delete an event row. Append only.
* `logical_date` is denormalized from `occurred_at` via `Clock::logical_date` so "what did I do yesterday" is an index scan, never a timezone calculation in SQL.
* The background Linear sync must not make an untouched todo look worked on — the `actor` CHECK (`user`/`sync`/`system`) and the `idx_todo_event_day` index (date, actor) keep this queryable and auditable.

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

### Test file naming

* In-file `#[cfg(test)] mod tests` — **unit tests** for the module. Default. Testing through the public API keeps tests valid across refactors. If you need to test internal workings, add a test that navigates the public API to reach the internal point, not a separate `pub(crate)` test module.
* `tests/<name>.rs` — integration tests in the `tests/` directory, exercising the crate's public API.
* `tests/e2e/*.rs` — e2e tests, behind a feature flag, speaking HTTP to a running server.

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

```rust
#[tokio::test]
async fn test_create_todo_rejects_empty_title() {
    let db = setup::memory_db().await;
    let svc = core::todo_service::new(db, Clock::fixed());
    let err = svc.create("".to_string()).await.unwrap_err();
    assert!(matches!(err, CoreError::Validation(_)));
}
```

### Fuzz tests

Use `cargo-fuzz` or `proptest` for every function that parses, decodes, validates, or transforms **untrusted input**: GraphQL input validation, sync cursor parsing, URL/key sanitizers, pagination cursor decoding, and any hand-written parser. Pure business logic with trusted inputs does not need fuzzing.

```rust
proptest! {
    #[test]
    fn parse_cursor_never_panics(raw in ".*") {
        // must never panic, whatever the input
        let _ = parse_cursor(&raw);
    }

    #[test]
    fn cursor_round_trips(offset in 0i64..1_000_000) {
        let c = Cursor { offset };
        let encoded = c.encode();
        let parsed = parse_cursor(&encoded).unwrap();
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
    use core::TodoService;
    pub struct FailingTodoService;
    impl TodoService for FailingTodoService {
        async fn create(&self, _: String) -> Result<entity::todo::Model, CoreError> {
            Err(CoreError::Conflict)
        }
        async fn set_status(&self, _: i64, _: &str) -> Result<(), CoreError> {
            Err(CoreError::NotFound)
        }
    }
}
```

Rules:

* One fake per trait, defined in the *consuming* crate's test module, not the producing crate.
* Use `mockall` only when a trait has many methods and the test needs per-call assertion. Prefer hand-written fakes for simple cases — they read better and carry no dependency.
* Mocks are for unit tests only. Integration tests use real dependencies; e2e tests must not use mocks.
* Do not mock the `entity` crate's `Model` types — they are plain data structs. Use them directly.
* Mock silo traits, not functions inside a crate. Mocks are for cross-crate boundaries.

### Mocking transactions correctly

If a service method wraps multiple writes in a transaction, the unit test must assert that a failure mid-transaction rolls back. Use a fake `DatabaseConnection` (or a trait abstracting the transaction) that records commits/rollbacks. Never assert commit/rollback mechanics in unit tests against a real db — that is the db layer's job, verified in its integration tests.

## Integration tests

Integration tests exercise a single crate against a real SQLite database (temp file or `:memory:`). Each crate that needs db access owns a test setup helper:

```rust
// crates/core/tests/common.rs
pub async fn setup_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    migration::Migrator::up(&db, None).await.unwrap();
    db
}
```

Rules:

* Use `sqlite::memory:` or a `tempfile`-backed db. Never touch the developer's `db.sqlite`.
* Migrations always run inside the test setup so integration tests validate the real, migrated schema.
* The sync crate's integration tests run against an `httptest::Server` that speaks the provider's wire format (verifying request encoding, auth headers, retry/backoff, error mapping). Tests that hit the real provider API are allowed only behind an explicit feature (`--features live-api`) and are never part of CI. This is 1. due to cost, 2. because of the non-deterministic nature of remote APIs and 3. because they're generally slow and have caused issues in the past.
* Integration tests use real dependencies for the crate under test and should use mocks for unrelated crates.

## E2E tests

E2E tests treat the system as a black box: a real SQLite db is created, migrations run, the server is started, and tests speak HTTP/GraphQL.

* Location: `tests/e2e/`, behind `#[cfg(feature = "e2e")]`, package `e2e`.
* Setup creates a temp db, runs migrations, starts the axum server on a random port, and constructs the full dependency graph exactly like `main.rs` does.
* Tests cover user-visible flows end to end (create → fetch → list → delete), and the error contract. They do not re-test edge cases already covered by unit tests.
* No assertions against database rows — assert only through the GraphQL/HTTP API, like a client would.

## The GraphQL layer (`crates/graphql`)

* The query surface is generated by `seaography-cli` into `crates/graphql`. The generated `query_root.rs` and `lib.rs` register entity modules and build the schema. Regenerate with `devenv tasks run server:generate-server`.
* **Mutations are hand-written** in `crates/graphql/src/mutation/`. They are intent-shaped (`planTodoForToday`, not `insertIntoTodoDayPlan`), validate input, delegate to `core` services, and return domain types.
* Custom queries that seaography can't express live in `query_root.rs` alongside the generated roots.
* Input types and custom output objects live in `types.rs`.
* The schema owns depth/complexity limits (`DEPTH_LIMIT`, `COMPLEXITY_LIMIT` env vars) for query safety.
* Handler unit tests use `async_graphql::Schema::execute` with mocked services and assert the response shape and errors.

### Regenerating the GraphQL server

```sh
devenv tasks run server:generate-entities   # migrate + generate entity
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
}
```

Rules:

* Config keys are **prefixed with the crate/domain name** (`DATABASE_URL`, `SYNC_LINEAR_TOKEN`, `SERVER_PORT`) so ownership is greppable and collisions are impossible. A crate never reads another crate's variables.
* `main` is the only place config is loaded (once per crate config, fail-fast with the crate name in the error).
* Cross-field validation beyond what the config format expresses lives in the crate's constructor, which returns an error for invalid config.
* No secrets in `config/default.toml`. Secrets come from env vars only, never get default values, and never get logged.

## Logging

* `tracing` is the logging facade, initialized once in `main`. No `println!` in library crates, no third-party logging libraries.
* Structured fields, stable keys: `tracing::info!(todo_id = id, "todo created")`.
* Levels: `DEBUG` for diagnostics, `INFO` for lifecycle and notable events, `WARN` for degraded-but-handled, `ERROR` only where the error is finally handled.
* Error logs will trigger an alert, so use sparingly.
* Always use span context (`#[tracing::instrument]`) so request-scoped attributes are included automatically.

## Tooling, lint, and devenv

Lint findings should be fixed, not suppressed. `#[allow(...)]` requires a specific lint name and a justification comment.

```ini
# devenv tasks
server:migrate             # sea-orm-cli migrate
server:generate-entities   # migrate + sea-orm-cli generate entity
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

## Comments

- Comments are the exception, not the rule. Add one only when the code cannot be made self-explanatory and the *why* is non-obvious; never write a comment that restates *what* the code does.
- Never include in a comment where the code is used. Comments like "called by `foo()`" or "used in `bar.rs`" are noise. The compiler and IDE can answer that question.
- For exported symbols, follow the language's doc-comment convention but only when the comment adds information beyond the identifier's name, and keep it to one sentence.
- Prefer extracting a well-named helper over writing a comment to explain a block.
- Extra comments a human wouldn't write: doc comments that restate the identifier name, comments narrating obvious code, per-field/per-constant annotations, section-divider banners, or TODO/FIXME markers without an owner and an actionable next step. Comments inconsistent with the rest of the file are equally bad.

## Conformity checklist

Before finishing any task, verify:

- [ ] Started from a failing test (unit for logic, integration for infra behavior).
- [ ] `cargo test` (units) passes and still runs fast.
- [ ] Schema changed?: migration written with `up` **and** `down`, entities regenerated, all compile errors fixed.
- [ ] New crate failure mode?: sentinel error added to `error.rs`, mapped at the GraphQL boundary, covered by a handler test.
- [ ] No concrete silo types, driver types, or SDK types leaked across a crate boundary.
- [ ] Crate depends on another crate?: via its trait, injected in `main`, mocked in unit tests.
- [ ] New config value?: added to the owning crate's `Config` struct with the crate prefix, loaded in `main`.
- [ ] No new static state or `lazy_static` for domain state.
- [ ] Errors wrapped with context via `thiserror` `#[from]` or `map_err`.
- [ ] `cargo clippy` clean.
- [ ] Touched schema?: `down` migration reverses `up`, integration test green against migrated schema, no `CURRENT_TIMESTAMP` defaults.
- [ ] New or changed parsing/validation of untrusted input?: fuzz/proptest target exists with seeds for valid, empty, and tricky inputs.
- [ ] Fuzzer found a crasher?: regression seed committed, fix done via the TDD loop.
- [ ] Add/update documentation.
- [ ] Never write code that panics outside of `main` (and `main` should return `Result`, not panic).

Hard "never" list: never edit generated entity or seaography code. Never use mocks in E2E tests. Never construct one silo inside another — silo dependencies arrive as trait-typed constructor parameters, wired in `main`. Never reference another crate's concrete implementation types. Never put logic in `main`. Never `sleep` in a unit test. Never compare error `Display` strings. Never hit a real remote API outside `--features live-api` tests. Never use the builder pattern — use a `Default`-able options struct. Never make a required dependency optional. Never use `CURRENT_TIMESTAMP` as a column default. Never add a CHECK constraint to a domain you expect to grow. Never edit or delete an event row — append only.
