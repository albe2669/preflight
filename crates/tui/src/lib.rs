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
pub mod widgets;

pub(crate) use app::AppMsg;

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use tokio::sync::mpsc;

use app::App;

/// Run the TUI. Connects to the GraphQL backend at `endpoint` (e.g.
/// `http://127.0.0.1:8000/`).
pub async fn run(endpoint: &str) -> anyhow::Result<()> {
    let client = gql::Client::new(endpoint);

    let mut app = App::default();
    let date_str = match client.clock().await {
        Ok(clock) => {
            app.day_start_hour = clock.day_start_hour;
            if let Ok(d) = clock.logical_date.parse::<chrono::NaiveDate>() {
                app.logical_date = d;
                app.review_date = d - chrono::Duration::days(1);
            }
            clock.logical_date
        }
        Err(_) => chrono::Local::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string(),
    };

    let fetched = client.fetch_all(&date_str).await;
    match fetched {
        Ok(d) => {
            app.data = crate::app::AppData::from_fetch_all(d);
            app.clamp_cursor_to_active();
            app.maybe_fetch_cursor_detail();
        }
        Err(e) => app.set_error(format!("connect failed: {e}")),
    }

    let (tx, mut rx) = mpsc::channel::<AppMsg>(32);
    app.init(client, tx);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, app, &mut rx).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    mut app: App,
    rx: &mut mpsc::Receiver<AppMsg>,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| views::render(f, &mut app))?;

        let poll = event::poll(Duration::from_millis(100))?;
        if poll {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if app.handle_key(key) {
                    break;
                }
            }
        }

        while let Ok(msg) = rx.try_recv() {
            app.handle_msg(msg);
        }

        app.tick_spinner();
    }
    Ok(())
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
