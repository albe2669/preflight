//! Sync status view — last sync per source, cursor state, manual trigger.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

use crate::app::App;
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

pub(crate) fn handle_navigate(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('j') | KeyCode::Down if app.cursor + 1 < app.data.sync.len() => {
            app.cursor += 1;
        }
        KeyCode::Char('k') | KeyCode::Up if app.cursor > 0 => {
            app.cursor -= 1;
        }
        KeyCode::Char('s') => {
            if let Some(s) = app.data.sync.get(app.cursor) {
                let source = s.source.clone();
                app.spawn_sync_source(&source);
            }
        }
        KeyCode::Char('S') => {
            app.spawn_sync_all();
        }
        _ => {}
    }
}

#[cfg(test)]
mod render_tests {
    use ratatui::{Terminal, backend::TestBackend};

    use crate::app::App;
    use crate::gql::SyncState;
    use crate::test_support::buffer_text;

    fn render_sync(app: &mut App) -> String {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                super::render(f, app, area);
            })
            .unwrap();
        buffer_text(terminal.backend().buffer())
    }

    #[test]
    fn test_sync_renders_source_rows() {
        let mut app = App::default();
        app.data.sync = vec![
            SyncState {
                source: "github".to_string(),
                cursor: Some("abc123".to_string()),
                last_synced_at: None,
                last_status: "ok".to_string(),
                last_error: None,
            },
            SyncState {
                source: "linear".to_string(),
                cursor: None,
                last_synced_at: None,
                last_status: "error".to_string(),
                last_error: Some("rate limited".to_string()),
            },
        ];

        let output = render_sync(&mut app);

        // Header row is present
        assert!(
            output.contains("SOURCE"),
            "expected header with SOURCE column"
        );
        assert!(
            output.contains("STATE"),
            "expected header with STATE column"
        );
        assert!(
            output.contains("LAST SYNC"),
            "expected header with LAST SYNC column"
        );
        assert!(
            output.contains("CURSOR"),
            "expected header with CURSOR column"
        );

        // Source rows rendered
        assert!(output.contains("github"), "expected github source row");
        assert!(output.contains("linear"), "expected linear source row");

        // Status glyphs
        assert!(
            output.contains("↻ ok"),
            "expected ok status indicator for github"
        );
        assert!(
            output.contains("▲ error"),
            "expected error status indicator for linear"
        );
    }

    #[test]
    fn test_sync_renders_offline_message_when_empty() {
        let mut app = App::default();
        app.data.sync = vec![];

        let output = render_sync(&mut app);

        assert!(
            output.contains("no sources configured"),
            "expected offline message when no sync sources"
        );
        assert!(
            output.contains("offline"),
            "expected 'offline' keyword in empty sync message"
        );
    }
}
