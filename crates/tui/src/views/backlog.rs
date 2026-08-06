//! Backlog view — all todos not on today's plan, grouped by state using
//! the shared grouped-list renderer.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;
use tokio::sync::mpsc;

use crate::app::{App, Mode, ToastKind, matches_filter};
use crate::frame;
use crate::gql;
use crate::theme::Palette;

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let backlog = app.backlog();

    // Apply the search filter.
    let filter = match &app.mode {
        Mode::Search { input } => input.clone(),
        _ => String::new(),
    };
    let filtered: Vec<&gql::Todo> = backlog
        .iter()
        .filter(|t| matches_filter(t, &filter))
        .copied()
        .collect();

    let groups = frame::group_by_status(&filtered, app.show_done);

    // No-matches line when the filter yields zero rows.
    if !filter.is_empty() && filtered.is_empty() {
        let no_match = ListItem::new(Line::from(Span::styled(
            format!("  no matches for \"{filter}\""),
            Style::default().fg(Palette::DIM),
        )));
        frame::render_grouped_list(
            f,
            area,
            &[],
            app.cursor,
            &std::collections::HashSet::new(),
            vec![no_match],
        );
        return;
    }

    frame::render_grouped_list(
        f,
        area,
        &groups,
        app.cursor,
        &std::collections::HashSet::new(),
        vec![],
    );
}

pub(crate) async fn handle_navigate(
    app: &mut App,
    key: KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
) -> anyhow::Result<bool> {
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
                                ToastKind::Success,
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
        KeyCode::Char('a') => {
            app.mode = Mode::InlineCreate {
                input: String::new(),
            };
        }
        KeyCode::Enter => {
            if let Some(td) = app.backlog().get(app.cursor) {
                let id = td.id;
                app.mode = Mode::Detail { id };
                app.detail = None;
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    match c.todo_events(id).await {
                        Ok(events) => {
                            let _ = t.send(crate::AppMsg::DetailData(id, events)).await;
                        }
                        Err(e) => {
                            let _ = t
                                .send(crate::AppMsg::Toast(ToastKind::Error, e.to_string()))
                                .await;
                        }
                    }
                });
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
    Ok(false)
}
