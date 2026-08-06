//! Today view — the primary view.
//!
//! Ordered plan rows for the logical date. `↻` marks carried_over, the
//! underlined continuation line carries blocked_reason, and soft-unplanned
//! rows drop into their own dim group. The inline-create prompt lives at the
//! bottom of the plan, not in a modal.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};
use tokio::sync::mpsc;

use crate::app::{App, ConfirmAction, Mode, ToastKind, View};
use crate::frame;
use crate::gql;
use crate::theme::{Glyph, Palette};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let plan = app.today_plan();
    let unplanned = app.today_unplanned();

    let mut items: Vec<ListItem> = Vec::new();

    // Section header: PLAN · n rows · m carried.
    let carried_count = app
        .data
        .plan
        .iter()
        .filter(|p| p.carried_over && p.removed_at.is_none())
        .count();
    let header = format!(
        "  PLAN · {} rows · {} carried over",
        plan.len(),
        carried_count
    );
    items.push(ListItem::new(Line::from(Span::styled(
        header,
        Style::default().fg(Palette::DIM),
    ))));

    // Plan rows.
    let active_rows: Vec<&crate::gql::PlanRow> = app
        .data
        .plan
        .iter()
        .filter(|p| p.removed_at.is_none())
        .collect();

    // Skip rows where the todo relation didn't resolve.
    let active_rows: Vec<(&crate::gql::PlanRow, &crate::gql::Todo)> = active_rows
        .iter()
        .filter_map(|p| p.todo.as_ref().map(|t| (*p, t)))
        .collect();
    for (i, (row, td)) in active_rows.iter().enumerate() {
        let is_cursor = i == app.cursor
            && matches!(
                app.mode,
                Mode::Navigate | Mode::Reorder { .. } | Mode::InlineEdit { .. }
            );
        let is_editing = matches!(&app.mode, Mode::InlineEdit { id, .. } if *id == td.id);
        let is_reorder = matches!(&app.mode, Mode::Reorder { source_id } if *source_id == td.id);

        if is_editing {
            // Inline edit field.
            let Mode::InlineEdit { input, .. } = &app.mode else {
                continue;
            };
            let line = Line::from(vec![
                Span::styled(
                    Glyph::CURSOR.to_string(),
                    Style::default().fg(Palette::ACCENT),
                ),
                Span::raw(format!("  {}  ", i + 1)),
                Span::styled(
                    format!(" {input}▏"),
                    Style::default()
                        .bg(Palette::ROW_HIGHLIGHT)
                        .add_modifier(Modifier::UNDERLINED),
                ),
            ]);
            items.push(ListItem::new(line));
            items.push(ListItem::new(Line::from(Span::styled(
                "          enter save · esc revert · ^u clear".to_string(),
                Style::default().fg(Palette::GHOST),
            ))));
            continue;
        }

        let marker = if is_reorder {
            Span::styled(
                Glyph::REORDER_GRIP.to_string(),
                Style::default().fg(Palette::ACCENT),
            )
        } else if is_cursor {
            Span::styled(
                Glyph::CURSOR.to_string(),
                Style::default().fg(Palette::ACCENT),
            )
        } else {
            Span::raw(" ")
        };

        let (sglyph, scolor) = frame::status_glyph(&td.status);
        let title_style = match td.status.as_str() {
            "started" => Style::default()
                .fg(Palette::TEXT)
                .add_modifier(Modifier::BOLD),
            "done" => Style::default().fg(Palette::DIM),
            "cancelled" => Style::default()
                .fg(Palette::CANCELLED)
                .add_modifier(Modifier::CROSSED_OUT),
            _ => Style::default().fg(Palette::TEXT),
        };

        let mut spans = vec![marker, Span::raw(format!("  {}  ", i + 1))];
        if row.carried_over {
            spans.push(Span::styled(
                Glyph::CARRIED.to_string(),
                Style::default().fg(Palette::DIM),
            ));
            spans.push(Span::raw(" "));
        } else {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            sglyph.to_string(),
            Style::default().fg(scolor),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(td.title.clone(), title_style));

        let line = Line::from(spans);
        if is_cursor {
            items.push(ListItem::new(line).style(Style::default().bg(Palette::ROW_HIGHLIGHT)));
        } else {
            items.push(ListItem::new(line));
        }

        // Blocked reason continuation line.
        if td.status == "blocked" {
            if let Some(ref reason) = td.blocked_reason {
                items.push(ListItem::new(frame::blocked_reason_line(reason)));
            }
        }
    }

    // Unplanned section.
    if !unplanned.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            format!("  UNPLANNED TODAY · {}", unplanned.len()),
            Style::default().fg(Palette::DIM),
        ))));
        for row in unplanned {
            if let Some(td) = &row.todo {
                let (sglyph, scolor) = frame::status_glyph(&td.status);
                items.push(ListItem::new(Line::from(vec![
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
    items.push(ListItem::new(create_line));

    let list = List::new(items)
        .style(Style::default().bg(Palette::BG).fg(Palette::TEXT))
        .highlight_style(Style::default().bg(Palette::ROW_HIGHLIGHT));

    let mut state = ListState::default();
    if !active_rows.is_empty() && app.cursor < active_rows.len() {
        state.select(Some(app.cursor));
    }
    f.render_stateful_widget(list, area, &mut state);
}

pub(crate) async fn handle_navigate(
    app: &mut App,
    key: KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
) -> anyhow::Result<bool> {
    let plan_count = app.today_plan().len();
    match key {
        KeyCode::Char('j') | KeyCode::Down
            if app.cursor + 1 < plan_count => {
                app.cursor += 1;
            }
        KeyCode::Char('k') | KeyCode::Up
            if app.cursor > 0 => {
                app.cursor -= 1;
            }
        KeyCode::Char('a') => {
            app.mode = Mode::InlineCreate {
                input: String::new(),
            };
        }
        KeyCode::Char('e') | KeyCode::Enter => {
            if let Some(td) = app.today_plan().get(app.cursor) {
                app.mode = Mode::InlineEdit {
                    id: td.id,
                    input: td.title.clone(),
                };
            }
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
        KeyCode::Char(' ') => {
            // Cycle status: todo -> started -> blocked -> done -> cancelled -> todo
            if let Some(td) = app.today_plan().get(app.cursor) {
                let next = match td.status.as_str() {
                    "todo" => "started",
                    "started" => "done",
                    "done" => "todo",
                    _ => "todo",
                };
                let id = td.id;
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    if c.set_status(id, next, None).await.is_ok() {
                        let _ = t.send(crate::AppMsg::Refresh).await;
                    }
                });
            }
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
