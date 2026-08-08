//! Today view — the primary view.
//!
//! Plan rows for the logical date, grouped by state using the shared
//! grouped-list renderer. `↻` marks carried_over, the underlined
//! continuation line carries blocked_reason, and soft-unplanned rows drop
//! into their own dim group. The inline-create prompt lives at the bottom
//! of the plan, not in a modal.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;
use tokio::sync::mpsc;

use crate::app::{
    App, ConfirmAction, LinkKind, Mode, SidebarField, ToastKind, View, matches_filter,
};
use crate::frame;
use crate::gql;
use crate::theme::Palette;

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let carried_ids: std::collections::HashSet<i32> = app
        .data
        .plan
        .iter()
        .filter(|p| p.carried_over && p.removed_at.is_none())
        .filter_map(|p| p.todo.as_ref().map(|t| t.id))
        .collect();
    let plan = app.today_plan();
    let unplanned = app.today_unplanned();

    // Apply the search filter to the plan rows.
    let filter = match &app.mode {
        Mode::Search { input } => input.clone(),
        _ => String::new(),
    };
    let filtered: Vec<&gql::Todo> = plan
        .iter()
        .filter(|t| matches_filter(t, &filter))
        .copied()
        .collect();

    // Group by state using the shared renderer.
    let groups = frame::group_by_status(&filtered, app.show_done);

    // Trailing items: unplanned section + inline-create prompt.
    let mut trailing: Vec<ListItem> = Vec::new();

    if filter.is_empty() && !unplanned.is_empty() {
        trailing.push(ListItem::new(Line::from(Span::styled(
            format!("  UNPLANNED TODAY · {}", unplanned.len()),
            Style::default().fg(Palette::DIM),
        ))));
        for row in &unplanned {
            if let Some(td) = &row.todo {
                let (sglyph, scolor) = frame::status_glyph(&td.status);
                trailing.push(ListItem::new(Line::from(vec![
                    Span::raw("   –  "),
                    Span::styled(sglyph.to_string(), Style::default().fg(scolor)),
                    Span::raw(" "),
                    Span::styled(
                        td.title.clone(),
                        Style::default()
                            .fg(Palette::GHOST)
                            .add_modifier(Modifier::CROSSED_OUT),
                    ),
                ])));
            }
        }
        trailing.push(ListItem::new(Line::from("")));
    }

    // Inline create prompt.
    let create_line = match &app.mode {
        Mode::InlineCreate { input } => Line::from(vec![
            Span::styled("  add ", Style::default().fg(Palette::ACCENT)),
            Span::styled(
                if input.is_empty() {
                    "what else is happening today?".to_string()
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
                "what else is happening today?",
                Style::default().fg(Palette::GHOST),
            ),
        ]),
    };
    trailing.push(ListItem::new(create_line));

    // No-matches line when the filter yields zero rows.
    if !filter.is_empty() && filtered.is_empty() {
        let no_match = Line::from(Span::styled(
            format!("  no matches for \"{filter}\""),
            Style::default().fg(Palette::DIM),
        ));
        let mut no_match_items = vec![ListItem::new(no_match)];
        no_match_items.extend(trailing);
        frame::render_grouped_list(
            f,
            area,
            &[],
            filtered.get(app.cursor).copied(),
            &carried_ids,
            no_match_items,
        );
        return;
    }

    frame::render_grouped_list(
        f,
        area,
        &groups,
        filtered.get(app.cursor).copied(),
        &carried_ids,
        trailing,
    );
}

pub(crate) async fn handle_navigate(
    app: &mut App,
    key: KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
) -> anyhow::Result<bool> {
    let plan_count = app.today_plan().len();
    match key {
        KeyCode::Char('j') | KeyCode::Down if app.cursor + 1 < plan_count => {
            app.cursor += 1;
        }
        KeyCode::Char('k') | KeyCode::Up if app.cursor > 0 => {
            app.cursor -= 1;
        }
        KeyCode::Char('a') => {
            app.mode = Mode::InlineCreate {
                input: String::new(),
            };
        }
        KeyCode::Char('r') => {
            if let Some(td) = app.today_plan().get(app.cursor) {
                app.mode = Mode::InlineEdit {
                    id: td.id,
                    input: td.title.clone(),
                };
            }
        }
        KeyCode::Enter | KeyCode::Char('e') => {
            if app.content_width < 100 {
                app.set_error("terminal too narrow for sidebar");
            } else if let Some(td) = app.today_plan().get(app.cursor) {
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
                    scroll: 0,
                };
                super::fetch_detail(app, client, tx, id);
            }
        }
        KeyCode::Char('D') => {
            app.show_done = !app.show_done;
        }
        KeyCode::Char('x') => {
            if let Some(td) = app.today_plan().get(app.cursor) {
                app.mode = Mode::Confirm {
                    action: ConfirmAction::Unplan { id: td.id },
                };
            }
        }
        KeyCode::Char('J') => {
            if let Some(td) = app.today_plan().get(app.cursor) {
                app.mode = Mode::Reorder { source_id: td.id };
            }
        }
        KeyCode::Char(' ') | KeyCode::Char('s') => {
            if let Some(td) = app.today_plan().get(app.cursor) {
                let selection = crate::app::status_index(&td.status).unwrap_or(0);
                app.mode = Mode::StatusSelect {
                    id: td.id,
                    selection,
                    reason: None,
                };
            }
        }
        KeyCode::Char('/') => {
            app.mode = Mode::Search {
                input: String::new(),
            };
        }
        KeyCode::Char('c') => {
            // Carry over yesterday's unfinished.
            let from = (app.logical_date - chrono::Duration::days(1))
                .format("%Y-%m-%d")
                .to_string();
            let to = app.logical_date.format("%Y-%m-%d").to_string();
            app.mode = Mode::Confirm {
                action: ConfirmAction::CarryOver { from, to },
            };
        }
        KeyCode::Char('b') => {
            // Pick from backlog — switch to Backlog view.
            app.cursor = 0;
            app.clamp_cursor_to_active();
        }
        KeyCode::Char('R') => {
            // Daily review of yesterday.
            app.view = View::Review;
            app.review_date = app.logical_date - chrono::Duration::days(1);
            fetch_review(app, client, tx).await;
        }
        _ => {}
    }
    Ok(false)
}

async fn fetch_review(app: &mut App, client: &gql::Client, tx: &mpsc::Sender<crate::AppMsg>) {
    let date = app.review_date.format("%Y-%m-%d").to_string();
    let c = client.clone();
    let t = tx.clone();
    tokio::spawn(async move {
        match c.daily_review(&date).await {
            Ok(r) => {
                let _ = t.send(crate::AppMsg::DailyReview(r)).await;
            }
            Err(e) => {
                let _ = t
                    .send(crate::AppMsg::Toast(ToastKind::Error, e.to_string()))
                    .await;
            }
        }
    });
}

#[cfg(test)]
mod render_tests {
    use ratatui::{Terminal, backend::TestBackend};

    use crate::app::tests::{make_plan_row, make_todo};
    use crate::app::{App, Mode};
    use crate::test_support::buffer_text;
    use crate::theme::Glyph;

    fn render_today(app: &mut App) -> String {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                crate::views::today::render(f, app, area);
            })
            .unwrap();
        buffer_text(terminal.backend().buffer())
    }

    /// Return the title of the todo row that carries the cursor glyph (the
    /// highlighted row), if a unique one exists.
    fn cursor_row_title(buf: &str) -> Option<&str> {
        buf.lines().find_map(|l| {
            if let Some(idx) = l.find(Glyph::CURSOR) {
                let after = &l[idx + Glyph::CURSOR.len_utf8()..];
                // Strip leading spaces and any status glyph, then the space.
                let t = after.trim_start();
                let t = t.trim_start_matches([
                    Glyph::STATUS_TODO,
                    Glyph::STATUS_STARTED,
                    Glyph::STATUS_BLOCKED,
                    Glyph::STATUS_DONE,
                    Glyph::STATUS_CANCELLED,
                ]);
                Some(t.trim_start())
            } else {
                None
            }
        })
    }

    #[test]
    fn test_cursor_highlights_display_order_row() {
        // The plan is position-ordered [A(todo), B(started)], but today_plan()
        // is display-ordered by status -> [B(started), A(todo)] (status group
        // order). Grouped progress from start to cursor must match what's
        // actually on screen, so cursor 0 selects B — the top visual row.
        let mut app = App {
            cursor: 0,
            data: crate::app::AppData {
                plan: vec![
                    make_plan_row(1, 0, make_todo(1, "PlanA", "todo"), None),
                    make_plan_row(2, 1, make_todo(2, "PlanB", "started"), None),
                ],
                todos: vec![
                    make_todo(1, "PlanA", "todo"),
                    make_todo(2, "PlanB", "started"),
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        let buf = render_today(&mut app);
        let title = cursor_row_title(&buf).unwrap_or("<none>");
        assert_eq!(
            title, "PlanB",
            "cursor 0 should highlight the display-order top row 'PlanB':\n{buf}"
        );
    }
    #[test]
    fn test_cursor_highlights_correct_row_across_groups() {
        // must highlight the matching todo row despite the interleaved group
        // headers, blank separators, and the trailing inline-create prompt.
        let mut app = App {
            cursor: 1,
            data: crate::app::AppData {
                plan: vec![
                    make_plan_row(1, 0, make_todo(1, "Started", "started"), None),
                    make_plan_row(2, 1, make_todo(2, "TodoTwo", "todo"), None),
                    make_plan_row(3, 2, make_todo(3, "TodoThree", "todo"), None),
                ],
                todos: vec![
                    make_todo(1, "Started", "started"),
                    make_todo(2, "TodoTwo", "todo"),
                    make_todo(3, "TodoThree", "todo"),
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        let buf = render_today(&mut app);
        let title = cursor_row_title(&buf).unwrap_or("<none>");
        assert_eq!(
            title, "TodoTwo",
            "cursor 1 should highlight today_plan()[1], got '{title}':\n{buf}"
        );
    }

    #[test]
    fn test_today_renders_plan_rows_with_headers() {
        let mut app = App::default();
        app.data.plan = vec![
            make_plan_row(1, 0, make_todo(1, "Task A", "todo"), None),
            make_plan_row(2, 1, make_todo(2, "Task B", "started"), None),
        ];
        app.data.todos = vec![
            make_todo(1, "Task A", "todo"),
            make_todo(2, "Task B", "started"),
        ];

        let buf = render_today(&mut app);

        // Both titles appear in the rendered output.
        assert!(
            buf.contains("Task A"),
            "expected 'Task A' in buffer:\n{buf}"
        );
        assert!(
            buf.contains("Task B"),
            "expected 'Task B' in buffer:\n{buf}"
        );

        // Group headers render with count.
        assert!(
            buf.contains("STARTED · 1"),
            "expected 'STARTED · 1' header:\n{buf}"
        );
        assert!(
            buf.contains("TODO · 1"),
            "expected 'TODO · 1' header:\n{buf}"
        );

        // Status glyphs: ○ for todo, ◐ for started.
        assert!(
            buf.contains(Glyph::STATUS_TODO),
            "expected todo glyph '○' in buffer:\n{buf}"
        );
        assert!(
            buf.contains(Glyph::STATUS_STARTED),
            "expected started glyph '◐' in buffer:\n{buf}"
        );
    }

    #[test]
    fn test_today_renders_carried_over_glyph() {
        let mut app = App::default();
        let mut row = make_plan_row(1, 0, make_todo(1, "Carried task", "todo"), None);
        row.carried_over = true;
        app.data.plan = vec![row];
        app.data.todos = vec![make_todo(1, "Carried task", "todo")];

        let buf = render_today(&mut app);

        assert!(
            buf.contains(Glyph::CARRIED),
            "expected carried-over glyph '↻' in buffer:\n{buf}"
        );
        assert!(
            buf.contains("Carried task"),
            "expected 'Carried task' in buffer:\n{buf}"
        );
    }

    #[test]
    fn test_today_renders_unplanned_section() {
        let mut app = App::default();
        // A plan row with removed_at set is "unplanned" — it was intended for
        // today but removed from the active plan.
        app.data.plan = vec![make_plan_row(
            1,
            0,
            make_todo(1, "Dropped task", "todo"),
            Some("2026-08-05"),
        )];
        app.data.todos = vec![make_todo(1, "Dropped task", "todo")];

        let buf = render_today(&mut app);

        assert!(
            buf.contains("UNPLANNED TODAY"),
            "expected 'UNPLANNED TODAY' section header:\n{buf}"
        );
        assert!(
            buf.contains("Dropped task"),
            "expected 'Dropped task' title in unplanned section:\n{buf}"
        );
    }

    #[test]
    fn test_today_renders_inline_create_prompt_in_navigate_mode() {
        let mut app = App {
            mode: Mode::Navigate,
            ..Default::default()
        };
        // Empty plan — no active rows.

        let buf = render_today(&mut app);

        assert!(
            buf.contains("add"),
            "expected 'add' in inline create prompt:\n{buf}"
        );
        assert!(
            buf.contains("what else is happening today?"),
            "expected inline create placeholder text:\n{buf}"
        );
    }

    #[test]
    fn test_today_cursor_at_index_1_highlights_second_row() {
        let mut app = App {
            cursor: 1,
            data: crate::app::AppData {
                plan: vec![
                    make_plan_row(1, 0, make_todo(1, "First", "todo"), None),
                    make_plan_row(2, 1, make_todo(2, "Second", "todo"), None),
                    make_plan_row(3, 2, make_todo(3, "Third", "todo"), None),
                ],
                todos: vec![
                    make_todo(1, "First", "todo"),
                    make_todo(2, "Second", "todo"),
                    make_todo(3, "Third", "todo"),
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        let buf = render_today(&mut app);

        // The cursor glyph should appear in the rendered output.
        assert!(
            buf.contains(Glyph::CURSOR),
            "expected cursor glyph '▌' in buffer:\n{buf}"
        );
    }
}
