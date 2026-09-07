//! Backlog view — all todos not on today's plan, grouped by state using
//! the shared grouped-list renderer.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;

use crate::app::{App, LinkKind, Mode, SidebarField, matches_filter};
use crate::gql;
use crate::theme::Palette;
use crate::widgets;

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let backlog = app.backlog();

    // Apply the search filter.
    let filter = match &app.mode {
        Mode::Search { input, .. } => input.clone(),
        _ => String::new(),
    };
    let filtered: Vec<&gql::Todo> = backlog
        .iter()
        .filter(|t| matches_filter(t, &filter))
        .copied()
        .collect();

    let groups = widgets::group_by_status(&filtered, app.show_done);

    // Inline-create prompt (a add). Matches Today's layout so the prompt is
    // visible at the bottom of the backlog list.
    let create_line = match &app.mode {
        Mode::InlineCreate { input, .. } => Line::from(vec![
            Span::styled("  add ", Style::default().fg(Palette::ACCENT)),
            Span::styled(
                if input.is_empty() {
                    "what else is on your mind?".to_string()
                } else {
                    format!("{input}▏")
                },
                Style::default()
                    .bg(Palette::ROW_HIGHLIGHT)
                    .fg(if input.is_empty() {
                        Palette::GHOST
                    } else {
                        Palette::TEXT
                    })
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ]),
        _ => Line::from(vec![
            Span::styled("  add ", Style::default().fg(Palette::ACCENT)),
            Span::styled(
                "what else is on your mind?",
                Style::default().fg(Palette::GHOST),
            ),
        ]),
    };
    let trailing = vec![ListItem::new(create_line)];

    // No-matches line when the filter yields zero rows.
    if !filter.is_empty() && filtered.is_empty() {
        let no_match = ListItem::new(Line::from(Span::styled(
            format!("  no matches for \"{filter}\""),
            Style::default().fg(Palette::DIM),
        )));
        let mut items = vec![no_match];
        items.extend(trailing);
        widgets::render_grouped_list(
            f,
            area,
            &[],
            filtered.get(app.cursor).copied(),
            &std::collections::HashSet::new(),
            items,
        );
        return;
    }

    widgets::render_grouped_list(
        f,
        area,
        &groups,
        filtered.get(app.cursor).copied(),
        &std::collections::HashSet::new(),
        trailing,
    );
}

pub(crate) fn handle_navigate(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('j') | KeyCode::Down => {
            app.cursor = app.cursor.saturating_add(1);
        }
        KeyCode::Char('k') | KeyCode::Up if app.cursor > 0 => {
            app.cursor -= 1;
        }
        KeyCode::Char('/') => {
            app.mode = Mode::Search {
                input: String::new(),
                caret: 0,
            };
        }
        KeyCode::Char('t') => {
            let backlog = app.backlog();
            if let Some(todo) = backlog.get(app.cursor) {
                app.spawn_plan_today(todo.id);
            }
        }
        KeyCode::Char('D') => {
            app.show_done = !app.show_done;
        }
        KeyCode::Char('a') => {
            if app.content_width < 100 {
                app.set_error("terminal too narrow for sidebar");
            } else {
                app.mode = Mode::SidebarAdd {
                    field: SidebarField::Title,
                    input_active: false,
                    title_input: String::new(),
                    title_caret: 0,
                    desc_input: String::new(),
                    desc_caret: 0,
                    desc_scroll: 0,
                    tag_input: String::new(),
                    tag_caret: 0,
                    link_kind: LinkKind::Pr,
                    link_selection: 0,
                    attaching: false,
                    link_search: String::new(),
                    pending_link_pr: Vec::new(),
                    scroll: 0,
                };
            }
        }
        KeyCode::Char('r') => {
            if let Some(td) = app.backlog().get(app.cursor) {
                app.mode = Mode::InlineEdit {
                    id: td.id,
                    input: td.title.clone(),
                    caret: td.title.len(),
                };
            }
        }
        KeyCode::Enter | KeyCode::Char('e') => {
            if let Some(td) = app.backlog().get(app.cursor) {
                let id = td.id;
                let desc = td.description.clone().unwrap_or_default();
                app.mode = Mode::SidebarEdit {
                    id,
                    field: SidebarField::Description,
                    input_active: false,
                    title_input: td.title.clone(),
                    title_caret: td.title.len(),
                    desc_input: desc,
                    desc_caret: 0,
                    desc_scroll: 0,
                    tag_input: String::new(),
                    tag_caret: 0,
                    link_kind: LinkKind::Pr,
                    link_selection: 0,
                    attaching: false,
                    link_search: String::new(),
                    scroll: 0,
                };
                app.spawn_fetch_detail(id);
            }
        }
        KeyCode::Char(' ') | KeyCode::Char('s') => {
            let backlog = app.backlog();
            if let Some(todo) = backlog.get(app.cursor) {
                let selection = crate::app::status_index(&todo.status).unwrap_or(0);
                app.mode = Mode::StatusSelect {
                    id: todo.id,
                    selection,
                    reason: None,
                };
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod render_tests {
    use ratatui::{Terminal, backend::TestBackend};

    use crate::app::App;
    use crate::app::tests::make_todo;
    use crate::test_support::buffer_text;

    fn render_backlog(app: &mut App) -> String {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                crate::views::backlog::render(f, app, area);
            })
            .unwrap();
        buffer_text(terminal.backend().buffer())
    }

    #[test]
    fn test_backlog_groups_todos_by_status_with_counts() {
        let mut app = App::default();
        app.data.todos = vec![
            make_todo(1, "Active task", "started"),
            make_todo(2, "Blocked task", "blocked"),
            make_todo(3, "Todo task", "todo"),
            make_todo(4, "Done task", "done"),
        ];
        app.show_done = true;

        let output = render_backlog(&mut app);

        assert!(
            output.contains("STARTED · 1"),
            "STARTED group header missing"
        );
        assert!(
            output.contains("BLOCKED · 1"),
            "BLOCKED group header missing"
        );
        assert!(output.contains("TODO · 1"), "TODO group header missing");
        assert!(output.contains("DONE · 1"), "DONE group header missing");
    }

    #[test]
    fn test_backlog_done_hidden_when_show_done_false() {
        let mut app = App::default();
        app.data.todos = vec![
            make_todo(1, "Active task", "started"),
            make_todo(2, "Blocked task", "blocked"),
            make_todo(3, "Todo task", "todo"),
            make_todo(4, "Done task", "done"),
        ];
        app.show_done = false;

        let output = render_backlog(&mut app);

        assert!(
            output.contains("DONE · 1 (collapsed — press D to expand)"),
            "DONE group should show collapsed header"
        );
        assert!(
            !output.contains("Done task"),
            "Done todo title should NOT appear when done group is collapsed"
        );
    }

    #[test]
    fn test_backlog_renders_inline_create_prompt_in_navigate_mode() {
        let mut app = App::default();
        let output = render_backlog(&mut app);
        assert!(
            output.contains("what else is on your mind?"),
            "backlog should show the inline-create placeholder:\n{output}"
        );
    }

    #[test]
    fn test_backlog_renders_active_inline_create_input() {
        let mut app = App {
            mode: crate::app::Mode::InlineCreate {
                input: "new backlog task".to_string(),
                caret: 16,
            },
            ..Default::default()
        };
        let output = render_backlog(&mut app);
        assert!(
            output.contains("new backlog task"),
            "backlog should render the active create input:\n{output}"
        );
    }
}
