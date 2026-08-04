# entity

Generated SeaORM entities for the todo planner — the shared domain vocabulary.

Every other crate speaks these `Model` types across boundaries; no crate
defines its own parallel data model. The crate is regenerated from the
migrated schema (see `migration/README.md` and the `gen:entities` devenv
task) and committed.

## Contents

- `src/<table>.rs` — one file per table. Generated; **never edited by hand**.
- `src/prelude.rs` — `Entity as <Name>` aliases for query code
  (`entity::prelude::Todo`).
- `src/sea_orm_active_enums.rs` — **hand-maintained** `ActiveEnum` types
  (`TodoStatus`, `EventKind`, `EventActor`, `PullRequestState`,
  `LinkRelation`) plus the Seaography `CustomInputType`/`CustomOutputType`
  bridges that expose them as GraphQL enums. Regeneration preserves manual
  enum additions; re-verify after regenerating.
- `src/lib.rs` — module registry, `register_entity_modules!`, and the
  `impl_custom_output_type_for_entity!` bridges (orphan rule forces these
  to live here).

## Rules

This crate contains only data models. The only non-model code is the enum
and Seaography bridge glue in `sea_orm_active_enums.rs` / `lib.rs`, which
must live in the defining crate for the orphan rule. No business logic, no
services, no constructors.
