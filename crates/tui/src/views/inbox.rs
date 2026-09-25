//! Inbox view — triage surface for PRs and Linear issues.
//! Grouped by source; each group carries its own sync indicator.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Row, Table, TableState};

use crate::app::{App, Mode, ToastKind};
use crate::theme::{Glyph, Palette};
use crate::widgets;

/// PRs in the order the inbox table displays them: filtered by the active
/// search, then sorted by (owner, repo). The state sort from `inbox_prs`
/// (closed/merged last) is applied first, so the repo sort stays stable
/// within open and closed groups — matching what `render` displays.
fn inbox_display_prs(app: &App) -> Vec<&crate::gql::PullRequest> {
    let filter = match &app.mode {
        Mode::Filter { input, .. } => input.clone(),
        _ => app.filter.clone(),
    };
    let mut prs: Vec<&crate::gql::PullRequest> = app
        .inbox_prs()
        .into_iter()
        .filter(|p| crate::views::overlays::pr_matches_search(p, &filter))
        .collect();
    prs.sort_by(|a, b| {
        (a.owner.as_str(), a.repo.as_str()).cmp(&(b.owner.as_str(), b.repo.as_str()))
    });
    prs
}

/// Linear issues in the order the inbox table displays them: filtered by
/// the active search. The state sort from `inbox_linears` (active before
/// terminal) is preserved — matching what `render` displays.
fn inbox_display_linears(app: &App) -> Vec<&crate::gql::LinearIssue> {
    let filter = match &app.mode {
        Mode::Filter { input, .. } => input.clone(),
        _ => app.filter.clone(),
    };
    app.inbox_linears()
        .into_iter()
        .filter(|l| crate::views::overlays::linear_matches_search(l, &filter))
        .collect()
}

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let prs = inbox_display_prs(app);
    let linears = inbox_display_linears(app);

    let pr_sync = app.sync_by_source("github");
    let pr_sync_label = sync_label(pr_sync, app.spinner, app.syncing.contains("github"));
    let lin_sync = app.sync_by_source("linear");
    let lin_sync_label = sync_label(lin_sync, app.spinner, app.syncing.contains("linear"));

    let header = Row::new(vec![
        Cell::from(Span::styled(
            "ST",
            Style::default()
                .fg(Palette::ACCENT)
                .add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "REPO",
            Style::default()
                .fg(Palette::ACCENT)
                .add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "AUTHOR",
            Style::default()
                .fg(Palette::ACCENT)
                .add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "TITLE",
            Style::default()
                .fg(Palette::ACCENT)
                .add_modifier(Modifier::BOLD),
        )),
    ])
    .bottom_margin(0);

    let mut rows: Vec<Row> = Vec::new();
    // Map flat item index (cursor space) -> table row index.
    let mut cursor_to_row: Vec<usize> = Vec::new();
    let mut row_idx: usize = 0;

    let hidden_prs = if app.show_closed {
        0
    } else {
        app.data
            .pulls
            .iter()
            .filter(|p| app.show_dismissed || p.dismissed_at.is_none())
            .filter(|p| matches!(p.state.as_str(), "closed" | "merged"))
            .count()
    };
    let pr_header = format!(
        "GITHUB · {} pull requests · {}{}",
        prs.len(),
        pr_sync_label,
        if hidden_prs > 0 {
            format!(" · {} closed/merged hidden", hidden_prs)
        } else {
            String::new()
        }
    );
    rows.push(group_header_row(&pr_header));
    row_idx += 1;

    let mut seen_closed = false;
    let mut current_repo: Option<(String, String)> = None;
    for (i, pr) in prs.iter().enumerate() {
        let repo_key = (pr.owner.clone(), pr.repo.clone());
        if current_repo.as_ref() != Some(&repo_key) {
            if current_repo.is_some() {
                rows.push(repo_divider_row(&format!("{}/{}", pr.owner, pr.repo)));
                row_idx += 1;
            }
            current_repo = Some(repo_key);
        }
        let is_closed = matches!(pr.state.as_str(), "closed" | "merged");
        if is_closed && !seen_closed {
            rows.push(sub_header_row("CLOSED / MERGED"));
            row_idx += 1;
            seen_closed = true;
        }
        cursor_to_row.push(row_idx);
        rows.push(pr_row(pr, i == app.cursor));
        row_idx += 1;
    }

    rows.push(Row::new(vec![Cell::from("")]));
    row_idx += 1;
    let hidden_linears = if app.show_closed {
        0
    } else {
        app.data
            .linears
            .iter()
            .filter(|l| app.show_dismissed || l.dismissed_at.is_none())
            .filter(|l| matches!(l.state_type.as_str(), "completed" | "canceled"))
            .count()
    };
    let lin_header = format!(
        "LINEAR · {} issues · {}{}",
        linears.len(),
        lin_sync_label,
        if hidden_linears > 0 {
            format!(" · {} completed/canceled hidden", hidden_linears)
        } else {
            String::new()
        }
    );
    rows.push(group_header_row(&lin_header));
    row_idx += 1;

    let mut seen_done = false;
    for (i, li) in linears.iter().enumerate() {
        let is_done = matches!(li.state_type.as_str(), "completed" | "canceled");
        if is_done && !seen_done {
            rows.push(sub_header_row("COMPLETED / CANCELED"));
            row_idx += 1;
            seen_done = true;
        }
        cursor_to_row.push(row_idx);
        rows.push(linear_row(li, i + prs.len() == app.cursor));
        row_idx += 1;
    }

    let widths = [
        Constraint::Length(4),
        Constraint::Length(22),
        Constraint::Length(14),
        Constraint::Min(10),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .style(Style::default().bg(Palette::BG).fg(Palette::TEXT))
        .row_highlight_style(Style::default().bg(Palette::ROW_HIGHLIGHT))
        .highlight_symbol("");

    let mut state = TableState::default();
    let selected = cursor_to_row.get(app.cursor).copied();
    state.select(selected);
    f.render_stateful_widget(table, area, &mut state);
}

pub fn render_legend_sidebar(f: &mut Frame, area: Rect) {
    use ratatui::widgets::{Block, Borders, Paragraph};

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Palette::BORDER))
        .title(Span::styled(" LEGEND ", Style::default().fg(Palette::DIM)));
    f.render_widget(block, area);

    let inner = Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    if inner.height == 0 {
        return;
    }

    let entries: &[(char, &str)] = &[
        (Glyph::PR_OPEN, "open PR"),
        (Glyph::PR_DRAFT, "draft PR"),
        (Glyph::PR_MERGED, "merged PR"),
        (Glyph::PR_CLOSED, "closed PR"),
        (Glyph::STATUS_DONE, "approved"),
        (Glyph::CHANGES_REQUESTED, "changes requested"),
        (Glyph::COPILOT_COMMENTS, "copilot comments"),
        (Glyph::MERGE_CONFLICTS, "merge conflicts"),
        (Glyph::ACTIONS_FAILING, "CI actions failing"),
        (Glyph::NEEDS_YOU, "needs your review"),
        ('?', "Linear triage"),
        ('·', "Linear backlog"),
        (Glyph::STATUS_TODO, "Linear unstarted"),
        (Glyph::STATUS_STARTED, "Linear started"),
    ];
    let mut lines: Vec<Line> = Vec::new();
    for (glyph, desc) in entries {
        lines.push(Line::from(vec![
            Span::styled(glyph.to_string(), Style::default().fg(Palette::ACCENT)),
            Span::raw(format!(" {desc}")),
        ]));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

/// Sync-state label for a group header.
fn sync_label(sync: Option<&crate::gql::SyncState>, spinner: u8, syncing: bool) -> String {
    if syncing {
        let frame = Glyph::SPINNER[(spinner / 2) as usize % Glyph::SPINNER.len()];
        return format!("{} syncing…", frame);
    }
    match sync {
        Some(s) if s.last_status == "syncing" || s.last_status == "in_progress" => {
            let frame = Glyph::SPINNER[(spinner / 2) as usize % Glyph::SPINNER.len()];
            format!("{} syncing…", frame)
        }
        Some(s) if s.last_status == "ok" || s.last_status == "success" => "↻ ok".to_string(),
        Some(s) if s.last_status == "error" => "▲ error".to_string(),
        Some(_) | None => "⌀ no token".to_string(),
    }
}

/// A full-width section header row; the label sits in the wide title column.
fn group_header_row(label: &str) -> Row<'static> {
    Row::new(vec![
        Cell::from(""),
        Cell::from(""),
        Cell::from(""),
        Cell::from(Span::styled(
            format!("╶ {label}"),
            Style::default().fg(Palette::DIM),
        )),
    ])
    .height(1)
}

/// A dim sub-section header for closed/done items.
fn sub_header_row(label: &str) -> Row<'static> {
    Row::new(vec![
        Cell::from(""),
        Cell::from(""),
        Cell::from(""),
        Cell::from(Span::styled(
            label.to_string(),
            Style::default().fg(Palette::GHOST),
        )),
    ])
    .height(1)
}

/// A dim divider row separating PR groups by repository.
fn repo_divider_row(label: &str) -> Row<'static> {
    Row::new(vec![
        Cell::from(""),
        Cell::from(""),
        Cell::from(""),
        Cell::from(Span::styled(
            format!("╶ {label}"),
            Style::default().fg(Palette::BORDER),
        )),
    ])
    .height(1)
}

/// Build the status cell for a PR: state glyph plus extra review icons.
fn pr_status_spans(pr: &crate::gql::PullRequest) -> Vec<Span<'static>> {
    let (g, c) = widgets::pr_state_glyph(&pr.state);
    let mut spans = vec![Span::styled(g.to_string(), Style::default().fg(c))];
    if pr.approved {
        spans.push(Span::styled(
            Glyph::STATUS_DONE.to_string(),
            Style::default().fg(Palette::DONE),
        ));
    }
    if pr.changes_requested {
        spans.push(Span::styled(
            Glyph::CHANGES_REQUESTED.to_string(),
            Style::default().fg(Palette::BLOCKED),
        ));
    }
    if pr.copilot_comments {
        spans.push(Span::styled(
            Glyph::COPILOT_COMMENTS.to_string(),
            Style::default().fg(Palette::ACCENT),
        ));
    }
    if pr.merge_conflicts {
        spans.push(Span::styled(
            Glyph::MERGE_CONFLICTS.to_string(),
            Style::default().fg(Palette::BLOCKED),
        ));
    }
    if pr.actions_failing {
        spans.push(Span::styled(
            Glyph::ACTIONS_FAILING.to_string(),
            Style::default().fg(Palette::BLOCKED),
        ));
    }
    if pr.review_requested {
        spans.push(Span::styled(
            Glyph::NEEDS_YOU.to_string(),
            Style::default().fg(Palette::ACCENT),
        ));
    }
    spans
}

/// A PR row: status icons, owner/repo, author, title.
fn pr_row(pr: &crate::gql::PullRequest, is_cursor: bool) -> Row<'static> {
    let title_style = if pr.dismissed_at.is_some() {
        Style::default()
            .fg(Palette::GHOST)
            .add_modifier(Modifier::CROSSED_OUT)
    } else {
        Style::default().fg(Palette::TEXT)
    };
    let repo = format!("{}/{}#{}", pr.owner, pr.repo, pr.number);
    let author = pr.author.clone().unwrap_or_else(|| "—".to_string());
    let mut title_spans = vec![Span::styled(pr.title.clone(), title_style)];
    if pr.dismissed_at.is_some() {
        title_spans.push(Span::styled(
            "  dismissed",
            Style::default().fg(Palette::GHOST),
        ));
    }
    let mut status_spans = pr_status_spans(pr);
    if is_cursor {
        status_spans.insert(
            0,
            Span::styled(
                Glyph::CURSOR.to_string(),
                Style::default().fg(Palette::ACCENT),
            ),
        );
    } else {
        status_spans.insert(0, Span::raw(" "));
    }
    Row::new(vec![
        Cell::from(Line::from(status_spans)),
        Cell::from(Span::styled(
            repo,
            Style::default().fg(if pr.dismissed_at.is_some() {
                Palette::DIM
            } else {
                Palette::TEXT
            }),
        )),
        Cell::from(Span::styled(author, Style::default().fg(Palette::DIM))),
        Cell::from(Line::from(title_spans)),
    ])
}

/// A Linear row: status icon, identifier, assignee, title.
fn linear_row(li: &crate::gql::LinearIssue, is_cursor: bool) -> Row<'static> {
    let (g, c) = widgets::linear_state_glyph(&li.state_type);
    let mut status = vec![];
    if is_cursor {
        status.push(Span::styled(
            Glyph::CURSOR.to_string(),
            Style::default().fg(Palette::ACCENT),
        ));
    } else {
        status.push(Span::raw(" "));
    }
    status.push(Span::styled(g.to_string(), Style::default().fg(c)));
    if li.assigned_to_me {
        status.push(Span::styled(
            Glyph::NEEDS_YOU.to_string(),
            Style::default().fg(Palette::ACCENT),
        ));
    }
    let title_style = if li.dismissed_at.is_some() {
        Style::default()
            .fg(Palette::GHOST)
            .add_modifier(Modifier::CROSSED_OUT)
    } else {
        Style::default().fg(Palette::TEXT)
    };
    let mut title_spans = vec![Span::styled(li.title.clone(), title_style)];
    title_spans.push(Span::styled(
        format!(
            "  {} {}",
            li.state_name,
            li.team_key.as_deref().unwrap_or("")
        ),
        Style::default().fg(Palette::DIM),
    ));
    let author = li.assignee_name.clone().unwrap_or_else(|| "—".to_string());
    Row::new(vec![
        Cell::from(Line::from(status)),
        Cell::from(Span::styled(
            li.identifier.clone(),
            Style::default().fg(if li.dismissed_at.is_some() {
                Palette::DIM
            } else {
                Palette::TEXT
            }),
        )),
        Cell::from(Span::styled(author, Style::default().fg(Palette::DIM))),
        Cell::from(Line::from(title_spans)),
    ])
}

fn open_url(url: &str) {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(cmd).arg(url).spawn();
}

pub(crate) fn handle_navigate(app: &mut App, key: KeyCode) {
    let total = inbox_display_prs(app).len() + inbox_display_linears(app).len();
    match key {
        KeyCode::Char('j') | KeyCode::Down if app.cursor + 1 < total => {
            app.cursor += 1;
        }
        KeyCode::Char('k') | KeyCode::Up if app.cursor > 0 => {
            app.cursor -= 1;
        }
        KeyCode::Char('C') => {
            let prs = inbox_display_prs(app);
            let linears = inbox_display_linears(app);
            if app.cursor < prs.len() {
                app.spawn_todo_from_pr(prs[app.cursor].id, true);
            } else {
                let li_idx = app.cursor - prs.len();
                if let Some(li) = linears.get(li_idx) {
                    app.spawn_todo_from_linear(li.id, true);
                }
            }
        }
        KeyCode::Char('d') => {
            let prs = inbox_display_prs(app);
            if app.cursor < prs.len() {
                app.spawn_dismiss_pr(prs[app.cursor].id);
            }
        }
        KeyCode::Char('D') => {
            app.show_closed = !app.show_closed;
            app.cursor = 0;
        }
        KeyCode::Char('s') => {
            let prs = inbox_display_prs(app);
            if app.cursor < prs.len() {
                app.spawn_sync_github();
            } else {
                app.spawn_sync_linear();
            }
        }
        KeyCode::Char('o') => {
            let prs = inbox_display_prs(app);
            let linears = inbox_display_linears(app);
            let url = if app.cursor < prs.len() {
                Some(prs[app.cursor].url.clone())
            } else {
                let li_idx = app.cursor - prs.len();
                linears.get(li_idx).map(|l| l.url.clone())
            };
            if let Some(url) = url {
                open_url(&url);
                app.set_toast(ToastKind::Success, format!("opened {url}"));
            }
        }
        KeyCode::Char('L') => {
            let prs = inbox_display_prs(app);
            let linears = inbox_display_linears(app);
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
        KeyCode::Char('/') => {
            app.mode = Mode::Filter {
                input: String::new(),
                caret: 0,
            };
        }
        _ => {}
    }
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

    #[test]
    fn test_inbox_renders_table_with_column_headers() {
        let mut app = App::default();
        app.data.pulls = vec![make_pr(1, None)];
        app.data.linears = vec![];
        let buffer = render_inbox(&mut app);
        // The Table renders a header row with ST, REPO, AUTHOR, TITLE.
        assert!(
            buffer.contains("REPO"),
            "expected REPO column header:\n{buffer}"
        );
        assert!(
            buffer.contains("AUTHOR"),
            "expected AUTHOR column header:\n{buffer}"
        );
        assert!(
            buffer.contains("TITLE"),
            "expected TITLE column header:\n{buffer}"
        );
    }

    #[test]
    fn test_inbox_orders_closed_prs_after_open() {
        let mut app = App {
            show_closed: true,
            ..Default::default()
        };
        app.data.pulls = vec![
            crate::gql::PullRequest {
                id: 1,
                owner: "owner".into(),
                repo: "repo".into(),
                number: 1,
                title: "ClosedOne".into(),
                url: "x".into(),
                author: None,
                state: "closed".into(),
                review_requested: false,
                authored_by_me: false,
                changes_requested: false,
                copilot_comments: false,
                merge_conflicts: false,
                approved: false,
                actions_failing: false,
                dismissed_at: None,
                remote_updated_at: None,
            },
            make_pr(2, None),
        ];
        let buffer = render_inbox(&mut app);
        let open_idx = buffer.find("PR #2");
        let closed_idx = buffer.find("ClosedOne");
        assert!(open_idx.is_some(), "open PR should appear:\n{buffer}");
        assert!(closed_idx.is_some(), "closed PR should appear:\n{buffer}");
        assert!(
            open_idx < closed_idx,
            "open PR should appear before closed PR:\n{buffer}"
        );
    }

    #[test]
    fn test_inbox_hides_closed_prs_by_default() {
        let mut app = App::default();
        app.data.pulls = vec![
            make_pr(1, None),
            crate::gql::PullRequest {
                id: 2,
                owner: "owner".into(),
                repo: "repo".into(),
                number: 2,
                title: "ClosedOne".into(),
                url: "x".into(),
                author: None,
                state: "closed".into(),
                review_requested: false,
                authored_by_me: false,
                changes_requested: false,
                copilot_comments: false,
                merge_conflicts: false,
                approved: false,
                actions_failing: false,
                dismissed_at: None,
                remote_updated_at: None,
            },
        ];
        let buffer = render_inbox(&mut app);
        assert!(buffer.contains("PR #1"), "open PR should appear");
        assert!(!buffer.contains("ClosedOne"), "closed PR should be hidden");
        assert!(
            buffer.contains("1 closed/merged hidden"),
            "header should show hidden count"
        );

        app.show_closed = true;
        let buffer = render_inbox(&mut app);
        assert!(
            buffer.contains("ClosedOne"),
            "closed PR should appear when toggled"
        );
        assert!(
            !buffer.contains("hidden"),
            "header should not show hidden count when show_closed is true"
        );
    }

    #[test]
    fn test_inbox_render_filter_narrows_prs() {
        use crate::app::Mode;
        let mut app = App::default();
        app.data.pulls = vec![make_pr(1, None), make_pr(2, None)];
        app.mode = Mode::Filter {
            input: "#1".into(),
            caret: 2,
        };
        let buffer = render_inbox(&mut app);
        assert!(buffer.contains("PR #1"), "matching PR should appear");
        assert!(
            !buffer.contains("PR #2"),
            "non-matching PR should be hidden"
        );
    }

    #[test]
    fn test_inbox_render_filter_narrows_linears() {
        use crate::app::Mode;
        let mut app = App::default();
        app.data.linears = vec![make_linear(1, None), make_linear(2, None)];
        app.mode = Mode::Filter {
            input: "PROJ-1".into(),
            caret: 6,
        };
        let buffer = render_inbox(&mut app);
        assert!(buffer.contains("PROJ-1"), "matching Linear should appear");
        assert!(
            !buffer.contains("PROJ-2"),
            "non-matching Linear should be hidden"
        );
    }

    #[test]
    fn test_inbox_renders_repo_dividers() {
        let mut app = App::default();
        app.data.pulls = vec![
            crate::gql::PullRequest {
                id: 1,
                owner: "owner-a".into(),
                repo: "repo-x".into(),
                number: 1,
                title: "PR #1".into(),
                url: "x".into(),
                author: None,
                state: "open".into(),
                review_requested: false,
                authored_by_me: false,
                changes_requested: false,
                copilot_comments: false,
                merge_conflicts: false,
                approved: false,
                actions_failing: false,
                dismissed_at: None,
                remote_updated_at: None,
            },
            crate::gql::PullRequest {
                id: 2,
                owner: "owner-b".into(),
                repo: "repo-y".into(),
                number: 2,
                title: "PR #2".into(),
                url: "x".into(),
                author: None,
                state: "open".into(),
                review_requested: false,
                authored_by_me: false,
                changes_requested: false,
                copilot_comments: false,
                merge_conflicts: false,
                approved: false,
                actions_failing: false,
                dismissed_at: None,
                remote_updated_at: None,
            },
        ];
        let buffer = render_inbox(&mut app);
        assert!(
            buffer.contains("owner-a/repo-x"),
            "first repo should appear in buffer:\n{buffer}"
        );
        assert!(
            buffer.contains("╶ owner-b/repo-y"),
            "second repo divider should appear with prefix:\n{buffer}"
        );
    }

    #[test]
    fn test_inbox_legend_sidebar_renders() {
        let mut app = App {
            view: crate::app::View::Inbox,
            ..Default::default()
        };
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| crate::views::render(f, &mut app))
            .unwrap();
        let buffer = buffer_text(terminal.backend().buffer());

        assert!(
            buffer.contains("LEGEND"),
            "legend sidebar title should appear"
        );
        assert!(
            buffer.contains("open PR"),
            "legend should describe the open PR glyph"
        );
        assert!(
            buffer.contains("changes requested"),
            "legend should describe the changes-requested glyph"
        );
    }
}
#[cfg(test)]
mod navigate_tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use tokio::sync::mpsc;

    use crate::app::tests::{make_linear, make_pr, make_todo};
    use crate::app::{App, Mode, ToastKind};
    use crate::gql;

    #[test]
    fn test_l_on_pr_row_opens_link_todo_mode_with_pr_id() {
        let mut app = App::default();
        app.data.pulls = vec![make_pr(5, None)];
        app.cursor = 0;

        super::handle_navigate(&mut app, KeyCode::Char('L'));

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

    #[test]
    fn test_l_on_linear_row_opens_link_todo_mode_with_issue_id() {
        let mut app = App::default();
        app.data.linears = vec![make_linear(9, None)];
        app.cursor = 0;

        super::handle_navigate(&mut app, KeyCode::Char('L'));

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

    #[test]
    fn test_l_out_of_range_does_nothing() {
        let mut app = App {
            cursor: 5, // no rows
            ..Default::default()
        };

        super::handle_navigate(&mut app, KeyCode::Char('L'));

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
        app.init(client, tx);

        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

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
        app.init(client, tx);

        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

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
        app.init(client, tx);

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

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
        app.view = crate::app::View::Inbox;
        app.data.pulls = vec![make_pr(5, None)];
        app.cursor = 0;
        app.init(client, tx);

        app.handle_key(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::NONE));

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
        app.view = crate::app::View::Inbox;
        app.data.linears = vec![make_linear(9, None)];
        app.cursor = 0;
        app.init(client, tx);

        app.handle_key(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::NONE));

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
        app.view = crate::app::View::Inbox;
        app.data.pulls = vec![make_pr(5, None)];
        app.cursor = 0;
        app.init(client, tx);

        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));

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
        app.view = crate::app::View::Inbox;
        app.data.linears = vec![make_linear(9, None)];
        app.cursor = 0;
        app.init(client, tx);

        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            captures.lock().is_empty(),
            "d on a Linear row must not issue a mutation"
        );
    }

    #[test]
    fn test_o_on_pr_row_surfaces_url() {
        let mut app = App::default();
        app.data.pulls = vec![make_pr(5, None)];
        app.cursor = 0;

        super::handle_navigate(&mut app, KeyCode::Char('o'));

        assert_eq!(app.mode, Mode::Navigate);
        let toast = app.toast.expect("o should raise a toast with the URL");
        assert_eq!(toast.kind, ToastKind::Success);
        assert!(
            toast.message.contains("example.com"),
            "o should surface the PR url, got {:?}",
            toast.message
        );
    }

    #[test]
    fn test_o_across_repos_opens_cursor_row_not_state_sorted() {
        let pr_high = gql::PullRequest {
            id: 778,
            owner: "zzz".to_string(),
            repo: "agent-api".to_string(),
            number: 778,
            title: "PR #778".to_string(),
            url: "https://zzz/agent-api/778".to_string(),
            author: None,
            state: "open".to_string(),
            review_requested: false,
            changes_requested: false,
            copilot_comments: false,
            merge_conflicts: false,
            authored_by_me: false,
            approved: false,
            actions_failing: false,
            dismissed_at: None,
            remote_updated_at: None,
        };
        let pr_low = gql::PullRequest {
            id: 802,
            owner: "aaa".to_string(),
            repo: "other".to_string(),
            number: 802,
            title: "PR #802".to_string(),
            url: "https://aaa/other/802".to_string(),
            author: None,
            state: "open".to_string(),
            review_requested: false,
            changes_requested: false,
            copilot_comments: false,
            merge_conflicts: false,
            authored_by_me: false,
            approved: false,
            actions_failing: false,
            dismissed_at: None,
            remote_updated_at: None,
        };
        let mut app = App::default();
        app.data.pulls = vec![pr_high.clone(), pr_low.clone()];
        app.cursor = 1;

        super::handle_navigate(&mut app, KeyCode::Char('o'));

        let toast = app.toast.expect("o should raise a toast with the URL");
        assert_eq!(toast.kind, ToastKind::Success);
        assert!(
            toast.message.contains("/778"),
            "cursor on the repo-sorted second row should open that row's PR, got {:?}",
            toast.message
        );
    }

    #[test]
    fn test_o_on_linear_row_surfaces_url() {
        let mut app = App::default();
        app.data.linears = vec![make_linear(9, None)];
        app.cursor = 0;

        super::handle_navigate(&mut app, KeyCode::Char('o'));

        let toast = app.toast.expect("o should raise a toast with the URL");
        assert_eq!(toast.kind, ToastKind::Success);
        assert!(
            toast.message.contains("example.com"),
            "o should surface the Linear url, got {:?}",
            toast.message
        );
    }

    #[test]
    fn test_d_toggles_show_closed() {
        let mut app = App::default();
        assert!(!app.show_closed, "show_closed should default to false");

        super::handle_navigate(&mut app, KeyCode::Char('D'));
        assert!(app.show_closed, "D should toggle show_closed to true");

        super::handle_navigate(&mut app, KeyCode::Char('D'));
        assert!(
            !app.show_closed,
            "D should toggle show_closed back to false"
        );
    }
    #[test]
    fn test_inbox_filter_mode_activates() {
        let mut app = App::default();
        app.view = crate::app::View::Inbox;
        super::handle_navigate(&mut app, KeyCode::Char('/'));
        assert!(matches!(app.mode, Mode::Filter { .. }));
    }
}
