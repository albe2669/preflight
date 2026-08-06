//! Backlog view — all todos not on today's plan, grouped by status.
//! Full implementation pending.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use tokio::sync::mpsc;

use crate::app::App;
use crate::gql;

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    use ratatui::style::Style;
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{List, ListItem, ListState};

    let backlog = app.backlog();
    let mut items: Vec<ListItem> = Vec::new();
    let groups: [(&str, &str); 4] = [
        ("STARTED", "started"),
        ("BLOCKED", "blocked"),
        ("TODO", "todo"),
        ("DONE", "done"),
    ];

    let mut idx = 0;
    for (label, status) in groups {
        let rows: Vec<&gql::Todo> = backlog
            .iter()
            .filter(|t| t.status == status)
            .copied()
            .collect();
        if rows.is_empty() && status != "done" {
            continue;
        }
        let header = if status == "done" && !app.show_done {
            format!(
                "  {} · {} (collapsed — press D to expand)",
                label,
                rows.len()
            )
        } else {
            format!("  {} · {}", label, rows.len())
        };
        items.push(ListItem::new(Line::from(Span::styled(
            header,
            Style::default().fg(Palette::DIM),
        ))));

        if status == "done" && !app.show_done {
            continue;
        }

        for (_i, todo) in rows.iter().enumerate() {
            let is_cursor = idx == app.cursor;
            let (g, c) = crate::frame::status_glyph(&todo.status);
            let title_style = match todo.status.as_str() {
                "started" => Style::default()
                    .fg(Palette::TEXT)
                    .add_modifier(ratatui::style::Modifier::BOLD),
                "done" => Style::default().fg(Palette::DIM),
                _ => Style::default().fg(Palette::TEXT),
            };
            let marker = if is_cursor {
                Span::styled(
                    Glyph::CURSOR.to_string(),
                    Style::default().fg(Palette::ACCENT),
                )
            } else {
                Span::raw(" ")
            };
            let line = Line::from(vec![
                marker,
                Span::raw("     "),
                Span::styled(g.to_string(), Style::default().fg(c)),
                Span::raw(" "),
                Span::styled(todo.title.clone(), title_style),
            ]);
            items.push(ListItem::new(line).style(if is_cursor {
                Style::default().bg(Palette::ROW_HIGHLIGHT)
            } else {
                Style::default()
            }));
            idx += 1;
        }
        items.push(ListItem::new(Line::from("")));
        idx += 1;
    }

    let list = List::new(items)
        .style(Style::default().bg(Palette::BG).fg(Palette::TEXT))
        .highlight_style(Style::default().bg(Palette::ROW_HIGHLIGHT));
    let mut state = ListState::default();
    state.select(Some(app.cursor));
    f.render_stateful_widget(list, area, &mut state);
}

pub async fn handle_navigate(
    app: &mut App,
    key: KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
) -> anyhow::Result<bool> {
    match key {
        KeyCode::Char('j') | KeyCode::Down => {
            app.cursor = app.cursor.saturating_add(1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.cursor > 0 {
                app.cursor -= 1;
            }
        }
        KeyCode::Char('/') => {
            app.mode = crate::app::Mode::Search {
                input: String::new(),
            };
        }
        KeyCode::Char('t') => {
            // Plan the cursor todo for today.
            let backlog = app.backlog();
            if let Some(todo) = backlog.get(app.cursor) {
                let id = todo.id;
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    if c.plan_today(id).await.is_ok() {
                        let _ = t
                            .send(crate::AppMsg::Toast(
                                crate::app::ToastKind::Success,
                                "planned for today".into(),
                            ))
                            .await;
                        let _ = t.send(crate::AppMsg::Refresh).await;
                    }
                });
            }
        }
        KeyCode::Char('D') => {
            app.show_done = !app.show_done;
        }
        KeyCode::Char(' ') => {
            let backlog = app.backlog();
            if let Some(todo) = backlog.get(app.cursor) {
                let next = match todo.status.as_str() {
                    "todo" => "started",
                    "started" => "done",
                    "done" => "todo",
                    _ => "todo",
                };
                let id = todo.id;
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    if c.set_status(id, next, None).await.is_ok() {
                        let _ = t.send(crate::AppMsg::Refresh).await;
                    }
                });
            }
        }
        _ => {}
    }
    Ok(false)
}

use crate::theme::{Glyph, Palette};
