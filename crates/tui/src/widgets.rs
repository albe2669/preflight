//! Shared list widgets: section headers, todo rows, status glyphs, panels,
//! and the grouped-by-status list renderer.

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};

use crate::app::STATUS_ORDER;
use crate::gql::Todo;
use crate::theme::{Glyph, Palette};

/// Render a section header (T2: UPPERCASE, dim, letter-spaced, 1 blank above).
pub fn section_header(title: &str) -> Line<'_> {
    Line::from(vec![Span::styled(
        title.to_string(),
        Style::default().fg(Palette::DIM),
    )])
    .alignment(Alignment::Left)
}

/// Build a list widget for todo rows with the given state.
pub fn todo_list<'a>(items: Vec<ListItem<'a>>, _state: &mut ListState) -> List<'a> {
    List::new(items)
        .style(Style::default().bg(Palette::BG).fg(Palette::TEXT))
        .highlight_style(Style::default().bg(Palette::ROW_HIGHLIGHT))
        .highlight_symbol("")
}

/// Format a todo row as a `Line` with status glyph + title + metadata.
pub fn todo_line(todo: &Todo, is_cursor: bool, position: Option<usize>, carried: bool) -> Line<'_> {
    let (glyph, color) = status_glyph(&todo.status);
    let marker = if is_cursor {
        Span::styled(
            Glyph::CURSOR.to_string(),
            Style::default().fg(Palette::ACCENT),
        )
    } else {
        Span::raw(" ")
    };

    let mut spans = vec![marker, Span::raw(" ")];

    if let Some(p) = position {
        spans.push(Span::raw(format!("{p:>2} ")));
    } else {
        spans.push(Span::raw("    "));
    }

    if carried {
        spans.push(Span::styled(
            Glyph::CARRIED.to_string(),
            Style::default().fg(Palette::DIM),
        ));
        spans.push(Span::raw(" "));
    } else {
        spans.push(Span::raw("  "));
    }

    spans.push(Span::styled(glyph.to_string(), Style::default().fg(color)));

    let title_style = match todo.status.as_str() {
        "started" => Style::default()
            .fg(Palette::TEXT)
            .add_modifier(Modifier::BOLD),
        "done" => Style::default().fg(Palette::DIM),
        "cancelled" => Style::default()
            .fg(Palette::CANCELLED)
            .add_modifier(Modifier::CROSSED_OUT),
        _ => Style::default().fg(Palette::TEXT),
    };
    spans.push(Span::raw(" "));
    spans.push(Span::styled(todo.title.clone(), title_style));

    // Blocked reason continuation line is rendered separately by the caller.
    Line::from(spans)
}

pub fn blocked_reason_line(reason: &str) -> Line<'_> {
    Line::from(vec![
        Span::raw("        "),
        Span::styled(
            Glyph::BLOCKED_REASON.to_string(),
            Style::default().fg(Palette::BLOCKED),
        ),
        Span::raw(" "),
        Span::styled(
            reason.to_string(),
            Style::default()
                .fg(Palette::BLOCKED)
                .add_modifier(Modifier::UNDERLINED),
        ),
    ])
}

pub fn status_glyph(status: &str) -> (char, ratatui::style::Color) {
    match status {
        "todo" => (Glyph::STATUS_TODO, Palette::TODO),
        "started" => (Glyph::STATUS_STARTED, Palette::STARTED),
        "blocked" => (Glyph::STATUS_BLOCKED, Palette::BLOCKED),
        "done" => (Glyph::STATUS_DONE, Palette::DONE),
        "cancelled" => (Glyph::STATUS_CANCELLED, Palette::CANCELLED),
        _ => ('?', Palette::DIM),
    }
}

pub fn pr_state_glyph(state: &str) -> (char, ratatui::style::Color) {
    match state {
        "open" => (Glyph::PR_OPEN, Palette::DONE),
        "draft" => (Glyph::PR_DRAFT, Palette::CANCELLED),
        "merged" => (Glyph::PR_MERGED, Palette::MERGED),
        "closed" => (Glyph::PR_CLOSED, Palette::BLOCKED),
        _ => ('?', Palette::DIM),
    }
}

pub fn linear_state_glyph(state_type: &str) -> (char, ratatui::style::Color) {
    match state_type {
        "triage" => ('?', Palette::ACCENT),
        "backlog" => ('·', Palette::CANCELLED),
        "unstarted" => (Glyph::STATUS_TODO, Palette::TODO),
        "started" => (Glyph::STATUS_STARTED, Palette::STARTED),
        "completed" => (Glyph::STATUS_DONE, Palette::DONE),
        "canceled" => (Glyph::STATUS_CANCELLED, Palette::CANCELLED),
        _ => ('?', Palette::DIM),
    }
}

/// A bordered panel with an optional accent border when focused
pub fn panel<'a>(title: Option<&str>, focused: bool) -> Block<'a> {
    let border_color = if focused {
        Palette::ACCENT
    } else {
        Palette::BORDER
    };
    let mut b = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    if let Some(t) = title {
        b = b.title(Span::styled(
            format!(" {} ", t),
            Style::default().fg(border_color),
        ));
    }
    b
}

// ---- Shared grouped-by-state list renderer (D3) ----

/// A group of rows under a state header, ready to render.
#[derive(Clone, Debug)]
pub struct GroupedSection<'a> {
    pub label: &'static str,
    pub status: &'static str,
    pub rows: Vec<&'a Todo>,
    pub collapsed: bool,
}

/// Group todos by status in the fixed display order (started, blocked, todo,
/// done, cancelled). Empty groups are elided, except `done` which is kept as
/// a collapsed header when `show_done` is false (the done-toggle).
pub fn group_by_status<'a>(rows: &'a [&'a Todo], show_done: bool) -> Vec<GroupedSection<'a>> {
    let mut groups: Vec<GroupedSection<'a>> = Vec::new();
    for &status in STATUS_ORDER.iter() {
        let matching: Vec<&Todo> = rows
            .iter()
            .copied()
            .filter(|t| t.status == status)
            .collect();
        if matching.is_empty() {
            continue;
        }
        let label = match status {
            "started" => "STARTED",
            "blocked" => "BLOCKED",
            "todo" => "TODO",
            "done" => "DONE",
            "cancelled" => "CANCELLED",
            _ => status,
        };
        groups.push(GroupedSection {
            label,
            status,
            rows: matching,
            collapsed: false,
        });
    }
    // Done-toggle: when show_done is false, keep the done group but mark it
    // collapsed so the renderer shows only the header with the count.
    if !show_done {
        for g in groups.iter_mut() {
            if g.status == "done" {
                g.collapsed = true;
            }
        }
    }
    groups
}

/// Render the grouped list into `area` as a stateful `List` widget.
///
/// `cursor_todo` is the todo the cursor points at in the *source-ordered*
/// list (the same order the caller built `groups` from). Groups reorder rows
/// by status, so the highlight must be located by todo identity, never by
/// positional index. The inline-create prompt (if any) and per-view hints are
/// appended by the caller via `trailing_items`.
pub fn render_grouped_list<'a>(
    f: &mut Frame,
    area: Rect,
    groups: &[GroupedSection<'a>],
    cursor_todo: Option<&Todo>,
    carried_ids: &std::collections::HashSet<i32>,
    trailing_items: Vec<ListItem<'a>>,
) -> ListState {
    let cursor_id = cursor_todo.map(|t| t.id);

    // Build the grouped items and, in the same pass, record the flat List
    // index that carries the cursor's todo (matched by id).
    let mut items: Vec<ListItem<'a>> = Vec::new();
    let mut selected_flat: Option<usize> = None;
    let mut flat: usize = 0;

    for group in groups {
        let collapsed = group.collapsed;
        let header = if collapsed {
            format!(
                "  {} · {} (collapsed — press D to expand)",
                group.label,
                group.rows.len()
            )
        } else {
            format!("  {} · {}", group.label, group.rows.len())
        };
        items.push(ListItem::new(Line::from(Span::styled(
            header,
            Style::default().fg(Palette::DIM),
        ))));
        flat += 1;

        if collapsed {
            continue;
        }

        for todo in group.rows.iter() {
            let is_cursor = cursor_id == Some(todo.id);
            if is_cursor {
                selected_flat = Some(flat);
            }
            let line = grouped_todo_line(todo, is_cursor, carried_ids.contains(&todo.id));
            if is_cursor {
                items.push(ListItem::new(line).style(Style::default().bg(Palette::ROW_HIGHLIGHT)));
            } else {
                items.push(ListItem::new(line));
            }
            flat += 1;
            // Blocked reason continuation line.
            if todo.status == "blocked" {
                if let Some(reason) = &todo.blocked_reason {
                    items.push(ListItem::new(blocked_reason_line(reason)));
                    flat += 1;
                }
            }
        }
        items.push(ListItem::new(Line::from("")));
        flat += 1;
    }

    items.extend(trailing_items);

    let list = List::new(items)
        .style(Style::default().bg(Palette::BG).fg(Palette::TEXT))
        .highlight_style(Style::default().bg(Palette::ROW_HIGHLIGHT));

    let mut state = ListState::default();
    state.select(selected_flat);
    f.render_stateful_widget(list, area, &mut state);
    state
}

/// Format a todo row for the grouped layout (no position column).
fn grouped_todo_line(todo: &Todo, is_cursor: bool, carried: bool) -> Line<'_> {
    let (glyph, color) = status_glyph(&todo.status);
    let marker = if is_cursor {
        Span::styled(
            Glyph::CURSOR.to_string(),
            Style::default().fg(Palette::ACCENT),
        )
    } else {
        Span::raw(" ")
    };
    let title_style = match todo.status.as_str() {
        "started" => Style::default()
            .fg(Palette::TEXT)
            .add_modifier(Modifier::BOLD),
        "done" => Style::default().fg(Palette::DIM),
        "cancelled" => Style::default()
            .fg(Palette::CANCELLED)
            .add_modifier(Modifier::CROSSED_OUT),
        _ => Style::default().fg(Palette::TEXT),
    };
    let mut spans = vec![
        marker,
        Span::raw("  "),
        Span::styled(glyph.to_string(), Style::default().fg(color)),
        Span::raw(" "),
    ];
    if carried {
        spans.push(Span::styled(
            Glyph::CARRIED.to_string(),
            Style::default().fg(Palette::DIM),
        ));
        spans.push(Span::raw(" "));
    }
    spans.push(Span::styled(todo.title.clone(), title_style));
    Line::from(spans)
}

/// Split the content area into a list (75%) + sidebar (25%) when wide
/// enough. Below 100 cols, returns the full area as the list and `None`
/// for the sidebar (D4).
pub fn split_content(area: Rect) -> (Rect, Option<Rect>) {
    if area.width < 100 {
        return (area, None);
    }
    let sidebar_width = (area.width / 4).max(25);
    let list_width = area.width.saturating_sub(sidebar_width);
    let list_area = Rect::new(area.x, area.y, list_width, area.height);
    let sidebar_area = Rect::new(area.x + list_width, area.y, sidebar_width, area.height);
    (list_area, Some(sidebar_area))
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;
    use ratatui::style::Modifier;

    use super::{
        blocked_reason_line, group_by_status, linear_state_glyph, pr_state_glyph, section_header,
        split_content, status_glyph,
    };
    use crate::theme::{Glyph, Palette};

    // -- status_glyph tests --

    #[test]
    fn test_status_glyph_todo() {
        let (ch, color) = status_glyph("todo");
        assert_eq!(ch, Glyph::STATUS_TODO);
        assert_eq!(color, Palette::TODO);
    }

    #[test]
    fn test_status_glyph_started() {
        let (ch, color) = status_glyph("started");
        assert_eq!(ch, Glyph::STATUS_STARTED);
        assert_eq!(color, Palette::STARTED);
    }

    #[test]
    fn test_status_glyph_blocked() {
        let (ch, color) = status_glyph("blocked");
        assert_eq!(ch, Glyph::STATUS_BLOCKED);
        assert_eq!(color, Palette::BLOCKED);
    }

    #[test]
    fn test_status_glyph_done() {
        let (ch, color) = status_glyph("done");
        assert_eq!(ch, Glyph::STATUS_DONE);
        assert_eq!(color, Palette::DONE);
    }

    #[test]
    fn test_status_glyph_cancelled() {
        let (ch, color) = status_glyph("cancelled");
        assert_eq!(ch, Glyph::STATUS_CANCELLED);
        assert_eq!(color, Palette::CANCELLED);
    }

    #[test]
    fn test_status_glyph_unknown() {
        let (ch, color) = status_glyph("unknown_status");
        assert_eq!(ch, '?');
        assert_eq!(color, Palette::DIM);
    }

    // -- pr_state_glyph tests --

    #[test]
    fn test_pr_state_glyph_open() {
        let (ch, color) = pr_state_glyph("open");
        assert_eq!(ch, Glyph::PR_OPEN);
        assert_eq!(color, Palette::DONE);
    }

    #[test]
    fn test_pr_state_glyph_draft() {
        let (ch, color) = pr_state_glyph("draft");
        assert_eq!(ch, Glyph::PR_DRAFT);
        assert_eq!(color, Palette::CANCELLED);
    }

    #[test]
    fn test_pr_state_glyph_merged() {
        let (ch, color) = pr_state_glyph("merged");
        assert_eq!(ch, Glyph::PR_MERGED);
        assert_eq!(color, Palette::MERGED);
    }

    #[test]
    fn test_pr_state_glyph_closed() {
        let (ch, color) = pr_state_glyph("closed");
        assert_eq!(ch, Glyph::PR_CLOSED);
        assert_eq!(color, Palette::BLOCKED);
    }

    #[test]
    fn test_pr_state_glyph_unknown() {
        let (ch, color) = pr_state_glyph("weird");
        assert_eq!(ch, '?');
        assert_eq!(color, Palette::DIM);
    }

    // -- linear_state_glyph tests --

    #[test]
    fn test_linear_state_glyph_triage() {
        let (ch, color) = linear_state_glyph("triage");
        assert_eq!(ch, '?');
        assert_eq!(color, Palette::ACCENT);
    }

    #[test]
    fn test_linear_state_glyph_backlog() {
        let (ch, color) = linear_state_glyph("backlog");
        assert_eq!(ch, '·');
        assert_eq!(color, Palette::CANCELLED);
    }

    #[test]
    fn test_linear_state_glyph_unstarted() {
        let (ch, color) = linear_state_glyph("unstarted");
        assert_eq!(ch, Glyph::STATUS_TODO);
        assert_eq!(color, Palette::TODO);
    }

    #[test]
    fn test_linear_state_glyph_started() {
        let (ch, color) = linear_state_glyph("started");
        assert_eq!(ch, Glyph::STATUS_STARTED);
        assert_eq!(color, Palette::STARTED);
    }

    #[test]
    fn test_linear_state_glyph_completed() {
        let (ch, color) = linear_state_glyph("completed");
        assert_eq!(ch, Glyph::STATUS_DONE);
        assert_eq!(color, Palette::DONE);
    }

    #[test]
    fn test_linear_state_glyph_canceled() {
        let (ch, color) = linear_state_glyph("canceled");
        assert_eq!(ch, Glyph::STATUS_CANCELLED);
        assert_eq!(color, Palette::CANCELLED);
    }

    #[test]
    fn test_linear_state_glyph_unknown() {
        let (ch, color) = linear_state_glyph("unknown_type");
        assert_eq!(ch, '?');
        assert_eq!(color, Palette::DIM);
    }

    // -- section_header tests --

    #[test]
    fn test_section_header_has_title() {
        let line = section_header("My Section");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].content, "My Section");
    }

    #[test]
    fn test_section_header_is_dim_styled() {
        let line = section_header("Header");
        let style = line.spans[0].style;
        assert_eq!(style.fg, Some(Palette::DIM));
    }

    // -- blocked_reason_line tests --

    #[test]
    fn test_blocked_reason_line_contains_reason() {
        let line = blocked_reason_line("waiting on CI");
        let reason_span = line.spans.iter().find(|s| s.content == "waiting on CI");
        assert!(
            reason_span.is_some(),
            "reason text should be in the line spans"
        );
        let span = reason_span.unwrap();
        assert_eq!(span.style.fg, Some(Palette::BLOCKED));
        assert!(span.style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_blocked_reason_line_has_glyph() {
        let line = blocked_reason_line("reason");
        let glyph_span = line
            .spans
            .iter()
            .find(|s| s.content == Glyph::BLOCKED_REASON.to_string());
        assert!(
            glyph_span.is_some(),
            "blocked reason glyph should be present"
        );
        assert_eq!(glyph_span.unwrap().style.fg, Some(Palette::BLOCKED));
    }

    // -- group_by_status tests (task 3.2) --

    fn make_todo(id: i32, status: &str) -> crate::gql::Todo {
        crate::gql::Todo {
            id,
            title: format!("Todo {id}"),
            description: None,
            status: status.to_string(),
            blocked_reason: None,
            sort_key: id,
            created_at: "2026-08-05 10:00:00 +00:00".to_string(),
            started_at: None,
            closed_at: None,
            tag: crate::gql::TagConnection { nodes: vec![] },
        }
    }

    fn make_titled_todo(id: i32, title: &str, status: &str) -> crate::gql::Todo {
        crate::gql::Todo {
            id,
            title: title.to_string(),
            description: None,
            status: status.to_string(),
            blocked_reason: None,
            sort_key: id,
            created_at: "2026-08-05 10:00:00 +00:00".to_string(),
            started_at: None,
            closed_at: None,
            tag: crate::gql::TagConnection { nodes: vec![] },
        }
    }

    #[test]
    fn test_group_by_status_empty_groups_skipped() {
        let t1 = make_todo(1, "started");
        let t2 = make_todo(2, "done");
        let rows: Vec<&crate::gql::Todo> = vec![&t1, &t2];
        let groups = group_by_status(&rows, true);
        // Only started and done have rows; blocked, todo, cancelled are skipped.
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].status, "started");
        assert_eq!(groups[1].status, "done");
    }

    #[test]
    fn test_group_by_status_fixed_order() {
        let t1 = make_todo(1, "cancelled");
        let t2 = make_todo(2, "todo");
        let t3 = make_todo(3, "started");
        let t4 = make_todo(4, "blocked");
        let t5 = make_todo(5, "done");
        let rows: Vec<&crate::gql::Todo> = vec![&t1, &t2, &t3, &t4, &t5];
        let groups = group_by_status(&rows, true);
        assert_eq!(groups.len(), 5);
        assert_eq!(groups[0].status, "started");
        assert_eq!(groups[1].status, "blocked");
        assert_eq!(groups[2].status, "todo");
        assert_eq!(groups[3].status, "done");
        assert_eq!(groups[4].status, "cancelled");
    }

    #[test]
    fn test_group_by_status_done_collapsed_when_show_done_false() {
        let t1 = make_todo(1, "todo");
        let t2 = make_todo(2, "done");
        let t3 = make_todo(3, "done");
        let rows: Vec<&crate::gql::Todo> = vec![&t1, &t2, &t3];
        let groups = group_by_status(&rows, false);
        let done_group = groups.iter().find(|g| g.status == "done").unwrap();
        assert!(done_group.collapsed);
        // Rows are kept so the count is correct.
        assert_eq!(done_group.rows.len(), 2);
    }

    #[test]
    fn test_group_by_status_done_expanded_when_show_done_true() {
        let t1 = make_todo(1, "done");
        let rows: Vec<&crate::gql::Todo> = vec![&t1];
        let groups = group_by_status(&rows, true);
        let done_group = groups.iter().find(|g| g.status == "done").unwrap();
        assert!(!done_group.collapsed);
        assert_eq!(done_group.rows.len(), 1);
    }

    // -- split_content tests (task 9.4) --

    #[test]
    fn test_split_content_narrow_returns_full_area_no_sidebar() {
        let area = Rect::new(0, 0, 80, 24);
        let (list, sidebar) = split_content(area);
        assert_eq!(list, area);
        assert!(sidebar.is_none());
    }

    #[test]
    fn test_split_content_wide_splits_list_and_sidebar() {
        let area = Rect::new(0, 0, 120, 24);
        let (list, sidebar) = split_content(area);
        assert!(list.width < area.width);
        assert!(sidebar.is_some());
        let sb = sidebar.unwrap();
        assert!(sb.width >= 25);
        assert_eq!(list.width + sb.width, area.width);
    }

    #[test]
    fn test_split_content_boundary_99_no_sidebar() {
        let area = Rect::new(0, 0, 99, 24);
        let (_, sidebar) = split_content(area);
        assert!(sidebar.is_none());
    }

    #[test]
    fn test_split_content_boundary_100_has_sidebar() {
        let area = Rect::new(0, 0, 100, 24);
        let (_, sidebar) = split_content(area);
        assert!(sidebar.is_some());
    }

    #[test]
    fn test_group_by_status_with_filter_end_to_end() {
        // Three todos: two match "rust", one doesn't.
        let t1 = make_titled_todo(1, "Write Rust docs", "started");
        let t2 = make_titled_todo(2, "Review PR", "todo");
        let t3 = make_titled_todo(3, "Rust refactor", "done");
        let all: Vec<&crate::gql::Todo> = vec![&t1, &t2, &t3];

        // Apply filter "rust" — only t1 and t3 survive.
        let filtered: Vec<&crate::gql::Todo> = all
            .iter()
            .copied()
            .filter(|t| crate::app::matches_filter(t, "rust"))
            .collect();
        let groups = group_by_status(&filtered, true);

        // Started has 1, done has 1, todo group is skipped (t2 filtered out).
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].status, "started");
        assert_eq!(groups[0].rows.len(), 1);
        assert_eq!(groups[1].status, "done");
        assert_eq!(groups[1].rows.len(), 1);
    }

    #[test]
    fn test_group_by_status_filter_no_matches_yields_empty() {
        let t1 = make_titled_todo(1, "Task", "todo");
        let all: Vec<&crate::gql::Todo> = vec![&t1];
        let filtered: Vec<&crate::gql::Todo> = all
            .iter()
            .copied()
            .filter(|t| crate::app::matches_filter(t, "zzzznonexistent"))
            .collect();
        assert!(filtered.is_empty());
        let groups = group_by_status(&filtered, true);
        assert!(groups.is_empty());
    }
}

#[cfg(test)]
mod render_tests {
    use ratatui::{Terminal, backend::TestBackend};

    use crate::app::tests::make_todo;
    use crate::gql::Todo;

    use crate::theme::Palette;

    use super::{GroupedSection, render_grouped_list};

    /// Render with the shared grouped-list renderer, returning the text of
    /// every row whose background carries the row-highlight color.
    fn highlighted_grouped_rows<'a>(
        groups: Vec<GroupedSection<'a>>,
        cursor_todo: Option<&Todo>,
    ) -> Vec<String> {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_grouped_list(
                    f,
                    f.area(),
                    &groups,
                    cursor_todo,
                    &std::collections::HashSet::new(),
                    vec![],
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        (0..buf.area.height)
            .filter(|y| {
                (0..buf.area.width).any(|x| buf[(x, *y)].style().bg == Some(Palette::ROW_HIGHLIGHT))
            })
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn test_grouped_list_cursor_offsets_complete_header() {
        let first = make_todo(1, "first started", "started");
        let second = make_todo(2, "second started", "started");
        let third = make_todo(3, "plain todo", "todo");
        let groups = vec![
            GroupedSection {
                label: "STARTED",
                status: "started",
                rows: vec![&first, &second],
                collapsed: false,
            },
            GroupedSection {
                label: "TODO",
                status: "todo",
                rows: vec![&third],
                collapsed: false,
            },
        ];

        // The STARTED header occupies flat index 0, so the highlight must
        // land on the cursor's todo (second) regardless of position.
        let rows = highlighted_grouped_rows(groups, Some(&second));
        assert_eq!(
            rows.len(),
            1,
            "expected exactly one highlighted row, got {rows:?}"
        );
        assert!(
            rows[0].contains("second started"),
            "expected highlight on the second todo row, got {:?}",
            rows[0]
        );
    }

    #[test]
    fn test_grouped_list_single_highlight_with_collapsed_done() {
        let started = make_todo(1, "started one", "started");
        let todo = make_todo(2, "todo one", "todo");
        let done_a = make_todo(3, "done a", "done");
        let done_b = make_todo(4, "done b", "done");
        let groups = vec![
            GroupedSection {
                label: "STARTED",
                status: "started",
                rows: vec![&started],
                collapsed: false,
            },
            GroupedSection {
                label: "TODO",
                status: "todo",
                rows: vec![&todo],
                collapsed: false,
            },
            GroupedSection {
                label: "DONE",
                status: "done",
                rows: vec![&done_a, &done_b],
                collapsed: true,
            },
        ];

        // cursor 1 = the sole TODO row (after the started row). The STARTED
        // header, group separators, and the collapsed DONE header all consume
        // visual lines, so the highlight must still land on the TODO row and
        // nowhere else — in particular not on any header line.
        let rows = highlighted_grouped_rows(groups, Some(&todo));
        assert_eq!(
            rows.len(),
            1,
            "expected exactly one highlighted row, got {rows:?}"
        );
        assert!(
            rows[0].contains("todo one"),
            "expected highlight on the TODO row, got {:?}",
            rows[0]
        );
    }
}
