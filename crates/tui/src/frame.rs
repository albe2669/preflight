//! Persistent frame chrome (header, tabs, status bar).
//!
//! The frame is the one thing that never moves between views: rows 1-2 are
//! chrome, row 3 a rule, rows 4-30 content, row 31 blank, row 32 the status
//! bar. Only the tab highlight, the context word, and the key hints change.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{App, View};
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

#[cfg(test)]
mod tests {
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
}

#[cfg(test)]
mod render_tests {
    use ratatui::{Terminal, backend::TestBackend};

    use crate::app::{App, Mode, View};
    use crate::gql::SyncState;
    use crate::test_support::buffer_text;
    use crate::theme::Glyph;

    use super::render_frame;

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
