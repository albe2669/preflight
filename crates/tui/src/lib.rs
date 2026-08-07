//! preflight — terminal UI.
//!
//! A ratatui TUI client for the preflight GraphQL backend. Visual design
//! follows `design/Preflight TUI Visual Design.dc.html` exactly: a dark,
//! matte ledger with one amber accent marking focus, five views (Today,
//! Backlog, Inbox, Review, Sync) plus a todo-detail overlay.

pub mod app;
pub mod frame;
pub mod gql;
pub mod theme;
pub mod views;

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use tokio::sync::mpsc;

use app::{App, Mode, ToastKind, View};

/// Run the TUI. Connects to the GraphQL backend at `endpoint` (e.g.
/// `http://127.0.0.1:8000/`).
pub async fn run(endpoint: &str) -> anyhow::Result<()> {
    let client = gql::Client::new(endpoint);

    // Initial fetch.
    let date_str = chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let fetched = client.fetch_all(&date_str).await;
    let mut app = App::default();
    match fetched {
        Ok(d) => {
            app.data = crate::app::AppData::from_fetch_all(d);
            app.clamp_cursor_to_active();
        }
        Err(e) => app.set_error(format!("connect failed: {e}")),
    }

    // Channel for async results arriving from background tasks.
    let (tx, mut rx) = mpsc::channel::<AppMsg>(32);

    // Terminal setup.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, app, client, &mut rx, tx.clone()).await;

    // Restore terminal.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

/// Messages from background async tasks back to the main loop.
pub(crate) enum AppMsg {
    FetchAll(gql::FetchAll),
    DailyReview(gql::DailyReview),
    DetailData(i32, Vec<gql::TodoEvent>),
    Toast(ToastKind, String),
    Refresh,
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    mut app: App,
    client: gql::Client,
    rx: &mut mpsc::Receiver<AppMsg>,
    tx: mpsc::Sender<AppMsg>,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| views::render(f, &mut app))?;

        // Poll events with a 100ms timeout so the spinner animates.
        let poll = event::poll(Duration::from_millis(100))?;
        if poll {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if handle_key(&mut app, key.code, &client, &tx).await? {
                    break;
                }
            }
        }

        // Drain async messages.
        while let Ok(msg) = rx.try_recv() {
            match msg {
                AppMsg::FetchAll(d) => {
                    app.data = crate::app::AppData::from_fetch_all(d);
                    app.clamp_cursor_to_active();
                }
                AppMsg::DailyReview(r) => app.review = Some(r),
                AppMsg::DetailData(id, events) => {
                    app.detail = Some(crate::app::DetailData {
                        tags: Vec::new(),
                        events,
                    });
                    let _ = id;
                }
                AppMsg::Toast(k, m) => app.set_toast(k, m),
                AppMsg::Refresh => {
                    let date = app.logical_date.format("%Y-%m-%d").to_string();
                    let c = client.clone();
                    let t = tx.clone();
                    tokio::spawn(async move {
                        match c.fetch_all(&date).await {
                            Ok(d) => {
                                let _ = t.send(AppMsg::FetchAll(d)).await;
                            }
                            Err(e) => {
                                let _ =
                                    t.send(AppMsg::Toast(ToastKind::Error, e.to_string())).await;
                            }
                        }
                    });
                }
            }
        }

        app.tick_spinner();
    }
    Ok(())
}

/// Handle a key. Returns Ok(true) to quit.
pub(crate) async fn handle_key(
    app: &mut App,
    key: KeyCode,
    client: &gql::Client,
    tx: &mpsc::Sender<AppMsg>,
) -> anyhow::Result<bool> {
    use KeyCode::*;

    // Global keys.
    match key {
        Char('q') if app.mode == Mode::Navigate => return Ok(true),
        Tab if app.mode == Mode::Navigate => {
            let idx = View::ALL.iter().position(|v| *v == app.view).unwrap_or(0);
            app.view = View::ALL[(idx + 1) % View::ALL.len()];
            app.cursor = 0;
            app.clamp_cursor_to_active();
            return Ok(false);
        }
        BackTab if app.mode == Mode::Navigate => {
            let idx = View::ALL.iter().position(|v| *v == app.view).unwrap_or(0);
            app.view = View::ALL[(idx + View::ALL.len() - 1) % View::ALL.len()];
            app.cursor = 0;
            app.clamp_cursor_to_active();
            return Ok(false);
        }
        Char('?') if app.mode == Mode::Navigate => {
            app.mode = Mode::Help {
                filter: String::new(),
            };
            app.help_scroll = 0;
            return Ok(false);
        }
        Esc => {
            match app.mode {
                Mode::Navigate => {
                    app.toast = None;
                    return Ok(false);
                }
                Mode::SidebarEdit { .. } => {
                    // Let the handler own the Esc logic
                }
                _ => {
                    app.mode = Mode::Navigate;
                    app.toast = None;
                    return Ok(false);
                }
            }
        }
        _ => {}
    }

    // Mode-specific handling. Clone the mode-matching data so we don't hold
    // an immutable borrow of `app` while the handlers need `&mut app`.
    match app.mode.clone() {
        Mode::Navigate => views::handle_navigate(app, key, client, tx).await,
        Mode::InlineCreate { input } => {
            views::handle_inline_create(app, key, client, tx, &input).await
        }
        Mode::InlineEdit { id, input } => {
            views::handle_inline_edit(app, key, client, tx, id, &input).await
        }
        Mode::Search { input } => views::handle_search(app, key, &input),
        Mode::Reorder { source_id } => views::handle_reorder(app, key, client, tx, source_id).await,
        Mode::Confirm { action } => views::handle_confirm(app, key, client, tx, &action).await,
        Mode::Help { filter } => views::handle_help(app, key, &filter),
        Mode::StatusSelect {
            id,
            selection,
            reason,
        } => views::handle_status_select(app, key, client, tx, id, selection, reason).await,
        Mode::SidebarEdit { .. } => views::handle_sidebar_edit(app, key, client, tx).await,
    }
}

/// Compute the content area available to a view (inside the frame chrome).
pub fn content_area(f: &Frame) -> Rect {
    let area = f.area();
    // Frame: top border (1) + tabs (1) + rule (1) + content + blank (1) + status (1).
    ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(3),
        ratatui::layout::Constraint::Min(0),
        ratatui::layout::Constraint::Length(2),
    ])
    .split(area)[1]
}

#[cfg(test)]
pub(crate) mod test_support {
    /// Extract visible text from a TestBackend buffer, one line per row,
    /// trimming trailing whitespace.
    pub(crate) fn buffer_text(buf: &ratatui::buffer::Buffer) -> String {
        let area = buf.area;
        (0..area.height)
            .map(|y| {
                let line: String = (0..area.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect();
                line.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
