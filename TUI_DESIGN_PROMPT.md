# Visual design prompt: preflight TUI

You are designing the **visual design** of a terminal user interface (TUI) for
`preflight`, a Rust todo planner. **Design the visual only — do not implement.**
Produce a complete visual design document (the kind a frontend design product
would output): the aesthetic direction, color system, type/attribute scale,
layout, components, and a high-fidelity mock for every view. 

---

## 1. What preflight is

A todo planner built on one idea: **"doing today" is not a flag on a todo, it's
a row in a per-day table.** A new logical date simply has no rows yet — that is
the morning reset, for free. Yesterday's plan is preserved permanently. A
todo's status is orthogonal to whether it's on today's list.

Four product decisions shape what the UI must show. The visual design must make
all four legible at a glance:

1. **Today is a per-date plan, not a flag.** The Today view is an ordered list
   for the current *logical date*; a brand-new day starts empty by design.
2. **There is an append-only event log with a denormalized `logical_date`.**
   "What did I work on yesterday" is an index scan, not timestamp math. The
   Review view reads this; it must never look like a raw timestamp dump.
3. **A logical day starts at a configurable hour (default 04:00 local), not
   midnight.** Work at 02:00 counts as *yesterday*. "Today" in the header must
   be the logical date, and the design must make the day-start boundary
   obvious enough that a user is never confused why late-night work isn't on
   today's plan.
4. **PRs and Linear issues are inbox rows, not todos.** They live in their own
   tables and are linked or converted by an explicit action. There is a triage
   surface visually distinct from the todo list.

---

## 2. What the UI shows (ground truth for the design)

Design only from real data. Do not invent fields, statuses, or badges that
don't exist here. If you truly believe more fields need to exist, propose them, do not force them.

### A todo
- `title`, `description?`
- `status` → `todo | started | blocked | done | cancelled`
- `blocked_reason?` (present only when `blocked`)
- tags (a todo has zero or more `slug` tags, e.g. `review`)
- linked PRs (each with a relation: `reviews | implements | references`)
- linked Linear issues
- timestamps: `created_at`, `started_at?`, `closed_at?`

### A day-plan row ("doing today")
- `plan_date`, `todo_id`, `position` (order), `carried_over: bool`
- `removed_at?` — a soft-unplan; "what did I intend yesterday" stays honest even
  if pulled off the list at 10am.

### The event log (append-only)
- `kind` → `created, title_changed, description_changed, status_changed,
  blocked, unblocked, tag_added, tag_removed, linked_pr, linked_linear,
  planned, unplanned, carried_over` (this set grows — never assume it's
  closed)
- `actor` → `user | sync | system` (a background sync must never make an
  untouched todo look "worked on")
- `occurred_at` + `logical_date`

### Inbox — pull requests
- `owner/repo #number`, `title`, `url`, `author?`
- `state` → `open | closed | merged | draft`
- `review_requested`, `authored_by_me`
- `dismissed_at?`

### Inbox — Linear issues
- `identifier` (e.g. `ENG-123`), `title`, `url`
- `state_name`, `state_type`, `priority?`
- `team_key?`, `assignee_name?`, `assigned_to_me`
- `dismissed_at?`

### Sync state
- One row per source (`linear`, `github`); last-sync cursor. Tokens may be
  absent — the app boots without network via a no-op path, and the UI must show
  that state clearly.

### Operations the UI can trigger (name them, don't invent more)
create / edit / set-status / plan-today / unplan-today / reorder / carry-over /
add-tag / remove-tag / convert-PR-to-todo / link-PR / dismiss-PR /
convert-issue-to-todo / link-issue / sync-GitHub / sync-Linear / daily-review.
If the design needs an action not in this list, flag it as a gap rather than
inventing it.

---

## 3. The design task

Produce a visual design for the TUI. A TUI is a **cell grid**, not a
pixel canvas: the primitives are characters, ANSI colors (256 or truecolor),
and text attributes (bold, dim, reverse, underline, italic, strikethrough),
plus box-drawing borders and block glyphs for bars. Design within those
primitives. Cover every section below.

### 3.1 Aesthetic direction

State the visual mood in one paragraph and a small mood board (3–5 reference
words, e.g. "calm, focused, monochrome-with-a-single-accent, high-information-
density"). The planner is used at the start of the day and in short check-ins;
the design should feel quiet and fast, not gamified or noisy. Justify the
direction against that use.

### 3.2 Color system

- Define a palette: background, surface (panels), border, primary text,
  secondary/dim text, and a single accent for the focused/selected element.
- Define **semantic colors** for each `TodoStatus` and each PR `state` and
  Linear `state_type`. Provide both a truecolor value and a 256-color fallback
  for each, and name the 16-color fallback (so it degrades on basic terminals).
- Color is always a **secondary cue** — never the only signal. Pair every
  semantic color with a glyph/shape (see 3.4).
- Specify light-on-dark as the default; describe a dark-on-light variant only
  if it adds no complexity. The design must be legible in monochrome (no color)
  — call out which attributes carry meaning when color is gone.
- Address color-blind safety: status must be distinguishable without relying
  on red/green alone.

### 3.3 Type & attribute scale

A TUI has no font size, so hierarchy is built from attributes + spacing:
- Define a scale: how titles, section headers, list items, metadata,
  helper/placeholder text, and the status bar differ (bold, dim, reverse,
  underline, color, and spacing above/below).
- Define how a selected/focused row differs from an unselected one, and how a
  "current todo under cursor" differs from a merely selected one (if you make
  that distinction).
- Define emphasis for an inline edit field vs. read-only text.
- Keep the number of distinct treatments small and consistent.

### 3.4 Glyph system & iconography

- Pick a status glyph per `TodoStatus` (e.g. `○ ◐ ⛿ ✓ ✕` or `[ ] [~] [!] [x]
  [-]`) — choose glyphs that render in common monospace fonts (Nerd Font is
  optional; provide a non-Nerd fallback for every glyph).
- Pick glyphs/marks for: `carried_over`, `blocked_reason` present, linked PR,
  linked Linear issue, review-requested, assigned-to-me, dismissed.
- Pick a relation indicator for `reviews | implements | references`.
- Document the exact character(s) for each, the Nerd-Font variant (if any), and
  the plain-font fallback. Never rely on a glyph that only exists in one font.

### 3.5 Layout system

- Define the overall screen structure: is there a persistent header (app
  name + logical date + day-start hint), a persistent footer/status bar
  (contextual help, last error, sync state), and a main content area? Show the
  base frame as a wireframe that holds for every view.
- Define panel behavior: fixed columns vs. collapsible detail pane (e.g. a
  todo detail that slides in beside the list). Specify minimum terminal size
  (e.g. 80×24) and how the layout degrades: what is hidden first at 80 cols, at
  120, at >160? How does it behave at <80 (graceful minimum, no wrap chaos)?
- Define spacing rules: gutters between panels, padding inside panels, blank
  rows between list groups, column alignment for tabular data (ids,
  identifiers, states).
- Define borders: which panels get borders, corner style, whether the focused
  panel's border uses the accent.

### 3.6 Component library

Design each reusable component as a labeled wireframe with its states. Cover at
least:

- **Todo row** — all states (`todo/started/blocked/done/cancelled`), with and
  without tags, with and without linked PR/issue badges, with and without
  `blocked_reason`, `carried_over` on vs. off, selected vs. unselected, being
  inline-edited.
- **Tag chip** — how a tag slug renders inline on a row.
- **Link badge** — a PR badge (owner/repo#number + relation) and a Linear
  badge (identifier + relation), distinct enough to tell apart at a glance.
- **Inbox row** (PR variant and Linear variant) — state, review-requested /
  assigned-to-me flags, dismissed state.
- **List** — empty state, populated state, loading state, and a "day rolled
  over — today is now empty" state that reads as intentional, not broken.
- **Panel / card** — bordered container with an optional header and focus
  treatment.
- **Status bar / footer** — contextual key hints, current view, logical date,
  sync indicator, last error.
- **Header** — app name, logical date (with day-start-hour hint), and a clear
  signal of *which* day is being shown (today vs. a picked date in Review).
- **Command palette / prompt line** — if present, how it appears and how a text
  input looks in a TUI (inline prompt with a label, cursor, and placeholder).
- **Confirmation / toast** — for destructive actions (carry-over, dismiss,
  cancel) and for transient success/error.
- **Spinner / progress** — for in-flight `syncGithub` / `syncLinear`, including
  the no-op ("no token configured") state.

### 3.7 Views (high-fidelity mocks)

Produce a full-screen ASCII mock for each view at ~100×32, plus a short caption.
Every view must use the base frame from 3.5.

1. **Today** — the primary view. Ordered day-plan list for the logical date,
   with status glyphs, tags, link badges, `blocked_reason`, `carried_over`
   marks. Show an inline-create prompt and a selected row being edited. Show
   the empty-day state as a second mock.
2. **Backlog** — all/other todos with a search/filter affordance; same row
   component, grouped or sorted by status.
3. **Inbox** — triage surface, grouped by source (GitHub / Linear), showing
   review-requested PRs and assigned-to-me issues, with per-row convert / link /
   dismiss affordances and a sync indicator per group.
4. **Review** — `DailyReview` for a picked date: four lists — planned, touched,
   completed, carried over — with the date in the header. Make the event-log
   nature legible (it's a review of a day, not a live list).
5. **Todo detail** — a single todo: full description, status, blocked reason,
   tags, linked PRs (with relation) and issues, and the event timeline for that
   todo. Show the timeline as a vertical, time-ordered list of `kind` entries
   with `actor` distinguished (`user/sync/system`).
6. **Sync status** — last sync per source, cursor state, a manual trigger, and
   the no-token / no-op state.

### 3.8 Interaction visuals

- Selection: how the focused row/panel is highlighted (reverse, accent border,
  leading marker, or a combination) — consistent across all views.
- Inline edit: how a text field looks while typing (cursor, field background,
  placeholder, commit/cancel affordance).
- Drag/reorder affordance: how a movable todo row signals it can be reordered,
   and the visual during a reorder.
- Confirmation: how a destructive action asks for confirmation, and how the
   result (success / error) is shown transiently without leaving the view.
- Async feedback: how an in-flight sync is shown (spinner + dimmed trigger),
   and how completion or failure surfaces.

### 3.9 States & error display

- **Empty states** for each view: no todos, no plan rows (the intentional
   morning reset), no inbox items, no events for a review date, no token
   configured. Each empty state should have one line of helpful, calm copy.
- **Loading states** for async fetches (today's plan, inbox, review, sync).
- **Error states**: map sentinel errors to user-facing visual treatment —
   `NotFound`, `Conflict` (title exists), validation errors, `SyncError` /
   rate-limited / no-token. Never show raw `Debug` output; design a compact,
   dismissible error treatment in the status bar or toast.
- **Offline / no-token** state across sync-driven views.

### 3.10 Motion & transitions

A TUI can animate sparingly (spinner frames, a brief flash on a state change,
   a slide for a detail pane). Specify which transitions animate, the frame
   rate / step budget (keep it cheap), and which are instant. Animation is
   optional polish — the design must read correctly with it disabled.

---

## 4. Output format

Produce one visual design document with these sections, in order:

1. **Aesthetic direction** — mood + 3–5 reference words + one-paragraph
   justification.
2. **Color system** — palette table (truecolor, 256-color, 16-color fallback,
   monochrome substitute) + semantic colors for every status and state.
3. **Type & attribute scale** — the treatments for each text role.
4. **Glyph system** — table: name, glyph, Nerd-Font variant, plain-font
   fallback.
5. **Layout system** — base-frame wireframe, panel behavior, minimum size,
   degradation rules, spacing & border rules.
6. **Component library** — one labeled wireframe per component, each with its
   states.
7. **Views** — one full-screen mock per view (~100×32) + caption; include the
   notable sub-states (empty, editing, error) as smaller mocks.
8. **Interaction visuals** — selection, inline edit, reorder, confirmation,
   async.
9. **States & errors** — empty, loading, error, offline per view.
10. **Motion** — what animates, what stays instant.
11. **Gaps** — anything the design needed that isn't in §2 or §3's operation
    list.

Keep prose tight. Prefer tables and wireframes over paragraphs. Every status,
field, badge, and operation you depict must exist in §2; if you want one that
doesn't, list it in §11 instead of inventing it.

Remember: **visual design only.** No implementation, no code, no crate
architecture, no data-path decisions. The deliverable is a visual design a
builder can implement against.
