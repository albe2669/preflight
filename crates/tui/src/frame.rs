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
        (View::Today, _) => {
            "j/k move  SPC status  a add  J/K reorder  x unplan  enter detail  ? keys".into()
        }
        (View::Backlog, _) => "/ search  t plan today  SPC status  # tag  D show done".into(),
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
