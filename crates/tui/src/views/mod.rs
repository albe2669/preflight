//! View rendering and key handling.
//!
//! Each view renders into the content area inside the frame chrome. Key
//! handlers are async (they issue GraphQL mutations then trigger a refetch).

pub mod backlog;
pub mod detail;
pub mod inbox;
pub mod review;
pub mod sync;
pub mod today;

use ratatui::Frame;
use tokio::sync::mpsc;

use crate::app::{App, ConfirmAction, Mode, ToastKind, View};
use crate::frame;
use crate::gql;
use crate::theme::Palette;

/// Top-level render: frame chrome + the active view's content + overlays.
pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let content = frame::render_frame(f, app, area);

    match app.view {
        View::Today => today::render(f, app, content),
        View::Backlog => backlog::render(f, app, content),
        View::Inbox => inbox::render(f, app, content),
        View::Review => review::render(f, app, content),
        View::Sync => sync::render(f, app, content),
    }

    // Overlays: detail pane and confirm dialog and toast.
    if let Mode::Detail { .. } = &app.mode {
        detail::render_overlay(f, app, area);
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
    let width = 50;
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

// ---- Key handlers dispatched by lib.rs ----

pub async fn handle_navigate(
    app: &mut App,
    key: ratatui::crossterm::event::KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
) -> anyhow::Result<bool> {
    match app.view {
        View::Today => today::handle_navigate(app, key, client, tx).await,
        View::Backlog => backlog::handle_navigate(app, key, client, tx).await,
        View::Inbox => inbox::handle_navigate(app, key, client, tx).await,
        View::Review => review::handle_navigate(app, key, client, tx).await,
        View::Sync => sync::handle_navigate(app, key, client, tx).await,
    }
}

pub async fn handle_inline_create(
    app: &mut App,
    key: ratatui::crossterm::event::KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
    _input: &str,
) -> anyhow::Result<bool> {
    use ratatui::crossterm::event::KeyCode::*;
    match key {
        Char(c) if c.is_alphanumeric() || c == ' ' || c == '#' || c == '-' || c == '_' => {
            let Mode::InlineCreate { input } = &mut app.mode else {
                return Ok(false);
            };
            input.push(c);
            Ok(false)
        }
        Backspace => {
            let Mode::InlineCreate { input } = &mut app.mode else {
                return Ok(false);
            };
            input.pop();
            Ok(false)
        }
        Enter => {
            let Mode::InlineCreate { input } = &app.mode else {
                return Ok(false);
            };
            let title = input.trim().to_string();
            if title.is_empty() {
                app.set_error("a title is required");
                return Ok(false);
            }
            let c = client.clone();
            let t = tx.clone();
            let title2 = title.clone();
            tokio::spawn(async move {
                match c.create_todo(&title2).await {
                    Ok(_) => {
                        let _ = t
                            .send(crate::AppMsg::Toast(
                                ToastKind::Success,
                                "todo created".into(),
                            ))
                            .await;
                        let _ = t.send(crate::AppMsg::Refresh).await;
                    }
                    Err(e) => {
                        let _ = t
                            .send(crate::AppMsg::Toast(ToastKind::Error, e.to_string()))
                            .await;
                    }
                }
            });
            app.mode = Mode::Navigate;
            Ok(false)
        }
        _ => Ok(false),
    }
}

pub async fn handle_inline_edit(
    app: &mut App,
    key: ratatui::crossterm::event::KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
    id: i32,
    _input: &str,
) -> anyhow::Result<bool> {
    use ratatui::crossterm::event::KeyCode::*;
    match key {
        Char(c) if c.is_alphanumeric() || c == ' ' || c == '#' || c == '-' || c == '_' => {
            if let Mode::InlineEdit { input, .. } = &mut app.mode {
                input.push(c);
            }
            Ok(false)
        }
        Backspace => {
            if let Mode::InlineEdit { input, .. } = &mut app.mode {
                input.pop();
            }
            Ok(false)
        }
        Enter => {
            let title = if let Mode::InlineEdit { input, .. } = &app.mode {
                input.trim().to_string()
            } else {
                return Ok(false);
            };
            if title.is_empty() {
                app.set_error("a title is required");
                return Ok(false);
            }
            let c = client.clone();
            let t = tx.clone();
            let title2 = title.clone();
            tokio::spawn(async move {
                match c.update_todo(id, Some(&title2)).await {
                    Ok(_) => {
                        let _ = t
                            .send(crate::AppMsg::Toast(ToastKind::Success, "saved".into()))
                            .await;
                        let _ = t.send(crate::AppMsg::Refresh).await;
                    }
                    Err(e) => {
                        let _ = t
                            .send(crate::AppMsg::Toast(ToastKind::Error, e.to_string()))
                            .await;
                    }
                }
            });
            app.mode = Mode::Navigate;
            Ok(false)
        }
        _ => Ok(false),
    }
}

pub fn handle_search(
    app: &mut App,
    key: ratatui::crossterm::event::KeyCode,
    _input: &str,
) -> anyhow::Result<bool> {
    use ratatui::crossterm::event::KeyCode::*;
    match key {
        Char(c) if c.is_alphanumeric() || c == ' ' || c == '#' || c == '-' || c == '_' => {
            if let Mode::Search { input } = &mut app.mode {
                input.push(c);
            }
        }
        Backspace => {
            if let Mode::Search { input } = &mut app.mode {
                input.pop();
            }
        }
        _ => {}
    }
    Ok(false)
}

pub async fn handle_reorder(
    app: &mut App,
    key: ratatui::crossterm::event::KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
    source_id: i32,
) -> anyhow::Result<bool> {
    use ratatui::crossterm::event::KeyCode::*;
    match key {
        Char('J') | Down => {
            // Move the source row down.
            let mut ids: Vec<i32> = app
                .data
                .plan
                .iter()
                .filter(|p| p.removed_at.is_none())
                .filter_map(|p| p.todo.as_ref().map(|t| t.id))
                .collect();
            if let Some(pos) = ids.iter().position(|&i| i == source_id) {
                if pos + 1 < ids.len() {
                    ids.swap(pos, pos + 1);
                }
            }
            let date = app.logical_date.format("%Y-%m-%d").to_string();
            let c = client.clone();
            let t = tx.clone();
            tokio::spawn(async move {
                if c.reorder(&date, &ids).await.is_ok() {
                    let _ = t.send(crate::AppMsg::Refresh).await;
                }
            });
        }
        Char('K') | Up => {
            let mut ids: Vec<i32> = app
                .data
                .plan
                .iter()
                .filter(|p| p.removed_at.is_none())
                .filter_map(|p| p.todo.as_ref().map(|t| t.id))
                .collect();
            if let Some(pos) = ids.iter().position(|&i| i == source_id) {
                if pos > 0 {
                    ids.swap(pos, pos - 1);
                }
            }
            let date = app.logical_date.format("%Y-%m-%d").to_string();
            let c = client.clone();
            let t = tx.clone();
            tokio::spawn(async move {
                if c.reorder(&date, &ids).await.is_ok() {
                    let _ = t.send(crate::AppMsg::Refresh).await;
                }
            });
        }
        Enter => {
            app.mode = Mode::Navigate;
        }
        _ => {}
    }
    Ok(false)
}

pub async fn handle_confirm(
    app: &mut App,
    key: ratatui::crossterm::event::KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
    action: &ConfirmAction,
) -> anyhow::Result<bool> {
    use ratatui::crossterm::event::KeyCode::*;
    match key {
        Char('y') | Char('Y') => {
            let c = client.clone();
            let t = tx.clone();
            match action.clone() {
                ConfirmAction::CarryOver { from, to } => {
                    tokio::spawn(async move {
                        match c.carry_over(&from, &to).await {
                            Ok(_) => {
                                let _ = t
                                    .send(crate::AppMsg::Toast(
                                        ToastKind::Success,
                                        "carried over".into(),
                                    ))
                                    .await;
                                let _ = t.send(crate::AppMsg::Refresh).await;
                            }
                            Err(e) => {
                                let _ = t
                                    .send(crate::AppMsg::Toast(ToastKind::Error, e.to_string()))
                                    .await;
                            }
                        }
                    });
                }
                ConfirmAction::Unplan { id } => {
                    tokio::spawn(async move {
                        if c.unplan_today(id).await.is_ok() {
                            let _ = t
                                .send(crate::AppMsg::Toast(ToastKind::Success, "unplanned".into()))
                                .await;
                            let _ = t.send(crate::AppMsg::Refresh).await;
                        }
                    });
                }
                ConfirmAction::Cancel { id } => {
                    tokio::spawn(async move {
                        if c.set_status(id, "cancelled", None).await.is_ok() {
                            let _ = t
                                .send(crate::AppMsg::Toast(ToastKind::Success, "cancelled".into()))
                                .await;
                            let _ = t.send(crate::AppMsg::Refresh).await;
                        }
                    });
                }
                ConfirmAction::DismissPr { id } => {
                    tokio::spawn(async move {
                        if c.dismiss_pr(id).await.is_ok() {
                            let _ = t
                                .send(crate::AppMsg::Toast(ToastKind::Success, "dismissed".into()))
                                .await;
                            let _ = t.send(crate::AppMsg::Refresh).await;
                        }
                    });
                }
            }
            app.mode = Mode::Navigate;
        }
        Char('n') | Char('N') => {
            app.mode = Mode::Navigate;
        }
        _ => {}
    }
    Ok(false)
}

pub async fn handle_detail(
    app: &mut App,
    key: ratatui::crossterm::event::KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
    id: i32,
) -> anyhow::Result<bool> {
    detail::handle(app, key, client, tx, id).await
}
