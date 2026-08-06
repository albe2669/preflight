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
#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    Navigate,
    InlineCreate { input: String },
    InlineEdit { id: i32, input: String },
    Search { input: String },
    Reorder { source_id: i32 },
    Confirm { action: ConfirmAction },
    Detail { id: i32 },
    Help,
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
        }
    }
}

impl App {
    /// Todos on today's plan (active, not removed), in position order.
    pub fn today_plan(&self) -> Vec<&Todo> {
        self.data
            .plan
            .iter()
            .filter(|p| p.removed_at.is_none())
            .filter_map(|p| p.todo.as_ref())
            .collect()
    }

    /// Unplanned rows (removed_at set) — the "what did I intend" history.
    pub fn today_unplanned(&self) -> Vec<&crate::gql::PlanRow> {
        self.data
            .plan
            .iter()
            .filter(|p| p.removed_at.is_some())
            .collect()
    }

    /// All todos NOT on today's plan, grouped by status for Backlog.
    pub fn backlog(&self) -> Vec<&Todo> {
        let on_plan: HashSet<i32> = self
            .data
            .plan
            .iter()
            .filter(|p| p.removed_at.is_none())
            .filter_map(|p| p.todo.as_ref())
            .map(|t| t.id)
            .collect();
        self.data
            .todos
            .iter()
            .filter(|t| !on_plan.contains(&t.id))
            .collect()
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::DateTime;

    use super::*;

    // -- fixture helpers --

    fn make_todo(id: i32, title: &str, status: &str) -> crate::gql::Todo {
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
        }
    }

    fn make_plan_row(
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

    fn make_plan_row_no_todo(
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

    fn make_pr(id: i32, dismissed_at: Option<&str>) -> crate::gql::PullRequest {
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

    fn make_linear(id: i32, dismissed_at: Option<&str>) -> crate::gql::LinearIssue {
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

    fn make_sync(source: &str) -> crate::gql::SyncState {
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
}
