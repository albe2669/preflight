# server

The axum server: wiring and lifecycle only.

Loads config, connects the SQLite pool, runs migrations, constructs every
silo (core services + sync services), builds the GraphQL schema, and serves
it. No business logic lives here.

## Exposed API

Binary `server` — serves GraphQL at `http://<host>:<port>/` with a Playground
at the same path.

## Internal flow

- `main.rs` — the wiring: `Config::load` → `db::connect` → `db::migrate` →
  construct `core::<svc>::new(db, clock)` and `sync::<src>::new(db, opts)` →
  `graphql::schema_builder(...)` → `routes::router(schema)` → `axum::serve`.
  Errors bubble up through `Result`; only `main` decides the exit code.
- `config.rs` — `Config` struct deserialized from `config/default.toml` (or
  `CONFIG` env var). `Config::load` never panics.
- `db.rs` — SQLite pool with `PRAGMA foreign_keys = ON`, WAL journal, and a
  busy timeout. `migrate` applies all pending migrations.
- `routes.rs` — the axum `Router`: `/` GET = Playground, `/` POST = GraphQL
  handler.

## External flow

Every silo is constructed here and injected as a trait object into the
GraphQL schema. This is the only place concrete implementations meet.
