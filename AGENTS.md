# AGENTS.md

Instructions for AI agents (and humans) working in this repository.
**Read `GUIDELINES.md` first — it is the authoritative architecture and
coding standard. This file enforces conformity to it.**

Every rule below is load-bearing. Violating one is a bug, even if the code
compiles. When in doubt, `GUIDELINES.md` wins over general Rust folklore.

---

## 1. Before you write any code

1. **Read `GUIDELINES.md` in full.** It is short and literal.
2. **Identify which crate(s) you are touching** and read that crate's
   `README.md` for its exposed API and flow boundaries.
3. **Check the conformity checklist** at the foot of `GUIDELINES.md` before
   you start, and re-check it before you finish.

## 2. Silo and trait contract (the most-violated rules)

- Every service consumed across a crate boundary is a **trait**, named for
  its role (`TodoService`, `GithubSync`), not `ITodo` or `Interface`.
- Concrete implementations are `pub(crate)`. The only pub constructor is a
  module-level `pub fn new(...) -> impl Trait`. Callers can never name the
  concrete type.
- Constructors: required deps are positional; optional behavior goes in a
  `Default`-able options struct (`GithubOptions`, `LinearOptions`). **Never
  the builder pattern.** Never make a required dependency optional.
- A crate depends on another crate **only through its traits, error types,
  and domain types** — never its private impl, config internals, or
  generated code (other than the shared `entity` crate). Dependencies arrive
  as constructor params, wired in `server/main`.
- `core` is the only crate that writes. `graphql` never touches `entity`
  for writes. `sync` upserts inbox tables only and does not depend on `core`.

**Self-check before finishing a change:**
- [ ] Did I add/keep a trait for any service another crate consumes?
- [ ] Is the concrete impl `pub(crate)`?
- [ ] Does the constructor return `impl Trait`?
- [ ] Did I wire the dependency in `server/main`, not construct it inside
      another silo?
- [ ] No `DbErr`, `QueryResult`, or provider SDK type in an exported signature?

## 3. TDD loop (mandatory)

No production code without a failing test first.

1. **Red** — write/extend a test that fails for the right reason. Run it;
   confirm the failure message.
2. **Green** — minimum code to pass. Run `cargo test`.
3. **Refactor** — clean up with tests green. Run `cargo clippy -- -D
   warnings` and `cargo test` again.

Never write production code and its tests in the same step "to save time".
A test with no meaningful assertion is worse than no test.

**Test tiers** (feature-gated):
- Unit (`#[cfg(test)] mod tests`) — mocks, fast, `cargo test`.
- Integration (`--features integration`) — real SQLite, temp file db.
- E2E (`--features e2e`) — HTTP to a running server.

Never `sleep` in a unit test — inject clocks. Never mock in E2E. Never hit
a real remote API outside `--features live-api`.

## 4. Schema changes

- Write a migration `mYYYYMMDD_NNNNNN_snake_case.rs` with `up` **and** `down`.
  `down` reverses `up` exactly; drop in reverse dependency order.
- Register it in `migration/src/lib.rs` `Migrator::migrations()`.
- No `CURRENT_TIMESTAMP` defaults — the app sets every timestamp explicitly.
- CHECK constraints only on closed, stable domains (`todo.status`,
  `todo_event.actor`). Open domains (`todo_event.kind`, `pull_request.state`)
  use the `ActiveEnum` as the guard, never a CHECK.
- After schema changes: apply migration, regenerate entities
  (`devenv tasks run gen:entities`), re-apply the hand-maintained enum typing
  in `sea_orm_active_enums.rs`, fix every compile error. **Never edit
  generated entity files by hand.**

## 5. Error handling

- `thiserror` sentinel errors per crate (`CoreError`, `SyncError`). Every
  variant has a `#[error("...")]`.
- Wrap on the way up with context (`#[from]` or `map_err`). Never leak
  `DbErr` across a crate boundary.
- Check with `matches!` / `if let`, never `Display` string comparison.
- Only the GraphQL layer translates errors into response fields/HTTP.

## 6. Config and logging

- Config keys are prefixed with the crate/domain (`DATABASE_URL`,
  `LINEAR_TOKEN`, `SERVER_PORT`). A crate never reads another crate's vars.
- `main` is the only place config is loaded. Inject it; resolvers never call
  `std::env::var`.
- No secrets in `config/default.toml`. Secrets come from env, never logged.
- `tracing` only. No `println!`/`eprintln!`/`dbg!` in library crates.

## 6.5 Nix packaging (flake + home-manager)

- The flake (`flake.nix`) exposes `packages.<system>.preflight`,
  `homeManagerModules.preflight`, and `overlays.default`
  (`pkgs.preflight`). `nix/` holds the packaging:
  `nix/package.nix` (naersk build), `nix/toml.nix` (config serializer), and
  `nix/modules/home-manager.nix` (`programs.preflight`).
- `nix/toml.nix` emits exactly the keys in `crates/server/src/config.rs`
  (its `SyncConfig` is `deny_unknown_fields`). Read that file before touching
  the serializer, and keep them in lockstep when the config schema changes.
- Config changes that add/remove a required option or rename a TOML key must
  update `nix/toml.nix` and `nix/modules/home-manager.nix` in the same change.
- Package dependency changes (Rust crates linking system libs) may add
  `buildInputs`/`nativeBuildInputs` in `nix/package.nix`.

## 7. Comments

Comments are the exception. Add one only when the *why* is non-obvious and
the code cannot be made self-explanatory. Never restate *what*. Never note
where code is used. No section banners, no per-field annotations, no
ownerless TODO/FIXME. Prefer a well-named helper over a comment.

## 8. Tooling

- `cargo fmt` (run it).
- `cargo clippy --all-targets -- -D warnings` must be clean. Fix findings,
  don't suppress. `#[allow(...)]` needs a specific lint name + justification.
- `cargo test` runs units only and must stay fast (<10s).

## 9. Commits

- **One atomic commit per change.** A commit is self-contained: it compiles,
  tests pass, and it implements one conformity fix (or a coherent group).
- Never commit generated entity or seaography output as part of a logic
  change — regenerate it via the devenv tasks.
- Never edit generated code by hand.

## 10. Hard "never" list (from GUIDELINES.md)

Never edit generated entity or seaography code. Never use mocks in E2E
tests. Never construct one silo inside another — deps arrive as trait-typed
constructor params wired in `main`. Never reference another crate's concrete
implementation types. Never put logic in `main`. Never `sleep` in a unit
test. Never compare error `Display` strings. Never hit a real remote API
outside `--features live-api`. Never use the builder pattern. Never make a
required dependency optional. Never use `CURRENT_TIMESTAMP` as a column
default. Never add a CHECK constraint to a domain you expect to grow. Never
edit or delete an event row — append only. Never write code that panics
outside `main` (and `main` returns `Result`, not panic).

---

## Conformity quick-check (run before yielding)

- [ ] Started from a failing test (unit for logic, integration for infra).
- [ ] `cargo test` (units) passes and stays fast.
- [ ] Schema changed? migration with `up` + `down`, entities regenerated,
      compile errors fixed.
- [ ] New crate failure mode? sentinel error added, mapped at GraphQL
      boundary, covered by a handler test.
- [ ] No concrete silo / driver / SDK types leaked across a boundary.
- [ ] Crate depends on another? via trait, injected in `main`, mocked in
      unit tests.
- [ ] New config value? in the owning crate's `Config`, crate-prefixed,
      loaded in `main`.
- [ ] No new static state / `lazy_static` for domain state.
- [ ] Errors wrapped with context via `thiserror` `#[from]` / `map_err`.
- [ ] `cargo clippy -- -D warnings` clean.
- [ ] Parsing/validation of untrusted input? fuzz/proptest target with seeds.
- [ ] Documentation added/updated.
- [ ] Config schema changed? `nix/toml.nix` and `nix/modules/home-manager.nix`
      updated and in lockstep with `crates/server/src/config.rs`.
- [ ] Nix change builds: `nix flake check` and `nix build .#preflight` pass;
      `nix/tests/eval-module.nix` still evaluates.
- [ ] No panics outside `main`.

If any box is unchecked and you cannot check it, say so explicitly and
state why — do not silently ship a non-conformity.
