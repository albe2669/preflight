//! Shared widgets and the persistent frame chrome (header, tabs, status bar).
//!
//! The frame is the one thing that never moves between views: rows 1-2 are
//! chrome, row 3 a rule, rows 4-30 content, row 31 blank, row 32 the status
//! bar. Only the tab highlight, the context word, and the key hints change.

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::app::{App, View};
use crate::gql::Todo;
use crate::theme::{Glyph, Palette};

/// Render the full frame chrome: top border with title + date, tab bar,
/// and bottom status bar. Returns the inner content rect.
pub fn render_frame(f: &mut Frame, app: &App, area: Rect) -> Rect {
    let chunks = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1), // top border
        ratatui::layout::Constraint::Length(1), // tab bar
        ratatui::layout::Constraint::Length(1), // rule
        ratatui::layout::Constraint::Min(0),    // content
        ratatui::layout::Constraint::Length(1), // blank
        ratatui::layout::Constraint::Length(1), // status bar
    ])
    .split(area);

    render_top_border(f, app, chunks[0]);
    render_tab_bar(f, app, chunks[1]);
    render_rule(f, chunks[2]);
    render_status_bar(f, app, chunks[5]);
    chunks[3]
}

fn render_top_border(f: &mut Frame, app: &App, area: Rect) {
    let date = format!(
        "{} {} · day {:02}:{:02}",
        weekday(app.logical_date),
        app.logical_date.format("%Y-%m-%d"),
        app.day_start_hour,
        0
    );
    let title = " preflight ";
    let line = Line::from(vec![
        Span::styled("┌ ", Style::default().fg(Palette::BORDER)),
        Span::styled(
            title,
            Style::default()
                .fg(Palette::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "─".repeat(
                area.width
                    .saturating_sub(title.len() as u16 + date.len() as u16 + 4)
                    as usize,
            ),
            Style::default().fg(Palette::BORDER),
        ),
        Span::styled(format!(" {date} "), Style::default().fg(Palette::DIM)),
        Span::styled("┐", Style::default().fg(Palette::BORDER)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn render_tab_bar(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![Span::raw(" ")];
    for (i, v) in View::ALL.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        if *v == app.view {
            spans.push(Span::styled(
                format!(" {} ", v.label()),
                Style::default()
                    .bg(Palette::ACCENT)
                    .fg(Palette::ACCENT_TEXT)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {} ", v.label()),
                Style::default().fg(Palette::DIM),
            ));
        }
    }
    // Right-aligned context word.
    let ctx = context_word(app);
    let pad = area
        .width
        .saturating_sub(spans.iter().map(|s| s.width() as u16).sum::<u16>() + ctx.len() as u16 + 2);
    spans.push(Span::raw(" ".repeat(pad as usize)));
    spans.push(Span::styled(ctx, Style::default().fg(Palette::DIM)));
    spans.push(Span::raw(" "));

    let line = Line::from(spans);
    let para = Paragraph::new(line).style(Style::default().bg(Palette::BG));
    f.render_widget(para, area);
}

fn context_word(app: &App) -> String {
    match app.view {
        View::Today => {
            let n = app.today_plan().len();
            let done = app
                .today_plan()
                .iter()
                .filter(|t| t.status == "done")
                .count();
            format!("{n} planned · {done} done")
        }
        View::Backlog => format!(
            "{} todos · {} on today",
            app.backlog().len(),
            app.today_plan().len()
        ),
        View::Inbox => {
            let need = app
                .inbox_prs()
                .iter()
                .filter(|p| p.review_requested)
                .count()
                + app
                    .inbox_linears()
                    .iter()
                    .filter(|l| l.assigned_to_me)
                    .count();
            let total = app.inbox_prs().len() + app.inbox_linears().len();
            format!("{need} need you · {total} total")
        }
        View::Review => {
            let n = app
                .review
                .as_ref()
                .map(|r| r.planned.len() + r.touched.len())
                .unwrap_or(0);
            format!("h/l change day · {n} entries")
        }
        View::Sync => {
            let n = app.data.sync.len();
            format!("{n} sources configured")
        }
    }
}

fn render_rule(f: &mut Frame, area: Rect) {
    let line = Line::from(vec![
        Span::styled("├", Style::default().fg(Palette::BORDER)),
        Span::styled(
            "─".repeat(area.width.saturating_sub(2) as usize),
            Style::default().fg(Palette::BORDER),
        ),
        Span::styled("┤", Style::default().fg(Palette::BORDER)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let hints = status_hints(app);
    let sync = sync_indicator(app);
    let view_label = format!(" {} ", app.view.label());
    let hint_len = hints.chars().count() as u16;
    let sync_len = sync.chars().count() as u16;
    let label_len = view_label.chars().count() as u16;
    let pad = area
        .width
        .saturating_sub(hint_len + sync_len + label_len + 3);

    let mut spans = vec![Span::raw(" ")];
    for span in hints_spans(&hints) {
        spans.push(span);
    }
    spans.push(Span::raw(" ".repeat(pad as usize)));
    spans.push(Span::raw(sync));
    spans.push(Span::styled(view_label, Style::default().fg(Palette::DIM)));

    let line = Line::from(spans);
    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(Palette::BORDER).fg(Palette::TEXT)),
        area,
    );
}

fn status_hints(app: &App) -> String {
    match (&app.view, &app.mode) {
        (_, crate::app::Mode::InlineCreate { .. }) => "enter save · esc cancel · # tag".into(),
        (_, crate::app::Mode::InlineEdit { .. }) => "enter save · esc revert".into(),
        (_, crate::app::Mode::Search { .. }) => "type to filter · esc clears".into(),
        (_, crate::app::Mode::Reorder { .. }) => "J/K move · enter drop · esc abort".into(),
        (_, crate::app::Mode::Confirm { .. }) => "y confirm · n cancel".into(),
        (
            _,
            crate::app::Mode::StatusSelect {
                reason: Some(_), ..
            },
        ) => "type reason · enter commit · esc cancel".into(),
        (_, crate::app::Mode::StatusSelect { .. }) => {
            "j/k move  1-5 select  enter commit  esc cancel".into()
        }
        (
            _,
            crate::app::Mode::SidebarEdit {
                input_active: true, ..
            },
        ) => "enter save field · esc back to fields · # tag".into(),
        (_, crate::app::Mode::SidebarEdit { .. }) => {
            "tab/j next field · shift-tab/k prev · enter edit · esc close · ? keys".into()
        }
        (
            _,
            crate::app::Mode::SidebarAdd {
                input_active: true, ..
            },
        ) => "enter create · esc cancel · # tag".into(),
        (_, crate::app::Mode::SidebarAdd { .. }) => {
            "tab/j next field · shift-tab/k prev · enter edit · esc cancel · ? keys".into()
        }
        (_, crate::app::Mode::Help { .. }) => "type to filter · esc close".into(),
        (View::Today, _) => {
            "j/k move  SPC status  a add  e edit  enter detail  D done  ? keys".into()
        }
        (View::Backlog, _) => {
            "/ search  t plan today  SPC status  D done  enter detail  a add".into()
        }
        (View::Inbox, _) => {
            "C convert  L link  d dismiss  o open url  s sync group  D show dismissed".into()
        }
        (View::Review, _) => {
            "h/l prev/next day  g pick date  enter open todo  a actor filter".into()
        }
        (View::Sync, _) => "s sync selected  S sync all  esc back".into(),
    }
}

fn sync_indicator(app: &App) -> String {
    let any_syncing = app
        .data
        .sync
        .iter()
        .any(|s| s.last_status == "syncing" || s.last_status == "in_progress");
    if any_syncing {
        let frame = Glyph::SPINNER[(app.spinner / 2) as usize % Glyph::SPINNER.len()];
        return format!(" {frame} ");
    }
    // last good sync, "↻ 3m"
    if let Some(s) = app.data.sync.first() {
        if let Some(ref ts) = s.last_synced_at {
            let rel = crate::app::relative_time(ts, chrono::Utc::now());
            return format!(" {} ", rel);
        }
    }
    String::new()
}

fn hints_spans(hints: &str) -> Vec<Span<'_>> {
    // Bold the key tokens (single letters / pairs before a space+word).
    let mut spans = Vec::new();
    for token in hints.split("  ") {
        let mut parts = token.splitn(2, ' ');
        if let (Some(key), Some(label)) = (parts.next(), parts.next()) {
            spans.push(Span::styled(
                key.to_string(),
                Style::default()
                    .fg(Palette::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!(" {label}  "),
                Style::default().fg(Palette::TEXT),
            ));
        } else {
            spans.push(Span::raw(format!("{token}  ")));
        }
    }
    spans
}

fn weekday(d: chrono::NaiveDate) -> &'static str {
    match d.format("%u").to_string().as_str() {
        "1" => "Mon",
        "2" => "Tue",
        "3" => "Wed",
        "4" => "Thu",
        "5" => "Fri",
        "6" => "Sat",
        _ => "Sun",
    }
}

/// Render a section header (T2: UPPERCASE, dim, letter-spaced, 1 blank above).
pub fn section_header(title: &str) -> Line<'_> {
    Line::from(vec![Span::styled(
        title.to_string(),
        Style::default().fg(Palette::DIM),
    )])
    .alignment(Alignment::Left)
}

/// Build a list widget for todo rows with the given state.
pub fn todo_list<'a>(items: Vec<ListItem<'a>>, _state: &mut ListState) -> List<'a> {
    List::new(items)
        .style(Style::default().bg(Palette::BG).fg(Palette::TEXT))
        .highlight_style(Style::default().bg(Palette::ROW_HIGHLIGHT))
        .highlight_symbol("")
}

/// Format a todo row as a `Line` with status glyph + title + metadata.
pub fn todo_line(todo: &Todo, is_cursor: bool, position: Option<usize>, carried: bool) -> Line<'_> {
    let (glyph, color) = status_glyph(&todo.status);
    let marker = if is_cursor {
        Span::styled(
            Glyph::CURSOR.to_string(),
            Style::default().fg(Palette::ACCENT),
        )
    } else {
        Span::raw(" ")
    };

    let mut spans = vec![marker, Span::raw(" ")];

    if let Some(p) = position {
        spans.push(Span::raw(format!("{p:>2} ")));
    } else {
        spans.push(Span::raw("    "));
    }

    if carried {
        spans.push(Span::styled(
            Glyph::CARRIED.to_string(),
            Style::default().fg(Palette::DIM),
        ));
        spans.push(Span::raw(" "));
    } else {
        spans.push(Span::raw("  "));
    }

    spans.push(Span::styled(glyph.to_string(), Style::default().fg(color)));

    let title_style = match todo.status.as_str() {
        "started" => Style::default()
            .fg(Palette::TEXT)
            .add_modifier(Modifier::BOLD),
        "done" => Style::default().fg(Palette::DIM),
        "cancelled" => Style::default()
            .fg(Palette::CANCELLED)
            .add_modifier(Modifier::CROSSED_OUT),
        _ => Style::default().fg(Palette::TEXT),
    };
    spans.push(Span::raw(" "));
    spans.push(Span::styled(todo.title.clone(), title_style));

    // Blocked reason continuation line is rendered separately by the caller.
    Line::from(spans)
}

pub fn blocked_reason_line(reason: &str) -> Line<'_> {
    Line::from(vec![
        Span::raw("        "),
        Span::styled(
            Glyph::BLOCKED_REASON.to_string(),
            Style::default().fg(Palette::BLOCKED),
        ),
        Span::raw(" "),
        Span::styled(
            reason.to_string(),
            Style::default()
                .fg(Palette::BLOCKED)
                .add_modifier(Modifier::UNDERLINED),
        ),
    ])
}

pub fn status_glyph(status: &str) -> (char, ratatui::style::Color) {
    match status {
        "todo" => (Glyph::STATUS_TODO, Palette::TODO),
        "started" => (Glyph::STATUS_STARTED, Palette::STARTED),
        "blocked" => (Glyph::STATUS_BLOCKED, Palette::BLOCKED),
        "done" => (Glyph::STATUS_DONE, Palette::DONE),
        "cancelled" => (Glyph::STATUS_CANCELLED, Palette::CANCELLED),
        _ => ('?', Palette::DIM),
    }
}

pub fn pr_state_glyph(state: &str) -> (char, ratatui::style::Color) {
    match state {
        "open" => (Glyph::PR_OPEN, Palette::DONE),
        "draft" => (Glyph::PR_DRAFT, Palette::CANCELLED),
        "merged" => (Glyph::PR_MERGED, Palette::MERGED),
        "closed" => (Glyph::PR_CLOSED, Palette::BLOCKED),
        _ => ('?', Palette::DIM),
    }
}

pub fn linear_state_glyph(state_type: &str) -> (char, ratatui::style::Color) {
    match state_type {
        "triage" => ('?', Palette::ACCENT),
        "backlog" => ('·', Palette::CANCELLED),
        "unstarted" => (Glyph::STATUS_TODO, Palette::TODO),
        "started" => (Glyph::STATUS_STARTED, Palette::STARTED),
        "completed" => (Glyph::STATUS_DONE, Palette::DONE),
        "canceled" => (Glyph::STATUS_CANCELLED, Palette::CANCELLED),
        _ => ('?', Palette::DIM),
    }
}

/// A bordered panel with an optional accent border when focused.
pub fn panel<'a>(title: Option<&str>, focused: bool) -> Block<'a> {
    let border_color = if focused {
        Palette::ACCENT
    } else {
        Palette::BORDER
    };
    let mut b = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    if let Some(t) = title {
        b = b.title(Span::styled(
            format!(" {} ", t),
            Style::default().fg(border_color),
        ));
    }
    b
}

pub fn render_todo_picker(f: &mut Frame, app: &crate::app::App, area: Rect) {
    let crate::app::Mode::LinkTodo { selection, .. } = &app.mode else {
        return;
    };
    let selection = *selection;

    let width = 46u16.min(area.width);
    let height = 12u16.min(area.height);
    let x = area.x + (area.width - width) / 2;
    let y = area.y + (area.height - height) / 2;
    let rect = Rect::new(x, y, width, height);

    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Palette::ACCENT))
        .title(Span::styled(
            " LINK TO TODO ",
            Style::default().fg(Palette::ACCENT),
        ));
    f.render_widget(block, rect);

    let inner = Rect::new(x + 1, y + 1, width - 2, height - 2);
    let mut items: Vec<ListItem> = Vec::new();
    for (i, todo) in app.data.todos.iter().enumerate() {
        let (glyph, color) = status_glyph(&todo.status);
        let is_current = i == selection;
        let marker = if is_current {
            Span::styled(
                Glyph::CURSOR.to_string(),
                Style::default().fg(Palette::ACCENT),
            )
        } else {
            Span::raw(" ")
        };
        let line = Line::from(vec![
            marker,
            Span::raw(" "),
            Span::styled(glyph.to_string(), Style::default().fg(color)),
            Span::raw(" "),
            Span::styled(
                todo.title.clone(),
                if is_current {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
        ]);
        items.push(ListItem::new(line));
    }

    let list = List::new(items).style(Style::default().bg(Palette::BG).fg(Palette::TEXT));
    let mut state = ListState::default();
    state.select(Some(selection));
    f.render_stateful_widget(list, inner, &mut state);
}

// ---- Shared grouped-by-state list renderer (D3) ----

use crate::app::STATUS_ORDER;

/// A group of rows under a state header, ready to render.
#[derive(Clone, Debug)]
pub struct GroupedSection<'a> {
    pub label: &'static str,
    pub status: &'static str,
    pub rows: Vec<&'a Todo>,
    pub collapsed: bool,
}
/// Group todos by status in the fixed display order (started, blocked, todo,
/// done, cancelled). Empty groups are elided, except `done` which is kept as
/// a collapsed header when `show_done` is false (the done-toggle).
pub fn group_by_status<'a>(rows: &'a [&'a Todo], show_done: bool) -> Vec<GroupedSection<'a>> {
    let mut groups: Vec<GroupedSection<'a>> = Vec::new();
    for &status in STATUS_ORDER.iter() {
        let matching: Vec<&Todo> = rows
            .iter()
            .copied()
            .filter(|t| t.status == status)
            .collect();
        if matching.is_empty() {
            continue;
        }
        let label = match status {
            "started" => "STARTED",
            "blocked" => "BLOCKED",
            "todo" => "TODO",
            "done" => "DONE",
            "cancelled" => "CANCELLED",
            _ => status,
        };
        groups.push(GroupedSection {
            label,
            status,
            rows: matching,
            collapsed: false,
        });
    }
    // Done-toggle: when show_done is false, keep the done group but mark it
    // collapsed so the renderer shows only the header with the count.
    if !show_done {
        for g in groups.iter_mut() {
            if g.status == "done" {
                g.collapsed = true;
            }
        }
    }
    groups
}

/// Render the grouped list into `area` as a stateful `List` widget.
///
/// `cursor_todo` is the todo the cursor points at in the *source-ordered*
/// list (the same order the caller built `groups` from). Groups reorder rows
/// by status, so the highlight must be located by todo identity, never by
/// positional index. The inline-create prompt (if any) and per-view hints are
/// appended by the caller via `trailing_items`.
pub fn render_grouped_list<'a>(
    f: &mut Frame,
    area: Rect,
    groups: &[GroupedSection<'a>],
    cursor_todo: Option<&Todo>,
    carried_ids: &std::collections::HashSet<i32>,
    trailing_items: Vec<ListItem<'a>>,
) -> ListState {
    let cursor_id = cursor_todo.map(|t| t.id);

    // Build the grouped items and, in the same pass, record the flat List
    // index that carries the cursor's todo (matched by id).
    let mut items: Vec<ListItem<'a>> = Vec::new();
    let mut selected_flat: Option<usize> = None;
    let mut flat: usize = 0;

    for group in groups {
        let collapsed = group.collapsed;
        let header = if collapsed {
            format!(
                "  {} · {} (collapsed — press D to expand)",
                group.label,
                group.rows.len()
            )
        } else {
            format!("  {} · {}", group.label, group.rows.len())
        };
        items.push(ListItem::new(Line::from(Span::styled(
            header,
            Style::default().fg(Palette::DIM),
        ))));
        flat += 1;

        if collapsed {
            continue;
        }

        for todo in group.rows.iter() {
            let is_cursor = cursor_id == Some(todo.id);
            if is_cursor {
                selected_flat = Some(flat);
            }
            let line = grouped_todo_line(todo, is_cursor, carried_ids.contains(&todo.id));
            if is_cursor {
                items.push(ListItem::new(line).style(Style::default().bg(Palette::ROW_HIGHLIGHT)));
            } else {
                items.push(ListItem::new(line));
            }
            flat += 1;
            // Blocked reason continuation line.
            if todo.status == "blocked" {
                if let Some(reason) = &todo.blocked_reason {
                    items.push(ListItem::new(blocked_reason_line(reason)));
                    flat += 1;
                }
            }
        }
        items.push(ListItem::new(Line::from("")));
        flat += 1;
    }

    items.extend(trailing_items);

    let list = List::new(items)
        .style(Style::default().bg(Palette::BG).fg(Palette::TEXT))
        .highlight_style(Style::default().bg(Palette::ROW_HIGHLIGHT));

    let mut state = ListState::default();
    state.select(selected_flat);
    f.render_stateful_widget(list, area, &mut state);
    state
}

/// Format a todo row for the grouped layout (no position column).
fn grouped_todo_line(todo: &Todo, is_cursor: bool, carried: bool) -> Line<'_> {
    let (glyph, color) = status_glyph(&todo.status);
    let marker = if is_cursor {
        Span::styled(
            Glyph::CURSOR.to_string(),
            Style::default().fg(Palette::ACCENT),
        )
    } else {
        Span::raw(" ")
    };
    let title_style = match todo.status.as_str() {
        "started" => Style::default()
            .fg(Palette::TEXT)
            .add_modifier(Modifier::BOLD),
        "done" => Style::default().fg(Palette::DIM),
        "cancelled" => Style::default()
            .fg(Palette::CANCELLED)
            .add_modifier(Modifier::CROSSED_OUT),
        _ => Style::default().fg(Palette::TEXT),
    };
    let mut spans = vec![
        marker,
        Span::raw("  "),
        Span::styled(glyph.to_string(), Style::default().fg(color)),
        Span::raw(" "),
    ];
    if carried {
        spans.push(Span::styled(
            Glyph::CARRIED.to_string(),
            Style::default().fg(Palette::DIM),
        ));
        spans.push(Span::raw(" "));
    }
    spans.push(Span::styled(todo.title.clone(), title_style));
    Line::from(spans)
}

/// Render the info sidebar for the highlighted todo (D4).
/// In sidebar edit mode, renders the edit form.
/// Otherwise, renders the read-only info card.
pub fn render_info_sidebar(f: &mut Frame, app: &crate::app::App, area: Rect) {
    // Check if we are in sidebar edit mode
    if let crate::app::Mode::SidebarEdit { .. } = app.mode {
        render_sidebar_edit_form(f, app, area);
        return;
    }
    if let crate::app::Mode::SidebarAdd { .. } = app.mode {
        render_sidebar_add_form(f, app, area);
        return;
    }

    // Read-only info card
    let todo = sidebar_todo(app);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Palette::BORDER))
        .title(Span::styled(" TODO ", Style::default().fg(Palette::DIM)));
    f.render_widget(block, area);

    let inner = Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    if inner.height == 0 {
        return;
    }

    let Some(todo) = todo else {
        let line = Line::from(Span::styled(
            "no todo highlighted",
            Style::default().fg(Palette::GHOST),
        ));
        f.render_widget(Paragraph::new(line), inner);
        return;
    };

    let (glyph, color) = status_glyph(&todo.status);
    let mut lines: Vec<Line> = Vec::new();

    // Status line.
    lines.push(Line::from(vec![
        Span::styled(glyph.to_string(), Style::default().fg(color)),
        Span::raw(" "),
        Span::styled(
            todo.status.clone(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ]));

    // Title.
    lines.push(Line::from(Span::styled(
        todo.title.clone(),
        Style::default().fg(Palette::TEXT),
    )));
    // Description (also shown in the edit form; keep the read-only detail in
    // sync so a todo's description is visible without entering edit mode).
    if let Some(desc) = &todo.description {
        if !desc.trim().is_empty() {
            for dl in desc.lines() {
                lines.push(Line::from(Span::styled(
                    format!("  {dl}"),
                    Style::default().fg(Palette::TEXT),
                )));
            }
            lines.push(Line::from(""));
        }
    }

    // Tags.
    if !todo.tag.nodes.is_empty() {
        let tag_strs: Vec<String> = todo
            .tag
            .nodes
            .iter()
            .map(|t| format!("#{}", t.slug))
            .collect();
        lines.push(Line::from(Span::styled(
            format!("tags  {}", tag_strs.join(" ")),
            Style::default().fg(Palette::DIM),
        )));
    }

    // Timestamps.
    let created = todo.created_at.split(' ').next().unwrap_or("");
    lines.push(Line::from(Span::styled(
        format!("created  {created}"),
        Style::default().fg(Palette::DIM),
    )));
    if let Some(started) = &todo.started_at {
        let s = started.split(' ').next().unwrap_or("");
        lines.push(Line::from(Span::styled(
            format!("started  {s}"),
            Style::default().fg(Palette::DIM),
        )));
    }
    if let Some(closed) = &todo.closed_at {
        let c = closed.split(' ').next().unwrap_or("");
        lines.push(Line::from(Span::styled(
            format!("closed   {c}"),
            Style::default().fg(Palette::DIM),
        )));
    }

    // Blocked reason.
    if todo.status == "blocked" {
        if let Some(reason) = &todo.blocked_reason {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(
                    Glyph::BLOCKED_REASON.to_string(),
                    Style::default().fg(Palette::BLOCKED),
                ),
                Span::raw(" "),
                Span::styled(reason.clone(), Style::default().fg(Palette::BLOCKED)),
            ]));
        }
    }

    // LINKS region from app.detail (read mode only; edit mode renders its own).
    if !matches!(app.mode, crate::app::Mode::SidebarEdit { .. }) {
        let loaded = app.detail_loaded_id;
        let detail = app.detail.as_ref();
        if loaded.is_some() && todo.id == loaded.unwrap() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "LINKS",
                Style::default()
                    .fg(Palette::ACCENT)
                    .add_modifier(Modifier::BOLD),
            )));
            if let Some(d) = detail {
                let has_links = !d.prs.is_empty() || !d.linears.is_empty();
                if !has_links {
                    lines.push(Line::from(Span::styled(
                        "  No linked PRs or issues",
                        Style::default().fg(Palette::GHOST),
                    )));
                }
                for pr in &d.prs {
                    if let Some(p) = &pr.pull_request {
                        let label = format!(
                            "  {} {}/{}#{} — {}",
                            pr.relation, p.owner, p.repo, p.number, p.title,
                        );
                        lines.push(Line::from(Span::styled(
                            label,
                            Style::default().fg(Palette::TEXT),
                        )));
                    }
                }
                for li in &d.linears {
                    if let Some(l) = &li.linear_issue {
                        let label = format!("  {} — {}", l.identifier, l.title);
                        lines.push(Line::from(Span::styled(
                            label,
                            Style::default().fg(Palette::TEXT),
                        )));
                    }
                }
            } else {
                lines.push(Line::from(Span::styled(
                    "  loading…",
                    Style::default().fg(Palette::GHOST),
                )));
            }
        }
    }

    // EVENT LOG region (read mode only): append-only timeline, most recent first.
    if !matches!(app.mode, crate::app::Mode::SidebarEdit { .. }) {
        let loaded = app.detail_loaded_id;
        let detail = app.detail.as_ref();
        if loaded.is_some() && todo.id == loaded.unwrap() {
            if let Some(d) = detail {
                if !d.events.is_empty() {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "EVENT LOG",
                        Style::default()
                            .fg(Palette::ACCENT)
                            .add_modifier(Modifier::BOLD),
                    )));
                    for ev in d.events.iter().rev() {
                        let parts: Vec<String> = vec![ev.kind.clone()]
                            .into_iter()
                            .chain(ev.field.clone())
                            .chain(ev.old_value.clone().map(|v| format!("\"{v}\"")))
                            .chain(ev.new_value.clone().map(|v| format!("-> \"{v}\"")))
                            .collect();
                        lines.push(Line::from(Span::styled(
                            format!("  {} · {}", parts.join(" "), ev.actor),
                            Style::default().fg(Palette::DIM),
                        )));
                    }
                }
            }
        }
    }

    f.render_widget(
        Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false }),
        inner,
    );
}
/// Render `text` with a block-cursor caret at byte index `caret`. Used by the
/// sidebar edit form so the user can see (and move) the insertion point.
fn text_with_caret(text: &str, caret: usize) -> String {
    let mut caret = caret.min(text.len());
    // Clamp to a char boundary so split_at never panics if a caret drifts to
    // a mid-char byte offset.
    while caret > 0 && !text.is_char_boundary(caret) {
        caret -= 1;
    }
    let (head, tail) = text.split_at(caret);
    let mut out = String::with_capacity(text.len() + 4);
    out.push_str(head);
    out.push(Glyph::CURSOR);
    out.push_str(tail);
    out
}

/// Build the header span for a sidebar form field. When the field is focused
/// the header is accent + bold (with a `▸` marker when input is active);
/// otherwise it is dim.
fn field_header(
    name: &str,
    field: crate::app::SidebarField,
    current: crate::app::SidebarField,
    input_active: bool,
) -> Span<'static> {
    if field == current {
        let prefix = if input_active { "▸ " } else { "  " };
        Span::styled(
            format!("{prefix}{name}"),
            Style::default()
                .fg(Palette::ACCENT)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(format!("  {name}"), Style::default().fg(Palette::DIM))
    }
}

/// True for PR states that are finished (closed or merged); open and draft
/// stay active.
fn pr_is_closed(state: &str) -> bool {
    matches!(state, "closed" | "merged")
}

/// True for Linear state types that are finished (completed or canceled).
fn linear_is_done(state_type: &str) -> bool {
    matches!(state_type, "completed" | "canceled")
}

/// Case-insensitive substring test against a PR's number (as text) or title.
fn pr_matches_search(pr: &crate::gql::PullRequest, search: &str) -> bool {
    if search.is_empty() {
        return true;
    }
    let s = search.to_ascii_lowercase();
    pr.title.to_ascii_lowercase().contains(&s)
        || format!("#{}", pr.number).contains(&s)
        || format!("{}", pr.number).contains(&s)
}

/// Case-insensitive substring test against a Linear issue's identifier or title.
fn linear_matches_search(li: &crate::gql::LinearIssue, search: &str) -> bool {
    if search.is_empty() {
        return true;
    }
    let s = search.to_ascii_lowercase();
    li.title.to_ascii_lowercase().contains(&s) || li.identifier.to_ascii_lowercase().contains(&s)
}

/// Filtered + sorted PR candidate indices for the link attach picker.
/// Open PRs come first, closed/merged after; ties keep source order.
pub(crate) fn pr_picker_candidates(app: &crate::app::App, search: &str) -> Vec<usize> {
    let mut idxs: Vec<usize> = (0..app.data.pulls.len())
        .filter(|&i| pr_matches_search(&app.data.pulls[i], search))
        .collect();
    idxs.sort_by_key(|&i| pr_is_closed(&app.data.pulls[i].state));
    idxs
}

/// Filtered + sorted Linear candidate indices for the link attach picker.
/// Active issues come first, completed/canceled after.
pub(crate) fn linear_picker_candidates(app: &crate::app::App, search: &str) -> Vec<usize> {
    let mut idxs: Vec<usize> = (0..app.data.linears.len())
        .filter(|&i| linear_matches_search(&app.data.linears[i], search))
        .collect();
    idxs.sort_by_key(|&i| linear_is_done(&app.data.linears[i].state_type));
    idxs
}

fn render_sidebar_edit_form(f: &mut Frame, app: &crate::app::App, area: Rect) {
    let crate::app::Mode::SidebarEdit {
        id,
        field,
        input_active,
        title_input,
        title_caret,
        desc_input,
        desc_caret,
        tag_input,
        tag_caret,
        link_kind,
        link_selection,
        attaching,
        link_search,
        scroll,
        ..
    } = &app.mode
    else {
        return;
    };

    let id = *id;
    let field = *field;
    let input_active = *input_active;
    let link_kind = *link_kind;
    let link_selection = *link_selection;
    let attaching = *attaching;
    let link_search: &str = link_search;
    let scroll = *scroll;
    let title_caret = *title_caret;
    let desc_caret = *desc_caret;
    let tag_caret = *tag_caret;
    let title_input: &str = title_input;
    let desc_input: &str = desc_input;
    let tag_input: &str = tag_input;

    let todo = app.data.todos.iter().find(|t| t.id == id);
    let Some(todo) = todo else {
        let line = Line::from(Span::styled(
            "todo not found",
            Style::default().fg(Palette::GHOST),
        ));
        f.render_widget(Paragraph::new(line), area);
        return;
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Palette::ACCENT))
        .title(Span::styled(" EDIT ", Style::default().fg(Palette::ACCENT)));
    f.render_widget(block, area);

    let inner = Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    if inner.height == 0 {
        return;
    }

    let (glyph, color) = status_glyph(&todo.status);
    let mut lines: Vec<Line> = Vec::new();

    // Title (read-only, always visible)
    lines.push(Line::from(vec![
        Span::styled(glyph.to_string(), Style::default().fg(color)),
        Span::raw(" "),
        Span::styled(todo.title.clone(), Style::default().fg(Palette::TEXT)),
    ]));
    lines.push(Line::from(""));

    // TITLE (editable form field)
    lines.push(Line::from(field_header(
        "TITLE",
        crate::app::SidebarField::Title,
        field,
        input_active,
    )));
    if input_active && field == crate::app::SidebarField::Title {
        lines.push(Line::from(Span::styled(
            text_with_caret(title_input, title_caret),
            Style::default().fg(Palette::TEXT),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            if title_input.is_empty() {
                "—"
            } else {
                title_input
            },
            Style::default().fg(Palette::DIM),
        )));
    }
    // DESCRIPTION
    lines.push(Line::from(field_header(
        "DESCRIPTION",
        crate::app::SidebarField::Description,
        field,
        input_active,
    )));
    if input_active && field == crate::app::SidebarField::Description {
        lines.push(Line::from(Span::styled(
            text_with_caret(desc_input, desc_caret),
            Style::default().fg(Palette::TEXT),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            if desc_input.is_empty() {
                "—"
            } else {
                desc_input
            },
            Style::default().fg(Palette::DIM),
        )));
    }
    lines.push(Line::from(""));

    // LINKS
    lines.push(Line::from(field_header(
        "LINKS",
        crate::app::SidebarField::Links,
        field,
        input_active,
    )));
    let detail = app.detail.as_ref();
    let has_links = detail
        .map(|d| !d.prs.is_empty() || !d.linears.is_empty())
        .unwrap_or(false);
    if !has_links && !input_active {
        lines.push(Line::from(Span::styled(
            "  No linked PRs or issues",
            Style::default().fg(Palette::GHOST),
        )));
    }
    // Show currently linked PRs and Linear issues (when not in the attach picker).
    if !input_active || field != crate::app::SidebarField::Links || !attaching {
        if let Some(d) = detail {
            for pr in &d.prs {
                if let Some(p) = &pr.pull_request {
                    let label = format!(
                        "  {} {}/{}#{} {}",
                        pr.relation, p.owner, p.repo, p.number, p.title,
                    );
                    lines.push(Line::from(Span::styled(
                        label,
                        Style::default().fg(Palette::TEXT),
                    )));
                }
            }
            for li in &d.linears {
                if let Some(l) = &li.linear_issue {
                    let label = format!("  {} {}", l.identifier, l.title);
                    lines.push(Line::from(Span::styled(
                        label,
                        Style::default().fg(Palette::TEXT),
                    )));
                }
            }
        }
    }
    if input_active && field == crate::app::SidebarField::Links {
        if attaching {
            // Attach picker: list synced candidates for the current kind,
            // filtered by `link_search` with open items before closed.
            let kind_str = match link_kind {
                crate::app::LinkKind::Pr => "PRs",
                crate::app::LinkKind::Linear => "Issues",
            };
            lines.push(Line::from(Span::styled(
                format!(
                    "  attach {kind_str} (tab: kind · j/k: move · enter: attach · esc: cancel)"
                ),
                Style::default().fg(Palette::DIM),
            )));
            // Search input line.
            lines.push(Line::from(Span::styled(
                format!("  search: {}", text_with_caret(link_search, 0)),
                Style::default().fg(Palette::DIM),
            )));
            let candidates: Vec<usize> = if link_kind == crate::app::LinkKind::Pr {
                pr_picker_candidates(app, link_search)
            } else {
                linear_picker_candidates(app, link_search)
            };
            if candidates.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  no candidates",
                    Style::default().fg(Palette::GHOST),
                )));
            } else {
                let sel = link_selection.min(candidates.len() - 1);
                for (vis, &data_idx) in candidates.iter().enumerate() {
                    let (selected, label) = if link_kind == crate::app::LinkKind::Pr {
                        let row = &app.data.pulls[data_idx];
                        (vis == sel, format!("PR #{} {}", row.number, row.title))
                    } else {
                        let row = &app.data.linears[data_idx];
                        (vis == sel, format!("{} {}", row.identifier, row.title))
                    };
                    if selected {
                        lines.push(Line::from(vec![
                            Span::styled(
                                Glyph::CURSOR.to_string(),
                                Style::default().fg(Palette::ACCENT),
                            ),
                            Span::raw(" "),
                            Span::styled(
                                label,
                                Style::default()
                                    .fg(Palette::ACCENT)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    } else {
                        lines.push(Line::from(Span::styled(
                            format!("  {label}"),
                            Style::default().fg(Palette::TEXT),
                        )));
                    }
                }
            }
        } else {
            let kind_str = match link_kind {
                crate::app::LinkKind::Pr => "PRs",
                crate::app::LinkKind::Linear => "Issues",
            };
            lines.push(Line::from(Span::styled(
                format!("  [{kind_str}] selection: {link_selection}",),
                Style::default().fg(Palette::DIM),
            )));
        }
    }

    // TAGS
    lines.push(Line::from(field_header(
        "TAGS",
        crate::app::SidebarField::Tags,
        field,
        input_active,
    )));
    let tag_strs: Vec<String> = todo
        .tag
        .nodes
        .iter()
        .map(|t| format!("#{}", t.slug))
        .collect();
    if input_active && field == crate::app::SidebarField::Tags {
        lines.push(Line::from(Span::styled(
            format!(
                "{} {}",
                tag_strs.join(" "),
                text_with_caret(tag_input, tag_caret)
            ),
            Style::default().fg(Palette::TEXT),
        )));
    } else if tag_strs.is_empty() {
        lines.push(Line::from(Span::styled(
            "  none",
            Style::default().fg(Palette::GHOST),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!("  {}", tag_strs.join(" ")),
            Style::default().fg(Palette::DIM),
        )));
    }

    // EVENT LOG: append-only timeline, most recent first.
    if let Some(d) = app.detail.as_ref() {
        if !d.events.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "EVENT LOG",
                Style::default()
                    .fg(Palette::ACCENT)
                    .add_modifier(Modifier::BOLD),
            )));
            for ev in d.events.iter().rev() {
                let parts: Vec<String> = vec![ev.kind.clone()]
                    .into_iter()
                    .chain(ev.field.clone())
                    .chain(ev.old_value.clone().map(|v| format!("\"{v}\"")))
                    .chain(ev.new_value.clone().map(|v| format!("-> \"{v}\"")))
                    .collect();
                lines.push(Line::from(Span::styled(
                    format!("  {} · {}", parts.join(" "), ev.actor),
                    Style::default().fg(Palette::DIM),
                )));
            }
        }
    }

    f.render_widget(
        Paragraph::new(lines)
            .wrap(ratatui::widgets::Wrap { trim: false })
            .scroll((scroll as u16, 0)),
        inner,
    );
}

/// Render the sidebar add form when in SidebarAdd mode. Mirrors the edit
/// form but with empty fields, no existing todo, and a CREATE title.
fn render_sidebar_add_form(f: &mut Frame, app: &crate::app::App, area: Rect) {
    let crate::app::Mode::SidebarAdd {
        field,
        input_active,
        title_input,
        title_caret,
        desc_input,
        desc_caret,
        tag_input,
        tag_caret,
        link_kind,
        link_selection,
        attaching,
        link_search,
        scroll,
        ..
    } = &app.mode
    else {
        return;
    };

    let field = *field;
    let input_active = *input_active;
    let link_kind = *link_kind;
    let link_selection = *link_selection;
    let attaching = *attaching;
    let scroll = *scroll;
    let title_caret = *title_caret;
    let desc_caret = *desc_caret;
    let tag_caret = *tag_caret;
    let title_input: &str = title_input;
    let desc_input: &str = desc_input;
    let tag_input: &str = tag_input;
    let link_search: &str = link_search;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Palette::ACCENT))
        .title(Span::styled(
            " CREATE ",
            Style::default().fg(Palette::ACCENT),
        ));
    f.render_widget(block, area);

    let inner = Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    if inner.height == 0 {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        "new todo",
        Style::default().fg(Palette::DIM),
    )));
    lines.push(Line::from(""));

    // TITLE
    lines.push(Line::from(field_header(
        "TITLE",
        crate::app::SidebarField::Title,
        field,
        input_active,
    )));
    if input_active && field == crate::app::SidebarField::Title {
        lines.push(Line::from(Span::styled(
            text_with_caret(title_input, title_caret),
            Style::default().fg(Palette::TEXT),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            if title_input.is_empty() {
                "—"
            } else {
                title_input
            },
            Style::default().fg(Palette::DIM),
        )));
    }
    lines.push(Line::from(""));

    // DESCRIPTION
    lines.push(Line::from(field_header(
        "DESCRIPTION",
        crate::app::SidebarField::Description,
        field,
        input_active,
    )));
    if input_active && field == crate::app::SidebarField::Description {
        lines.push(Line::from(Span::styled(
            text_with_caret(desc_input, desc_caret),
            Style::default().fg(Palette::TEXT),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            if desc_input.is_empty() {
                "—"
            } else {
                desc_input
            },
            Style::default().fg(Palette::DIM),
        )));
    }
    lines.push(Line::from(""));

    // LINKS
    lines.push(Line::from(field_header(
        "LINKS",
        crate::app::SidebarField::Links,
        field,
        input_active,
    )));
    if input_active && field == crate::app::SidebarField::Links {
        if attaching {
            let kind_str = match link_kind {
                crate::app::LinkKind::Pr => "PRs",
                crate::app::LinkKind::Linear => "Issues",
            };
            lines.push(Line::from(Span::styled(
                format!(
                    "  attach {kind_str} (tab: kind · j/k: move · enter: attach · esc: cancel)"
                ),
                Style::default().fg(Palette::DIM),
            )));
            lines.push(Line::from(Span::styled(
                format!("  search: {}", text_with_caret(link_search, 0)),
                Style::default().fg(Palette::DIM),
            )));
            let candidates: Vec<usize> = if link_kind == crate::app::LinkKind::Pr {
                pr_picker_candidates(app, link_search)
            } else {
                linear_picker_candidates(app, link_search)
            };
            if candidates.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  no candidates",
                    Style::default().fg(Palette::GHOST),
                )));
            } else {
                let sel = link_selection.min(candidates.len() - 1);
                for (vis, &data_idx) in candidates.iter().enumerate() {
                    let (selected, label) = if link_kind == crate::app::LinkKind::Pr {
                        let row = &app.data.pulls[data_idx];
                        (vis == sel, format!("PR #{} {}", row.number, row.title))
                    } else {
                        let row = &app.data.linears[data_idx];
                        (vis == sel, format!("{} {}", row.identifier, row.title))
                    };
                    if selected {
                        lines.push(Line::from(vec![
                            Span::styled(
                                Glyph::CURSOR.to_string(),
                                Style::default().fg(Palette::ACCENT),
                            ),
                            Span::raw(" "),
                            Span::styled(
                                label,
                                Style::default()
                                    .fg(Palette::ACCENT)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    } else {
                        lines.push(Line::from(Span::styled(
                            format!("  {label}"),
                            Style::default().fg(Palette::TEXT),
                        )));
                    }
                }
            }
        } else {
            lines.push(Line::from(Span::styled(
                "  press a to attach a PR or issue",
                Style::default().fg(Palette::GHOST),
            )));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "  —",
            Style::default().fg(Palette::DIM),
        )));
    }
    lines.push(Line::from(""));

    // TAGS
    lines.push(Line::from(field_header(
        "TAGS",
        crate::app::SidebarField::Tags,
        field,
        input_active,
    )));
    if input_active && field == crate::app::SidebarField::Tags {
        lines.push(Line::from(Span::styled(
            text_with_caret(tag_input, tag_caret),
            Style::default().fg(Palette::TEXT),
        )));
    } else if tag_input.is_empty() {
        lines.push(Line::from(Span::styled(
            "  —",
            Style::default().fg(Palette::DIM),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!("  {}", tag_input),
            Style::default().fg(Palette::DIM),
        )));
    }

    f.render_widget(
        Paragraph::new(lines)
            .wrap(ratatui::widgets::Wrap { trim: false })
            .scroll((scroll as u16, 0)),
        inner,
    );
}

/// Resolve the highlighted todo for the sidebar: the cursor todo from the
/// active view's row set at draw time.
fn sidebar_todo(app: &crate::app::App) -> Option<&Todo> {
    match app.view {
        crate::app::View::Today => app.today_plan().get(app.cursor).copied(),
        crate::app::View::Backlog => app.backlog().get(app.cursor).copied(),
        _ => None,
    }
}

/// Split the content area into a list (75%) + sidebar (25%) when wide
/// enough. Below 100 cols, returns the full area as the list and `None`
/// for the sidebar (D4).
pub fn split_content(area: Rect) -> (Rect, Option<Rect>) {
    if area.width < 100 {
        return (area, None);
    }
    let sidebar_width = (area.width / 4).max(25);
    let list_width = area.width.saturating_sub(sidebar_width);
    let list_area = Rect::new(area.x, area.y, list_width, area.height);
    let sidebar_area = Rect::new(area.x + list_width, area.y, sidebar_width, area.height);
    (list_area, Some(sidebar_area))
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;
    use ratatui::style::Modifier;

    use crate::frame::{
        blocked_reason_line, group_by_status, linear_state_glyph, pr_state_glyph, section_header,
        split_content, status_glyph,
    };
    use crate::theme::{Glyph, Palette};

    use crate::app::{App, Mode, SidebarField};

    // -- status_hints tests --

    #[test]
    fn test_status_hints_sidebar_edit_inactive_shows_field_nav() {
        let mut app = App::default();
        app.mode = Mode::SidebarEdit {
            id: 1,
            field: SidebarField::Title,
            input_active: false,
            title_input: String::new(),
            title_caret: 0,
            desc_input: String::new(),
            desc_caret: 0,
            desc_scroll: 0,
            tag_input: String::new(),
            tag_caret: 0,
            link_kind: crate::app::LinkKind::Pr,
            link_selection: 0,
            attaching: false,
            link_search: String::new(),
            scroll: 0,
        };
        let hints = crate::frame::status_hints(&app);
        assert!(
            hints.contains("enter edit"),
            "inactive sidebar should hint how to edit a field: {hints}"
        );
        assert!(
            hints.contains("tab/j"),
            "inactive sidebar should hint field navigation: {hints}"
        );
    }

    #[test]
    fn test_status_hints_sidebar_edit_active_shows_save() {
        let mut app = App::default();
        app.mode = Mode::SidebarEdit {
            id: 1,
            field: SidebarField::Title,
            input_active: true,
            title_input: String::new(),
            title_caret: 0,
            desc_input: String::new(),
            desc_caret: 0,
            desc_scroll: 0,
            tag_input: String::new(),
            tag_caret: 0,
            link_kind: crate::app::LinkKind::Pr,
            link_selection: 0,
            attaching: false,
            link_search: String::new(),
            scroll: 0,
        };
        let hints = crate::frame::status_hints(&app);
        assert!(
            hints.contains("enter save field"),
            "active sidebar should hint how to save a field: {hints}"
        );
    }

    // -- status_glyph tests --

    #[test]
    fn test_status_glyph_todo() {
        let (ch, color) = status_glyph("todo");
        assert_eq!(ch, Glyph::STATUS_TODO);
        assert_eq!(color, Palette::TODO);
    }

    #[test]
    fn test_status_glyph_started() {
        let (ch, color) = status_glyph("started");
        assert_eq!(ch, Glyph::STATUS_STARTED);
        assert_eq!(color, Palette::STARTED);
    }

    #[test]
    fn test_status_glyph_blocked() {
        let (ch, color) = status_glyph("blocked");
        assert_eq!(ch, Glyph::STATUS_BLOCKED);
        assert_eq!(color, Palette::BLOCKED);
    }

    #[test]
    fn test_status_glyph_done() {
        let (ch, color) = status_glyph("done");
        assert_eq!(ch, Glyph::STATUS_DONE);
        assert_eq!(color, Palette::DONE);
    }

    #[test]
    fn test_status_glyph_cancelled() {
        let (ch, color) = status_glyph("cancelled");
        assert_eq!(ch, Glyph::STATUS_CANCELLED);
        assert_eq!(color, Palette::CANCELLED);
    }

    #[test]
    fn test_status_glyph_unknown() {
        let (ch, color) = status_glyph("unknown_status");
        assert_eq!(ch, '?');
        assert_eq!(color, Palette::DIM);
    }

    // -- pr_state_glyph tests --

    #[test]
    fn test_pr_state_glyph_open() {
        let (ch, color) = pr_state_glyph("open");
        assert_eq!(ch, Glyph::PR_OPEN);
        assert_eq!(color, Palette::DONE);
    }

    #[test]
    fn test_pr_state_glyph_draft() {
        let (ch, color) = pr_state_glyph("draft");
        assert_eq!(ch, Glyph::PR_DRAFT);
        assert_eq!(color, Palette::CANCELLED);
    }

    #[test]
    fn test_pr_state_glyph_merged() {
        let (ch, color) = pr_state_glyph("merged");
        assert_eq!(ch, Glyph::PR_MERGED);
        assert_eq!(color, Palette::MERGED);
    }

    #[test]
    fn test_pr_state_glyph_closed() {
        let (ch, color) = pr_state_glyph("closed");
        assert_eq!(ch, Glyph::PR_CLOSED);
        assert_eq!(color, Palette::BLOCKED);
    }

    #[test]
    fn test_pr_state_glyph_unknown() {
        let (ch, color) = pr_state_glyph("weird");
        assert_eq!(ch, '?');
        assert_eq!(color, Palette::DIM);
    }

    // -- linear_state_glyph tests --

    #[test]
    fn test_linear_state_glyph_triage() {
        let (ch, color) = linear_state_glyph("triage");
        assert_eq!(ch, '?');
        assert_eq!(color, Palette::ACCENT);
    }

    #[test]
    fn test_linear_state_glyph_backlog() {
        let (ch, color) = linear_state_glyph("backlog");
        assert_eq!(ch, '·');
        assert_eq!(color, Palette::CANCELLED);
    }

    #[test]
    fn test_linear_state_glyph_unstarted() {
        let (ch, color) = linear_state_glyph("unstarted");
        assert_eq!(ch, Glyph::STATUS_TODO);
        assert_eq!(color, Palette::TODO);
    }

    #[test]
    fn test_linear_state_glyph_started() {
        let (ch, color) = linear_state_glyph("started");
        assert_eq!(ch, Glyph::STATUS_STARTED);
        assert_eq!(color, Palette::STARTED);
    }

    #[test]
    fn test_linear_state_glyph_completed() {
        let (ch, color) = linear_state_glyph("completed");
        assert_eq!(ch, Glyph::STATUS_DONE);
        assert_eq!(color, Palette::DONE);
    }

    #[test]
    fn test_linear_state_glyph_canceled() {
        let (ch, color) = linear_state_glyph("canceled");
        assert_eq!(ch, Glyph::STATUS_CANCELLED);
        assert_eq!(color, Palette::CANCELLED);
    }

    #[test]
    fn test_linear_state_glyph_unknown() {
        let (ch, color) = linear_state_glyph("unknown_type");
        assert_eq!(ch, '?');
        assert_eq!(color, Palette::DIM);
    }

    // -- section_header tests --

    #[test]
    fn test_section_header_has_title() {
        let line = section_header("My Section");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].content, "My Section");
    }

    #[test]
    fn test_section_header_is_dim_styled() {
        let line = section_header("Header");
        let style = line.spans[0].style;
        assert_eq!(style.fg, Some(Palette::DIM));
    }

    // -- blocked_reason_line tests --

    #[test]
    fn test_blocked_reason_line_contains_reason() {
        let line = blocked_reason_line("waiting on CI");
        let reason_span = line.spans.iter().find(|s| s.content == "waiting on CI");
        assert!(
            reason_span.is_some(),
            "reason text should be in the line spans"
        );
        let span = reason_span.unwrap();
        assert_eq!(span.style.fg, Some(Palette::BLOCKED));
        assert!(span.style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_blocked_reason_line_has_glyph() {
        let line = blocked_reason_line("reason");
        let glyph_span = line
            .spans
            .iter()
            .find(|s| s.content == Glyph::BLOCKED_REASON.to_string());
        assert!(
            glyph_span.is_some(),
            "blocked reason glyph should be present"
        );
        assert_eq!(glyph_span.unwrap().style.fg, Some(Palette::BLOCKED));
    }

    // -- group_by_status tests (task 3.2) --

    fn make_todo(id: i32, status: &str) -> crate::gql::Todo {
        crate::gql::Todo {
            id,
            title: format!("Todo {id}"),
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

    fn make_titled_todo(id: i32, title: &str, status: &str) -> crate::gql::Todo {
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

    #[test]
    fn test_group_by_status_empty_groups_skipped() {
        let t1 = make_todo(1, "started");
        let t2 = make_todo(2, "done");
        let rows: Vec<&crate::gql::Todo> = vec![&t1, &t2];
        let groups = group_by_status(&rows, true);
        // Only started and done have rows; blocked, todo, cancelled are skipped.
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].status, "started");
        assert_eq!(groups[1].status, "done");
    }

    #[test]
    fn test_group_by_status_fixed_order() {
        let t1 = make_todo(1, "cancelled");
        let t2 = make_todo(2, "todo");
        let t3 = make_todo(3, "started");
        let t4 = make_todo(4, "blocked");
        let t5 = make_todo(5, "done");
        let rows: Vec<&crate::gql::Todo> = vec![&t1, &t2, &t3, &t4, &t5];
        let groups = group_by_status(&rows, true);
        assert_eq!(groups.len(), 5);
        assert_eq!(groups[0].status, "started");
        assert_eq!(groups[1].status, "blocked");
        assert_eq!(groups[2].status, "todo");
        assert_eq!(groups[3].status, "done");
        assert_eq!(groups[4].status, "cancelled");
    }

    #[test]
    fn test_group_by_status_done_collapsed_when_show_done_false() {
        let t1 = make_todo(1, "todo");
        let t2 = make_todo(2, "done");
        let t3 = make_todo(3, "done");
        let rows: Vec<&crate::gql::Todo> = vec![&t1, &t2, &t3];
        let groups = group_by_status(&rows, false);
        let done_group = groups.iter().find(|g| g.status == "done").unwrap();
        assert!(done_group.collapsed);
        // Rows are kept so the count is correct.
        assert_eq!(done_group.rows.len(), 2);
    }

    #[test]
    fn test_group_by_status_done_expanded_when_show_done_true() {
        let t1 = make_todo(1, "done");
        let rows: Vec<&crate::gql::Todo> = vec![&t1];
        let groups = group_by_status(&rows, true);
        let done_group = groups.iter().find(|g| g.status == "done").unwrap();
        assert!(!done_group.collapsed);
        assert_eq!(done_group.rows.len(), 1);
    }

    // -- split_content tests (task 9.4) --

    #[test]
    fn test_split_content_narrow_returns_full_area_no_sidebar() {
        let area = Rect::new(0, 0, 80, 24);
        let (list, sidebar) = split_content(area);
        assert_eq!(list, area);
        assert!(sidebar.is_none());
    }

    #[test]
    fn test_split_content_wide_splits_list_and_sidebar() {
        let area = Rect::new(0, 0, 120, 24);
        let (list, sidebar) = split_content(area);
        assert!(list.width < area.width);
        assert!(sidebar.is_some());
        let sb = sidebar.unwrap();
        assert!(sb.width >= 25);
        assert_eq!(list.width + sb.width, area.width);
    }

    #[test]
    fn test_split_content_boundary_99_no_sidebar() {
        let area = Rect::new(0, 0, 99, 24);
        let (_, sidebar) = split_content(area);
        assert!(sidebar.is_none());
    }

    #[test]
    fn test_split_content_boundary_100_has_sidebar() {
        let area = Rect::new(0, 0, 100, 24);
        let (_, sidebar) = split_content(area);
        assert!(sidebar.is_some());
    }

    #[test]
    fn test_group_by_status_with_filter_end_to_end() {
        // Three todos: two match "rust", one doesn't.
        let t1 = make_titled_todo(1, "Write Rust docs", "started");
        let t2 = make_titled_todo(2, "Review PR", "todo");
        let t3 = make_titled_todo(3, "Rust refactor", "done");
        let all: Vec<&crate::gql::Todo> = vec![&t1, &t2, &t3];

        // Apply filter "rust" — only t1 and t3 survive.
        let filtered: Vec<&crate::gql::Todo> = all
            .iter()
            .copied()
            .filter(|t| crate::app::matches_filter(t, "rust"))
            .collect();
        let groups = group_by_status(&filtered, true);

        // Started has 1, done has 1, todo group is skipped (t2 filtered out).
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].status, "started");
        assert_eq!(groups[0].rows.len(), 1);
        assert_eq!(groups[1].status, "done");
        assert_eq!(groups[1].rows.len(), 1);
    }

    #[test]
    fn test_group_by_status_filter_no_matches_yields_empty() {
        let t1 = make_titled_todo(1, "Task", "todo");
        let all: Vec<&crate::gql::Todo> = vec![&t1];
        let filtered: Vec<&crate::gql::Todo> = all
            .iter()
            .copied()
            .filter(|t| crate::app::matches_filter(t, "zzzznonexistent"))
            .collect();
        assert!(filtered.is_empty());
        let groups = group_by_status(&filtered, true);
        assert!(groups.is_empty());
    }
}
#[cfg(test)]
mod render_tests {
    use ratatui::{Terminal, backend::TestBackend};

    use crate::app::tests::make_todo;
    use crate::app::{App, Mode, View};
    use crate::gql::{SyncState, Todo};
    use crate::test_support::buffer_text;
    use crate::theme::{Glyph, Palette};

    use super::{GroupedSection, render_frame, render_grouped_list};

    fn render_frame_text(app: &App) -> String {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_frame(f, app, f.area());
            })
            .unwrap();
        buffer_text(terminal.backend().buffer())
    }

    /// Render with the shared grouped-list renderer, returning the text of
    /// every row whose background carries the row-highlight color.
    fn highlighted_grouped_rows<'a>(
        groups: Vec<GroupedSection<'a>>,
        cursor_todo: Option<&Todo>,
    ) -> Vec<String> {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_grouped_list(
                    f,
                    f.area(),
                    &groups,
                    cursor_todo,
                    &std::collections::HashSet::new(),
                    vec![],
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        (0..buf.area.height)
            .filter(|y| {
                (0..buf.area.width).any(|x| buf[(x, *y)].style().bg == Some(Palette::ROW_HIGHLIGHT))
            })
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }
    #[test]
    fn test_grouped_list_cursor_offsets_complete_header() {
        let first = make_todo(1, "first started", "started");
        let second = make_todo(2, "second started", "started");
        let third = make_todo(3, "plain todo", "todo");
        let groups = vec![
            GroupedSection {
                label: "STARTED",
                status: "started",
                rows: vec![&first, &second],
                collapsed: false,
            },
            GroupedSection {
                label: "TODO",
                status: "todo",
                rows: vec![&third],
                collapsed: false,
            },
        ];

        // The STARTED header occupies flat index 0, so the highlight must
        // land on the cursor's todo (second) regardless of position.
        let rows = highlighted_grouped_rows(groups, Some(&second));
        assert_eq!(
            rows.len(),
            1,
            "expected exactly one highlighted row, got {rows:?}"
        );
        assert!(
            rows[0].contains("second started"),
            "expected highlight on the second todo row, got {:?}",
            rows[0]
        );
    }
    #[test]
    fn test_grouped_list_single_highlight_with_collapsed_done() {
        let started = make_todo(1, "started one", "started");
        let todo = make_todo(2, "todo one", "todo");
        let done_a = make_todo(3, "done a", "done");
        let done_b = make_todo(4, "done b", "done");
        let groups = vec![
            GroupedSection {
                label: "STARTED",
                status: "started",
                rows: vec![&started],
                collapsed: false,
            },
            GroupedSection {
                label: "TODO",
                status: "todo",
                rows: vec![&todo],
                collapsed: false,
            },
            GroupedSection {
                label: "DONE",
                status: "done",
                rows: vec![&done_a, &done_b],
                collapsed: true,
            },
        ];

        // cursor 1 = the sole TODO row (after the started row). The STARTED
        // header, group separators, and the collapsed DONE header all consume
        // visual lines, so the highlight must still land on the TODO row and
        // nowhere else — in particular not on any header line.
        let rows = highlighted_grouped_rows(groups, Some(&todo));
        assert_eq!(
            rows.len(),
            1,
            "expected exactly one highlighted row, got {rows:?}"
        );
        assert!(
            rows[0].contains("todo one"),
            "expected highlight on the TODO row, got {:?}",
            rows[0]
        );
    }
    #[test]
    fn test_render_frame_shows_date_with_weekday() {
        let app = App {
            logical_date: chrono::NaiveDate::from_ymd_opt(2026, 8, 6).unwrap(),
            ..Default::default()
        };
        let text = render_frame_text(&app);
        assert!(
            text.contains("Thu"),
            "expected weekday 'Thu' in frame, got:\n{}",
            text
        );
        assert!(
            text.contains("2026-08-06"),
            "expected date '2026-08-06' in frame, got:\n{}",
            text
        );
    }

    #[test]
    fn test_render_frame_tab_bar_highlights_active_view() {
        let app = App {
            view: View::Backlog,
            ..Default::default()
        };
        let text = render_frame_text(&app);

        // All five tab labels should appear
        assert!(
            text.contains("Today"),
            "expected 'Today' tab label, got:\n{}",
            text
        );
        assert!(
            text.contains("Backlog"),
            "expected 'Backlog' tab label, got:\n{}",
            text
        );
        assert!(
            text.contains("Inbox"),
            "expected 'Inbox' tab label, got:\n{}",
            text
        );
        assert!(
            text.contains("Review"),
            "expected 'Review' tab label, got:\n{}",
            text
        );
        assert!(
            text.contains("Sync"),
            "expected 'Sync' tab label, got:\n{}",
            text
        );

        // Active view label is surrounded by spaces (rendered as " Backlog ")
        let tab_line = text.lines().nth(1).unwrap();
        assert!(
            tab_line.contains(" Backlog "),
            "expected active tab ' Backlog ' with surrounding spaces, got tab line:\n{}",
            tab_line
        );
    }

    #[test]
    fn test_render_frame_status_bar_shows_navigate_hints() {
        let app = App {
            view: View::Today,
            mode: Mode::Navigate,
            ..Default::default()
        };
        let text = render_frame_text(&app);

        // Status hints for Today/Navigate include "j/k move", "SPC status", "a add"
        let status_line = text.lines().last().unwrap();
        assert!(
            status_line.contains("j/k move"),
            "expected 'j/k move' hint in status bar, got:\n{}",
            status_line
        );
        assert!(
            status_line.contains("SPC status"),
            "expected 'SPC status' hint in status bar, got:\n{}",
            status_line
        );
        assert!(
            status_line.contains("a add"),
            "expected 'a add' hint in status bar, got:\n{}",
            status_line
        );
    }

    #[test]
    fn test_render_frame_sync_indicator_when_syncing() {
        let app = App {
            data: crate::app::AppData {
                sync: vec![SyncState {
                    source: "github".to_string(),
                    cursor: None,
                    last_synced_at: None,
                    last_status: "syncing".to_string(),
                    last_error: None,
                }],
                ..Default::default()
            },
            ..Default::default()
        };
        let text = render_frame_text(&app);
        let status_line = text.lines().last().unwrap();
        // When syncing, the status bar should contain one of the spinner glyphs
        let has_spinner = Glyph::SPINNER.iter().any(|&c| status_line.contains(c));
        assert!(
            has_spinner,
            "expected a spinner character in status bar when syncing, got:\n{}",
            status_line
        );
    }

    #[test]
    fn test_render_frame_context_word_changes_per_view() {
        use crate::app::tests::{make_plan_row, make_todo};

        // Today view with 2 plan todos
        let app = App {
            view: View::Today,
            data: crate::app::AppData {
                plan: vec![
                    make_plan_row(1, 0, make_todo(1, "First", "started"), None),
                    make_plan_row(2, 1, make_todo(2, "Second", "todo"), None),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        let text = render_frame_text(&app);

        let tab_line = text.lines().nth(1).unwrap();
        assert!(
            tab_line.contains("2 planned"),
            "expected '2 planned' in context word, got tab line:\n{}",
            tab_line
        );

        // Sync view with 3 sync entries
        let app = App {
            view: View::Sync,
            data: crate::app::AppData {
                sync: vec![
                    SyncState {
                        source: "github".to_string(),
                        cursor: None,
                        last_synced_at: None,
                        last_status: "idle".to_string(),
                        last_error: None,
                    },
                    SyncState {
                        source: "linear".to_string(),
                        cursor: None,
                        last_synced_at: None,
                        last_status: "idle".to_string(),
                        last_error: None,
                    },
                    SyncState {
                        source: "jira".to_string(),
                        cursor: None,
                        last_synced_at: None,
                        last_status: "idle".to_string(),
                        last_error: None,
                    },
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        let text = render_frame_text(&app);

        let tab_line = text.lines().nth(1).unwrap();
        assert!(
            tab_line.contains("3 sources configured"),
            "expected '3 sources configured' in context word, got tab line:\n{}",
            tab_line
        );
    }
}
