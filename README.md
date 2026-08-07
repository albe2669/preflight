# preflight

A todo planner built on one central idea: **"doing today" is not a flag on a
todo, it's a row in a per-day table.** That gives the morning reset for free
(no cron job, nothing to clear), preserves yesterday's plan permanently, and
keeps the todo's status completely orthogonal to whether it's on today's list.

## Architecture

```
preflight/
├── Cargo.toml                 # workspace (resolver 3)
├── config/default.toml        # db path, tz, day_start_hour, sync tokens; every secret has a *_path variant
├── crates/
│   ├── todo/src/               # todo domain — reusable, the only writer
│   │   ├── entity/             # todo, todo_event, todo_day_plan, tag, todo_tag
│   │   │   └── enums.rs        # TodoStatus, EventKind, EventActor
│   │   ├── clock.rs            # logical_date (the only date logic)
│   │   ├── events.rs           # append-only writer, used by every mutation
│   │   ├── todo_service.rs     # create / edit / set_status
│   │   ├── day_plan.rs         # plan, unplan, reorder, carry_over
│   │   ├── review.rs           # daily review (planned, touched, completed, carriedOver)
│   │   └── error.rs
│   ├── github/src/             # github domain — reusable
│   │   ├── entity/             # pull_request + PullRequestState enum
│   │   ├── sync.rs             # one-way PR puller + upsert (stub body, real upsert)
│   │   ├── cursor.rs            # sync_state helpers
│   │   └── error.rs
│   ├── linear/src/             # linear domain — reusable
│   │   ├── entity/             # linear_issue
│   │   ├── sync.rs             # one-way issue puller + upsert (stub body, real upsert)
│   │   ├── cursor.rs
│   │   └── error.rs
│   ├── links/src/              # cross-domain composition (app-specific)
│   │   ├── entity/             # join tables: todo_pull_request, todo_linear_issue
│   │   │   └── enums.rs        # LinkRelation
│   │   ├── link_service.rs     # link + convert PR / Linear issue + tags
│   │   └── error.rs
│   ├── sync-state/src/         # shared sync cursor entity
│   │   └── entity/             # sync_state
│   ├── migration/src/          # 7 sea-orm-migration files + idens
│   ├── graphql/src/            # Seaography queries + hand-written mutations
│   │   ├── lib.rs  query_root.rs  query.rs  types.rs  entities.rs
│   │   └── mutation/           # todo.rs  day_plan.rs  tags.rs  convert.rs  sync.rs
│   └── server/src/
│       ├── main.rs  db.rs  config.rs  routes.rs
```

Domain crates (`todo`, `github`, `linear`) are self-contained and reusable —
no edges between them. `links` is the composition crate: it owns the join
tables that bridge domains and the `Related` impls that connect them (orphan-safe
because `Related` is a `sea_orm` trait implemented for local `links` entities).

The rule that keeps the event log honest: **`graphql` never touches entities
for writes.** Every mutation goes through `todo` or `links`, and every write
appends to `todo_event` in the same transaction.

Every service is a **producer-side trait** (`todo_domain::TodoService`,
`todo_domain::DayPlanService`, `links::LinkService`, …) with a private impl
and a `pub fn new(...) -> impl Trait` constructor. `server/main` constructs
each silo, wraps it in `Arc<dyn Trait>`, and injects it into the GraphQL
schema — the only place concrete implementations meet.

## The four decisions

1. **`todo_day_plan(plan_date, todo_id)`** instead of a boolean. Reset every
   morning = a new date has no rows yet. Yesterday's plan is just
   `WHERE plan_date = :yesterday`.
2. **`todo_event` is append-only and carries a denormalized `logical_date`.**
   "What did I work on yesterday" is an index scan, not timestamp math.
3. **A logical day starts at a configurable hour (04:00 local), not midnight.**
   Computed once in `todo_domain::Clock`, stored as a plain `DATE`.
4. **PRs and Linear issues are inbox rows, not todos.** They live in their own
   tables and are linked or converted by an explicit action in `links`.

## Running

```sh
cargo run -p migration -- up    # apply migrations (creates db.sqlite)
cargo run -p server             # serve GraphQL at http://127.0.0.1:8000
```

The SQLite pool is built with `PRAGMA foreign_keys = ON` (per-connection, off
by default), `journal_mode(Wal)`, and a busy timeout — see `server/src/db.rs`.

## Installing via Nix (home-manager)

The repo ships a Nix flake that builds the `server` crate into a `preflight`
binary and provides a `programs.preflight` home-manager module. This is the
way to install and configure preflight on a NixOS/home-manager system
(build-config-through-Nix instead of cloning the repo + devenv).

Reference the flake's module and overlay in your home-manager configuration:

```nix
# flake.nix inputs
preflight = {
  url = "github:<you>/preflight";
  inputs.nixpkgs.follows = "nixpkgs";
};

# home-manager config
imports = [ preflight.homeManagerModules.preflight ];
home.packages = with pkgs; [ preflight.packages.${pkgs.stdenv.hostPlatform.system}.default ];
```

Then enable it:

```nix
programs.preflight.enable = true;
programs.preflight.settings = {
  clock.timezone = "America/New_York";
  # server.defaults: host = "127.0.0.1", port = 8000 (matching config/default.toml)
};
```

The module installs a `preflight` launcher that hands the rendered
`config.toml` to the server via the `CONFIG` env var, so `preflight` on your
`PATH` starts the server with the Nix-built configuration.

### Options

- `programs.preflight.enable` — install the packaged binary and launcher.
- `programs.preflight.package` — the derivation to install. Defaults to the
  flake package (exposed as `pkgs.preflight` via the `overlays.default`).
- `programs.preflight.settings.database.path` — absolute SQLite path.
  **Defaults to `~/.local/state/preflight/db.sqlite`.** A packaged binary has
  no project root, so a relative path is not meaningful — always use an
  absolute path here (this differs from source runs, where `db.sqlite`
  resolves against the repo root).
- `programs.preflight.settings.clock` — `timezone`, `dayStartHour`.
- `programs.preflight.settings.sync` — `githubToken`, `githubTokenPath`,
  `linearToken`, `linearTokenPath`, `github.*`, `linear.*`.
- `programs.preflight.settings.server` — `host`, `port`, `depthLimit`,
  `complexityLimit`.
- `programs.preflight.configFile` — a complete `config.toml` to launch with.
  When set it takes precedence over any `settings`-rendered content (the
  module does not merge the two).

### Secrets with sops-nix

Point a `*_token_path` at a decrypted sops-nix secret file. The app reads the
token from the file (path wins over the inline token), matching the
source-run behavior. Enable `sops-nix` yourself and reference the resolved
path:

```nix
sops.secrets."preflight-github" = { };
sops.secrets."preflight-linear" = { };

programs.preflight.settings.sync = {
  githubTokenPath = config.sops.secrets."preflight-github".path;
  linearTokenPath = config.sops.secrets."preflight-linear".path;
};
```

An inline `githubToken` / `linearToken` still works; it is only overridden
when the corresponding `*_token_path` is set.

## GraphQL

Seaography generates the **query** surface (filter/sort/pagination/relations
over all ten tables). Generated CRUD mutations are disabled
(`register_entity!(builder, module, mutation: false)`); mutations are
hand-written intent-shaped resolvers that delegate to domain services:

```graphql
createTodo(title: String!, description: String): Todo!
updateTodo(id: Int!, title: String, description: String): Todo!
setTodoStatus(id: Int!, status: TodoStatus!, blockedReason: String): Todo!

planForToday(todoId: Int!): TodoDayPlan!
unplanForToday(todoId: Int!): Boolean!
reorderDayPlan(date: Date!, todoIds: [Int!]!): [TodoDayPlan!]!
carryOverUnfinished(from: Date!, to: Date!): [TodoDayPlan!]!

addTag(todoId: Int!, slug: String!): Todo!
removeTag(todoId: Int!, slug: String!): Todo!

todoFromPullRequest(pullRequestId: Int!, planToday: Boolean = true): Todo!
linkPullRequest(todoId: Int!, pullRequestId: Int!, relation: LinkRelations!): Todo!
dismissPullRequest(id: Int!): PullRequest!
todoFromLinearIssue(linearIssueId: Int!, planToday: Boolean = false): Todo!
linkLinearIssue(todoId: Int!, linearIssueId: Int!): Todo!

syncLinear: SyncState!
syncGithub: SyncState!

dailyReview(date: Date!): DailyReview!
```

Enum values mirror their database `string_value` (e.g. `TodoStatus` accepts
`todo`, `started`, `blocked`, `done`, `cancelled`).
