//! Sync status view — last sync per source, cursor state, manual trigger.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use tokio::sync::mpsc;

use crate::app::{App, ToastKind};
use crate::gql;
use crate::theme::{Glyph, Palette};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    if app.data.sync.is_empty() {
        let p = Paragraph::new(Line::from(Span::styled(
            "  no sources configured — preflight works fully offline".to_string(),
            Style::default().fg(Palette::DIM),
        )));
        f.render_widget(p, area);
        return;
    }

    let mut items: Vec<ListItem> = Vec::new();

    // Header row.
    items.push(ListItem::new(Line::from(Span::styled(
        "  SOURCE      STATE                 LAST SYNC          CURSOR".to_string(),
        Style::default().fg(Palette::DIM),
    ))));

    for (i, s) in app.data.sync.iter().enumerate() {
        let is_cursor = i == app.cursor;
        let marker = if is_cursor {
            Span::styled(
                Glyph::CURSOR.to_string(),
                Style::default().fg(Palette::ACCENT),
            )
        } else {
            Span::raw(" ")
        };

        let state_label = match s.last_status.as_str() {
            "syncing" | "in_progress" => {
                let frame = Glyph::SPINNER[(app.spinner / 2) as usize % Glyph::SPINNER.len()];
                Span::styled(
                    format!("{} syncing…", frame),
                    Style::default().fg(Palette::ACCENT),
                )
            }
            "ok" | "success" => Span::styled("↻ ok", Style::default().fg(Palette::DONE)),
            "error" => Span::styled("▲ error", Style::default().fg(Palette::BLOCKED)),
            "no_token" | "offline" => {
                Span::styled("⌀ no token", Style::default().fg(Palette::GHOST))
            }
            other => Span::styled(other.to_string(), Style::default().fg(Palette::DIM)),
        };

        let last = s
            .last_synced_at
            .as_ref()
            .map(|ts| crate::app::relative_time(ts, chrono::Utc::now()))
            .unwrap_or_else(|| "never".to_string());
        let cursor = s.cursor.as_deref().unwrap_or("—");

        let line = Line::from(vec![
            marker,
            Span::raw(format!(" {:<10} ", s.source)),
            state_label,
            Span::raw(format!("  {:<18}  {}", last, cursor)),
        ]);
        items.push(ListItem::new(line).style(if is_cursor {
            Style::default().bg(Palette::ROW_HIGHLIGHT)
        } else {
            Style::default()
        }));

        // Error detail line.
        if let Some(ref err) = s.last_error {
            items.push(ListItem::new(Line::from(Span::styled(
                format!("              ▲ {}", err),
                Style::default().fg(Palette::BLOCKED),
            ))));
        }
    }

    let list = List::new(items)
        .style(Style::default().bg(Palette::BG).fg(Palette::TEXT))
        .highlight_style(Style::default().bg(Palette::ROW_HIGHLIGHT));
    let mut state = ListState::default();
    if app.cursor < app.data.sync.len() {
        state.select(Some(app.cursor));
    }
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
            if app.cursor + 1 < app.data.sync.len() {
                app.cursor += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.cursor > 0 {
                app.cursor -= 1;
            }
        }
        KeyCode::Char('s') => {
            // Sync the selected source.
            if let Some(s) = app.data.sync.get(app.cursor) {
                let source = s.source.clone();
                let c = client.clone();
                let t = tx.clone();
                tokio::spawn(async move {
                    let res = if source == "linear" {
                        c.sync_linear().await
                    } else {
                        c.sync_github().await
                    };
                    match res {
                        Ok(_) => {
                            let _ = t
                                .send(crate::AppMsg::Toast(
                                    ToastKind::Success,
                                    format!("{} synced", source),
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
        KeyCode::Char('S') => {
            // Sync all configured sources.
            let c = client.clone();
            let t = tx.clone();
            tokio::spawn(async move {
                let _ = c.sync_linear().await;
                let _ = c.sync_github().await;
                let _ = t.send(crate::AppMsg::Refresh).await;
            });
        }
        _ => {}
    }
    Ok(false)
}
