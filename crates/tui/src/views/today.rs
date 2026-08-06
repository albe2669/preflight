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

use crate::app::{App, ConfirmAction, Mode, ToastKind, View, matches_filter};
use crate::frame;
use crate::gql;
use crate::theme::Palette;

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
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
        frame::render_grouped_list(f, area, &[], app.cursor, no_match_items);
        return;
    }

    frame::render_grouped_list(f, area, &groups, app.cursor, trailing);
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
        KeyCode::Char('e') => {
            if let Some(td) = app.today_plan().get(app.cursor) {
                app.mode = Mode::InlineEdit {
                    id: td.id,
                    input: td.title.clone(),
                };
            }
        }
        KeyCode::Enter => {
            if let Some(td) = app.today_plan().get(app.cursor) {
                let id = td.id;
                app.mode = Mode::Detail { id };
                app.detail = None;
                fetch_detail(app, client, tx, id);
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
            app.view = View::Backlog;
            app.cursor = 0;
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

fn fetch_detail(_app: &mut App, client: &gql::Client, tx: &mpsc::Sender<crate::AppMsg>, id: i32) {
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
