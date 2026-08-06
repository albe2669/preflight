//! Application state: the active view, interaction mode, cursor, and the
//! cached data fetched from the backend.
//!
//! The TUI is a thin client: it fetches data, holds it here, and renders from
//! it. Mutations update the backend then trigger a refetch so this cache stays
//! authoritative.

use std::collections::HashSet;

use crate::gql::{DailyReview, FetchAll, PullRequest, SyncState, Tag, Todo, TodoEvent};

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
    InlineEdit { id: i64, input: String },
    Search { input: String },
    Reorder { source_id: i64 },
    Confirm { action: ConfirmAction },
    Detail { id: i64 },
}

/// Destructive actions that require confirmation per the design.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfirmAction {
    CarryOver { from: String, to: String },
    Unplan { id: i64 },
    Cancel { id: i64 },
    DismissPr { id: i64 },
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

/// Global app state. Owned by the event loop, mutated by key handlers.
pub struct App {
    pub view: View,
    pub mode: Mode,
    pub data: FetchAll,
    pub review: Option<DailyReview>,
    pub review_date: chrono::NaiveDate,
    pub logical_date: chrono::NaiveDate,
    pub day_start_hour: i64,
    pub cursor: usize,
    pub marked: HashSet<i64>,
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
            data: FetchAll::default(),
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
            .map(|p| &p.todo)
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
        let on_plan: HashSet<i64> = self
            .data
            .plan
            .iter()
            .filter(|p| p.removed_at.is_none())
            .map(|p| p.todo.id)
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
                    *ttl = remaining;
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
