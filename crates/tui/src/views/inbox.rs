//! Inbox view — triage surface for PRs and Linear issues.
//! Grouped by source; each group carries its own sync indicator.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};
use tokio::sync::mpsc;

use crate::app::{App, Mode, ToastKind};
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
            format!("{}/{}#{}", pr.owner, pr.repo, pr.number),
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
        KeyCode::Char('L') => {
            // Link the selected row to an existing todo.
            let prs = app.inbox_prs();
            let linears = app.inbox_linears();
            let mode = if app.cursor < prs.len() {
                Some(Mode::LinkTodo {
                    pr_id: Some(prs[app.cursor].id),
                    linear_issue_id: None,
                    selection: 0,
                })
            } else {
                let li_idx = app.cursor - prs.len();
                linears.get(li_idx).map(|li| Mode::LinkTodo {
                    pr_id: None,
                    linear_issue_id: Some(li.id),
                    selection: 0,
                })
            };
            if let Some(m) = mode {
                app.mode = m;
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

#[cfg(test)]
mod navigate_tests {
    use ratatui::crossterm::event::KeyCode;
    use tokio::sync::mpsc;

    use crate::app::tests::{make_linear, make_pr, make_todo};
    use crate::app::{App, Mode, ToastKind};
    use crate::gql;

    fn test_client() -> (gql::Client, mpsc::Sender<crate::AppMsg>) {
        let client = gql::Client::new("http://127.0.0.1:0");
        let (tx, _rx) = mpsc::channel::<crate::AppMsg>(32);
        (client, tx)
    }

    #[tokio::test]
    async fn test_l_on_pr_row_opens_link_todo_mode_with_pr_id() {
        let (client, tx) = test_client();
        let mut app = App::default();
        app.data.pulls = vec![make_pr(5, None)];
        app.cursor = 0;

        super::handle_navigate(&mut app, KeyCode::Char('L'), &client, &tx)
            .await
            .unwrap();

        assert_eq!(
            app.mode,
            Mode::LinkTodo {
                pr_id: Some(5),
                linear_issue_id: None,
                selection: 0
            },
            "L on a PR row should open the link-to-todo picker with the PR id set"
        );
    }

    #[tokio::test]
    async fn test_l_on_linear_row_opens_link_todo_mode_with_issue_id() {
        let (client, tx) = test_client();
        let mut app = App::default();
        app.data.linears = vec![make_linear(9, None)];
        app.cursor = 0;

        super::handle_navigate(&mut app, KeyCode::Char('L'), &client, &tx)
            .await
            .unwrap();

        assert_eq!(
            app.mode,
            Mode::LinkTodo {
                pr_id: None,
                linear_issue_id: Some(9),
                selection: 0
            },
            "L on a Linear row should open the picker with the issue id set"
        );
    }

    #[tokio::test]
    async fn test_l_out_of_range_does_nothing() {
        let (client, tx) = test_client();
        let mut app = App {
            cursor: 5, // no rows
            ..Default::default()
        };

        super::handle_navigate(&mut app, KeyCode::Char('L'), &client, &tx)
            .await
            .unwrap();

        assert_eq!(app.mode, Mode::Navigate, "L out of range is a no-op");
    }

    /// Minimal GraphQL mock: captures posted bodies, replies with the matching
    /// mutation payload so the client decodes success.
    async fn spawn_gql_mock() -> (
        String,
        std::sync::Arc<parking_lot::Mutex<Vec<serde_json::Value>>>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let captures: std::sync::Arc<parking_lot::Mutex<Vec<serde_json::Value>>> =
            std::sync::Arc::new(parking_lot::Mutex::new(Vec::new()));
        let caps = captures.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let g = caps.clone();
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut buf = Vec::new();
                    let mut tmp = [0u8; 1024];
                    loop {
                        let n = socket.read(&mut tmp).await.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&tmp[..n]);
                        if buf.len() > 1_000_000 || buf.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    let op_name = if let Some(idx) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        if let Ok(val) =
                            serde_json::from_slice::<serde_json::Value>(&buf[idx + 4..])
                        {
                            g.lock().push(val.clone());
                            val.get("operationName")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string()
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };
                    let todo = r#"{"id":1,"title":"T","description":null,"status":"todo","blockedReason":null,"sortKey":1,"createdAt":"2026-08-05 10:00:00 +00:00","startedAt":null,"closedAt":null,"tag":{"nodes":[]}}"#;
                    let body = if op_name.contains("LinkPullRequest") {
                        format!(r#"{{"data":{{"linkPullRequest":{todo}}}}}"#)
                    } else if op_name.contains("LinkLinearIssue") {
                        format!(r#"{{"data":{{"linkLinearIssue":{todo}}}}}"#)
                    } else {
                        r#"{"data":{}}"#.to_string()
                    };
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    let _ = socket.flush().await;
                });
            }
        });
        (format!("http://{addr}"), captures)
    }

    fn enter_link_todo(app: &mut App) {
        app.mode = Mode::LinkTodo {
            pr_id: Some(10),
            linear_issue_id: None,
            selection: 0,
        };
    }

    #[tokio::test]
    async fn test_link_todo_confirm_pr_posts_link_pull_request() {
        let (base_url, captures) = spawn_gql_mock().await;
        let client = gql::Client::new(&base_url);
        let (tx, mut rx) = mpsc::channel::<crate::AppMsg>(32);
        let mut app = App::default();
        app.data.todos = vec![make_todo(1, "My Task", "todo")];
        enter_link_todo(&mut app);

        crate::views::handle_link_todo(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();

        assert_eq!(app.mode, Mode::Navigate, "confirm exits the picker");
        for _ in 0..20 {
            if !captures.lock().is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        {
            let captured = captures.lock();
            assert_eq!(captured.len(), 1, "confirm should post a link mutation");
            let op_name = captured[0]
                .get("operationName")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            assert!(
                op_name.contains("linkPullRequest") || op_name.contains("LinkPullRequest"),
                "expected linkPullRequest operation, got {op_name:?}"
            );
            let vars = captured[0].get("variables").cloned().unwrap_or_default();
            assert_eq!(vars.get("todoId").and_then(|v| v.as_i64()), Some(1));
            assert_eq!(vars.get("prId").and_then(|v| v.as_i64()), Some(10));
            assert_eq!(
                vars.get("relation").and_then(|v| v.as_str()),
                Some("references"),
                "default relation should be References"
            );
        }
        // Success path sends a toast + Refresh to the channel.
        let mut saw_refresh = false;
        let mut saw_toast = false;
        for _ in 0..20 {
            match rx.try_recv() {
                Ok(crate::AppMsg::Refresh) => saw_refresh = true,
                Ok(crate::AppMsg::Toast(ToastKind::Success, _)) => saw_toast = true,
                Ok(_) => {}
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(10)).await,
            }
            if saw_refresh && saw_toast {
                break;
            }
        }
        assert!(saw_toast, "success should raise a toast");
        assert!(saw_refresh, "success should trigger a refresh");
    }

    #[tokio::test]
    async fn test_link_todo_confirm_linear_posts_link_linear_issue() {
        let (base_url, captures) = spawn_gql_mock().await;
        let client = gql::Client::new(&base_url);
        let (tx, _rx) = mpsc::channel(32);
        let mut app = App::default();
        app.data.todos = vec![make_todo(1, "My Task", "todo")];
        app.mode = Mode::LinkTodo {
            pr_id: None,
            linear_issue_id: Some(7),
            selection: 0,
        };

        crate::views::handle_link_todo(&mut app, KeyCode::Enter, &client, &tx)
            .await
            .unwrap();

        for _ in 0..20 {
            if !captures.lock().is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        let captured = captures.lock();
        assert_eq!(captured.len(), 1, "confirm should post a link mutation");
        let op_name = captured[0]
            .get("operationName")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(
            op_name.contains("linkLinearIssue") || op_name.contains("LinkLinearIssue"),
            "expected linkLinearIssue operation, got {op_name:?}"
        );
        let vars = captured[0].get("variables").cloned().unwrap_or_default();
        assert_eq!(vars.get("todoId").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(vars.get("issueId").and_then(|v| v.as_i64()), Some(7));
    }

    #[tokio::test]
    async fn test_link_todo_esc_cancels_without_mutating() {
        let (base_url, captures) = spawn_gql_mock().await;
        let client = gql::Client::new(&base_url);
        let (tx, _rx) = mpsc::channel(32);
        let mut app = App::default();
        app.data.todos = vec![make_todo(1, "My Task", "todo")];
        enter_link_todo(&mut app);

        crate::views::handle_link_todo(&mut app, KeyCode::Esc, &client, &tx)
            .await
            .unwrap();

        assert_eq!(app.mode, Mode::Navigate, "Esc exits the picker");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(captures.lock().is_empty(), "Esc must not issue a mutation");
    }

    fn render_link_picker(app: &mut App) -> String {
        let backend = ratatui::backend::TestBackend::new(120, 40);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|f| crate::views::render(f, app)).unwrap();
        crate::test_support::buffer_text(terminal.backend().buffer())
    }

    #[test]
    fn test_link_todo_picker_lists_todos() {
        let mut app = App {
            view: crate::app::View::Inbox,
            mode: Mode::LinkTodo {
                pr_id: Some(10),
                linear_issue_id: None,
                selection: 0,
            },
            data: crate::app::AppData {
                todos: vec![
                    make_todo(1, "Alpha Todo", "todo"),
                    make_todo(2, "Beta Todo", "started"),
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        let output = render_link_picker(&mut app);
        assert!(
            output.contains("LINK TO TODO"),
            "picker should show a header:\n{output}"
        );
        assert!(
            output.contains("Alpha Todo"),
            "picker should list todo Alpha:\n{output}"
        );
        assert!(
            output.contains("Beta Todo"),
            "picker should list todo Beta:\n{output}"
        );
    }

    // -- C/d/o dispatch regression tests (task 2) --

    #[tokio::test]
    async fn test_c_on_pr_row_issues_todo_from_pr_mutation() {
        let (base_url, captures) = spawn_gql_mock().await;
        let client = gql::Client::new(&base_url);
        let (tx, _rx) = mpsc::channel(32);
        let mut app = App::default();
        app.data.pulls = vec![make_pr(5, None)];
        app.cursor = 0;

        super::handle_navigate(&mut app, KeyCode::Char('C'), &client, &tx)
            .await
            .unwrap();

        for _ in 0..20 {
            if !captures.lock().is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        let captured = captures.lock();
        assert_eq!(captured.len(), 1, "C on a PR row should post a mutation");
        let op_name = captured[0]
            .get("operationName")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(
            op_name.contains("TodoFromPr"),
            "expected todoFromPullRequest operation, got {op_name:?}"
        );
        let vars = captured[0].get("variables").cloned().unwrap_or_default();
        assert_eq!(vars.get("prId").and_then(|v| v.as_i64()), Some(5));
        assert_eq!(vars.get("planToday").and_then(|v| v.as_bool()), Some(true));
    }

    #[tokio::test]
    async fn test_c_on_linear_row_issues_todo_from_linear_mutation() {
        let (base_url, captures) = spawn_gql_mock().await;
        let client = gql::Client::new(&base_url);
        let (tx, _rx) = mpsc::channel(32);
        let mut app = App::default();
        app.data.linears = vec![make_linear(9, None)];
        app.cursor = 0;

        super::handle_navigate(&mut app, KeyCode::Char('C'), &client, &tx)
            .await
            .unwrap();

        for _ in 0..20 {
            if !captures.lock().is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        let captured = captures.lock();
        assert_eq!(
            captured.len(),
            1,
            "C on a Linear row should post a mutation"
        );
        let op_name = captured[0]
            .get("operationName")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(
            op_name.contains("TodoFromLinear"),
            "expected todoFromLinearIssue operation, got {op_name:?}"
        );
        let vars = captured[0].get("variables").cloned().unwrap_or_default();
        assert_eq!(vars.get("issueId").and_then(|v| v.as_i64()), Some(9));
        assert_eq!(vars.get("planToday").and_then(|v| v.as_bool()), Some(true));
    }

    #[tokio::test]
    async fn test_d_on_pr_row_issues_dismiss_mutation() {
        let (base_url, captures) = spawn_gql_mock().await;
        let client = gql::Client::new(&base_url);
        let (tx, _rx) = mpsc::channel(32);
        let mut app = App::default();
        app.data.pulls = vec![make_pr(5, None)];
        app.cursor = 0;

        super::handle_navigate(&mut app, KeyCode::Char('d'), &client, &tx)
            .await
            .unwrap();

        for _ in 0..20 {
            if !captures.lock().is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        let captured = captures.lock();
        assert_eq!(captured.len(), 1, "d on a PR row should post a mutation");
        let op_name = captured[0]
            .get("operationName")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert_eq!(
            op_name, "DismissPrMut",
            "expected dismissPullRequest, got {op_name:?}"
        );
    }

    #[tokio::test]
    async fn test_d_on_linear_row_is_a_noop() {
        let (base_url, captures) = spawn_gql_mock().await;
        let client = gql::Client::new(&base_url);
        let (tx, _rx) = mpsc::channel(32);
        let mut app = App::default();
        app.data.linears = vec![make_linear(9, None)];
        app.cursor = 0;

        super::handle_navigate(&mut app, KeyCode::Char('d'), &client, &tx)
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            captures.lock().is_empty(),
            "d on a Linear row must not issue a mutation"
        );
    }

    #[tokio::test]
    async fn test_o_on_pr_row_surfaces_url() {
        let (client, tx) = test_client();
        let mut app = App::default();
        app.data.pulls = vec![make_pr(5, None)];
        app.cursor = 0;

        super::handle_navigate(&mut app, KeyCode::Char('o'), &client, &tx)
            .await
            .unwrap();

        assert_eq!(app.mode, Mode::Navigate);
        let toast = app.toast.expect("o should raise a toast with the URL");
        assert_eq!(toast.kind, ToastKind::Success);
        assert!(
            toast.message.contains("example.com"),
            "o should surface the PR url, got {:?}",
            toast.message
        );
    }

    #[tokio::test]
    async fn test_o_on_linear_row_surfaces_url() {
        let (client, tx) = test_client();
        let mut app = App::default();
        app.data.linears = vec![make_linear(9, None)];
        app.cursor = 0;

        super::handle_navigate(&mut app, KeyCode::Char('o'), &client, &tx)
            .await
            .unwrap();

        let toast = app.toast.expect("o should raise a toast with the URL");
        assert_eq!(toast.kind, ToastKind::Success);
        assert!(
            toast.message.contains("example.com"),
            "o should surface the Linear url, got {:?}",
            toast.message
        );
    }
}
