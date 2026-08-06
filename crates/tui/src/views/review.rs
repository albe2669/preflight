//! Review view — a day sheet read from the event log.
//! Four fixed lists: planned, touched, completed, carried over.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};
use tokio::sync::mpsc;

use crate::app::{App, ToastKind};
use crate::frame;
use crate::gql;
use crate::theme::{Glyph, Palette};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let review = match &app.review {
        Some(r) => r,
        None => {
            let p = Paragraph::new(Line::from(Span::styled(
                format!(
                    "  {} loading review for {}…",
                    Glyph::SPINNER[2],
                    app.review_date.format("%Y-%m-%d")
                ),
                Style::default().fg(Palette::DIM),
            )));
            f.render_widget(p, area);
            return;
        }
    };

    // Summary line.
    let summary = format!(
        "  logical day {} 04:00 → {} 04:00 · {} planned · {} touched · {} completed · {} carried",
        review.date,
        review.date,
        review.planned.len(),
        review.touched.len(),
        review.completed.len(),
        review.carried_over.len(),
    );

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1), // blank
        Constraint::Min(0),    // two columns
        Constraint::Length(1), // footer
    ])
    .split(area);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            summary,
            Style::default().fg(Palette::DIM),
        ))),
        chunks[0],
    );

    // Two-column layout: left = planned + completed, right = touched + carried.
    let cols = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[2]);

    let left = list_panel("PLANNED", &review.planned, false);
    f.render_widget(left, cols[0]);

    let right = list_panel("TOUCHED", &review.touched, true);
    f.render_widget(right, cols[1]);

    // Footer: read-only notice.
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  ─ read-only · this day is closed ─",
            Style::default().fg(Palette::GHOST),
        ))),
        chunks[3],
    );
}

fn list_panel<'a>(title: &str, todos: &[gql::Todo], _is_touched: bool) -> List<'a> {
    let mut items: Vec<ListItem> = Vec::new();
    items.push(ListItem::new(Line::from(Span::styled(
        format!("  {} · {}", title, todos.len()),
        Style::default().fg(Palette::DIM),
    ))));
    for todo in todos {
        let (g, c) = frame::status_glyph(&todo.status);
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
        items.push(ListItem::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(g.to_string(), Style::default().fg(c)),
            Span::raw(" "),
            Span::styled(todo.title.clone(), title_style),
        ])));
    }
    List::new(items).style(Style::default().bg(Palette::BG).fg(Palette::TEXT))
}

pub async fn handle_navigate(
    app: &mut App,
    key: KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
) -> anyhow::Result<bool> {
    match key {
        KeyCode::Char('h') | KeyCode::Left => {
            app.review_date = app.review_date - chrono::Duration::days(1);
            fetch_review(app, client, tx).await;
        }
        KeyCode::Char('l') | KeyCode::Right => {
            app.review_date = app.review_date + chrono::Duration::days(1);
            fetch_review(app, client, tx).await;
        }
        KeyCode::Char('g') => {
            // Go to today's review.
            app.review_date = app.logical_date;
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
