# core

The only crate that writes to the database.

Every mutation — from a GraphQL resolver or a sync puller — goes through a
service here, and every service write appends to `todo_event` in the same
transaction. That invariant is what keeps the event log honest: there is no
code path that changes a todo without recording it.

## Exposed API

Each service is a **producer-side trait** consumed by the GraphQL layer:

- `TodoService` — create / update / set status
- `DayPlanService` — plan / unplan / reorder / carry over
- `LinkService` — link + convert PR / Linear issue; add / remove tags
- `ReviewService` — daily review queries (planned, touched, completed, carried over)

Concrete implementations are `pub(crate)`; callers construct them via the
module-level `new(db, clock) -> impl <Trait>` constructor (called from
`server/main` only). `EventWriter` is `pub(crate)` — internal to core.

Domain types re-exported: `Clock`, `Error`, `Result`, `DailyReview`.

## Internal flow

A service method opens a `transaction`, performs the write, appends a
`todo_event` row via `EventWriter::append` on the same transaction, and
commits. `logical_date` is computed once from `Clock` (not in SQL) so
"what did I do yesterday" is an index scan over `todo_event.logical_date`.

## External flow

`graphql` consumes the traits as `Arc<dyn TodoService>` (etc.), injected via
the schema's `Context` data and wired in `server/main`. `sync` does **not**
depend on this crate.
