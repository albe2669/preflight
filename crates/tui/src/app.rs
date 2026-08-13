//! Application state: the active view, interaction mode, cursor, and the
//! cached data fetched from the backend.
//!
//! The TUI is a thin client: it fetches data, holds it here, and renders from
//! it. Mutations update the backend then trigger a refetch so this cache stays
//! authoritative.

use std::collections::HashSet;

use crate::gql::{DailyReview, PullRequest, SyncState, Tag, Todo, TodoEvent};

/// The five top-level views plus the detail overlay.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Today,
    Backlog,
    Inbox,
    Review,
    Sync,
}

impl View {
    pub const ALL: [Self; 5] = [
        Self::Today,
        Self::Backlog,
        Self::Inbox,
        Self::Review,
        Self::Sync,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Backlog => "Backlog",
            Self::Inbox => "Inbox",
            Self::Review => "Review",
            Self::Sync => "Sync",
        }
    }
}

/// Interaction mode. The TUI is modal: most of the time it's navigating; a
/// few modes capture input (inline edit, search, confirm).
///
/// `StatusSelect` drives a centered popup: `selection` is the 0-indexed
/// highlight among the five statuses; `reason` is `Some` only while the
/// blocked-reason sub-prompt is active (entered when the user commits
/// `blocked`). See design D1.
#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    Navigate,
    InlineCreate {
        input: String,
    },
    InlineEdit {
        id: i32,
        input: String,
    },
    Search {
        input: String,
    },
    Reorder {
        source_id: i32,
    },
    Confirm {
        action: ConfirmAction,
    },
    Help {
        filter: String,
    },
    StatusSelect {
        id: i32,
        selection: usize,
        reason: Option<String>,
    },
    SidebarEdit {
        id: i32,
        field: SidebarField,
        input_active: bool,
        title_input: String,
        title_caret: usize,
        desc_input: String,
        desc_caret: usize,
        desc_scroll: usize,
        tag_input: String,
        tag_caret: usize,
        link_kind: LinkKind,
        link_selection: usize,
        attaching: bool,
        scroll: usize,
    },
}

/// Which field is focused in the sidebar edit form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarField {
    Title,
    Description,
    Links,
    Tags,
}

/// Which type of link the sidebar edit form is currently browsing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    Pr,
    Linear,
}

/// Destructive actions that require confirmation per the design.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfirmAction {
    CarryOver { from: String, to: String },
    Unplan { id: i32 },
    Cancel { id: i32 },
    DismissPr { id: i32 },
}

/// A transient toast: success fades after 2.5s, errors are sticky.
#[derive(Clone, Debug)]
pub struct Toast {
    pub kind: ToastKind,
    pub message: String,
    /// Remaining display time; None means sticky.
    pub ttl: Option<std::time::Duration>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Error,
}

/// Flattened data fetched from the backend. Converted from the cynic
/// `FetchAll` QueryFragment when data arrives in the event loop.
#[derive(Clone, Debug, Default)]
pub struct AppData {
    pub todos: Vec<Todo>,
    pub plan: Vec<crate::gql::PlanRow>,
    pub pulls: Vec<PullRequest>,
    pub linears: Vec<crate::gql::LinearIssue>,
    pub sync: Vec<SyncState>,
}

impl AppData {
    /// Convert from the cynic `FetchAll` query result.
    pub fn from_fetch_all(f: crate::gql::FetchAll) -> Self {
        Self {
            todos: f.todo,
            plan: f.plan,
            pulls: f.pulls,
            linears: f.linears,
            sync: f.sync,
        }
    }
}

/// Global app state. Owned by the event loop, mutated by key handlers.
pub struct App {
    pub view: View,
    pub mode: Mode,
    pub data: AppData,
    pub review: Option<DailyReview>,
    pub review_date: chrono::NaiveDate,
    pub logical_date: chrono::NaiveDate,
    pub day_start_hour: i32,
    pub cursor: usize,
    pub marked: HashSet<i32>,
    pub toast: Option<Toast>,
    pub spinner: u8,
    pub show_dismissed: bool,
    pub show_done: bool,
    pub detail: Option<DetailData>,
    pub help_scroll: usize,
    pub detail_loaded_id: Option<i32>,
    pub content_width: u16,
}

/// Lazy-loaded data for the detail overlay.
#[derive(Clone, Debug, Default)]
pub struct DetailData {
    pub tags: Vec<Tag>,
    pub events: Vec<TodoEvent>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            view: View::Today,
            mode: Mode::Navigate,
            data: AppData::default(),
            review: None,
            review_date: chrono::Local::now().date_naive() - chrono::Duration::days(1),
            logical_date: chrono::Local::now().date_naive(),
            day_start_hour: 4,
            cursor: 0,
            marked: HashSet::new(),
            toast: None,
            spinner: 0,
            show_dismissed: false,
            show_done: false,
            detail: None,
            help_scroll: 0,
            detail_loaded_id: None,
            content_width: 0,
        }
    }
}
impl App {
    /// The on-screen (grouped) display order: rows sorted by status in the
    /// fixed [`STATUS_ORDER`], stable within a status so source order is
    /// preserved inside a group. Cursor navigation, render highlighting, and
    /// row actions all treat the cursor as an index into this order, which is
    /// why it must match how the grouped list walks its rows.
    fn display_order<'a>(&self, rows: Vec<&'a Todo>) -> Vec<&'a Todo> {
        let mut rows = rows;
        rows.sort_by_key(|t| status_index(&t.status).unwrap_or(0));
        rows
    }

    /// Todos on today's plan (active, not removed), in display (status-grouped)
    /// order.
    pub fn today_plan(&self) -> Vec<&Todo> {
        self.display_order(
            self.data
                .plan
                .iter()
                .filter(|p| p.removed_at.is_none())
                .filter_map(|p| p.todo.as_ref())
                .collect(),
        )
    }
    /// Unplanned rows (removed_at set) — the "what did I intend" history.
    pub fn today_unplanned(&self) -> Vec<&crate::gql::PlanRow> {
        self.data
            .plan
            .iter()
            .filter(|p| p.removed_at.is_some())
            .collect()
    }

    /// All todos NOT on today's plan, in display (status-grouped) order for
    /// Backlog.
    pub fn backlog(&self) -> Vec<&Todo> {
        let on_plan: HashSet<i32> = self
            .data
            .plan
            .iter()
            .filter(|p| p.removed_at.is_none())
            .filter_map(|p| p.todo.as_ref())
            .map(|t| t.id)
            .collect();
        self.display_order(
            self.data
                .todos
                .iter()
                .filter(|t| !on_plan.contains(&t.id))
                .collect(),
        )
    }

    /// Non-dismissed PRs.
    pub fn inbox_prs(&self) -> Vec<&PullRequest> {
        self.data
            .pulls
            .iter()
            .filter(|p| self.show_dismissed || p.dismissed_at.is_none())
            .collect()
    }

    pub fn inbox_linears(&self) -> Vec<&crate::gql::LinearIssue> {
        self.data
            .linears
            .iter()
            .filter(|l| self.show_dismissed || l.dismissed_at.is_none())
            .collect()
    }

    pub fn sync_by_source(&self, source: &str) -> Option<&SyncState> {
        self.data.sync.iter().find(|s| s.source == source)
    }

    pub fn set_toast(&mut self, kind: ToastKind, msg: impl Into<String>) {
        self.toast = Some(Toast {
            kind,
            message: msg.into(),
            ttl: if kind == ToastKind::Success {
                Some(std::time::Duration::from_millis(2500))
            } else {
                None
            },
        });
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.set_toast(ToastKind::Error, msg);
    }

    pub fn tick_spinner(&mut self) {
        self.spinner = self.spinner.wrapping_add(1);
        if let Some(t) = &mut self.toast {
            if let Some(ttl) = &mut t.ttl {
                // Decrement by the tick interval (≈100ms).
                if let Some(remaining) = ttl.checked_sub(std::time::Duration::from_millis(100)) {
                    if remaining.is_zero() {
                        self.toast = None;
                    } else {
                        *ttl = remaining;
                    }
                } else {
                    self.toast = None;
                }
            }
        }
    }

    /// Point the cursor at the first active (`todo`/`started`/`blocked`)
    /// row for the current list view, skipping leading `done`/`cancelled`
    /// rows, or 0 when every row is terminal. Only applies when the cursor
    /// is still at its unset default, so subsequent j/k navigation is
    /// untouched.
    pub fn clamp_cursor_to_active(&mut self) {
        if self.cursor != 0 {
            return;
        }
        let rows = match self.view {
            View::Today => self.today_plan(),
            View::Backlog => self.backlog(),
            _ => return,
        };
        self.cursor = rows
            .iter()
            .position(|t| matches!(t.status.as_str(), "todo" | "started" | "blocked"))
            .unwrap_or(0);
    }
}

/// Format a backend timestamp (e.g. "2026-08-05 14:31:00 +00:00") as a
/// relative time like "3m ago" or a short clock "14:31".
pub fn relative_time(ts: &str, now: chrono::DateTime<chrono::Utc>) -> String {
    let parsed = chrono::DateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S %z");
    let Ok(t) = parsed else { return String::new() };
    let dur = now.signed_duration_since(t.with_timezone(&chrono::Utc));
    let mins = dur.num_minutes();
    if mins < 1 {
        "just now".into()
    } else if mins < 60 {
        format!("{mins}m ago")
    } else if mins < 60 * 24 {
        format!("{}h ago", mins / 60)
    } else {
        format!("{}d ago", mins / 60 / 24)
    }
}

pub fn short_clock(ts: &str) -> String {
    let parsed = chrono::DateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S %z");
    let Ok(t) = parsed else { return String::new() };
    t.format("%H:%M").to_string()
}

/// The five todo statuses in the fixed display order used by the grouped
/// list and the status-selection popup: started, blocked, todo, done,
/// cancelled.
pub const STATUS_ORDER: [&str; 5] = ["started", "blocked", "todo", "done", "cancelled"];

/// Index of a status string in [`STATUS_ORDER`], or `None` if unknown.
pub fn status_index(status: &str) -> Option<usize> {
    STATUS_ORDER.iter().position(|s| *s == status)
}

/// Case-insensitive substring filter: a todo matches if its title or any
/// tag slug contains `filter`. An empty filter matches everything.
pub fn matches_filter(todo: &Todo, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let needle = filter.to_lowercase();
    if todo.title.to_lowercase().contains(&needle) {
        return true;
    }
    todo.tag
        .nodes
        .iter()
        .any(|t| t.slug.to_lowercase().contains(&needle))
}

/// Helper for the create-then-plan decision: a todo created from the Today
/// view should be planned for today; from Backlog it stays unplanned.
pub fn should_plan_after_create(view: View) -> bool {
    view == View::Today
}

#[cfg(test)]
pub(crate) mod tests {
    use std::time::Duration;

    use chrono::DateTime;

    use super::*;

    // -- fixture helpers --

    pub(crate) fn make_todo(id: i32, title: &str, status: &str) -> crate::gql::Todo {
        crate::gql::Todo {
            id,
            title: title.to_string(),
            description: None,
            status: status.to_string(),
            blocked_reason: None,
            sort_key: id,
            created_at: "2026-08-05 10:00:00 +00:00".to_string(),
            started_at: None,
            closed_at: None,
            tag: crate::gql::TagConnection { nodes: vec![] },
        }
    }

    pub(crate) fn make_todo_with_tags(
        id: i32,
        title: &str,
        status: &str,
        slugs: &[&str],
    ) -> crate::gql::Todo {
        let nodes = slugs
            .iter()
            .map(|s| crate::gql::Tag {
                slug: s.to_string(),
                name: s.to_string(),
            })
            .collect();
        crate::gql::Todo {
            id,
            title: title.to_string(),
            description: None,
            status: status.to_string(),
            blocked_reason: None,
            sort_key: id,
            created_at: "2026-08-05 10:00:00 +00:00".to_string(),
            started_at: None,
            closed_at: None,
            tag: crate::gql::TagConnection { nodes },
        }
    }

    pub(crate) fn make_plan_row(
        id: i32,
        position: i32,
        todo: crate::gql::Todo,
        removed_at: Option<&str>,
    ) -> crate::gql::PlanRow {
        crate::gql::PlanRow {
            id,
            position,
            carried_over: false,
            removed_at: removed_at.map(|s| s.to_string()),
            todo: Some(todo),
        }
    }

    pub(crate) fn make_plan_row_no_todo(
        id: i32,
        position: i32,
        removed_at: Option<&str>,
    ) -> crate::gql::PlanRow {
        crate::gql::PlanRow {
            id,
            position,
            carried_over: false,
            removed_at: removed_at.map(|s| s.to_string()),
            todo: None,
        }
    }

    pub(crate) fn make_pr(id: i32, dismissed_at: Option<&str>) -> crate::gql::PullRequest {
        crate::gql::PullRequest {
            id,
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            number: id,
            title: format!("PR #{id}"),
            url: "https://example.com".to_string(),
            author: None,
            state: "open".to_string(),
            review_requested: false,
            authored_by_me: false,
            dismissed_at: dismissed_at.map(|s| s.to_string()),
        }
    }

    pub(crate) fn make_linear(id: i32, dismissed_at: Option<&str>) -> crate::gql::LinearIssue {
        crate::gql::LinearIssue {
            id,
            identifier: format!("PROJ-{id}"),
            title: format!("Issue #{id}"),
            url: "https://example.com".to_string(),
            state_name: "Todo".to_string(),
            state_type: "triage".to_string(),
            priority: None,
            team_key: None,
            assignee_name: None,
            assigned_to_me: false,
            dismissed_at: dismissed_at.map(|s| s.to_string()),
        }
    }

    pub(crate) fn make_sync(source: &str) -> crate::gql::SyncState {
        crate::gql::SyncState {
            source: source.to_string(),
            cursor: None,
            last_synced_at: None,
            last_status: "ok".to_string(),
            last_error: None,
        }
    }

    fn now_utc() -> chrono::DateTime<chrono::Utc> {
        DateTime::parse_from_str("2026-08-05 14:31:00 +00:00", "%Y-%m-%d %H:%M:%S %z")
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    // -- View tests --

    #[test]
    fn test_view_label_returns_correct_strings() {
        assert_eq!(View::Today.label(), "Today");
        assert_eq!(View::Backlog.label(), "Backlog");
        assert_eq!(View::Inbox.label(), "Inbox");
        assert_eq!(View::Review.label(), "Review");
        assert_eq!(View::Sync.label(), "Sync");
    }

    #[test]
    fn test_view_all_has_five_elements_in_order() {
        assert_eq!(View::ALL.len(), 5);
        assert_eq!(View::ALL[0], View::Today);
        assert_eq!(View::ALL[1], View::Backlog);
        assert_eq!(View::ALL[2], View::Inbox);
        assert_eq!(View::ALL[3], View::Review);
        assert_eq!(View::ALL[4], View::Sync);
    }

    // -- relative_time tests --

    #[test]
    fn test_relative_time_just_now() {
        let now = now_utc();
        // 30 seconds ago
        let ts = "2026-08-05 14:30:30 +00:00";
        assert_eq!(relative_time(ts, now), "just now");
    }

    #[test]
    fn test_relative_time_minutes_ago() {
        let now = now_utc();
        let ts = "2026-08-05 14:28:00 +00:00"; // 3m ago
        assert_eq!(relative_time(ts, now), "3m ago");
    }

    #[test]
    fn test_relative_time_hours_ago() {
        let now = now_utc();
        let ts = "2026-08-05 12:31:00 +00:00"; // 2h ago
        assert_eq!(relative_time(ts, now), "2h ago");
    }

    #[test]
    fn test_relative_time_days_ago() {
        let now = now_utc();
        let ts = "2026-08-01 14:31:00 +00:00"; // 4 days ago
        assert_eq!(relative_time(ts, now), "4d ago");
    }

    #[test]
    fn test_relative_time_unparseable_returns_empty() {
        let now = now_utc();
        assert_eq!(relative_time("not a date", now), "");
        assert_eq!(relative_time("", now), "");
    }

    // -- short_clock tests --

    #[test]
    fn test_short_clock_valid() {
        let ts = "2026-08-05 14:31:00 +00:00";
        assert_eq!(short_clock(ts), "14:31");
    }

    #[test]
    fn test_short_clock_unparseable_returns_empty() {
        assert_eq!(short_clock("bad"), "");
        assert_eq!(short_clock(""), "");
    }

    #[test]
    fn test_short_clock_midnight() {
        let ts = "2026-08-05 00:00:00 +00:00";
        assert_eq!(short_clock(ts), "00:00");
    }

    #[test]
    fn test_short_clock_23_59() {
        let ts = "2026-08-05 23:59:00 +00:00";
        assert_eq!(short_clock(ts), "23:59");
    }

    // -- AppData::from_fetch_all --

    #[test]
    fn test_app_data_from_fetch_all() {
        use crate::gql::FetchAll;
        let todo = make_todo(1, "T1", "todo");
        let pr = make_pr(1, None);
        let sync = make_sync("github");
        let fetch = FetchAll {
            todo: vec![todo.clone()],
            plan: vec![],
            pulls: vec![pr.clone()],
            linears: vec![],
            sync: vec![sync.clone()],
        };
        let data = AppData::from_fetch_all(fetch);
        assert_eq!(data.todos.len(), 1);
        assert_eq!(data.todos[0].id, 1);
        assert_eq!(data.pulls.len(), 1);
        assert_eq!(data.sync.len(), 1);
        assert_eq!(data.sync[0].source, "github");
    }

    #[test]
    fn test_app_data_from_fetch_all_preserves_tags() {
        use crate::gql::FetchAll;
        let todo = make_todo_with_tags(7, "Tagged", "todo", &["rust", "tui"]);
        let fetch = FetchAll {
            todo: vec![todo],
            plan: vec![],
            pulls: vec![],
            linears: vec![],
            sync: vec![],
        };
        let data = AppData::from_fetch_all(fetch);
        assert_eq!(data.todos.len(), 1);
        assert_eq!(data.todos[0].tag.nodes.len(), 2);
        assert_eq!(data.todos[0].tag.nodes[0].slug, "rust");
        assert_eq!(data.todos[0].tag.nodes[1].slug, "tui");
    }

    // -- today_plan tests --

    #[test]
    fn test_today_plan_filters_removed_and_no_todo() {
        let mut app = App::default();
        let active_todo = make_todo(1, "Active", "todo");
        let removed_todo = make_todo(2, "Removed", "todo");
        let orphan_row = make_plan_row_no_todo(3, 1, None); // no todo

        app.data.plan = vec![
            make_plan_row(1, 1, active_todo, None), // active
            make_plan_row(2, 2, removed_todo, Some("2026-08-05")), // removed
            orphan_row,                             // no todo
        ];

        let plan = app.today_plan();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].id, 1);
    }

    #[test]
    fn test_today_plan_order_by_position() {
        let mut app = App::default();
        app.data.plan = vec![
            make_plan_row(2, 2, make_todo(2, "B", "todo"), None),
            make_plan_row(1, 1, make_todo(1, "A", "todo"), None),
        ];

        let plan = app.today_plan();
        assert_eq!(plan.len(), 2);
        // Plan rows are iterated in vec order, so position doesn't reorder — the
        // vec order is the position order
        assert_eq!(plan[0].id, 2);
        assert_eq!(plan[1].id, 1);
    }

    // -- today_unplanned tests --

    #[test]
    fn test_today_unplanned_only_removed() {
        let mut app = App::default();
        app.data.plan = vec![
            make_plan_row(1, 1, make_todo(1, "A", "todo"), None),
            make_plan_row(2, 2, make_todo(2, "B", "todo"), Some("2026-08-05")),
        ];

        let unplanned = app.today_unplanned();
        assert_eq!(unplanned.len(), 1);
        assert_eq!(unplanned[0].id, 2);
    }

    // -- backlog tests --

    #[test]
    fn test_backlog_excludes_planned_todos() {
        let mut app = App::default();
        let planned = make_todo(1, "Planned", "todo");
        let unplanned = make_todo(2, "Backlog item", "todo");

        app.data.todos = vec![planned.clone(), unplanned.clone()];
        app.data.plan = vec![make_plan_row(1, 1, planned, None)];

        let backlog = app.backlog();
        assert_eq!(backlog.len(), 1);
        assert_eq!(backlog[0].id, 2);
    }

    #[test]
    fn test_backlog_includes_removed_plan_todos() {
        let mut app = App::default();
        let todo = make_todo(1, "Was planned", "todo");
        app.data.todos = vec![todo.clone()];
        // removed_at set => not on active plan => should appear in backlog
        app.data.plan = vec![make_plan_row(1, 1, todo.clone(), Some("2026-08-05"))];

        let backlog = app.backlog();
        assert_eq!(backlog.len(), 1);
        assert_eq!(backlog[0].id, 1);
    }

    // -- inbox_prs tests --

    #[test]
    fn test_inbox_prs_filters_dismissed() {
        let mut app = App::default();
        app.data.pulls = vec![make_pr(1, None), make_pr(2, Some("2026-08-05"))];
        app.show_dismissed = false;

        let prs = app.inbox_prs();
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].id, 1);
    }

    #[test]
    fn test_inbox_prs_shows_dismissed_when_flagged() {
        let mut app = App::default();
        app.data.pulls = vec![make_pr(1, None), make_pr(2, Some("2026-08-05"))];
        app.show_dismissed = true;

        let prs = app.inbox_prs();
        assert_eq!(prs.len(), 2);
    }

    // -- inbox_linears tests --

    #[test]
    fn test_inbox_linears_filters_dismissed() {
        let mut app = App::default();
        app.data.linears = vec![make_linear(1, None), make_linear(2, Some("2026-08-05"))];
        app.show_dismissed = false;

        let linears = app.inbox_linears();
        assert_eq!(linears.len(), 1);
        assert_eq!(linears[0].id, 1);
    }

    #[test]
    fn test_inbox_linears_shows_dismissed_when_flagged() {
        let mut app = App::default();
        app.data.linears = vec![make_linear(1, None), make_linear(2, Some("2026-08-05"))];
        app.show_dismissed = true;

        let linears = app.inbox_linears();
        assert_eq!(linears.len(), 2);
    }

    // -- sync_by_source tests --

    #[test]
    fn test_sync_by_source_finds_existing() {
        let mut app = App::default();
        app.data.sync = vec![make_sync("github"), make_sync("linear")];

        assert!(app.sync_by_source("github").is_some());
        assert_eq!(app.sync_by_source("github").unwrap().source, "github");
    }

    #[test]
    fn test_sync_by_source_returns_none_missing() {
        let mut app = App::default();
        app.data.sync = vec![make_sync("github")];

        assert!(app.sync_by_source("nonexistent").is_none());
    }

    // -- set_toast / set_error tests --

    #[test]
    fn test_set_toast_success_has_ttl() {
        let mut app = App::default();
        app.set_toast(ToastKind::Success, "done");
        assert!(app.toast.is_some());
        let toast = app.toast.as_ref().unwrap();
        assert_eq!(toast.kind, ToastKind::Success);
        assert_eq!(toast.message, "done");
        assert_eq!(toast.ttl, Some(Duration::from_millis(2500)));
    }

    #[test]
    fn test_set_toast_error_is_sticky() {
        let mut app = App::default();
        app.set_toast(ToastKind::Error, "oops");
        assert!(app.toast.is_some());
        let toast = app.toast.as_ref().unwrap();
        assert_eq!(toast.kind, ToastKind::Error);
        assert_eq!(toast.message, "oops");
        assert!(
            toast.ttl.is_none(),
            "error toast TTL should be None (sticky)"
        );
    }

    #[test]
    fn test_set_error_sets_error_toast() {
        let mut app = App::default();
        app.set_error("something broke");
        assert!(app.toast.is_some());
        let toast = app.toast.as_ref().unwrap();
        assert_eq!(toast.kind, ToastKind::Error);
        assert_eq!(toast.message, "something broke");
        assert!(toast.ttl.is_none());
    }

    // -- tick_spinner tests --

    #[test]
    fn test_tick_spinner_increments_spinner() {
        let mut app = App {
            spinner: 0,
            ..Default::default()
        };
        app.tick_spinner();
        assert_eq!(app.spinner, 1);
    }

    #[test]
    fn test_tick_spinner_wraps_at_max() {
        let mut app = App {
            spinner: u8::MAX,
            ..Default::default()
        };
        app.tick_spinner();
        assert_eq!(app.spinner, 0);
    }

    #[test]
    fn test_tick_spinner_decrements_success_ttl() {
        let mut app = App {
            toast: Some(Toast {
                kind: ToastKind::Success,
                message: "ok".to_string(),
                ttl: Some(Duration::from_millis(200)),
            }),
            ..Default::default()
        };
        app.tick_spinner();
        let ttl = app.toast.as_ref().unwrap().ttl;
        assert_eq!(ttl, Some(Duration::from_millis(100)));
    }

    #[test]
    fn test_tick_spinner_clears_toast_when_ttl_expires() {
        let mut app = App {
            toast: Some(Toast {
                kind: ToastKind::Success,
                message: "ok".to_string(),
                ttl: Some(Duration::from_millis(50)),
            }),
            ..Default::default()
        };
        // 100ms tick > 50ms TTL => should clear
        app.tick_spinner();
        assert!(
            app.toast.is_none(),
            "toast should be cleared when TTL < tick interval"
        );
    }

    #[test]
    fn test_tick_spinner_does_not_clear_error_toast() {
        let mut app = App {
            toast: Some(Toast {
                kind: ToastKind::Error,
                message: "error".to_string(),
                ttl: None,
            }),
            ..Default::default()
        };
        app.tick_spinner();
        assert!(app.toast.is_some(), "error toast should remain (sticky)");
    }

    #[test]
    fn test_tick_spinner_2500ms_ttl_takes_25_ticks() {
        let mut app = App {
            toast: Some(Toast {
                kind: ToastKind::Success,
                message: "ok".to_string(),
                ttl: Some(Duration::from_millis(2500)),
            }),
            ..Default::default()
        };
        for _ in 0..25 {
            app.tick_spinner();
        }
        assert!(
            app.toast.is_none(),
            "toast with 2500ms TTL should expire after 25 ticks of 100ms"
        );
    }

    #[test]
    fn test_tick_spinner_2500ms_ttl_present_after_24_ticks() {
        let mut app = App {
            toast: Some(Toast {
                kind: ToastKind::Success,
                message: "ok".to_string(),
                ttl: Some(Duration::from_millis(2500)),
            }),
            ..Default::default()
        };
        for _ in 0..24 {
            app.tick_spinner();
        }
        assert!(
            app.toast.is_some(),
            "toast with 2500ms TTL should still exist after 24 ticks"
        );
        assert_eq!(
            app.toast.as_ref().unwrap().ttl,
            Some(Duration::from_millis(100)),
            "remaining TTL should be 100ms after 24 ticks"
        );
    }

    // -- Default App state tests --

    #[test]
    fn test_default_app_state() {
        let app = App::default();
        assert_eq!(app.view, View::Today);
        assert_eq!(app.mode, Mode::Navigate);
        assert_eq!(app.cursor, 0);
        assert_eq!(app.spinner, 0);
        assert!(!app.show_dismissed);
        assert!(!app.show_done);
        assert!(app.toast.is_none());
        assert!(app.data.todos.is_empty());
        assert!(app.data.plan.is_empty());
        assert!(app.data.pulls.is_empty());
        assert!(app.data.linears.is_empty());
        assert!(app.data.sync.is_empty());
    }

    // -- status_order / status_index tests --

    #[test]
    fn test_status_order_has_five_in_fixed_sequence() {
        assert_eq!(STATUS_ORDER.len(), 5);
        assert_eq!(
            STATUS_ORDER,
            ["started", "blocked", "todo", "done", "cancelled"]
        );
    }

    #[test]
    fn test_status_index_known() {
        assert_eq!(status_index("started"), Some(0));
        assert_eq!(status_index("blocked"), Some(1));
        assert_eq!(status_index("todo"), Some(2));
        assert_eq!(status_index("done"), Some(3));
        assert_eq!(status_index("cancelled"), Some(4));
    }

    #[test]
    fn test_status_index_unknown_returns_none() {
        assert!(status_index("unknown").is_none());
        assert!(status_index("").is_none());
    }

    // -- matches_filter tests --

    #[test]
    fn test_matches_filter_empty_matches_all() {
        let todo = make_todo(1, "Write docs", "todo");
        assert!(matches_filter(&todo, ""));
    }

    #[test]
    fn test_matches_filter_title_substring_case_insensitive() {
        let todo = make_todo(1, "Write Rust docs", "todo");
        assert!(matches_filter(&todo, "rust"));
        assert!(matches_filter(&todo, "RUST"));
        assert!(matches_filter(&todo, "write"));
        assert!(!matches_filter(&todo, "python"));
    }

    #[test]
    fn test_matches_filter_tag_slug() {
        let todo = make_todo_with_tags(1, "Task", "todo", &["backend", "urgent"]);
        assert!(matches_filter(&todo, "back"));
        assert!(matches_filter(&todo, "URGENT"));
        assert!(!matches_filter(&todo, "frontend"));
    }

    #[test]
    fn test_matches_filter_no_tags_falls_back_to_title() {
        let todo = make_todo(1, "Refactor", "todo");
        assert!(matches_filter(&todo, "refac"));
        assert!(!matches_filter(&todo, "meeting"));
    }

    // -- should_plan_after_create tests --

    #[test]
    fn test_should_plan_after_create_today() {
        assert!(should_plan_after_create(View::Today));
    }

    #[test]
    fn test_should_plan_after_create_backlog() {
        assert!(!should_plan_after_create(View::Backlog));
        assert!(!should_plan_after_create(View::Inbox));
        assert!(!should_plan_after_create(View::Review));
        assert!(!should_plan_after_create(View::Sync));
    }

    // -- clamp_cursor_to_active tests --

    #[test]
    fn test_clamp_cursor_lands_on_first_active_row() {
        let mut app = App {
            view: View::Today,
            data: crate::app::AppData {
                plan: vec![
                    make_plan_row(1, 0, make_todo(1, "done one", "done"), None),
                    make_plan_row(2, 1, make_todo(2, "started one", "started"), None),
                    make_plan_row(3, 2, make_todo(3, "todo one", "todo"), None),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        app.clamp_cursor_to_active();
        // today_plan() is display-ordered: [started, todo, done]. The first
        // active row (started) is index 0.
        assert_eq!(
            app.cursor, 0,
            "cursor should land on the first active (started) row"
        );
    }

    #[test]
    fn test_clamp_cursor_falls_back_to_zero_all_terminal() {
        let mut app = App {
            view: View::Today,
            data: crate::app::AppData {
                plan: vec![
                    make_plan_row(1, 0, make_todo(1, "done one", "done"), None),
                    make_plan_row(2, 1, make_todo(2, "cancelled one", "cancelled"), None),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        app.clamp_cursor_to_active();
        assert_eq!(app.cursor, 0);
    }

    #[test]
    fn test_clamp_cursor_uses_backlog_rows_in_backlog_view() {
        let mut app = App {
            view: View::Backlog,
            data: crate::app::AppData {
                // Backlog excludes todos that appear on today's plan.
                plan: vec![make_plan_row(1, 0, make_todo(1, "planned", "done"), None)],
                todos: vec![
                    make_todo(2, "done unplanned", "done"),
                    make_todo(3, "todo unplanned", "todo"),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        app.clamp_cursor_to_active();
        // backlog() is display-ordered: [todo, done]; the first active row
        // (todo) is index 0.
        assert_eq!(app.cursor, 0);
    }

    #[test]
    fn test_clamp_cursor_leaves_explicit_cursor_alone() {
        let mut app = App {
            view: View::Today,
            cursor: 3,
            data: crate::app::AppData {
                plan: vec![
                    make_plan_row(1, 0, make_todo(1, "done", "done"), None),
                    make_plan_row(2, 1, make_todo(2, "todo", "todo"), None),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        app.clamp_cursor_to_active();
        assert_eq!(app.cursor, 3);
    }

    #[test]
    fn test_clamp_cursor_is_noop_outside_list_views() {
        let mut app = App {
            view: View::Sync,
            data: crate::app::AppData {
                plan: vec![make_plan_row(1, 0, make_todo(1, "done", "done"), None)],
                ..Default::default()
            },
            ..Default::default()
        };
        app.clamp_cursor_to_active();
        assert_eq!(app.cursor, 0);
    }

    // -- display order (cursor navigation matches on-screen groups) --

    #[test]
    fn test_today_plan_is_display_ordered_by_status() {
        // Position order is [cancelled, started, todo]; display order groups
        // by status (started, blocked, todo, done, cancelled) so arrow
        // navigation moves through categories in on-screen order.
        let app = App {
            view: View::Today,
            data: crate::app::AppData {
                plan: vec![
                    make_plan_row(1, 0, make_todo(1, "canc", "cancelled"), None),
                    make_plan_row(2, 1, make_todo(2, "start", "started"), None),
                    make_plan_row(3, 2, make_todo(3, "tod", "todo"), None),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        let titles: Vec<&str> = app.today_plan().iter().map(|t| t.title.as_str()).collect();
        assert_eq!(
            titles,
            vec!["start", "tod", "canc"],
            "today_plan must be display-ordered (started, todo, cancelled)"
        );
    }

    #[test]
    fn test_backlog_is_display_ordered_by_status() {
        // Fetch order is [todo, started, cancelled]; display order is
        // [started, todo, cancelled].
        let app = App {
            view: View::Backlog,
            data: crate::app::AppData {
                plan: vec![],
                todos: vec![
                    make_todo(1, "tod", "todo"),
                    make_todo(2, "start", "started"),
                    make_todo(3, "canc", "cancelled"),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        let titles: Vec<&str> = app.backlog().iter().map(|t| t.title.as_str()).collect();
        assert_eq!(
            titles,
            vec!["start", "tod", "canc"],
            "backlog must be display-ordered (started, todo, cancelled)"
        );
    }
}
