# preflight

A todo planner built on one central idea: **"doing today" is not a flag on a
todo, it's a row in a per-day table.** That gives the morning reset for free
(no cron job, nothing to clear), preserves yesterday's plan permanently, and
keeps the todo's status completely orthogonal to whether it's on today's list.

## Architecture

```
preflight/
├── Cargo.toml                 # workspace (resolver 3, all 6 members)
├── config/default.toml        # db path, tz, day_start_hour, sync tokens
├── crates/
│   ├── entity/src/            # SeaORM entities, enums hand-maintained
│   │   ├── todo.rs  tag.rs  todo_tag.rs  todo_day_plan.rs  todo_event.rs
│   │   ├── pull_request.rs  todo_pull_request.rs
│   │   ├── linear_issue.rs  todo_linear_issue.rs  sync_state.rs
│   │   └── sea_orm_active_enums.rs   # TodoStatus, EventKind, EventActor, …
│   ├── migration/src/         # 7 sea-orm-migration files + idens
│   ├── core/src/              # the ONLY crate that writes
│   │   ├── clock.rs           # logical_date (the only date logic)
│   │   ├── events.rs          # append-only writer, used by every mutation
│   │   ├── todo_service.rs    # create / edit / set_status
│   │   ├── day_plan.rs        # plan, unplan, reorder, carry_over
│   │   ├── links.rs           # link + convert PR / Linear issue + tags
│   │   ├── review.rs          # daily review (planned, touched, completed, carriedOver)
│   │   └── error.rs
│   ├── sync/src/              # one-way pullers (stub bodies, real upserts)
│   │   ├── linear.rs  github.rs  cursor.rs
│   ├── graphql/src/           # Seaography queries + hand-written mutations
│   │   ├── lib.rs  query_root.rs  query.rs  types.rs
│   │   └── mutation/          # todo.rs  day_plan.rs  tags.rs  convert.rs  sync.rs
│   └── server/src/
│       ├── main.rs  db.rs  config.rs  routes.rs
```

The rule that keeps the event log honest: **`graphql` never touches `entity`
for writes.** Every mutation goes through `core`, and every `core` write
appends to `todo_event` in the same transaction.

## The four decisions

1. **`todo_day_plan(plan_date, todo_id)`** instead of a boolean. Reset every
   morning = a new date has no rows yet. Yesterday's plan is just
   `WHERE plan_date = :yesterday`.
2. **`todo_event` is append-only and carries a denormalized `logical_date`.**
   "What did I work on yesterday" is an index scan, not timestamp math.
3. **A logical day starts at a configurable hour (04:00 local), not midnight.**
   Computed once in `core::Clock`, stored as a plain `DATE`.
4. **PRs and Linear issues are inbox rows, not todos.** They live in their own
   tables and are linked or converted by an explicit action.

## Running

```sh
cargo run -p migration -- up    # apply migrations (creates db.sqlite)
cargo run -p server             # serve GraphQL at http://127.0.0.1:8000
```

The SQLite pool is built with `PRAGMA foreign_keys = ON` (per-connection, off
by default), `journal_mode(Wal)`, and a busy timeout — see `server/src/db.rs`.

## GraphQL

Seaography generates the **query** surface (filter/sort/pagination/relations
over all ten tables). Generated CRUD mutations are disabled
(`register_entity!(builder, module, mutation: false)`); mutations are
hand-written intent-shaped resolvers that delegate to `core`:

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
linkPullRequest(todoId: Int!, pullRequestId: Int!, relation: LinkRelation!): Todo!
dismissPullRequest(id: Int!): PullRequest!
todoFromLinearIssue(linearIssueId: Int!, planToday: Boolean = false): Todo!
linkLinearIssue(todoId: Int!, linearIssueId: Int!): Todo!

syncLinear: SyncState!
syncGithub: SyncState!

dailyReview(date: Date!): DailyReview!
```

Enum values mirror their database `string_value` (e.g. `TodoStatus` accepts
`todo`, `started`, `blocked`, `done`, `cancelled`).
