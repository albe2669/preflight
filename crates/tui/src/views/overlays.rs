//! Overlay handlers: confirm dialog, status-select popup, link-to-todo picker.
//!
//! These are modal overlays rendered on top of the active view. The confirm
//! dialog handles y/n; the status-select popup navigates the 5-status list and
//! commits via GraphQL; the link-to-todo picker lets the user attach a PR or
//! Linear issue to an existing todo.

use ratatui::Frame;

use crate::app::{App, ConfirmAction, Mode};
use crate::theme::{Glyph, Palette};
use crate::widgets::status_glyph;

pub(crate) fn handle_confirm(
    app: &mut App,
    key: ratatui::crossterm::event::KeyCode,
    action: &ConfirmAction,
) {
    use ratatui::crossterm::event::KeyCode::*;
    match key {
        Char('y') | Char('Y') => {
            match action.clone() {
                ConfirmAction::CarryOver { from, to } => {
                    app.spawn_carry_over(&from, &to);
                }
                ConfirmAction::Unplan { id } => {
                    app.spawn_unplan(id);
                }
                ConfirmAction::Cancel { id } => {
                    app.spawn_cancel(id);
                }
                ConfirmAction::DismissPr { id } => {
                    app.spawn_dismiss_pr(id);
                }
            }
            app.mode = Mode::Navigate;
        }
        Char('n') | Char('N') => {
            app.mode = Mode::Navigate;
        }
        _ => {}
    }
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

pub(crate) fn handle_status_select(app: &mut App, key: ratatui::crossterm::event::KeyCode) {
    let (id, reason) = match &app.mode {
        Mode::StatusSelect { id, reason, .. } => (*id, reason.clone()),
        _ => return,
    };
    use ratatui::crossterm::event::KeyCode::*;
    match key {
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
        Enter if reason.is_some() => {
            let blocked_reason = match &app.mode {
                Mode::StatusSelect {
                    reason: Some(r), ..
                } => r.clone(),
                _ => String::new(),
            };
            app.spawn_set_status(id, "blocked", Some(&blocked_reason));
            app.mode = Mode::Navigate;
        }
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
                _ => 0,
            };
            let Some(status) = status_by_index(chosen) else {
                app.mode = Mode::Navigate;
                return;
            };
            if status == "blocked" {
                if let Mode::StatusSelect { reason, .. } = &mut app.mode {
                    *reason = Some(String::new());
                }
            } else {
                app.spawn_set_status(id, status, None);
                app.mode = Mode::Navigate;
            }
        }
        _ => {}
    }
}

pub(crate) fn render_status_select(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
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
        let (glyph, color) = status_glyph(status);
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

// ---- Link-to-todo picker (Inbox `L`) ----

pub(crate) fn handle_link_todo(app: &mut App, key: ratatui::crossterm::event::KeyCode) {
    use ratatui::crossterm::event::KeyCode::*;

    let n = app.data.todos.len();
    match key {
        Char('j') | Down => {
            if let Mode::LinkTodo { selection, .. } = &mut app.mode {
                if *selection + 1 < n {
                    *selection += 1;
                }
            }
        }
        Char('k') | Up => {
            if let Mode::LinkTodo { selection, .. } = &mut app.mode {
                *selection = selection.saturating_sub(1);
            }
        }
        Esc => {
            app.mode = Mode::Navigate;
            app.toast = None;
        }
        Enter => {
            let (todo_id, pr_id, issue_id) = match &app.mode {
                Mode::LinkTodo {
                    selection,
                    pr_id,
                    linear_issue_id,
                    ..
                } => {
                    let Some(todo) = app.data.todos.get(*selection) else {
                        return;
                    };
                    (todo.id, *pr_id, *linear_issue_id)
                }
                _ => return,
            };
            if let Some(pr) = pr_id {
                app.spawn_link_pr(todo_id, pr);
            } else if let Some(issue) = issue_id {
                app.spawn_link_linear(todo_id, issue);
            }
            app.mode = Mode::Navigate;
        }
        _ => {}
    }
}

pub(crate) fn render_link_todo(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    render_todo_picker(f, app, area);
}

// ---- Todo picker (moved from frame.rs) ----

pub fn render_todo_picker(f: &mut Frame, app: &crate::app::App, area: ratatui::layout::Rect) {
    let crate::app::Mode::LinkTodo { selection, .. } = &app.mode else {
        return;
    };
    let selection = *selection;

    let width = 46u16.min(area.width);
    let height = 12u16.min(area.height);
    let x = area.x + (area.width - width) / 2;
    let y = area.y + (area.height - height) / 2;
    let rect = ratatui::layout::Rect::new(x, y, width, height);

    f.render_widget(ratatui::widgets::Clear, rect);
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(ratatui::style::Style::default().fg(Palette::ACCENT))
        .title(ratatui::text::Span::styled(
            " LINK TO TODO ",
            ratatui::style::Style::default().fg(Palette::ACCENT),
        ));
    f.render_widget(block, rect);

    let inner = ratatui::layout::Rect::new(x + 1, y + 1, width - 2, height - 2);
    let mut items: Vec<ratatui::widgets::ListItem> = Vec::new();
    for (i, todo) in app.data.todos.iter().enumerate() {
        let (glyph, color) = status_glyph(&todo.status);
        let is_current = i == selection;
        let marker = if is_current {
            ratatui::text::Span::styled(
                Glyph::CURSOR.to_string(),
                ratatui::style::Style::default().fg(Palette::ACCENT),
            )
        } else {
            ratatui::text::Span::raw(" ")
        };
        let line = ratatui::text::Line::from(vec![
            marker,
            ratatui::text::Span::raw(" "),
            ratatui::text::Span::styled(
                glyph.to_string(),
                ratatui::style::Style::default().fg(color),
            ),
            ratatui::text::Span::raw(" "),
            ratatui::text::Span::styled(
                todo.title.clone(),
                if is_current {
                    ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::BOLD)
                } else {
                    ratatui::style::Style::default()
                },
            ),
        ]);
        items.push(ratatui::widgets::ListItem::new(line));
    }

    let list = ratatui::widgets::List::new(items).style(
        ratatui::style::Style::default()
            .bg(Palette::BG)
            .fg(Palette::TEXT),
    );
    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(selection));
    f.render_stateful_widget(list, inner, &mut state);
}

// ---- Picker candidate helpers (moved from frame.rs) ----

/// True for PR states that are finished (closed or merged); open and draft
/// stay active.
fn pr_is_closed(state: &str) -> bool {
    matches!(state, "closed" | "merged")
}

/// True for Linear state types that are finished (completed or canceled).
fn linear_is_done(state_type: &str) -> bool {
    matches!(state_type, "completed" | "canceled")
}

/// Case-insensitive substring test against a PR's number (as text) or title.
pub(crate) fn pr_matches_search(pr: &crate::gql::PullRequest, search: &str) -> bool {
    if search.is_empty() {
        return true;
    }
    let s = search.to_ascii_lowercase();
    pr.title.to_ascii_lowercase().contains(&s)
        || format!("#{}", pr.number).contains(&s)
        || format!("{}", pr.number).contains(&s)
}

/// Case-insensitive substring test against a Linear issue's identifier or title.
pub(crate) fn linear_matches_search(li: &crate::gql::LinearIssue, search: &str) -> bool {
    if search.is_empty() {
        return true;
    }
    let s = search.to_ascii_lowercase();
    li.title.to_ascii_lowercase().contains(&s) || li.identifier.to_ascii_lowercase().contains(&s)
}

/// Filtered + sorted PR candidate indices for the link attach picker.
/// Open PRs come first, closed/merged after; within each group, most recently
/// updated first. PRs with unknown update time sort last in their group.
pub(crate) fn pr_picker_candidates(app: &crate::app::App, search: &str) -> Vec<usize> {
    let mut idxs: Vec<usize> = (0..app.data.pulls.len())
        .filter(|&i| pr_matches_search(&app.data.pulls[i], search))
        .collect();
    idxs.sort_by(|&a, &b| {
        let pa = &app.data.pulls[a];
        let pb = &app.data.pulls[b];
        pr_is_closed(&pa.state)
            .cmp(&pr_is_closed(&pb.state))
            .then_with(|| {
                let ta = pa.remote_updated_at.as_deref();
                let tb = pb.remote_updated_at.as_deref();
                match (ta, tb) {
                    (Some(a), Some(b)) => b.cmp(a),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            })
    });
    idxs
}

/// Filtered + sorted Linear candidate indices for the link attach picker.
/// Active issues come first, completed/canceled after.
pub(crate) fn linear_picker_candidates(app: &crate::app::App, search: &str) -> Vec<usize> {
    let mut idxs: Vec<usize> = (0..app.data.linears.len())
        .filter(|&i| linear_matches_search(&app.data.linears[i], search))
        .collect();
    idxs.sort_by_key(|&i| linear_is_done(&app.data.linears[i].state_type));
    idxs
}

pub(crate) fn render_command_palette(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    use ratatui::layout::Rect;
    use ratatui::style::{Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Clear, List, ListItem};

    let Mode::Command {
        input, selection, ..
    } = &app.mode
    else {
        return;
    };
    let input = input.clone();
    let selection = *selection;

    let filtered = crate::app::input::filter_commands(app, &input);
    let width = 50u16.min(area.width);
    let visible = 8u16;
    let height = (filtered.len() as u16 + 3).min(visible + 3);
    let x = area.x + (area.width - width) / 2;
    let y = area.y + (area.height - height) / 2;
    let popup = Rect::new(x, y, width, height);

    f.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Palette::ACCENT))
        .title(Span::styled(
            " Command ",
            Style::default().fg(Palette::ACCENT),
        ));
    f.render_widget(block, popup);

    let inner = Rect::new(
        x + 1,
        y + 1,
        width.saturating_sub(2),
        height.saturating_sub(2),
    );

    let input_line = Line::from(format!(":{input}\u{258f}"));
    f.render_widget(
        ratatui::widgets::Paragraph::new(input_line),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    if !filtered.is_empty() {
        let start = selection
            .saturating_sub(visible as usize - 1)
            .min(filtered.len().saturating_sub(visible as usize));
        let lines: Vec<ListItem> = filtered
            .iter()
            .skip(start)
            .take(visible as usize)
            .enumerate()
            .map(|(i, cmd)| {
                let abs = start + i;
                let style = if abs == selection {
                    Style::default()
                        .bg(Palette::ROW_HIGHLIGHT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(cmd.label.clone()).style(style))
            })
            .collect();
        let list = List::new(lines);
        let list_area = Rect::new(
            inner.x,
            inner.y + 1,
            inner.width,
            inner.height.saturating_sub(1),
        );
        f.render_widget(list, list_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_command_palette_renders_single_result() {
        use crate::app::{App, Mode};
        use crate::test_support::buffer_text;
        use ratatui::{Terminal, backend::TestBackend};

        let app = App {
            mode: Mode::Command {
                input: "backlog".into(),
                selection: 0,
                caret: 7,
            },
            ..Default::default()
        };
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_command_palette(f, &app, f.area()))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer());
        assert!(
            text.contains("backlog"),
            "single filtered result must be visible in the palette:\n{text}"
        );
    }

    #[test]
    fn test_pr_picker_sorts_open_first_then_recency() {
        use crate::app::App;

        let mut app = App::default();
        app.data.pulls = vec![
            crate::gql::PullRequest {
                id: 1,
                owner: "o".into(),
                repo: "r".into(),
                number: 1,
                title: "Closed old".into(),
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
                remote_updated_at: Some("2024-01-01T00:00:00Z".into()),
            },
            crate::gql::PullRequest {
                id: 2,
                owner: "o".into(),
                repo: "r".into(),
                number: 2,
                title: "Open new".into(),
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
                remote_updated_at: Some("2024-06-01T00:00:00Z".into()),
            },
            crate::gql::PullRequest {
                id: 3,
                owner: "o".into(),
                repo: "r".into(),
                number: 3,
                title: "Open older".into(),
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
                remote_updated_at: Some("2024-03-01T00:00:00Z".into()),
            },
        ];
        let result = pr_picker_candidates(&app, "");
        assert_eq!(result, vec![1, 2, 0]);
    }

    #[test]
    fn test_pr_picker_none_updated_sorts_last() {
        use crate::app::App;

        let mut app = App::default();
        app.data.pulls = vec![
            crate::gql::PullRequest {
                id: 1,
                owner: "o".into(),
                repo: "r".into(),
                number: 1,
                title: "Open no date".into(),
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
                owner: "o".into(),
                repo: "r".into(),
                number: 2,
                title: "Open with date".into(),
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
                remote_updated_at: Some("2024-06-01T00:00:00Z".into()),
            },
        ];
        let result = pr_picker_candidates(&app, "");
        assert_eq!(result, vec![1, 0]);
    }
}
