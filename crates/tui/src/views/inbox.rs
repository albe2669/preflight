//! Inbox view — triage surface for PRs and Linear issues.
//! Grouped by source; each group carries its own sync indicator.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};
use tokio::sync::mpsc;

use crate::app::{App, ToastKind};
use crate::frame;
use crate::gql;
use crate::theme::{Glyph, Palette};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let prs = app.inbox_prs();
    let linears = app.inbox_linears();

    let mut items: Vec<ListItem> = Vec::new();
    let mut idx = 0;

    // GitHub group.
    let pr_sync = app.sync_by_source("github");
    let pr_sync_label = match pr_sync {
        Some(s) if s.last_status == "syncing" || s.last_status == "in_progress" => {
            let frame = Glyph::SPINNER[(app.spinner / 2) as usize % Glyph::SPINNER.len()];
            format!("{} syncing…", frame)
        }
        Some(s) if s.last_status == "ok" || s.last_status == "success" => "↻ ok".to_string(),
        Some(s) if s.last_status == "error" => "▲ error".to_string(),
        Some(_) | None => "⌀ no token".to_string(),
    };
    items.push(ListItem::new(Line::from(Span::styled(
        format!(
            "  ╶ GITHUB ╴ {} pull requests          {}",
            prs.len(),
            pr_sync_label
        ),
        Style::default().fg(Palette::DIM),
    ))));

    for pr in prs {
        let is_cursor = idx == app.cursor;
        let (g, c) = frame::pr_state_glyph(&pr.state);
        let marker = if pr.review_requested {
            Span::styled(
                Glyph::NEEDS_YOU.to_string(),
                Style::default().fg(Palette::ACCENT),
            )
        } else {
            Span::raw(" ")
        };
        let cursor_mark = if is_cursor {
            Span::styled(
                Glyph::CURSOR.to_string(),
                Style::default().fg(Palette::ACCENT),
            )
        } else {
            Span::raw(" ")
        };

        let mut spans = vec![cursor_mark, marker, Span::raw(" ")];
        let title_style = if pr.dismissed_at.is_some() {
            Style::default()
                .fg(Palette::GHOST)
                .add_modifier(Modifier::CROSSED_OUT)
        } else {
            Style::default().fg(Palette::TEXT)
        };
        spans.push(Span::styled(g.to_string(), Style::default().fg(c)));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("{}#{}", pr.owner, pr.number),
            Style::default().fg(if pr.dismissed_at.is_some() {
                Palette::DIM
            } else {
                Palette::TEXT
            }),
        ));
        spans.push(Span::raw("  "));
        spans.push(Span::styled(pr.title.clone(), title_style));
        if pr.review_requested {
            spans.push(Span::styled("  rev", Style::default().fg(Palette::DIM)));
        }
        if pr.authored_by_me {
            spans.push(Span::styled("  mine", Style::default().fg(Palette::DIM)));
        }
        if pr.dismissed_at.is_some() {
            spans.push(Span::styled(
                "  dismissed",
                Style::default().fg(Palette::GHOST),
            ));
        }

        let line = Line::from(spans);
        items.push(ListItem::new(line).style(if is_cursor {
            Style::default().bg(Palette::ROW_HIGHLIGHT)
        } else {
            Style::default()
        }));
        idx += 1;
    }

    items.push(ListItem::new(Line::from("")));

    // Linear group.
    let lin_sync = app.sync_by_source("linear");
    let lin_sync_label = match lin_sync {
        Some(s) if s.last_status == "syncing" || s.last_status == "in_progress" => {
            let frame = Glyph::SPINNER[(app.spinner / 2) as usize % Glyph::SPINNER.len()];
            format!("{} syncing…", frame)
        }
        Some(s) if s.last_status == "ok" || s.last_status == "success" => "↻ ok".to_string(),
        Some(s) if s.last_status == "error" => "▲ error".to_string(),
        Some(_) | None => "⌀ no token".to_string(),
    };
    items.push(ListItem::new(Line::from(Span::styled(
        format!(
            "  ╶ LINEAR ╴ {} issues          {}",
            linears.len(),
            lin_sync_label
        ),
        Style::default().fg(Palette::DIM),
    ))));

    for li in linears {
        let is_cursor = idx == app.cursor;
        let (g, c) = frame::linear_state_glyph(&li.state_type);
        let marker = if li.assigned_to_me {
            Span::styled(
                Glyph::NEEDS_YOU.to_string(),
                Style::default().fg(Palette::ACCENT),
            )
        } else {
            Span::raw(" ")
        };
        let cursor_mark = if is_cursor {
            Span::styled(
                Glyph::CURSOR.to_string(),
                Style::default().fg(Palette::ACCENT),
            )
        } else {
            Span::raw(" ")
        };

        let title_style = if li.dismissed_at.is_some() {
            Style::default()
                .fg(Palette::GHOST)
                .add_modifier(Modifier::CROSSED_OUT)
        } else {
            Style::default().fg(Palette::TEXT)
        };
        let line = Line::from(vec![
            cursor_mark,
            marker,
            Span::raw(" "),
            Span::styled(g.to_string(), Style::default().fg(c)),
            Span::raw(" "),
            Span::styled(
                format!("{:<10}", li.identifier),
                Style::default().fg(if li.dismissed_at.is_some() {
                    Palette::DIM
                } else {
                    Palette::TEXT
                }),
            ),
            Span::raw(" "),
            Span::styled(li.title.clone(), title_style),
            Span::styled(
                format!(
                    "  {} {}",
                    li.state_name,
                    li.team_key.as_deref().unwrap_or("")
                ),
                Style::default().fg(Palette::DIM),
            ),
        ]);
        items.push(ListItem::new(line).style(if is_cursor {
            Style::default().bg(Palette::ROW_HIGHLIGHT)
        } else {
            Style::default()
        }));
        idx += 1;
    }

    let list = List::new(items)
        .style(Style::default().bg(Palette::BG).fg(Palette::TEXT))
        .highlight_style(Style::default().bg(Palette::ROW_HIGHLIGHT));
    let mut state = ListState::default();
    state.select(Some(app.cursor));
    f.render_stateful_widget(list, area, &mut state);
}

pub(crate) async fn handle_navigate(
    app: &mut App,
    key: KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<crate::AppMsg>,
) -> anyhow::Result<bool> {
    let total = app.inbox_prs().len() + app.inbox_linears().len();
    match key {
        KeyCode::Char('j') | KeyCode::Down if app.cursor + 1 < total => {
            app.cursor += 1;
        }
        KeyCode::Char('k') | KeyCode::Up if app.cursor > 0 => {
            app.cursor -= 1;
        }
        KeyCode::Char('C') => {
            // Convert selected inbox row to todo, plan today.
            let prs = app.inbox_prs();
            let linears = app.inbox_linears();
            if app.cursor < prs.len() {
                let id = prs[app.cursor].id;
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    match c.todo_from_pr(id, true).await {
                        Ok(_) => {
                            let _ = t
                                .send(crate::AppMsg::Toast(
                                    ToastKind::Success,
                                    "converted to todo".into(),
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
            } else {
                let li_idx = app.cursor - prs.len();
                if let Some(li) = linears.get(li_idx) {
                    let id = li.id;
                    let c = client.clone();
                    let t = tx.clone();
                    tokio::spawn(async move {
                        match c.todo_from_linear(id, true).await {
                            Ok(_) => {
                                let _ = t
                                    .send(crate::AppMsg::Toast(
                                        ToastKind::Success,
                                        "converted to todo".into(),
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
            }
        }
        KeyCode::Char('d') => {
            // Dismiss PR.
            let prs = app.inbox_prs();
            if app.cursor < prs.len() {
                let id = prs[app.cursor].id;
                let c = client.clone();
                let t = tx.clone();
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
        KeyCode::Char('D') => {
            app.show_dismissed = !app.show_dismissed;
        }
        KeyCode::Char('s') => {
            // Sync the group of the selected row.
            let prs = app.inbox_prs();
            let c = client.clone();
            let t = tx.clone();
            if app.cursor < prs.len() {
                tokio::spawn(async move {
                    if c.sync_github().await.is_ok() {
                        let _ = t.send(crate::AppMsg::Refresh).await;
                    }
                });
            } else {
                tokio::spawn(async move {
                    if c.sync_linear().await.is_ok() {
                        let _ = t.send(crate::AppMsg::Refresh).await;
                    }
                });
            }
        }
        KeyCode::Char('o') => {
            // Open URL — print to status bar (no browser open in TUI).
            let prs = app.inbox_prs();
            let linears = app.inbox_linears();
            if app.cursor < prs.len() {
                app.set_toast(ToastKind::Success, prs[app.cursor].url.clone());
            } else {
                let li_idx = app.cursor - prs.len();
                if let Some(li) = linears.get(li_idx) {
                    app.set_toast(ToastKind::Success, li.url.clone());
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

#[cfg(test)]
mod render_tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::app::App;
    use crate::app::tests::{make_linear, make_pr};
    use crate::test_support::buffer_text;

    fn render_inbox(app: &mut App) -> String {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                crate::views::inbox::render(f, app, area);
            })
            .unwrap();
        buffer_text(terminal.backend().buffer())
    }

    #[test]
    fn test_inbox_renders_pr_and_linear_groups() {
        let mut app = App::default();
        app.data.pulls = vec![make_pr(1, None), make_pr(2, None)];
        app.data.linears = vec![make_linear(3, None)];
        app.show_dismissed = false;

        let buffer = render_inbox(&mut app);

        assert!(buffer.contains("GITHUB"), "expected GITHUB group header");
        assert!(
            buffer.contains("2 pull requests"),
            "expected PR count in header"
        );
        assert!(buffer.contains("LINEAR"), "expected LINEAR group header");
        assert!(
            buffer.contains("1 issues"),
            "expected Linear count in header"
        );
        assert!(buffer.contains("PR #1"), "expected PR #1 title");
        assert!(buffer.contains("PR #2"), "expected PR #2 title");
        assert!(
            buffer.contains("Issue #3"),
            "expected Linear Issue #3 title"
        );
    }

    #[test]
    fn test_inbox_hides_dismissed_when_show_dismissed_false() {
        let mut app = App::default();
        app.data.pulls = vec![
            make_pr(1, None),               // non-dismissed
            make_pr(2, Some("2026-08-05")), // dismissed
        ];
        app.show_dismissed = false;

        let buffer = render_inbox(&mut app);
        assert!(
            buffer.contains("PR #1"),
            "non-dismissed PR title should appear"
        );
        assert!(
            !buffer.contains("PR #2"),
            "dismissed PR title should NOT appear when show_dismissed is false"
        );

        // Enable show_dismissed and re-render
        app.show_dismissed = true;
        let buffer = render_inbox(&mut app);
        assert!(
            buffer.contains("PR #1"),
            "non-dismissed PR should still appear"
        );
        assert!(
            buffer.contains("PR #2"),
            "dismissed PR should appear when show_dismissed is true"
        );
    }
}
