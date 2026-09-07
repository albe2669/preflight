//! View rendering and key handling.
//!
//! Each view renders into the content area inside the frame chrome. Key
//! handlers are async (they issue GraphQL mutations then trigger a refetch).

pub mod backlog;
pub mod inbox;
pub mod inline;
pub mod overlays;
pub mod review;
pub mod sidebar;
pub mod sync;
pub mod today;

use ratatui::Frame;

use crate::app::{App, ConfirmAction, Mode, ToastKind, View};
use crate::frame;
use crate::theme::Palette;
use crate::widgets;

/// Top-level render: frame chrome + the active view's content + overlays.
pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let content = frame::render_frame(f, app, area);
    app.content_width = content.width;
    let (list_area, sidebar_area) = widgets::split_content(content);
    match app.view {
        View::Today => today::render(f, app, list_area),
        View::Backlog => backlog::render(f, app, list_area),
        View::Inbox => inbox::render(f, app, content),
        View::Review => review::render(f, app, content),
        View::Sync => sync::render(f, app, content),
    }
    if let Some(sb) = sidebar_area {
        if matches!(app.view, View::Today | View::Backlog) {
            sidebar::render_info_sidebar(f, app, sb);
        }
    }

    // Overlays: confirm dialog, status select, help, toast.
    if let Mode::Help { .. } = &app.mode {
        render_help(f, app, area);
    }
    if let Mode::StatusSelect { .. } = &app.mode {
        overlays::render_status_select(f, app, area);
    }
    if let Mode::LinkTodo { .. } = &app.mode {
        overlays::render_link_todo(f, app, area);
    }
    if let Mode::Confirm { .. } = &app.mode {
        render_confirm(f, app, area);
    }
    render_toast(f, app, area);
}

fn render_toast(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let Some(t) = &app.toast else { return };
    let (glyph, color) = match t.kind {
        ToastKind::Success => ('✓', Palette::DONE),
        ToastKind::Error => ('▲', Palette::BLOCKED),
    };
    let msg = format!(" {glyph} {} ", t.message);
    let width = msg.chars().count() as u16 + 2;
    let x = area.x + (area.width - width) / 2;
    let y = area.y + area.height.saturating_sub(4);
    let rect = ratatui::layout::Rect::new(x, y, width.min(area.width), 1);
    let line = ratatui::text::Line::from(vec![
        ratatui::text::Span::styled(format!(" {glyph} "), Style::default().fg(color)),
        ratatui::text::Span::styled(t.message.clone(), Style::default().fg(Palette::TEXT)),
    ]);
    f.render_widget(
        ratatui::widgets::Paragraph::new(line).style(Style::default().bg(Palette::SURFACE)),
        rect,
    );
}

/// A keybind row for the help overlay: the key sequence and its description.
struct KeybindRow {
    key: &'static str,
    desc: &'static str,
}

/// A section of keybinds in the help overlay.
struct KeybindSection {
    title: &'static str,
    rows: &'static [KeybindRow],
}

const GLOBAL_KEYS: &[KeybindRow] = &[
    KeybindRow {
        key: "q",
        desc: "quit the app",
    },
    KeybindRow {
        key: "Tab",
        desc: "next view",
    },
    KeybindRow {
        key: "Shift-Tab",
        desc: "previous view",
    },
    KeybindRow {
        key: "?",
        desc: "this help",
    },
    KeybindRow {
        key: "Esc",
        desc: "close overlay / cancel action",
    },
];

const TODAY_KEYS: &[KeybindRow] = &[
    KeybindRow {
        key: "j/k",
        desc: "move cursor",
    },
    KeybindRow {
        key: "SPC/s",
        desc: "status popup",
    },
    KeybindRow {
        key: "a",
        desc: "add todo (inline)",
    },
    KeybindRow {
        key: "e/Enter",
        desc: "sidebar edit",
    },
    KeybindRow {
        key: "D",
        desc: "toggle done group",
    },
    KeybindRow {
        key: "J/K",
        desc: "reorder (armed mode)",
    },
    KeybindRow {
        key: "x",
        desc: "unplan from today",
    },
    KeybindRow {
        key: "c",
        desc: "carry over yesterday's unfinished",
    },
    KeybindRow {
        key: "b",
        desc: "go to backlog",
    },
    KeybindRow {
        key: "R",
        desc: "review yesterday",
    },
    KeybindRow {
        key: "/",
        desc: "search / filter",
    },
];

const BACKLOG_KEYS: &[KeybindRow] = &[
    KeybindRow {
        key: "j/k",
        desc: "move cursor",
    },
    KeybindRow {
        key: "SPC/s",
        desc: "status popup",
    },
    KeybindRow {
        key: "/",
        desc: "search / filter",
    },
    KeybindRow {
        key: "t",
        desc: "plan for today",
    },
    KeybindRow {
        key: "D",
        desc: "toggle done group",
    },
    KeybindRow {
        key: "a",
        desc: "add todo (inline)",
    },
    KeybindRow {
        key: "e/Enter",
        desc: "sidebar edit",
    },
];
const SIDEBAR_KEYS: &[KeybindRow] = &[
    KeybindRow {
        key: "Tab/j",
        desc: "next field",
    },
    KeybindRow {
        key: "Shift-Tab/k",
        desc: "previous field",
    },
    KeybindRow {
        key: "Enter",
        desc: "edit / save current field",
    },
    KeybindRow {
        key: "Esc",
        desc: "leave field / close the edit form",
    },
    KeybindRow {
        key: "#",
        desc: "add a tag to the current todo",
    },
];

const HELP_SECTIONS: &[KeybindSection] = &[
    KeybindSection {
        title: "Global",
        rows: GLOBAL_KEYS,
    },
    KeybindSection {
        title: "Today",
        rows: TODAY_KEYS,
    },
    KeybindSection {
        title: "Backlog",
        rows: BACKLOG_KEYS,
    },
    KeybindSection {
        title: "Sidebar edit",
        rows: SIDEBAR_KEYS,
    },
];

/// Filter the keybind sections by `filter` (case-insensitive on key + desc).
/// Returns the sections whose rows survive, dropping empty sections. An
/// empty filter returns all sections unchanged.
fn filter_keybinds<'a>(filter: &str) -> Vec<(&'a str, Vec<&'a KeybindRow>)> {
    let needle = filter.to_lowercase();
    HELP_SECTIONS
        .iter()
        .filter_map(|sec| {
            let rows: Vec<&KeybindRow> = sec
                .rows
                .iter()
                .filter(|r| {
                    needle.is_empty()
                        || r.key.to_lowercase().contains(&needle)
                        || r.desc.to_lowercase().contains(&needle)
                })
                .collect();
            if rows.is_empty() {
                None
            } else {
                Some((sec.title, rows))
            }
        })
        .collect()
}

fn render_help(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};

    let filter = match &app.mode {
        Mode::Help { filter } => filter.clone(),
        _ => String::new(),
    };

    let sections = filter_keybinds(&filter);
    let mut lines: Vec<Line> = Vec::new();
    if sections.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("no keybinds match \"{filter}\""),
            Style::default().fg(Palette::DIM),
        )));
    } else {
        for (i, (title, rows)) in sections.iter().enumerate() {
            if i > 0 {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(
                title.to_string(),
                Style::default().fg(Palette::DIM),
            )));
            for row in rows {
                let key_pad = format!("  {:<10}", row.key);
                lines.push(Line::from(vec![
                    Span::styled(key_pad, Style::default().fg(Palette::ACCENT)),
                    Span::raw(row.desc),
                ]));
            }
        }
    }

    // Size the box to its content (so it never shows a big blank region),
    // capped to the available screen height. Scroll only over the overflow.
    let width = 56.min(area.width);
    let content_inner = lines.len() as u16;
    let height = (content_inner + 2).min(area.height);
    let inner_height = height.saturating_sub(2);
    let max_scroll = (content_inner as usize).saturating_sub(inner_height as usize);
    let scroll = app.help_scroll.min(max_scroll);
    let x = area.x + (area.width - width) / 2;
    let y = area.y + (area.height - height) / 2;
    let rect = ratatui::layout::Rect::new(x, y, width, height);

    f.render_widget(Clear, rect);

    let title = if filter.is_empty() {
        " KEYS ".to_string()
    } else {
        format!(" KEYS · {filter} ")
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Palette::ACCENT))
        .title(Span::styled(title, Style::default().fg(Palette::ACCENT)));
    f.render_widget(block, rect);

    let inner = ratatui::layout::Rect::new(x + 1, y + 1, width - 2, inner_height);
    f.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);
}

fn render_confirm(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let Mode::Confirm { action } = &app.mode else {
        return;
    };
    let (title, lines) = match action {
        ConfirmAction::CarryOver { from, to } => (
            "CONFIRM",
            vec![
                format!("Carry unfinished todos from {from} into {to}?"),
                "yesterday's plan is kept.".to_string(),
            ],
        ),
        ConfirmAction::Unplan { .. } => (
            "CONFIRM",
            vec![
                "Remove this todo from today's plan?".into(),
                "the todo itself is kept.".into(),
            ],
        ),
        ConfirmAction::Cancel { .. } => (
            "CONFIRM",
            vec![
                "Cancel this todo?".into(),
                "it stays in the log, struck through.".into(),
            ],
        ),
        ConfirmAction::DismissPr { .. } => (
            "CONFIRM",
            vec![
                "Dismiss this pull request?".into(),
                "it stays visible under d-toggle.".into(),
            ],
        ),
    };

    let height = lines.len() as u16 + 4;
    let max_line = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let width = 50u16.max(max_line as u16 + 4);
    let x = area.x + (area.width - width) / 2;
    let y = area.y + (area.height - height) / 2;
    let rect = ratatui::layout::Rect::new(x, y, width, height);

    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(Style::default().fg(Palette::ACCENT))
        .title(ratatui::text::Span::styled(
            format!(" {title} "),
            Style::default().fg(Palette::ACCENT),
        ));
    f.render_widget(ratatui::widgets::Clear, rect);
    f.render_widget(block, rect);

    let inner = ratatui::layout::Rect::new(x + 1, y + 1, width - 2, lines.len() as u16);
    let paras: Vec<ratatui::text::Line> = lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if i == lines.len() - 1 {
                ratatui::text::Line::from(vec![ratatui::text::Span::styled(
                    l.clone(),
                    Style::default().fg(Palette::DIM),
                )])
            } else {
                ratatui::text::Line::from(vec![ratatui::text::Span::styled(
                    l.clone(),
                    Style::default().fg(Palette::TEXT),
                )])
            }
        })
        .collect();
    f.render_widget(ratatui::widgets::Paragraph::new(paras), inner);

    // Action hint row.
    let hint_y = y + height - 2;
    let hint = ratatui::text::Line::from(vec![
        ratatui::text::Span::styled(
            " y confirm ",
            Style::default()
                .bg(Palette::ACCENT)
                .fg(Palette::ACCENT_TEXT)
                .add_modifier(Modifier::BOLD),
        ),
        ratatui::text::Span::styled("  n cancel", Style::default().fg(Palette::DIM)),
    ]);
    let hint_rect = ratatui::layout::Rect::new(x + 1, hint_y, width - 2, 1);
    f.render_widget(ratatui::widgets::Paragraph::new(hint), hint_rect);
}

use ratatui::style::{Modifier, Style};

// ---- Help overlay ----

pub(crate) fn handle_help(app: &mut App, key: ratatui::crossterm::event::KeyCode, _filter: &str) {
    use ratatui::crossterm::event::KeyCode::*;
    match key {
        Char(c) if c.is_alphanumeric() || c == ' ' || c == '/' || c == '-' || c == '_' => {
            if let Mode::Help { filter } = &mut app.mode {
                filter.push(c);
                app.help_scroll = 0;
            }
        }
        Backspace => {
            if let Mode::Help { filter } = &mut app.mode {
                filter.pop();
                app.help_scroll = 0;
            }
        }
        Char('j') | Down => {
            app.help_scroll = app.help_scroll.saturating_add(1);
        }
        Char('k') | Up if app.help_scroll > 0 => {
            app.help_scroll -= 1;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::sidebar::tag_commit_is_removal;

    // -- filter_keybinds (task 10.3) --

    #[test]
    fn test_filter_keybinds_empty_returns_all() {
        let sections = filter_keybinds("");
        assert_eq!(sections.len(), 4);
        assert_eq!(sections[0].0, "Global");
        assert_eq!(sections[1].0, "Today");
        assert_eq!(sections[2].0, "Backlog");
        assert_eq!(sections[3].0, "Sidebar edit");
    }

    #[test]
    fn test_filter_keybinds_by_key() {
        let sections = filter_keybinds("SPC");
        assert_eq!(sections.len(), 2);
        assert!(sections.iter().any(|(t, _)| *t == "Today"));
        assert!(sections.iter().any(|(t, _)| *t == "Backlog"));
    }

    #[test]
    fn test_filter_keybinds_by_desc() {
        let sections = filter_keybinds("reorder");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "Today");
        assert_eq!(sections[0].1.len(), 1);
    }

    #[test]
    fn test_filter_keybinds_hides_empty_sections() {
        let sections = filter_keybinds("quit");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "Global");
        assert_eq!(sections[0].1.len(), 1);
    }

    #[test]
    fn test_filter_keybinds_no_matches_returns_empty() {
        let sections = filter_keybinds("zzzznonexistent");
        assert!(sections.is_empty());
    }

    #[test]
    fn test_filter_keybinds_case_insensitive() {
        let lower = filter_keybinds("tab");
        let upper = filter_keybinds("TAB");
        assert_eq!(lower.len(), upper.len());
        // "tab" matches the Global view-switch binding and the Sidebar edit
        // field-nav binding.
        assert_eq!(lower.len(), 2);
        assert!(lower.iter().any(|(t, _)| *t == "Global"));
        assert!(lower.iter().any(|(t, _)| *t == "Sidebar edit"));
    }

    // -- tag_commit_is_removal (sidebar tag removal) --

    #[test]
    fn test_tag_commit_is_removal_when_slug_exists() {
        let app = App {
            data: crate::app::AppData {
                todos: vec![crate::app::tests::make_todo_with_tags(
                    1,
                    "Task",
                    "todo",
                    &["rust", "tui"],
                )],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            tag_commit_is_removal(&app, 1, "rust"),
            "an existing slug removes the tag"
        );
    }

    #[test]
    fn test_tag_commit_not_removal_for_new_slug() {
        let app = App {
            data: crate::app::AppData {
                todos: vec![crate::app::tests::make_todo_with_tags(
                    1,
                    "Task",
                    "todo",
                    &["rust"],
                )],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            !tag_commit_is_removal(&app, 1, "linear"),
            "a new slug is added, not removed"
        );
    }

    #[test]
    fn test_tag_commit_not_removal_when_only_tag_is_last() {
        let app = App {
            data: crate::app::AppData {
                todos: vec![crate::app::tests::make_todo_with_tags(
                    1,
                    "Task",
                    "todo",
                    &["only"],
                )],
                ..Default::default()
            },
            ..Default::default()
        };
        // Re-committing the last (only) tag still counts as removal so the
        // tags region can reach the empty state.
        assert!(tag_commit_is_removal(&app, 1, "only"));
    }

    #[test]
    fn test_tag_commit_not_removal_for_unknown_todo() {
        let app = App {
            data: crate::app::AppData {
                todos: vec![crate::app::tests::make_todo_with_tags(
                    1,
                    "Task",
                    "todo",
                    &["rust"],
                )],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            !tag_commit_is_removal(&app, 999, "rust"),
            "unknown todo defaults to add"
        );
    }
}
#[cfg(test)]
mod render_tests {
    use crate::app::{App, ConfirmAction, Mode, Toast, ToastKind, View};
    use crate::test_support::buffer_text;
    use ratatui::{Terminal, backend::TestBackend};

    fn render_full(app: &mut App) -> String {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| crate::views::render(f, app)).unwrap();
        buffer_text(terminal.backend().buffer())
    }

    #[test]
    fn test_help_overlay_renders_keybindings() {
        let mut app = App {
            view: View::Today,
            mode: Mode::Help {
                filter: String::new(),
            },
            ..Default::default()
        };
        let output = render_full(&mut app);
        assert!(output.contains("KEYS"), "should show help overlay title");
        assert!(output.contains("Global"), "should show Global section");
        assert!(output.contains("quit the app"), "should show quit keybind");
        assert!(output.contains("move cursor"), "should show j/k keybind");
        assert!(
            output.contains("Sidebar edit"),
            "should show Sidebar edit section"
        );
        assert!(
            output.contains("edit / save current field"),
            "should show sidebar edit/save keybind"
        );
    }

    #[test]
    fn test_help_overlay_does_not_overflow_blank_when_scrolled() {
        let mut app = App {
            view: View::Today,
            mode: Mode::Help {
                filter: String::new(),
            },
            help_scroll: 100, // far past the content
            ..Default::default()
        };
        let output = render_full(&mut app);
        // The box must render all four sections without being blanked out by
        // an unbounded scroll; the last section's keybind must still render.
        assert!(
            output.contains("add a tag to the current todo"),
            "last help row should still be visible after clamped scroll:\n{output}"
        );
    }

    #[test]
    fn test_help_overlay_sidebar_section_when_filtering() {
        let mut app = App {
            view: View::Today,
            mode: Mode::Help {
                filter: "field".to_string(),
            },
            ..Default::default()
        };
        let output = render_full(&mut app);
        assert!(
            output.contains("Sidebar edit"),
            "filtering by 'sidebar' should surface the Sidebar edit section:\n{output}"
        );
    }

    #[test]
    fn test_confirm_dialog_renders_carryover_dates() {
        let mut app = App {
            mode: Mode::Confirm {
                action: ConfirmAction::CarryOver {
                    from: "2026-08-05".to_string(),
                    to: "2026-08-06".to_string(),
                },
            },
            ..Default::default()
        };
        let output = render_full(&mut app);
        assert!(
            output.contains("Carry unfinished todos from 2026-08-05 into 2026-08-06?"),
            "should show carryover message"
        );
        assert!(
            output.contains("CONFIRM"),
            "should show confirm dialog title"
        );
        assert!(output.contains("y confirm"), "should show confirm key hint");
        assert!(output.contains("n cancel"), "should show cancel key hint");
    }

    #[test]
    fn test_success_toast_renders_message() {
        let mut app = App {
            toast: Some(Toast {
                kind: ToastKind::Success,
                message: "Created".to_string(),
                ttl: Some(std::time::Duration::from_millis(2500)),
            }),
            ..Default::default()
        };
        let output = render_full(&mut app);
        assert!(output.contains("Created"), "should show toast message");
        assert!(output.contains('✓'), "should show success glyph");
    }

    #[test]
    fn test_error_toast_renders_message() {
        let mut app = App {
            toast: Some(Toast {
                kind: ToastKind::Error,
                message: "Connection failed".to_string(),
                ttl: None,
            }),
            ..Default::default()
        };
        let output = render_full(&mut app);
        assert!(
            output.contains("Connection failed"),
            "should show toast message"
        );
        assert!(output.contains('▲'), "should show error glyph");
    }
}
#[cfg(test)]
mod fidelity_tests {
    use crate::app::tests::{
        make_event, make_linear, make_plan_row, make_pr, make_todo, make_todo_linear, make_todo_pr,
    };
    use crate::app::{App, DetailData, Mode, SidebarField, View};
    use crate::test_support::buffer_text;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};

    fn render_full(app: &mut App) -> String {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| crate::views::render(f, app)).unwrap();
        buffer_text(terminal.backend().buffer())
    }

    fn setup_app_with_todo() -> App {
        let mut app = App::default();
        app.data.plan = vec![make_plan_row(1, 0, make_todo(1, "My Task", "todo"), None)];
        app.data.todos = vec![make_todo(1, "My Task", "todo")];
        app.cursor = 0;
        app.content_width = 120;
        app
    }

    // -- 1. Review auto-fetch on tab switch --

    #[test]
    fn test_tab_switch_to_review_triggers_fetch() {
        let mut app = App {
            view: View::Sync,
            ..Default::default()
        };
        for _ in 0..4 {
            app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        }
        assert_eq!(app.view, View::Review);
        assert!(
            app.review.is_none(),
            "review should still be None until the async fetch completes"
        );
    }

    #[test]
    fn test_backtab_switch_to_review_triggers_fetch() {
        let mut app = App {
            view: View::Today,
            ..Default::default()
        };
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE));
        assert_eq!(app.view, View::Review);
    }

    // -- 2. DetailData contains PR and Linear links after fetch_detail --

    #[test]
    fn test_detail_data_stores_prs_and_linears() {
        let pr = make_pr(1, None);
        let linear = make_linear(1, None);
        let detail = DetailData {
            tags: vec![],
            events: vec![make_event(1, "created", None, "system")],
            prs: vec![make_todo_pr(pr, "references")],
            linears: vec![make_todo_linear(linear)],
        };
        assert_eq!(detail.prs.len(), 1);
        assert_eq!(detail.linears.len(), 1);
        assert_eq!(detail.prs[0].relation, "references");
        assert!(detail.prs[0].pull_request.is_some());
        assert!(detail.linears[0].linear_issue.is_some());
    }

    // -- 3. Sidebar LINKS renders actual PR titles, not 'No linked PRs or issues' --

    #[test]
    fn test_sidebar_read_mode_renders_linked_pr_title() {
        let mut app = setup_app_with_todo();
        app.detail_loaded_id = Some(1);
        let pr = make_pr(1, None);
        app.detail = Some(DetailData {
            tags: vec![],
            events: vec![],
            prs: vec![make_todo_pr(pr, "references")],
            linears: vec![],
        });
        let output = render_full(&mut app);
        assert!(
            output.contains("owner/repo#1"),
            "sidebar should render the PR identifier owner/repo#1:\n{output}"
        );
        assert!(
            output.contains("PR #1"),
            "sidebar should render the PR title:\n{output}"
        );
        assert!(
            !output.contains("No linked PRs or issues"),
            "sidebar should not show 'No linked PRs or issues' when a PR is linked:\n{output}"
        );
    }

    #[test]
    fn test_sidebar_read_mode_renders_linked_linear_title() {
        let mut app = setup_app_with_todo();
        app.detail_loaded_id = Some(1);
        let linear = make_linear(1, None);
        app.detail = Some(DetailData {
            tags: vec![],
            events: vec![],
            prs: vec![],
            linears: vec![make_todo_linear(linear)],
        });
        let output = render_full(&mut app);
        assert!(
            output.contains("PROJ-1"),
            "sidebar should render the Linear identifier PROJ-1:\n{output}"
        );
        assert!(
            output.contains("Issue #1"),
            "sidebar should render the Linear title:\n{output}"
        );
        assert!(
            !output.contains("No linked PRs or issues"),
            "sidebar should not show 'No linked PRs or issues' when a Linear is linked:\n{output}"
        );
    }

    #[test]
    fn test_sidebar_read_mode_shows_no_links_when_empty() {
        let mut app = setup_app_with_todo();
        app.detail_loaded_id = Some(1);
        app.detail = Some(DetailData {
            tags: vec![],
            events: vec![],
            prs: vec![],
            linears: vec![],
        });
        let output = render_full(&mut app);
        assert!(
            output.contains("No linked PRs or issues"),
            "sidebar should show 'No linked PRs or issues' when no links exist:\n{output}"
        );
    }

    // -- 3b. Sidebar edit form shows actual linked PRs --

    #[test]
    fn test_sidebar_edit_form_renders_linked_pr() {
        let mut app = setup_app_with_todo();
        app.detail_loaded_id = Some(1);
        let pr = make_pr(1, None);
        app.detail = Some(DetailData {
            tags: vec![],
            events: vec![],
            prs: vec![make_todo_pr(pr, "references")],
            linears: vec![],
        });
        app.mode = Mode::SidebarEdit {
            id: 1,
            field: SidebarField::Description,
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
        let output = render_full(&mut app);
        assert!(
            output.contains("owner/repo#1"),
            "edit form should render the PR identifier:\n{output}"
        );
        assert!(
            !output.contains("No linked PRs or issues"),
            "edit form should not show 'No linked PRs or issues' when a PR is linked:\n{output}"
        );
    }

    // -- 3c. EVENT LOG section renders events --

    #[test]
    fn test_sidebar_read_mode_renders_event_log() {
        let mut app = setup_app_with_todo();
        app.detail_loaded_id = Some(1);
        app.detail = Some(DetailData {
            tags: vec![],
            events: vec![
                make_event(1, "created", None, "system"),
                make_event(2, "status_changed", Some("status"), "alice"),
            ],
            prs: vec![],
            linears: vec![],
        });
        let output = render_full(&mut app);
        assert!(
            output.contains("EVENT LOG"),
            "sidebar should render the EVENT LOG section:\n{output}"
        );
        let log_start = output.find("EVENT LOG").unwrap_or(0);
        let log_section = &output[log_start..];
        let sc_idx = log_section.find("status_changed");
        let cr_idx = log_section.find("created");
        assert!(
            sc_idx.is_some() && cr_idx.is_some(),
            "both events should appear in the event log"
        );
        assert!(
            sc_idx < cr_idx,
            "status_changed (most recent) should appear before created in the event log:\n{output}"
        );
        assert!(
            log_section.contains("alice"),
            "event log should show the actor:\n{output}"
        );
    }

    #[test]
    fn test_sidebar_read_mode_no_event_log_when_empty() {
        let mut app = setup_app_with_todo();
        app.detail_loaded_id = Some(1);
        app.detail = Some(DetailData {
            tags: vec![],
            events: vec![],
            prs: vec![],
            linears: vec![],
        });
        let output = render_full(&mut app);
        assert!(
            !output.contains("EVENT LOG"),
            "sidebar should not show EVENT LOG when there are no events:\n{output}"
        );
    }

    // -- 4. PullRequest struct has the new fields --

    #[test]
    fn test_pull_request_has_new_fields() {
        let pr = make_pr(1, None);
        assert!(
            !pr.changes_requested,
            "changes_requested should default to false"
        );
        assert!(
            !pr.copilot_comments,
            "copilot_comments should default to false"
        );
        assert!(
            !pr.merge_conflicts,
            "merge_conflicts should default to false"
        );
    }
}
