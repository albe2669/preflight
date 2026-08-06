//! Todo detail — overlay pane with full description, tags, links, timeline.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use tokio::sync::mpsc;

use crate::app::{App, Mode};
use crate::frame;
use crate::gql;
use crate::theme::{Glyph, Palette};

pub fn render_overlay(f: &mut Frame, app: &mut App, area: Rect) {
    let Mode::Detail { id } = app.mode else {
        return;
    };
    let todo = app.data.todos.iter().find(|t| t.id == id);
    let Some(todo) = todo else { return };

    let width = 70.min(area.width);
    let height = 28.min(area.height);
    let x = area.x + (area.width - width) / 2;
    let y = area.y + (area.height - height) / 2;
    let rect = Rect::new(x, y, width, height);

    f.render_widget(Clear, rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Palette::ACCENT))
        .title(Span::styled(
            format!(" TODO #{} ", id),
            Style::default().fg(Palette::ACCENT),
        ));
    f.render_widget(block, rect);

    let inner = Rect::new(x + 1, y + 1, width - 2, height - 2);
    let chunks = Layout::vertical([
        Constraint::Length(2), // title
        Constraint::Length(2), // status + tags
        Constraint::Length(1), // blank
        Constraint::Length(6), // description
        Constraint::Length(1), // blank
        Constraint::Min(0),    // timeline
    ])
    .split(inner);

    // Title.
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
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(g.to_string(), Style::default().fg(c)),
            Span::raw(" "),
            Span::styled(todo.title.clone(), title_style),
        ])),
        chunks[0],
    );

    // Status + meta.
    let created = todo.created_at.split(' ').next().unwrap_or("");
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("status  ", Style::default().fg(Palette::DIM)),
            Span::styled(todo.status.clone(), Style::default().fg(c)),
            Span::styled(
                format!("   created {} ", created),
                Style::default().fg(Palette::DIM),
            ),
        ])),
        chunks[1],
    );

    // Description.
    let desc = todo.description.as_deref().unwrap_or("—");
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "DESCRIPTION",
                Style::default().fg(Palette::DIM),
            )),
            Line::from(desc.to_string()),
        ])
        .wrap(Wrap { trim: true }),
        chunks[3],
    );

    // Timeline.
    let detail = app.detail.as_ref();
    let events = detail.map(|d| d.events.as_slice()).unwrap_or(&[]);
    let mut items: Vec<ListItem> = Vec::new();
    items.push(ListItem::new(Line::from(Span::styled(
        "TIMELINE  newest last",
        Style::default().fg(Palette::DIM),
    ))));
    for ev in events {
        let (actor_glyph, actor_color) = match ev.actor.as_str() {
            "user" => (Glyph::ACTOR_USER, Palette::TEXT),
            "sync" => (Glyph::ACTOR_SYNC, Palette::DIM),
            "system" => (Glyph::ACTOR_SYSTEM, Palette::DIM),
            _ => ('?', Palette::DIM),
        };
        let time = ev.occurred_at.split(' ').nth(1).unwrap_or("");
        let line = Line::from(vec![
            Span::styled(format!("{} ", time), Style::default().fg(Palette::GHOST)),
            Span::styled(actor_glyph.to_string(), Style::default().fg(actor_color)),
            Span::raw(" "),
            Span::styled(
                ev.kind.clone(),
                if ev.actor == "system" {
                    Style::default()
                        .fg(Palette::DIM)
                        .add_modifier(Modifier::ITALIC)
                } else if ev.actor == "sync" {
                    Style::default().fg(Palette::DIM)
                } else {
                    Style::default().fg(Palette::TEXT)
                },
            ),
            Span::styled(
                format!(" {}", ev.new_value.as_deref().unwrap_or("")),
                Style::default().fg(Palette::DIM),
            ),
        ]);
        items.push(ListItem::new(line));
    }
    f.render_widget(
        List::new(items).style(Style::default().bg(Palette::BG).fg(Palette::TEXT)),
        chunks[5],
    );
}

pub(crate) async fn handle(
    app: &mut App,
    key: KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
    id: i32,
) -> anyhow::Result<bool> {
    match key {
        KeyCode::Esc => {
            app.mode = Mode::Navigate;
        }
        KeyCode::Char('e') => {
            if let Some(todo) = app.data.todos.iter().find(|t| t.id == id) {
                app.mode = Mode::InlineEdit {
                    id,
                    input: todo.title.clone(),
                };
            }
        }
        KeyCode::Char(' ') => {
            if let Some(todo) = app.data.todos.iter().find(|t| t.id == id) {
                let next = match todo.status.as_str() {
                    "todo" => "started",
                    "started" => "done",
                    "done" => "todo",
                    _ => "todo",
                };
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
