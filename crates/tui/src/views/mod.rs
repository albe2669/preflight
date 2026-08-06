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
use crate::theme::{Glyph, Palette};

/// Top-level render: frame chrome + the active view's content + overlays.
pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let content = frame::render_frame(f, app, area);

    let (list_area, sidebar_area) = frame::split_content(content);
    match app.view {
        View::Today => today::render(f, app, list_area),
        View::Backlog => backlog::render(f, app, list_area),
        View::Inbox => inbox::render(f, app, content),
        View::Review => review::render(f, app, content),
        View::Sync => sync::render(f, app, content),
    }
    if let Some(sb) = sidebar_area {
        if matches!(app.view, View::Today | View::Backlog) {
            frame::render_info_sidebar(f, app, sb);
        }
    }

    // Overlays: detail pane and confirm dialog and toast.
    if let Mode::Detail { .. } = &app.mode {
        detail::render_overlay(f, app, area);
    }
    if let Mode::Help { .. } = &app.mode {
        render_help(f, app, area);
    }
    if let Mode::StatusSelect { .. } = &app.mode {
        render_status_select(f, app, area);
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
        key: "e",
        desc: "edit title (inline)",
    },
    KeybindRow {
        key: "Enter",
        desc: "open detail",
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
        key: "Enter",
        desc: "open detail",
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
    use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

    let filter = match &app.mode {
        Mode::Help { filter } => filter.clone(),
        _ => String::new(),
    };

    let width = 56.min(area.width);
    let height = 24.min(area.height);
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

    let inner = ratatui::layout::Rect::new(x + 1, y + 1, width - 2, height - 2);
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((app.help_scroll as u16, 0)),
        inner,
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

// ---- Key handlers dispatched by lib.rs ----

pub(crate) async fn handle_navigate(
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

pub(crate) async fn handle_inline_create(
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
            let plan_after = crate::app::should_plan_after_create(app.view);
            let c = client.clone();
            let t = tx.clone();
            let title2 = title.clone();
            tokio::spawn(async move {
                match c.create_todo(&title2).await {
                    Ok(todo) => {
                        if plan_after {
                            if let Err(e) = c.plan_today(todo.id).await {
                                let _ = t
                                    .send(crate::AppMsg::Toast(
                                        ToastKind::Error,
                                        format!("plan failed: {e}"),
                                    ))
                                    .await;
                            }
                        }
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

pub(crate) async fn handle_inline_edit(
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

pub(crate) fn handle_search(
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

pub(crate) async fn handle_reorder(
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

pub(crate) async fn handle_confirm(
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

pub(crate) async fn handle_detail(
    app: &mut App,
    key: ratatui::crossterm::event::KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
    id: i32,
) -> anyhow::Result<bool> {
    detail::handle(app, key, client, tx, id).await
}

// ---- Status-selection popup ----

/// Pure selection movement: wraps `selection` within the 5-element list.
pub(crate) fn next_status(selection: usize, direction: i32) -> usize {
    let len = crate::app::STATUS_ORDER.len();
    let n = selection as i32 + direction;
    (n.rem_euclid(len as i32)) as usize
}

/// The status string at index `n`, or `None` if out of range.
pub(crate) fn status_by_index(n: usize) -> Option<&'static str> {
    crate::app::STATUS_ORDER.get(n).copied()
}

pub(crate) async fn handle_status_select(
    app: &mut App,
    key: ratatui::crossterm::event::KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
    id: i32,
    selection: usize,
    reason: Option<String>,
) -> anyhow::Result<bool> {
    use ratatui::crossterm::event::KeyCode::*;
    match key {
        // While the blocked-reason sub-prompt is active, typing updates the reason.
        Char(c)
            if reason.is_some() && (c.is_alphanumeric() || c == ' ' || c == '-' || c == '_') =>
        {
            if let Mode::StatusSelect {
                reason: Some(r), ..
            } = &mut app.mode
            {
                r.push(c);
            }
        }
        Backspace if reason.is_some() => {
            if let Mode::StatusSelect {
                reason: Some(r), ..
            } = &mut app.mode
            {
                r.pop();
            }
        }
        // Submit the blocked reason.
        Enter if reason.is_some() => {
            let blocked_reason = match &app.mode {
                Mode::StatusSelect {
                    reason: Some(r), ..
                } => r.clone(),
                _ => String::new(),
            };
            let c = client.clone();
            let t = tx.clone();
            tokio::spawn(async move {
                match c.set_status(id, "blocked", Some(&blocked_reason)).await {
                    Ok(_) => {
                        let _ = t
                            .send(crate::AppMsg::Toast(
                                ToastKind::Success,
                                "status set".into(),
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
        }
        // Normal popup navigation.
        Char('j') | Down => {
            if let Mode::StatusSelect { selection, .. } = &mut app.mode {
                *selection = next_status(*selection, 1);
            }
        }
        Char('k') | Up => {
            if let Mode::StatusSelect { selection, .. } = &mut app.mode {
                *selection = next_status(*selection, -1);
            }
        }
        Char(d @ '1'..='5') => {
            let idx = (d as u8 - b'1') as usize;
            if let Mode::StatusSelect { selection, .. } = &mut app.mode {
                *selection = idx;
            }
        }
        Enter => {
            let chosen = match &app.mode {
                Mode::StatusSelect { selection, .. } => *selection,
                _ => selection,
            };
            let Some(status) = status_by_index(chosen) else {
                app.mode = Mode::Navigate;
                return Ok(false);
            };
            if status == "blocked" {
                // Enter the blocked-reason sub-prompt.
                if let Mode::StatusSelect { reason, .. } = &mut app.mode {
                    *reason = Some(String::new());
                }
            } else {
                let c = client.clone();
                let t = tx.clone();
                let s = status.to_string();
                tokio::spawn(async move {
                    match c.set_status(id, &s, None).await {
                        Ok(_) => {
                            let _ = t
                                .send(crate::AppMsg::Toast(
                                    ToastKind::Success,
                                    "status set".into(),
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
            }
        }
        _ => {}
    }
    Ok(false)
}

fn render_status_select(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    use ratatui::layout::Rect;
    use ratatui::style::{Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};

    let Mode::StatusSelect {
        selection, reason, ..
    } = &app.mode
    else {
        return;
    };
    let selection = *selection;
    let reason = reason.clone();

    let width = 34u16.min(area.width);
    let height = 9u16.min(area.height);
    let x = area.x + (area.width - width) / 2;
    let y = area.y + (area.height - height) / 2;
    let rect = Rect::new(x, y, width, height);

    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Palette::ACCENT))
        .title(Span::styled(
            " STATUS ",
            Style::default().fg(Palette::ACCENT),
        ));
    f.render_widget(block, rect);

    let inner = Rect::new(x + 1, y + 1, width - 2, height - 2);

    let mut items: Vec<ListItem> = Vec::new();
    for (i, status) in crate::app::STATUS_ORDER.iter().enumerate() {
        let (glyph, color) = frame::status_glyph(status);
        let is_current = i == selection;
        let marker = if is_current {
            Span::styled(
                Glyph::CURSOR.to_string(),
                Style::default().fg(Palette::ACCENT),
            )
        } else {
            Span::raw(" ")
        };
        let num = format!(" {}. ", i + 1);
        let line = Line::from(vec![
            marker,
            Span::raw(num),
            Span::styled(glyph.to_string(), Style::default().fg(color)),
            Span::raw(" "),
            Span::styled(
                status.to_string(),
                if is_current {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
        ]);
        items.push(ListItem::new(line));
    }

    let list = List::new(items).style(Style::default().bg(Palette::BG).fg(Palette::TEXT));
    let mut state = ListState::default();
    state.select(Some(selection));
    f.render_stateful_widget(list, inner, &mut state);

    // Blocked-reason inline prompt.
    if let Some(r) = &reason {
        let prompt_y = y + height - 2;
        let prompt_rect = Rect::new(x + 1, prompt_y, width - 2, 1);
        let prompt_line = Line::from(vec![
            Span::styled("reason ", Style::default().fg(Palette::BLOCKED)),
            Span::styled(
                if r.is_empty() {
                    "blocked because…▏".to_string()
                } else {
                    format!("{r}▏")
                },
                Style::default()
                    .fg(Palette::TEXT)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ]);
        f.render_widget(
            ratatui::widgets::Paragraph::new(prompt_line)
                .style(Style::default().bg(Palette::SURFACE)),
            prompt_rect,
        );
    }
}

// ---- Help overlay ----

pub(crate) fn handle_help(
    app: &mut App,
    key: ratatui::crossterm::event::KeyCode,
    _filter: &str,
) -> anyhow::Result<bool> {
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
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- next_status / status_by_index (task 8.3) --

    #[test]
    fn test_next_status_forward_wraps() {
        assert_eq!(next_status(0, 1), 1);
        assert_eq!(next_status(4, 1), 0);
        assert_eq!(next_status(2, 1), 3);
    }

    #[test]
    fn test_next_status_backward_wraps() {
        assert_eq!(next_status(0, -1), 4);
        assert_eq!(next_status(4, -1), 3);
        assert_eq!(next_status(2, -1), 1);
    }

    #[test]
    fn test_status_by_index_valid() {
        assert_eq!(status_by_index(0), Some("started"));
        assert_eq!(status_by_index(1), Some("blocked"));
        assert_eq!(status_by_index(2), Some("todo"));
        assert_eq!(status_by_index(3), Some("done"));
        assert_eq!(status_by_index(4), Some("cancelled"));
    }

    #[test]
    fn test_status_by_index_out_of_range() {
        assert!(status_by_index(5).is_none());
        assert!(status_by_index(100).is_none());
    }

    // -- filter_keybinds (task 10.3) --

    #[test]
    fn test_filter_keybinds_empty_returns_all() {
        let sections = filter_keybinds("");
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].0, "Global");
        assert_eq!(sections[1].0, "Today");
        assert_eq!(sections[2].0, "Backlog");
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
        assert_eq!(lower.len(), 1);
        assert_eq!(lower[0].0, "Global");
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
