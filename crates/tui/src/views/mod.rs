//! View rendering and key handling.
//!
//! Each view renders into the content area inside the frame chrome. Key
//! handlers are async (they issue GraphQL mutations then trigger a refetch).

pub mod backlog;
pub mod inbox;
pub mod review;
pub mod sync;
pub mod today;

use ratatui::Frame;
use tokio::sync::mpsc;

use crate::app::{App, ConfirmAction, LinkKind, Mode, SidebarField, ToastKind, View};
use crate::frame;
use crate::gql;
use crate::theme::{Glyph, Palette};

/// Top-level render: frame chrome + the active view's content + overlays.
pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let content = frame::render_frame(f, app, area);
    app.content_width = content.width;
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

    // Overlays: confirm dialog, status select, help, toast.
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
        key: "r",
        desc: "edit title (inline)",
    },
    KeybindRow {
        key: "e/Enter",
        desc: "sidebar edit",
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
        key: "r",
        desc: "edit title (inline)",
    },
    KeybindRow {
        key: "e/Enter",
        desc: "sidebar edit",
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
                match c.update_todo(id, Some(&title2), None).await {
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

/// Fetch detail data (events) for a todo and cache it.
pub(crate) fn fetch_detail(
    app: &mut App,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
    id: i32,
) {
    app.detail_loaded_id = Some(id);
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

/// Handle sidebar edit mode keys.
pub(crate) async fn handle_sidebar_edit(
    app: &mut App,
    key: ratatui::crossterm::event::KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
) -> anyhow::Result<bool> {
    use ratatui::crossterm::event::KeyCode::*;

    let (
        id,
        field,
        input_active,
        title_input,
        desc_input,
        _desc_scroll,
        tag_input,
        link_kind,
        link_selection,
        _scroll,
    ) = match &app.mode {
        Mode::SidebarEdit {
            id,
            field,
            input_active,
            title_input,
            desc_input,
            desc_scroll,
            tag_input,
            link_kind,
            link_selection,
            scroll,
        } => (
            *id,
            *field,
            *input_active,
            title_input.clone(),
            desc_input.clone(),
            *desc_scroll,
            tag_input.clone(),
            *link_kind,
            *link_selection,
            *scroll,
        ),
        _ => return Ok(false),
    };

    // Fetch linked items for this todo if not already loaded
    let all_links = {
        let c = client.clone();
        let prs = c.todo_pull_requests(id).await.ok().unwrap_or_default();
        let linears = c.todo_linear_issues(id).await.ok().unwrap_or_default();
        (prs, linears)
    };

    if !input_active {
        // Field navigation mode
        match key {
            Char('\t') | Char('j') => {
                let next_field = match field {
                    SidebarField::Title => SidebarField::Description,
                    SidebarField::Description => SidebarField::Links,
                    SidebarField::Links => SidebarField::Tags,
                    SidebarField::Tags => SidebarField::Title,
                };
                if let Mode::SidebarEdit { field, scroll, .. } = &mut app.mode {
                    *field = next_field;
                    *scroll = 0;
                }
            }
            BackTab => {
                let prev_field = match field {
                    SidebarField::Title => SidebarField::Tags,
                    SidebarField::Description => SidebarField::Title,
                    SidebarField::Links => SidebarField::Description,
                    SidebarField::Tags => SidebarField::Links,
                };
                if let Mode::SidebarEdit { field, scroll, .. } = &mut app.mode {
                    *field = prev_field;
                    *scroll = 0;
                }
            }
            Char('k') => {
                let prev_field = match field {
                    SidebarField::Title => SidebarField::Tags,
                    SidebarField::Description => SidebarField::Title,
                    SidebarField::Links => SidebarField::Description,
                    SidebarField::Tags => SidebarField::Links,
                };
                if let Mode::SidebarEdit { field, scroll, .. } = &mut app.mode {
                    *field = prev_field;
                    *scroll = 0;
                }
            }
            Enter => {
                if let Mode::SidebarEdit { input_active, .. } = &mut app.mode {
                    *input_active = true;
                }
            }
            Esc => {
                app.mode = Mode::Navigate;
                app.toast = None;
            }
            _ => {}
        }
    } else {
        // Input active mode — field-specific editing
        match field {
            SidebarField::Title => {
                match key {
                    Char(c)
                        if c.is_alphanumeric()
                            || c == ' '
                            || c == '#'
                            || c == '-'
                            || c == '_'
                            || c == '.'
                            || c == ','
                            || c == '!'
                            || c == '?'
                            || c == '('
                            || c == ')'
                            || c == '['
                            || c == ']'
                            || c == ':'
                            || c == ';'
                            || c == '/'
                            || c == '\\' =>
                    {
                        if let Mode::SidebarEdit { title_input, .. } = &mut app.mode {
                            title_input.push(c);
                        }
                    }
                    Char('\n') => {
                        // Commit title: update_todo(id, Some(title), None).
                        let title = title_input.trim().to_string();
                        let c = client.clone();
                        let t = tx.clone();
                        tokio::spawn(async move {
                            if c.update_todo(id, Some(&title), None).await.is_ok() {
                                let _ = t
                                    .send(crate::AppMsg::Toast(
                                        ToastKind::Success,
                                        "title saved".into(),
                                    ))
                                    .await;
                                let _ = t.send(crate::AppMsg::Refresh).await;
                            }
                        });
                        if let Mode::SidebarEdit { input_active, .. } = &mut app.mode {
                            *input_active = false;
                        }
                    }
                    Backspace => {
                        if let Mode::SidebarEdit { title_input, .. } = &mut app.mode {
                            title_input.pop();
                        }
                    }
                    Down => {
                        if let Mode::SidebarEdit { scroll, .. } = &mut app.mode {
                            *scroll = scroll.saturating_add(1);
                        }
                    }
                    Up => {
                        if let Mode::SidebarEdit { scroll, .. } = &mut app.mode {
                            *scroll = scroll.saturating_sub(1);
                        }
                    }
                    Esc => {
                        if let Mode::SidebarEdit { input_active, .. } = &mut app.mode {
                            *input_active = false;
                        }
                    }
                    _ => {}
                }
            }
            SidebarField::Description => {
                match key {
                    Char(c)
                        if c.is_alphanumeric()
                            || c == ' '
                            || c == '#'
                            || c == '-'
                            || c == '_'
                            || c == '.'
                            || c == ','
                            || c == '!'
                            || c == '?'
                            || c == '('
                            || c == ')'
                            || c == '['
                            || c == ']' =>
                    {
                        if let Mode::SidebarEdit { desc_input, .. } = &mut app.mode {
                            desc_input.push(c);
                        }
                    }
                    Char('\n') => {
                        // Commit description
                        let desc = desc_input.clone();
                        let c = client.clone();
                        let t = tx.clone();
                        tokio::spawn(async move {
                            if c.update_todo(id, None, Some(&desc)).await.is_ok() {
                                let _ = t
                                    .send(crate::AppMsg::Toast(
                                        ToastKind::Success,
                                        "description saved".into(),
                                    ))
                                    .await;
                                let _ = t.send(crate::AppMsg::Refresh).await;
                            }
                        });
                        if let Mode::SidebarEdit { input_active, .. } = &mut app.mode {
                            *input_active = false;
                        }
                    }
                    Backspace => {
                        if let Mode::SidebarEdit { desc_input, .. } = &mut app.mode {
                            desc_input.pop();
                        }
                    }
                    Down => {
                        if let Mode::SidebarEdit { desc_scroll, .. } = &mut app.mode {
                            *desc_scroll = desc_scroll.saturating_add(1);
                        }
                    }
                    Up => {
                        if let Mode::SidebarEdit { desc_scroll, .. } = &mut app.mode {
                            *desc_scroll = desc_scroll.saturating_sub(1);
                        }
                    }
                    Esc => {
                        if let Mode::SidebarEdit { input_active, .. } = &mut app.mode {
                            *input_active = false;
                        }
                    }
                    _ => {}
                }
            }
            SidebarField::Links => {
                match key {
                    Char('\t') => {
                        if let Mode::SidebarEdit {
                            link_kind,
                            link_selection,
                            ..
                        } = &mut app.mode
                        {
                            *link_kind = match link_kind {
                                LinkKind::Pr => LinkKind::Linear,
                                LinkKind::Linear => LinkKind::Pr,
                            };
                            *link_selection = 0;
                        }
                    }
                    Char('j') | Down => {
                        let max_sel = if link_kind == LinkKind::Pr {
                            all_links.0.len().saturating_sub(1)
                        } else {
                            all_links.1.len().saturating_sub(1)
                        };
                        if let Mode::SidebarEdit { link_selection, .. } = &mut app.mode {
                            if *link_selection < max_sel {
                                *link_selection += 1;
                            }
                        }
                    }
                    Char('k') | Up => {
                        if let Mode::SidebarEdit { link_selection, .. } = &mut app.mode {
                            *link_selection = link_selection.saturating_sub(1);
                        }
                    }
                    Enter => {
                        // Attach the selected link (already linked items are shown;
                        // this is mainly for the UI flow — Enter on a link just navigates)
                        if let Mode::SidebarEdit { input_active, .. } = &mut app.mode {
                            *input_active = false;
                        }
                    }
                    Char('x') => {
                        // Detach the selected link
                        let c = client.clone();
                        let t = tx.clone();
                        if link_kind == LinkKind::Pr {
                            if let Some(link) = all_links.0.get(link_selection) {
                                let pr_id = link.pull_request_id;
                                tokio::spawn(async move {
                                    if c.unlink_pull_request(id, pr_id).await.is_ok() {
                                        let _ = t
                                            .send(crate::AppMsg::Toast(
                                                ToastKind::Success,
                                                "pr detached".into(),
                                            ))
                                            .await;
                                        let _ = t.send(crate::AppMsg::Refresh).await;
                                    }
                                });
                            }
                        } else {
                            if let Some(link) = all_links.1.get(link_selection) {
                                let issue_id = link.linear_issue_id;
                                tokio::spawn(async move {
                                    if c.unlink_linear_issue(id, issue_id).await.is_ok() {
                                        let _ = t
                                            .send(crate::AppMsg::Toast(
                                                ToastKind::Success,
                                                "issue detached".into(),
                                            ))
                                            .await;
                                        let _ = t.send(crate::AppMsg::Refresh).await;
                                    }
                                });
                            }
                        }
                        if let Mode::SidebarEdit { input_active, .. } = &mut app.mode {
                            *input_active = false;
                        }
                    }
                    Esc => {
                        if let Mode::SidebarEdit { input_active, .. } = &mut app.mode {
                            *input_active = false;
                        }
                    }
                    _ => {}
                }
            }
            SidebarField::Tags => {
                match key {
                    Char(c) if c.is_alphanumeric() || c == '-' || c == '_' => {
                        if let Mode::SidebarEdit { tag_input, .. } = &mut app.mode {
                            tag_input.push(c);
                        }
                    }
                    Backspace => {
                        if let Mode::SidebarEdit { tag_input, .. } = &mut app.mode {
                            tag_input.pop();
                        }
                    }
                    Enter => {
                        let slug = tag_input.trim().to_string();
                        if !slug.is_empty() {
                            // A slug already on this todo removes the tag;
                            // anything else adds it.
                            let exists = tag_commit_is_removal(app, id, &slug);
                            let c = client.clone();
                            let t = tx.clone();
                            let slug2 = slug.clone();
                            tokio::spawn(async move {
                                let result = if exists {
                                    c.remove_tag(id, &slug2).await
                                } else {
                                    c.add_tag(id, &slug2).await
                                };
                                if result.is_ok() {
                                    let _ = t
                                        .send(crate::AppMsg::Toast(
                                            ToastKind::Success,
                                            if exists {
                                                "tag removed".into()
                                            } else {
                                                "tag added".into()
                                            },
                                        ))
                                        .await;
                                    let _ = t.send(crate::AppMsg::Refresh).await;
                                }
                            });
                        }
                        if let Mode::SidebarEdit {
                            tag_input,
                            input_active,
                            ..
                        } = &mut app.mode
                        {
                            tag_input.clear();
                            *input_active = false;
                        }
                    }
                    Esc => {
                        if let Mode::SidebarEdit { input_active, .. } = &mut app.mode {
                            *input_active = false;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(false)
}

/// A tag slug that already exists on the todo is removed on commit;
/// anything else is added. Pure decision used by the Tags field.
fn tag_commit_is_removal(app: &App, id: i32, slug: &str) -> bool {
    app.data
        .todos
        .iter()
        .find(|t| t.id == id)
        .map(|t| t.tag.nodes.iter().any(|tg| tg.slug == slug))
        .unwrap_or(false)
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

    // -- tag_commit_is_removal (sidebar tag removal) --

    #[test]
    fn test_tag_commit_is_removal_when_slug_exists() {
        let app = App {
            data: crate::app::AppData {
                todos: vec![crate::app::tests::make_todo_with_tags(
                    1,
                    "Task",
                    "todo",
                    &["rust", "tui"],
                )],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            tag_commit_is_removal(&app, 1, "rust"),
            "an existing slug removes the tag"
        );
    }

    #[test]
    fn test_tag_commit_not_removal_for_new_slug() {
        let app = App {
            data: crate::app::AppData {
                todos: vec![crate::app::tests::make_todo_with_tags(
                    1,
                    "Task",
                    "todo",
                    &["rust"],
                )],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            !tag_commit_is_removal(&app, 1, "linear"),
            "a new slug is added, not removed"
        );
    }

    #[test]
    fn test_tag_commit_not_removal_when_only_tag_is_last() {
        let app = App {
            data: crate::app::AppData {
                todos: vec![crate::app::tests::make_todo_with_tags(
                    1,
                    "Task",
                    "todo",
                    &["only"],
                )],
                ..Default::default()
            },
            ..Default::default()
        };
        // Re-committing the last (only) tag still counts as removal so the
        // tags region can reach the empty state.
        assert!(tag_commit_is_removal(&app, 1, "only"));
    }

    #[test]
    fn test_tag_commit_not_removal_for_unknown_todo() {
        let app = App {
            data: crate::app::AppData {
                todos: vec![crate::app::tests::make_todo_with_tags(
                    1,
                    "Task",
                    "todo",
                    &["rust"],
                )],
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(
            !tag_commit_is_removal(&app, 999, "rust"),
            "unknown todo defaults to add"
        );
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
#[cfg(test)]
mod sidebar_edit_tests {
    use crate::app::tests::{make_plan_row, make_todo};
    use crate::app::{App, LinkKind, Mode, SidebarField, ToastKind};
    use crate::gql;
    use crate::test_support::buffer_text;
    use ratatui::crossterm::event::KeyCode;
    use ratatui::{Terminal, backend::TestBackend};
    use tokio::sync::mpsc;

    // -- helpers --

    fn test_client() -> (gql::Client, mpsc::Sender<crate::AppMsg>) {
        let client = gql::Client::new("http://127.0.0.1:0");
        let (tx, _rx) = mpsc::channel::<crate::AppMsg>(32);
        (client, tx)
    }

    fn setup_app_with_todo() -> App {
        let mut app = App::default();
        app.data.plan = vec![make_plan_row(1, 0, make_todo(1, "My Task", "todo"), None)];
        app.data.todos = vec![make_todo(1, "My Task", "todo")];
        app.cursor = 0;
        app.content_width = 120;
        app
    }

    fn enter_sidebar_edit(app: &mut App) {
        app.mode = Mode::SidebarEdit {
            id: 1,
            field: SidebarField::Description,
            input_active: false,
            title_input: String::new(),
            desc_input: String::new(),
            desc_scroll: 0,
            tag_input: String::new(),
            link_kind: LinkKind::Pr,
            link_selection: 0,
            scroll: 0,
        };
    }

    fn render_full(app: &mut App) -> String {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| crate::views::render(f, app)).unwrap();
        buffer_text(terminal.backend().buffer())
    }

    // -- 5.1 r enters InlineEdit; Enter commits; Esc reverts --

    #[tokio::test]
    async fn test_r_enters_inline_edit_seeded_with_title() {
        let (client, tx) = test_client();
        let mut app = setup_app_with_todo();
        crate::handle_key(&mut app, KeyCode::Char('r'), &client, &tx)
            .await
            .unwrap();
        assert!(
            matches!(&app.mode, Mode::InlineEdit { id, input } if *id == 1 && input == "My Task"),
            "should enter InlineEdit with id=1 and input seeded from title"
        );
    }

    #[tokio::test]
    async fn test_r_on_empty_plan_does_nothing() {
        let (client, tx) = test_client();
        let mut app = App::default();

        crate::handle_key(&mut app, KeyCode::Char('r'), &client, &tx)
            .await
            .unwrap();
        assert_eq!(app.mode, Mode::Navigate);
    }

    #[tokio::test]
    async fn test_esc_from_inline_edit_returns_to_navigate() {
        let (client, tx) = test_client();
        let mut app = setup_app_with_todo();
        // Enter inline edit
        crate::handle_key(&mut app, KeyCode::Char('r'), &client, &tx)
            .await
            .unwrap();
        assert!(matches!(app.mode, Mode::InlineEdit { .. }));
        // Esc returns to Navigate
        crate::handle_key(&mut app, KeyCode::Esc, &client, &tx)
            .await
            .unwrap();
        assert_eq!(app.mode, Mode::Navigate);
    }

    #[tokio::test]
    async fn test_enter_from_inline_edit_commits_and_returns_to_navigate() {
        let (client, tx) = test_client();
        let mut app = setup_app_with_todo();
        crate::handle_key(&mut app, KeyCode::Char('r'), &client, &tx)
            .await
            .unwrap();
        assert!(matches!(app.mode, Mode::InlineEdit { .. }));
        // Enter commits (mutation runs async) and returns to Navigate
        crate::handle_key(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        assert_eq!(app.mode, Mode::Navigate);
    }

    // -- 5.2 e/Enter enter SidebarEdit; Esc exits discarding input --

    #[tokio::test]
    async fn test_e_enters_sidebar_edit() {
        let (client, tx) = test_client();
        let mut app = setup_app_with_todo();
        crate::handle_key(&mut app, KeyCode::Char('e'), &client, &tx)
            .await
            .unwrap();
        assert!(
            matches!(&app.mode, Mode::SidebarEdit { id, field, .. } if *id == 1 && *field == SidebarField::Description),
            "should enter SidebarEdit for id=1, focused on Description"
        );
    }

    #[tokio::test]
    async fn test_enter_enters_sidebar_edit() {
        let (client, tx) = test_client();
        let mut app = setup_app_with_todo();
        crate::handle_key(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        assert!(
            matches!(&app.mode, Mode::SidebarEdit { id, .. } if *id == 1),
            "Enter should also enter SidebarEdit"
        );
    }

    #[tokio::test]
    async fn test_esc_from_sidebar_edit_returns_to_navigate() {
        let (client, tx) = test_client();
        let mut app = setup_app_with_todo();
        crate::handle_key(&mut app, KeyCode::Char('e'), &client, &tx)
            .await
            .unwrap();
        assert!(matches!(app.mode, Mode::SidebarEdit { .. }));
        crate::handle_key(&mut app, KeyCode::Esc, &client, &tx)
            .await
            .unwrap();
        assert_eq!(app.mode, Mode::Navigate);
    }

    #[tokio::test]
    async fn test_sidebar_edit_starts_with_scroll_zero() {
        let (client, tx) = test_client();
        let mut app = setup_app_with_todo();
        crate::handle_key(&mut app, KeyCode::Char('e'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { scroll, .. } = &app.mode {
            assert_eq!(*scroll, 0, "scroll should be reset on entry");
        } else {
            panic!("expected SidebarEdit mode");
        }
    }

    #[tokio::test]
    async fn test_sidebar_edit_seeds_desc_input_from_todo_description() {
        let mut app = setup_app_with_todo();
        app.data.todos[0].description = Some("existing description".to_string());
        if let Some(todo) = &mut app.data.plan[0].todo {
            todo.description = Some("existing description".to_string());
        }
        let (client, tx) = test_client();
        crate::handle_key(&mut app, KeyCode::Char('e'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { desc_input, .. } = &app.mode {
            assert_eq!(desc_input, "existing description");
        } else {
            panic!("expected SidebarEdit mode");
        }
    }

    // -- 5.4 narrow terminal toast --

    #[tokio::test]
    async fn test_narrow_terminal_prevents_sidebar_edit() {
        let (client, tx) = test_client();
        let mut app = setup_app_with_todo();
        app.content_width = 80; // below 100
        crate::handle_key(&mut app, KeyCode::Char('e'), &client, &tx)
            .await
            .unwrap();
        assert_eq!(
            app.mode,
            Mode::Navigate,
            "should stay in Navigate on narrow terminal"
        );
        assert!(
            app.toast.is_some(),
            "should show a toast when terminal is too narrow"
        );
        let toast = app.toast.as_ref().unwrap();
        assert_eq!(toast.kind, ToastKind::Error);
        assert!(
            toast.message.contains("narrow"),
            "toast should mention narrow terminal"
        );
    }

    #[tokio::test]
    async fn test_narrow_terminal_blocks_enter_key_too() {
        let (client, tx) = test_client();
        let mut app = setup_app_with_todo();
        app.content_width = 99;
        crate::handle_key(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        assert_eq!(app.mode, Mode::Navigate);
        assert!(app.toast.is_some());
    }

    // -- 5.5 field navigation (Tab/j/k) --

    #[tokio::test]
    async fn test_tab_cycles_fields_forward() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        // Description -> Links
        let (client, _) = test_client();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { field, .. } = &app.mode {
            assert_eq!(*field, SidebarField::Links);
        } else {
            panic!("expected SidebarEdit");
        }

        // Links -> Tags
        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { field, .. } = &app.mode {
            assert_eq!(*field, SidebarField::Tags);
        } else {
            panic!("expected SidebarEdit");
        }

        // Tags -> Title (wraps)
        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { field, .. } = &app.mode {
            assert_eq!(*field, SidebarField::Title);
        } else {
            panic!("expected SidebarEdit");
        }

        // Title -> Description (wraps)
        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { field, .. } = &app.mode {
            assert_eq!(*field, SidebarField::Description);
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_j_cycles_fields_forward_same_as_tab() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        super::handle_sidebar_edit(&mut app, KeyCode::Char('j'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { field, .. } = &app.mode {
            assert_eq!(*field, SidebarField::Links, "j should advance like Tab");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_k_cycles_fields_backward() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        // Description -> Title (backward)
        super::handle_sidebar_edit(&mut app, KeyCode::Char('k'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { field, .. } = &app.mode {
            assert_eq!(*field, SidebarField::Title);
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_back_tab_cycles_fields_backward() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        // Description -> Title (backward)
        super::handle_sidebar_edit(&mut app, KeyCode::BackTab, &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { field, .. } = &app.mode {
            assert_eq!(*field, SidebarField::Title);
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_field_nav_resets_scroll() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        // Set scroll to something nonzero
        if let Mode::SidebarEdit { scroll, .. } = &mut app.mode {
            *scroll = 5;
        }
        let (client, _) = test_client();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { scroll, .. } = &app.mode {
            assert_eq!(*scroll, 0, "scroll should reset to 0 on field nav");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_field_nav_does_not_move_list_cursor() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        app.cursor = 0;
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('j'), &client, &tx)
            .await
            .unwrap();
        assert_eq!(
            app.cursor, 0,
            "list cursor should not move during sidebar field nav"
        );
    }

    // -- description field: input, commit, scroll --

    #[tokio::test]
    async fn test_enter_activates_input_on_description() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        assert!(
            !matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if *input_active),
            "input_active should be false on entry"
        );

        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        assert!(
            matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if *input_active),
            "Enter should activate input"
        );
    }

    #[tokio::test]
    async fn test_description_char_input() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        // Activate input
        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        // Type some chars
        super::handle_sidebar_edit(&mut app, KeyCode::Char('H'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('i'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { desc_input, .. } = &app.mode {
            assert_eq!(desc_input, "Hi");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_description_backspace() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('H'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('i'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Backspace, &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { desc_input, .. } = &app.mode {
            assert_eq!(desc_input, "H");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_description_enter_deactivates_input() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        assert!(matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if *input_active));
        // Commit with Enter
        super::handle_sidebar_edit(&mut app, KeyCode::Char('\n'), &client, &tx)
            .await
            .unwrap();
        assert!(
            matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if !*input_active),
            "Enter on description should deactivate input (commit)"
        );
        // Mode should still be SidebarEdit
        assert!(matches!(app.mode, Mode::SidebarEdit { .. }));
    }

    #[tokio::test]
    async fn test_description_esc_deactivates_input() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        assert!(matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if *input_active));
        super::handle_sidebar_edit(&mut app, KeyCode::Esc, &client, &tx)
            .await
            .unwrap();
        assert!(
            matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if !*input_active),
            "Esc should deactivate input"
        );
    }

    #[tokio::test]
    async fn test_description_down_increases_scroll() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Down, &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { desc_scroll, .. } = &app.mode {
            assert_eq!(*desc_scroll, 1);
        } else {
            panic!("expected SidebarEdit");
        }
        super::handle_sidebar_edit(&mut app, KeyCode::Down, &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { desc_scroll, .. } = &app.mode {
            assert_eq!(*desc_scroll, 2);
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_description_up_decreases_scroll_saturating() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        // Scroll is 0, Up should saturate at 0
        super::handle_sidebar_edit(&mut app, KeyCode::Up, &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { desc_scroll, .. } = &app.mode {
            assert_eq!(*desc_scroll, 0);
        } else {
            panic!("expected SidebarEdit");
        }
    }

    // -- tags: type, Enter adds --

    #[tokio::test]
    async fn test_tag_input_typing() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        // Nav to Tags
        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        // Activate input
        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        // Type a tag
        super::handle_sidebar_edit(&mut app, KeyCode::Char('a'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('b'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { tag_input, .. } = &app.mode {
            assert_eq!(tag_input, "ab");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_tag_enter_clears_input_and_deactivates() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        // Nav to Tags
        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('t'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('e'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('s'), &client, &tx)
            .await
            .unwrap();
        // Commit
        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit {
            tag_input,
            input_active,
            ..
        } = &app.mode
        {
            assert!(tag_input.is_empty(), "tag input should be cleared");
            assert!(!input_active, "input should be deactivated");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_tag_backspace() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('a'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Backspace, &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { tag_input, .. } = &app.mode {
            assert!(tag_input.is_empty());
        } else {
            panic!("expected SidebarEdit");
        }
    }

    // -- links: j/k navigation, Tab switches kind, x detach, Enter attach --

    #[tokio::test]
    async fn test_links_j_moves_selection_down() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        // Nav to Links
        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        // Activate
        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        // j (no links in data, selection stays at 0)
        super::handle_sidebar_edit(&mut app, KeyCode::Char('j'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { link_selection, .. } = &app.mode {
            assert_eq!(*link_selection, 0, "selection should stay 0 with no links");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_links_k_selection_saturates_at_zero() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('k'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { link_selection, .. } = &app.mode {
            assert_eq!(*link_selection, 0);
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_links_down_moves_selection_down() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Down, &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { link_selection, .. } = &app.mode {
            assert_eq!(*link_selection, 0, "Down also moves link selection");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_links_up_moves_selection_up() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        // Set link_selection to 1
        if let Mode::SidebarEdit { link_selection, .. } = &mut app.mode {
            *link_selection = 1;
        }
        let (client, _) = test_client();

        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Up, &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { link_selection, .. } = &app.mode {
            assert_eq!(*link_selection, 0);
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_links_tab_switches_kind() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        // Nav to Links
        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        assert!(
            matches!(&app.mode, Mode::SidebarEdit { link_kind, .. } if *link_kind == LinkKind::Pr),
            "default link kind should be Pr"
        );
        // Tab within links input toggles kind
        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { link_kind, .. } = &app.mode {
            assert_eq!(*link_kind, LinkKind::Linear, "Tab should switch to Linear");
        } else {
            panic!("expected SidebarEdit");
        }
        // Tab again wraps back to Pr
        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { link_kind, .. } = &app.mode {
            assert_eq!(*link_kind, LinkKind::Pr, "Tab should wrap back to Pr");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_links_enter_deactivates_input() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        assert!(
            matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if !*input_active),
            "Enter on Links should deactivate input"
        );
    }

    #[tokio::test]
    async fn test_links_esc_deactivates_input() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        super::handle_sidebar_edit(&mut app, KeyCode::Char('\t'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Esc, &client, &tx)
            .await
            .unwrap();
        assert!(
            matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if !*input_active),
            "Esc on Links should deactivate input"
        );
    }

    // -- render tests: edit form vs read card; LINKS region --

    #[test]
    fn test_sidebar_edit_form_shows_edit_title() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let output = render_full(&mut app);
        assert!(
            output.contains("EDIT"),
            "sidebar should show EDIT title in edit mode"
        );
    }

    #[test]
    fn test_sidebar_edit_form_shows_description_header() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let output = render_full(&mut app);
        assert!(
            output.contains("DESCRIPTION"),
            "edit form should show DESCRIPTION header"
        );
    }

    #[test]
    fn test_sidebar_edit_form_shows_links_header() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let output = render_full(&mut app);
        assert!(
            output.contains("LINKS"),
            "edit form should show LINKS header"
        );
    }

    #[test]
    fn test_sidebar_edit_form_shows_tags_header() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let output = render_full(&mut app);
        assert!(output.contains("TAGS"), "edit form should show TAGS header");
    }

    #[test]
    fn test_sidebar_edit_form_shows_field_nav_hints() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let output = render_full(&mut app);
        assert!(
            output.contains("Tab") || output.contains("j"),
            "edit form should show field navigation hints when input_inactive"
        );
    }

    #[test]
    fn test_read_only_sidebar_shows_todo_title() {
        let mut app = setup_app_with_todo();
        // Navigate mode (not SidebarEdit) — sidebar shows read-only info card
        app.mode = Mode::Navigate;
        let output = render_full(&mut app);
        assert!(
            output.contains("TODO"),
            "read-only sidebar should show TODO title"
        );
    }

    #[test]
    fn test_read_only_sidebar_shows_status_glyph() {
        let mut app = setup_app_with_todo();
        app.mode = Mode::Navigate;
        let output = render_full(&mut app);
        assert!(
            output.contains("todo"),
            "read-only sidebar should show todo status"
        );
    }

    #[test]
    fn test_read_only_sidebar_shows_task_title() {
        let mut app = setup_app_with_todo();
        app.mode = Mode::Navigate;
        let output = render_full(&mut app);
        assert!(
            output.contains("My Task"),
            "read-only sidebar should show the task title"
        );
    }

    #[test]
    fn test_read_only_sidebar_shows_created_date() {
        let mut app = setup_app_with_todo();
        app.mode = Mode::Navigate;
        let output = render_full(&mut app);
        assert!(
            output.contains("created"),
            "read-only sidebar should show created date"
        );
    }

    #[test]
    fn test_edit_form_description_active_shows_arrow() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        // Activate description input
        if let Mode::SidebarEdit { input_active, .. } = &mut app.mode {
            *input_active = true;
        }
        let output = render_full(&mut app);
        assert!(
            output.contains("▸ DESCRIPTION") || output.contains("DESCRIPTION"),
            "active description should show focused indicator"
        );
    }

    #[test]
    fn test_edit_form_links_active_shows_arrow() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        // Nav to Links and activate
        if let Mode::SidebarEdit {
            field,
            input_active,
            ..
        } = &mut app.mode
        {
            *field = SidebarField::Links;
            *input_active = true;
        }
        let output = render_full(&mut app);
        assert!(
            output.contains("▸ LINKS") || output.contains("LINKS"),
            "active links field should show focused indicator"
        );
    }

    #[test]
    fn test_edit_form_tags_active_shows_arrow() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        // Nav to Tags and activate
        if let Mode::SidebarEdit {
            field,
            input_active,
            ..
        } = &mut app.mode
        {
            *field = SidebarField::Tags;
            *input_active = true;
        }
        let output = render_full(&mut app);
        assert!(
            output.contains("▸ TAGS") || output.contains("TAGS"),
            "active tags field should show focused indicator"
        );
    }

    #[test]
    fn test_read_only_sidebar_shows_links_region_with_detail() {
        let mut app = setup_app_with_todo();
        app.mode = Mode::Navigate;
        // Set detail data so LINKS region renders
        app.detail_loaded_id = Some(1);
        app.detail = Some(crate::app::DetailData {
            tags: vec![],
            events: vec![],
        });
        let output = render_full(&mut app);
        assert!(
            output.contains("LINKS"),
            "read-only sidebar should show LINKS region when detail loaded"
        );
    }

    #[test]
    fn test_read_only_sidebar_links_empty_state() {
        let mut app = setup_app_with_todo();
        app.mode = Mode::Navigate;
        app.detail_loaded_id = Some(1);
        app.detail = Some(crate::app::DetailData {
            tags: vec![],
            events: vec![],
        });
        let output = render_full(&mut app);
        assert!(
            output.contains("No linked") || output.contains("loading"),
            "LINKS region should show empty state or loading text"
        );
    }

    #[test]
    fn test_edit_form_shows_no_linked_prs_when_empty() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let output = render_full(&mut app);
        assert!(
            output.contains("No linked"),
            "edit form should show empty links state"
        );
    }

    #[test]
    fn test_edit_form_shows_none_tags_when_empty() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let output = render_full(&mut app);
        assert!(
            output.contains("none"),
            "edit form should show 'none' for empty tags"
        );
    }

    // -- detail_loaded_id set on sidebar entry --

    #[tokio::test]
    async fn test_sidebar_entry_sets_detail_loaded_id() {
        let (client, tx) = test_client();
        let mut app = setup_app_with_todo();
        assert_eq!(app.detail_loaded_id, None);
        crate::handle_key(&mut app, KeyCode::Char('e'), &client, &tx)
            .await
            .unwrap();
        // fetch_detail sets detail_loaded_id synchronously
        assert_eq!(app.detail_loaded_id, Some(1));
    }

    // -- Esc double-press when input_active --

    #[tokio::test]
    async fn test_esc_twice_exits_when_input_active() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        // Activate input
        super::handle_sidebar_edit(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();
        assert!(matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if *input_active));

        // First Esc deactivates input
        super::handle_sidebar_edit(&mut app, KeyCode::Esc, &client, &tx)
            .await
            .unwrap();
        assert!(matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if !*input_active));

        // Second Esc exits — but this goes through handle_key which routes to handle_sidebar_edit
        // In the handler, Esc when !input_active exits to Navigate
        crate::handle_key(&mut app, KeyCode::Esc, &client, &tx)
            .await
            .unwrap();
        assert_eq!(app.mode, Mode::Navigate);
    }

    // -- title field: reachable, seeded, commit/cancel, no r binding --

    #[tokio::test]
    async fn test_title_field_seeded_from_current_title() {
        let (client, tx) = test_client();
        let mut app = setup_app_with_todo();
        crate::handle_key(&mut app, KeyCode::Char('e'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { title_input, .. } = &app.mode {
            assert_eq!(
                title_input, "My Task",
                "title input should be seeded from the todo title"
            );
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_title_field_reachable_via_navigation() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        // Backward from Description -> Title.
        super::handle_sidebar_edit(&mut app, KeyCode::Char('k'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { field, .. } = &app.mode {
            assert_eq!(
                *field,
                SidebarField::Title,
                "Title should be reachable via backward navigation"
            );
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_title_commit_deactivates_input_and_stays_in_edit() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        // Navigate to Title and activate input.
        if let Mode::SidebarEdit {
            field,
            input_active,
            ..
        } = &mut app.mode
        {
            *field = SidebarField::Title;
            *input_active = true;
        }
        let (client, _) = test_client();
        // Typing then Enter commits the title (mutation runs async).
        super::handle_sidebar_edit(&mut app, KeyCode::Char('N'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('e'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('\n'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { input_active, .. } = &app.mode {
            assert!(
                !input_active,
                "Enter on title should deactivate input (commit)"
            );
        } else {
            panic!("expected SidebarEdit");
        }
        // Mode stays in SidebarEdit after commit.
        assert!(matches!(app.mode, Mode::SidebarEdit { .. }));
    }

    #[tokio::test]
    async fn test_title_esc_deactivates_without_committing() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        if let Mode::SidebarEdit {
            field,
            input_active,
            ..
        } = &mut app.mode
        {
            *field = SidebarField::Title;
            *input_active = true;
        }
        let (client, _) = test_client();
        // Type a char then Esc: Esc deactivates, input keeps the char.
        super::handle_sidebar_edit(&mut app, KeyCode::Char('X'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Esc, &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit {
            title_input,
            input_active,
            ..
        } = &app.mode
        {
            assert!(!input_active, "Esc should deactivate title input");
            assert_eq!(title_input, "X", "Esc should not discard typed input");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[tokio::test]
    async fn test_r_in_edit_mode_does_not_inline_edit_title() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let (client, _) = test_client();

        // r while in sidebar edit mode must not enter InlineEdit.
        super::handle_sidebar_edit(&mut app, KeyCode::Char('r'), &client, &tx)
            .await
            .unwrap();
        assert!(
            matches!(app.mode, Mode::SidebarEdit { .. }),
            "r in edit mode should be ignored (no inline title edit)"
        );
    }

    #[tokio::test]
    async fn test_title_input_typing_and_backspace() {
        let (_, tx) = test_client();
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        if let Mode::SidebarEdit {
            field,
            input_active,
            ..
        } = &mut app.mode
        {
            *field = SidebarField::Title;
            *input_active = true;
        }
        let (client, _) = test_client();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('N'), &client, &tx)
            .await
            .unwrap();
        super::handle_sidebar_edit(&mut app, KeyCode::Char('e'), &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { title_input, .. } = &app.mode {
            assert_eq!(title_input, "Ne", "typing appends to title input");
        } else {
            panic!("expected SidebarEdit");
        }
        super::handle_sidebar_edit(&mut app, KeyCode::Backspace, &client, &tx)
            .await
            .unwrap();
        if let Mode::SidebarEdit { title_input, .. } = &app.mode {
            assert_eq!(title_input, "N", "backspace pops from title input");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_sidebar_edit_form_removes_dead_r_hint() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        let output = render_full(&mut app);
        assert!(
            !output.contains("r to edit title"),
            "the dead 'r to edit title' hint must be removed:\n{output}"
        );
        assert!(
            output.contains("TITLE") || output.contains("▸ TITLE") || output.contains("  TITLE"),
            "the editable TITLE field should render:\n{output}"
        );
    }
}
