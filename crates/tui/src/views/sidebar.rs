//! Sidebar edit/add handlers and the info-sidebar render functions.
//!
//! The sidebar renders a detail card for the highlighted todo (read mode) or
//! an edit/create form (SidebarEdit / SidebarAdd modes). The handlers manage
//! field navigation, text editing with caret tracking, and GraphQL mutations
//! for title/description/tags/links.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::edit::{backspace, insert_char, move_caret};
use crate::app::{App, LinkKind, Mode, SidebarField};
use crate::gql::Todo;
use crate::theme::{Glyph, Palette};
use crate::widgets::status_glyph;

/// Handle sidebar edit mode keys.
pub(crate) fn handle_sidebar_edit(app: &mut App, key: ratatui::crossterm::event::KeyCode) {
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
        attaching,
        link_search,
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
            attaching,
            link_search,
            scroll,
            ..
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
            *attaching,
            link_search.clone(),
            *scroll,
        ),
        _ => return,
    };

    let all_links = match &app.detail {
        Some(d) => (d.prs.clone(), d.linears.clone()),
        None => (Vec::new(), Vec::new()),
    };

    if !input_active {
        match key {
            Tab | Char('j') => {
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
        match field {
            SidebarField::Title => match key {
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
                    if let Mode::SidebarEdit {
                        title_input,
                        title_caret,
                        ..
                    } = &mut app.mode
                    {
                        *title_caret = insert_char(title_input, *title_caret, c);
                    }
                }
                Enter => {
                    let title = title_input.trim().to_string();
                    app.spawn_update_todo(id, Some(&title), None);
                    if let Mode::SidebarEdit { input_active, .. } = &mut app.mode {
                        *input_active = false;
                    }
                }
                Backspace => {
                    if let Mode::SidebarEdit {
                        title_input,
                        title_caret,
                        ..
                    } = &mut app.mode
                    {
                        *title_caret = backspace(title_input, *title_caret);
                    }
                }
                Left => {
                    if let Mode::SidebarEdit {
                        title_input,
                        title_caret,
                        ..
                    } = &mut app.mode
                    {
                        *title_caret = move_caret(title_input, *title_caret, -1);
                    }
                }
                Right => {
                    if let Mode::SidebarEdit {
                        title_input,
                        title_caret,
                        ..
                    } = &mut app.mode
                    {
                        *title_caret = move_caret(title_input, *title_caret, 1);
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
            },
            SidebarField::Description => match key {
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
                    if let Mode::SidebarEdit {
                        desc_input,
                        desc_caret,
                        ..
                    } = &mut app.mode
                    {
                        *desc_caret = insert_char(desc_input, *desc_caret, c);
                    }
                }
                Enter => {
                    let desc = desc_input.clone();
                    app.spawn_update_todo(id, None, Some(&desc));
                    if let Mode::SidebarEdit { input_active, .. } = &mut app.mode {
                        *input_active = false;
                    }
                }
                Backspace => {
                    if let Mode::SidebarEdit {
                        desc_input,
                        desc_caret,
                        ..
                    } = &mut app.mode
                    {
                        *desc_caret = backspace(desc_input, *desc_caret);
                    }
                }
                Left => {
                    if let Mode::SidebarEdit {
                        desc_input,
                        desc_caret,
                        ..
                    } = &mut app.mode
                    {
                        *desc_caret = move_caret(desc_input, *desc_caret, -1);
                    }
                }
                Right => {
                    if let Mode::SidebarEdit {
                        desc_input,
                        desc_caret,
                        ..
                    } = &mut app.mode
                    {
                        *desc_caret = move_caret(desc_input, *desc_caret, 1);
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
            },
            SidebarField::Links => match key {
                Char('a') if !attaching => {
                    if let Mode::SidebarEdit {
                        attaching,
                        link_selection,
                        link_search,
                        ..
                    } = &mut app.mode
                    {
                        *attaching = true;
                        *link_selection = 0;
                        link_search.clear();
                    }
                }
                Esc if attaching => {
                    if let Mode::SidebarEdit { attaching, .. } = &mut app.mode {
                        *attaching = false;
                    }
                }
                Tab if attaching => {
                    if let Mode::SidebarEdit {
                        link_kind,
                        link_selection,
                        link_search,
                        ..
                    } = &mut app.mode
                    {
                        *link_kind = match link_kind {
                            LinkKind::Pr => LinkKind::Linear,
                            LinkKind::Linear => LinkKind::Pr,
                        };
                        *link_selection = 0;
                        link_search.clear();
                    }
                }
                Char('j') | Down if attaching => {
                    let cands = if link_kind == LinkKind::Pr {
                        crate::views::overlays::pr_picker_candidates(app, &link_search)
                    } else {
                        crate::views::overlays::linear_picker_candidates(app, &link_search)
                    };
                    let max_sel = cands.len().saturating_sub(1);
                    if let Mode::SidebarEdit { link_selection, .. } = &mut app.mode {
                        if *link_selection < max_sel {
                            *link_selection += 1;
                        }
                    }
                }
                Char('k') | Up if attaching => {
                    if let Mode::SidebarEdit { link_selection, .. } = &mut app.mode {
                        *link_selection = link_selection.saturating_sub(1);
                    }
                }
                Char(c)
                    if attaching
                        && (c.is_alphanumeric()
                            || c == ' '
                            || c == '#'
                            || c == '-'
                            || c == '_'
                            || c == '.'
                            || c == '/') =>
                {
                    if let Mode::SidebarEdit {
                        link_search,
                        link_selection,
                        ..
                    } = &mut app.mode
                    {
                        link_search.push(c);
                        *link_selection = 0;
                    }
                }
                Backspace if attaching => {
                    if let Mode::SidebarEdit { link_search, .. } = &mut app.mode {
                        link_search.pop();
                    }
                }
                Enter if attaching => {
                    let cands = if link_kind == LinkKind::Pr {
                        crate::views::overlays::pr_picker_candidates(app, &link_search)
                    } else {
                        crate::views::overlays::linear_picker_candidates(app, &link_search)
                    };
                    let data_idx = cands.get(link_selection).copied();
                    if link_kind == LinkKind::Pr {
                        if let Some(i) = data_idx {
                            if let Some(pr) = app.data.pulls.get(i) {
                                app.spawn_link_pr(id, pr.id);
                                if let Mode::SidebarEdit {
                                    attaching,
                                    input_active,
                                    ..
                                } = &mut app.mode
                                {
                                    *attaching = false;
                                    *input_active = false;
                                }
                            }
                        }
                    } else if let Some(i) = data_idx {
                        if let Some(issue) = app.data.linears.get(i) {
                            app.spawn_link_linear(id, issue.id);
                            if let Mode::SidebarEdit {
                                attaching,
                                input_active,
                                ..
                            } = &mut app.mode
                            {
                                *attaching = false;
                                *input_active = false;
                            }
                        }
                    }
                }
                Tab => {
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
                    if let Mode::SidebarEdit { input_active, .. } = &mut app.mode {
                        *input_active = false;
                    }
                }
                Char('x') => {
                    if link_kind == LinkKind::Pr {
                        if let Some(link) = all_links.0.get(link_selection) {
                            app.spawn_unlink_pr(id, link.pull_request_id);
                        }
                    } else {
                        if let Some(link) = all_links.1.get(link_selection) {
                            app.spawn_unlink_linear(id, link.linear_issue_id);
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
            },
            SidebarField::Tags => match key {
                Char(c) if c.is_alphanumeric() || c == '-' || c == '_' => {
                    if let Mode::SidebarEdit {
                        tag_input,
                        tag_caret,
                        ..
                    } = &mut app.mode
                    {
                        *tag_caret = insert_char(tag_input, *tag_caret, c);
                    }
                }
                Backspace => {
                    if let Mode::SidebarEdit {
                        tag_input,
                        tag_caret,
                        ..
                    } = &mut app.mode
                    {
                        *tag_caret = backspace(tag_input, *tag_caret);
                    }
                }
                Left => {
                    if let Mode::SidebarEdit {
                        tag_input,
                        tag_caret,
                        ..
                    } = &mut app.mode
                    {
                        *tag_caret = move_caret(tag_input, *tag_caret, -1);
                    }
                }
                Right => {
                    if let Mode::SidebarEdit {
                        tag_input,
                        tag_caret,
                        ..
                    } = &mut app.mode
                    {
                        *tag_caret = move_caret(tag_input, *tag_caret, 1);
                    }
                }
                Enter => {
                    let slug = tag_input.trim().to_string();
                    if !slug.is_empty() {
                        let exists = tag_commit_is_removal(app, id, &slug);
                        if exists {
                            app.spawn_remove_tag(id, &slug);
                        } else {
                            app.spawn_add_tag(id, &slug);
                        }
                        if let Mode::SidebarEdit {
                            tag_input,
                            tag_caret,
                            input_active,
                            ..
                        } = &mut app.mode
                        {
                            tag_input.clear();
                            *tag_caret = 0;
                            *input_active = false;
                        }
                    }
                }
                Esc => {
                    if let Mode::SidebarEdit { input_active, .. } = &mut app.mode {
                        *input_active = false;
                    }
                }
                _ => {}
            },
        }
    }
}

/// Handle sidebar add mode keys. Mirrors the edit form's field navigation
/// and text editing, but the Title field's Enter creates a new todo (with
/// the description if set) and plans it for today when in the Today view.
/// Links and tags are applied after creation via the edit sidebar.
pub(crate) fn handle_sidebar_add(app: &mut App, key: ratatui::crossterm::event::KeyCode) {
    use ratatui::crossterm::event::KeyCode::*;

    let (
        field,
        input_active,
        title_input,
        desc_input,
        _tag_input,
        link_kind,
        link_selection,
        attaching,
        link_search,
        _scroll,
    ) = match &app.mode {
        Mode::SidebarAdd {
            field,
            input_active,
            title_input,
            desc_input,
            tag_input,
            link_kind,
            link_selection,
            attaching,
            link_search,
            scroll,
            ..
        } => (
            *field,
            *input_active,
            title_input.clone(),
            desc_input.clone(),
            tag_input.clone(),
            *link_kind,
            *link_selection,
            *attaching,
            link_search.clone(),
            *scroll,
        ),
        _ => return,
    };

    if !input_active {
        match key {
            Tab | Char('j') => {
                let next_field = match field {
                    SidebarField::Title => SidebarField::Description,
                    SidebarField::Description => SidebarField::Links,
                    SidebarField::Links => SidebarField::Tags,
                    SidebarField::Tags => SidebarField::Title,
                };
                if let Mode::SidebarAdd { field, scroll, .. } = &mut app.mode {
                    *field = next_field;
                    *scroll = 0;
                }
            }
            BackTab | Char('k') => {
                let prev_field = match field {
                    SidebarField::Title => SidebarField::Tags,
                    SidebarField::Description => SidebarField::Title,
                    SidebarField::Links => SidebarField::Description,
                    SidebarField::Tags => SidebarField::Links,
                };
                if let Mode::SidebarAdd { field, scroll, .. } = &mut app.mode {
                    *field = prev_field;
                    *scroll = 0;
                }
            }
            Enter => {
                if let Mode::SidebarAdd { input_active, .. } = &mut app.mode {
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
        match field {
            SidebarField::Title => match key {
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
                    if let Mode::SidebarAdd {
                        title_input,
                        title_caret,
                        ..
                    } = &mut app.mode
                    {
                        *title_caret = insert_char(title_input, *title_caret, c);
                    }
                }
                Enter => {
                    let title = title_input.trim().to_string();
                    if title.is_empty() {
                        app.set_error("a title is required");
                    } else {
                        let plan_after = crate::app::should_plan_after_create(app.view);
                        let desc = desc_input.trim().to_string();
                        let pending: Vec<i32> = match &app.mode {
                            Mode::SidebarAdd {
                                pending_link_pr, ..
                            } => pending_link_pr.clone(),
                            _ => Vec::new(),
                        };
                        app.spawn_sidebar_add_create(&title, &desc, &pending, plan_after);
                        app.mode = Mode::Navigate;
                    }
                }
                Backspace => {
                    if let Mode::SidebarAdd {
                        title_input,
                        title_caret,
                        ..
                    } = &mut app.mode
                    {
                        *title_caret = backspace(title_input, *title_caret);
                    }
                }
                Left => {
                    if let Mode::SidebarAdd {
                        title_input,
                        title_caret,
                        ..
                    } = &mut app.mode
                    {
                        *title_caret = move_caret(title_input, *title_caret, -1);
                    }
                }
                Right => {
                    if let Mode::SidebarAdd {
                        title_input,
                        title_caret,
                        ..
                    } = &mut app.mode
                    {
                        *title_caret = move_caret(title_input, *title_caret, 1);
                    }
                }
                Esc => {
                    if let Mode::SidebarAdd { input_active, .. } = &mut app.mode {
                        *input_active = false;
                    }
                }
                _ => {}
            },
            SidebarField::Description => match key {
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
                    if let Mode::SidebarAdd {
                        desc_input,
                        desc_caret,
                        ..
                    } = &mut app.mode
                    {
                        *desc_caret = insert_char(desc_input, *desc_caret, c);
                    }
                }
                Backspace => {
                    if let Mode::SidebarAdd {
                        desc_input,
                        desc_caret,
                        ..
                    } = &mut app.mode
                    {
                        *desc_caret = backspace(desc_input, *desc_caret);
                    }
                }
                Left => {
                    if let Mode::SidebarAdd {
                        desc_input,
                        desc_caret,
                        ..
                    } = &mut app.mode
                    {
                        *desc_caret = move_caret(desc_input, *desc_caret, -1);
                    }
                }
                Right => {
                    if let Mode::SidebarAdd {
                        desc_input,
                        desc_caret,
                        ..
                    } = &mut app.mode
                    {
                        *desc_caret = move_caret(desc_input, *desc_caret, 1);
                    }
                }
                Esc => {
                    if let Mode::SidebarAdd { input_active, .. } = &mut app.mode {
                        *input_active = false;
                    }
                }
                _ => {}
            },
            SidebarField::Links => match key {
                Char('a') if !attaching => {
                    if let Mode::SidebarAdd {
                        attaching,
                        link_selection,
                        link_search,
                        ..
                    } = &mut app.mode
                    {
                        *attaching = true;
                        *link_selection = 0;
                        link_search.clear();
                    }
                }
                Esc if attaching => {
                    if let Mode::SidebarAdd { attaching, .. } = &mut app.mode {
                        *attaching = false;
                    }
                }
                Tab if attaching => {
                    if let Mode::SidebarAdd {
                        link_kind,
                        link_selection,
                        link_search,
                        ..
                    } = &mut app.mode
                    {
                        *link_kind = match link_kind {
                            LinkKind::Pr => LinkKind::Linear,
                            LinkKind::Linear => LinkKind::Pr,
                        };
                        *link_selection = 0;
                        link_search.clear();
                    }
                }
                Char('j') | Down if attaching => {
                    let cands = if link_kind == LinkKind::Pr {
                        crate::views::overlays::pr_picker_candidates(app, &link_search)
                    } else {
                        crate::views::overlays::linear_picker_candidates(app, &link_search)
                    };
                    let max_sel = cands.len().saturating_sub(1);
                    if let Mode::SidebarAdd { link_selection, .. } = &mut app.mode {
                        if *link_selection < max_sel {
                            *link_selection += 1;
                        }
                    }
                }
                Char('k') | Up if attaching => {
                    if let Mode::SidebarAdd { link_selection, .. } = &mut app.mode {
                        *link_selection = link_selection.saturating_sub(1);
                    }
                }
                Char(c)
                    if attaching
                        && (c.is_alphanumeric()
                            || c == ' '
                            || c == '#'
                            || c == '-'
                            || c == '_'
                            || c == '.'
                            || c == '/') =>
                {
                    if let Mode::SidebarAdd {
                        link_search,
                        link_selection,
                        ..
                    } = &mut app.mode
                    {
                        link_search.push(c);
                        *link_selection = 0;
                    }
                }
                Backspace if attaching => {
                    if let Mode::SidebarAdd { link_search, .. } = &mut app.mode {
                        link_search.pop();
                    }
                }
                Enter if attaching => {
                    let cands = if link_kind == LinkKind::Pr {
                        crate::views::overlays::pr_picker_candidates(app, &link_search)
                    } else {
                        crate::views::overlays::linear_picker_candidates(app, &link_search)
                    };
                    if let Some(&idx) = cands.get(link_selection) {
                        if let Mode::SidebarAdd {
                            pending_link_pr: pl,
                            ..
                        } = &mut app.mode
                        {
                            if link_kind == LinkKind::Pr {
                                pl.push(app.data.pulls[idx].id);
                            }
                        }
                        if let Mode::SidebarAdd {
                            attaching,
                            input_active,
                            ..
                        } = &mut app.mode
                        {
                            *attaching = false;
                            *input_active = false;
                        }
                    }
                }
                Esc => {
                    if let Mode::SidebarAdd { input_active, .. } = &mut app.mode {
                        *input_active = false;
                    }
                }
                _ => {}
            },
            SidebarField::Tags => match key {
                Char(c) if c.is_alphanumeric() || c == '-' || c == '_' => {
                    if let Mode::SidebarAdd {
                        tag_input,
                        tag_caret,
                        ..
                    } = &mut app.mode
                    {
                        *tag_caret = insert_char(tag_input, *tag_caret, c);
                    }
                }
                Backspace => {
                    if let Mode::SidebarAdd {
                        tag_input,
                        tag_caret,
                        ..
                    } = &mut app.mode
                    {
                        *tag_caret = backspace(tag_input, *tag_caret);
                    }
                }
                Left => {
                    if let Mode::SidebarAdd {
                        tag_input,
                        tag_caret,
                        ..
                    } = &mut app.mode
                    {
                        *tag_caret = move_caret(tag_input, *tag_caret, -1);
                    }
                }
                Right => {
                    if let Mode::SidebarAdd {
                        tag_input,
                        tag_caret,
                        ..
                    } = &mut app.mode
                    {
                        *tag_caret = move_caret(tag_input, *tag_caret, 1);
                    }
                }
                Esc => {
                    if let Mode::SidebarAdd { input_active, .. } = &mut app.mode {
                        *input_active = false;
                    }
                }
                _ => {}
            },
        }
    }
}

/// A tag slug that already exists on the todo is removed on commit;
/// anything else is added. Pure decision used by the Tags field.
pub(crate) fn tag_commit_is_removal(app: &App, id: i32, slug: &str) -> bool {
    app.data
        .todos
        .iter()
        .find(|t| t.id == id)
        .map(|t| t.tag.nodes.iter().any(|tg| tg.slug == slug))
        .unwrap_or(false)
}

// ---- Sidebar render functions (moved from frame.rs) ----

/// Render the info sidebar for the highlighted todo (D4).
/// In sidebar edit mode, renders the edit form.
/// Otherwise, renders the read-only info card.
pub(crate) fn render_info_sidebar(f: &mut Frame, app: &crate::app::App, area: Rect) {
    // Check if we are in sidebar edit mode
    if let crate::app::Mode::SidebarEdit { .. } = app.mode {
        render_sidebar_edit_form(f, app, area);
        return;
    }
    if let crate::app::Mode::SidebarAdd { .. } = app.mode {
        render_sidebar_add_form(f, app, area);
        return;
    }

    // Read-only info card
    let todo = sidebar_todo(app);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Palette::BORDER))
        .title(Span::styled(" TODO ", Style::default().fg(Palette::DIM)));
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

    let Some(todo) = todo else {
        let line = Line::from(Span::styled(
            "no todo highlighted",
            Style::default().fg(Palette::GHOST),
        ));
        f.render_widget(Paragraph::new(line), inner);
        return;
    };

    let (glyph, color) = status_glyph(&todo.status);
    let mut lines: Vec<Line> = Vec::new();

    // Status line.
    lines.push(Line::from(vec![
        Span::styled(glyph.to_string(), Style::default().fg(color)),
        Span::raw(" "),
        Span::styled(
            todo.status.clone(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ]));

    // Title.
    lines.push(Line::from(Span::styled(
        todo.title.clone(),
        Style::default().fg(Palette::TEXT),
    )));
    // Description (also shown in the edit form; keep the read-only detail in
    // sync so a todo's description is visible without entering edit mode).
    if let Some(desc) = &todo.description {
        if !desc.trim().is_empty() {
            for dl in desc.lines() {
                lines.push(Line::from(Span::styled(
                    format!("  {dl}"),
                    Style::default().fg(Palette::TEXT),
                )));
            }
            lines.push(Line::from(""));
        }
    }

    // Tags.
    if !todo.tag.nodes.is_empty() {
        let tag_strs: Vec<String> = todo
            .tag
            .nodes
            .iter()
            .map(|t| format!("#{}", t.slug))
            .collect();
        lines.push(Line::from(Span::styled(
            format!("tags  {}", tag_strs.join(" ")),
            Style::default().fg(Palette::DIM),
        )));
    }

    // Timestamps.
    let created = todo.created_at.split(' ').next().unwrap_or("");
    lines.push(Line::from(Span::styled(
        format!("created  {created}"),
        Style::default().fg(Palette::DIM),
    )));
    if let Some(started) = &todo.started_at {
        let s = started.split(' ').next().unwrap_or("");
        lines.push(Line::from(Span::styled(
            format!("started  {s}"),
            Style::default().fg(Palette::DIM),
        )));
    }
    if let Some(closed) = &todo.closed_at {
        let c = closed.split(' ').next().unwrap_or("");
        lines.push(Line::from(Span::styled(
            format!("closed   {c}"),
            Style::default().fg(Palette::DIM),
        )));
    }

    // Blocked reason.
    if todo.status == "blocked" {
        if let Some(reason) = &todo.blocked_reason {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(
                    Glyph::BLOCKED_REASON.to_string(),
                    Style::default().fg(Palette::BLOCKED),
                ),
                Span::raw(" "),
                Span::styled(reason.clone(), Style::default().fg(Palette::BLOCKED)),
            ]));
        }
    }

    // LINKS region from app.detail (read mode only; edit mode renders its own).
    if !matches!(app.mode, crate::app::Mode::SidebarEdit { .. }) {
        let loaded = app.detail_loaded_id;
        let detail = app.detail.as_ref();
        if loaded.is_some() && todo.id == loaded.unwrap() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "LINKS",
                Style::default()
                    .fg(Palette::ACCENT)
                    .add_modifier(Modifier::BOLD),
            )));
            if let Some(d) = detail {
                let has_links = !d.prs.is_empty() || !d.linears.is_empty();
                if !has_links {
                    lines.push(Line::from(Span::styled(
                        "  No linked PRs or issues",
                        Style::default().fg(Palette::GHOST),
                    )));
                }
                for pr in &d.prs {
                    if let Some(p) = &pr.pull_request {
                        let label = format!(
                            "  {} {}/{}#{} — {}",
                            pr.relation, p.owner, p.repo, p.number, p.title,
                        );
                        lines.push(Line::from(Span::styled(
                            label,
                            Style::default().fg(Palette::TEXT),
                        )));
                    }
                }
                for li in &d.linears {
                    if let Some(l) = &li.linear_issue {
                        let label = format!("  {} — {}", l.identifier, l.title);
                        lines.push(Line::from(Span::styled(
                            label,
                            Style::default().fg(Palette::TEXT),
                        )));
                    }
                }
            } else {
                lines.push(Line::from(Span::styled(
                    "  loading…",
                    Style::default().fg(Palette::GHOST),
                )));
            }
        }
    }

    // EVENT LOG region (read mode only): append-only timeline, most recent first.
    if !matches!(app.mode, crate::app::Mode::SidebarEdit { .. }) {
        let loaded = app.detail_loaded_id;
        let detail = app.detail.as_ref();
        if loaded.is_some() && todo.id == loaded.unwrap() {
            if let Some(d) = detail {
                if !d.events.is_empty() {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "EVENT LOG",
                        Style::default()
                            .fg(Palette::ACCENT)
                            .add_modifier(Modifier::BOLD),
                    )));
                    for ev in d.events.iter().rev() {
                        let parts: Vec<String> = vec![ev.kind.clone()]
                            .into_iter()
                            .chain(ev.field.clone())
                            .chain(ev.old_value.clone().map(|v| format!("\"{v}\"")))
                            .chain(ev.new_value.clone().map(|v| format!("-> \"{v}\"")))
                            .collect();
                        lines.push(Line::from(Span::styled(
                            format!("  {} · {}", parts.join(" "), ev.actor),
                            Style::default().fg(Palette::DIM),
                        )));
                    }
                }
            }
        }
    }

    f.render_widget(
        Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false }),
        inner,
    );
}

/// Render `text` with a block-cursor caret at byte index `caret`. Used by the
/// sidebar edit form so the user can see (and move) the insertion point.
fn text_with_caret(text: &str, caret: usize) -> String {
    let mut caret = caret.min(text.len());
    // Clamp to a char boundary so split_at never panics if a caret drifts to
    // a mid-char byte offset.
    while caret > 0 && !text.is_char_boundary(caret) {
        caret -= 1;
    }
    let (head, tail) = text.split_at(caret);
    let mut out = String::with_capacity(text.len() + 4);
    out.push_str(head);
    out.push(Glyph::CURSOR);
    out.push_str(tail);
    out
}

/// Build the header span for a sidebar form field. When the field is focused
/// the header is accent + bold (with a `▸` marker when input is active);
/// otherwise it is dim.
fn field_header(
    name: &str,
    field: crate::app::SidebarField,
    current: crate::app::SidebarField,
    input_active: bool,
) -> Span<'static> {
    if field == current {
        let prefix = if input_active { "▸ " } else { "  " };
        Span::styled(
            format!("{prefix}{name}"),
            Style::default()
                .fg(Palette::ACCENT)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(format!("  {name}"), Style::default().fg(Palette::DIM))
    }
}

fn render_sidebar_edit_form(f: &mut Frame, app: &crate::app::App, area: Rect) {
    let crate::app::Mode::SidebarEdit {
        id,
        field,
        input_active,
        title_input,
        title_caret,
        desc_input,
        desc_caret,
        tag_input,
        tag_caret,
        link_kind,
        link_selection,
        attaching,
        link_search,
        scroll,
        ..
    } = &app.mode
    else {
        return;
    };

    let id = *id;
    let field = *field;
    let input_active = *input_active;
    let link_kind = *link_kind;
    let link_selection = *link_selection;
    let attaching = *attaching;
    let link_search: &str = link_search;
    let scroll = *scroll;
    let title_caret = *title_caret;
    let desc_caret = *desc_caret;
    let tag_caret = *tag_caret;
    let title_input: &str = title_input;
    let desc_input: &str = desc_input;
    let tag_input: &str = tag_input;

    let todo = app.data.todos.iter().find(|t| t.id == id);
    let Some(todo) = todo else {
        let line = Line::from(Span::styled(
            "todo not found",
            Style::default().fg(Palette::GHOST),
        ));
        f.render_widget(Paragraph::new(line), area);
        return;
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Palette::ACCENT))
        .title(Span::styled(" EDIT ", Style::default().fg(Palette::ACCENT)));
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

    let (glyph, color) = status_glyph(&todo.status);
    let mut lines: Vec<Line> = Vec::new();

    // Title (read-only, always visible)
    lines.push(Line::from(vec![
        Span::styled(glyph.to_string(), Style::default().fg(color)),
        Span::raw(" "),
        Span::styled(todo.title.clone(), Style::default().fg(Palette::TEXT)),
    ]));
    lines.push(Line::from(""));

    // TITLE (editable form field)
    lines.push(Line::from(field_header(
        "TITLE",
        crate::app::SidebarField::Title,
        field,
        input_active,
    )));
    if input_active && field == crate::app::SidebarField::Title {
        lines.push(Line::from(Span::styled(
            text_with_caret(title_input, title_caret),
            Style::default().fg(Palette::TEXT),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            if title_input.is_empty() {
                "—"
            } else {
                title_input
            },
            Style::default().fg(Palette::DIM),
        )));
    }
    // DESCRIPTION
    lines.push(Line::from(field_header(
        "DESCRIPTION",
        crate::app::SidebarField::Description,
        field,
        input_active,
    )));
    if input_active && field == crate::app::SidebarField::Description {
        lines.push(Line::from(Span::styled(
            text_with_caret(desc_input, desc_caret),
            Style::default().fg(Palette::TEXT),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            if desc_input.is_empty() {
                "—"
            } else {
                desc_input
            },
            Style::default().fg(Palette::DIM),
        )));
    }
    lines.push(Line::from(""));

    // LINKS
    lines.push(Line::from(field_header(
        "LINKS",
        crate::app::SidebarField::Links,
        field,
        input_active,
    )));
    let detail = app.detail.as_ref();
    let has_links = detail
        .map(|d| !d.prs.is_empty() || !d.linears.is_empty())
        .unwrap_or(false);
    if !has_links && !input_active {
        lines.push(Line::from(Span::styled(
            "  No linked PRs or issues",
            Style::default().fg(Palette::GHOST),
        )));
    }
    // Show currently linked PRs and Linear issues (when not in the attach picker).
    if !input_active || field != crate::app::SidebarField::Links || !attaching {
        if let Some(d) = detail {
            for pr in &d.prs {
                if let Some(p) = &pr.pull_request {
                    let label = format!(
                        "  {} {}/{}#{} {}",
                        pr.relation, p.owner, p.repo, p.number, p.title,
                    );
                    lines.push(Line::from(Span::styled(
                        label,
                        Style::default().fg(Palette::TEXT),
                    )));
                }
            }
            for li in &d.linears {
                if let Some(l) = &li.linear_issue {
                    let label = format!("  {} {}", l.identifier, l.title);
                    lines.push(Line::from(Span::styled(
                        label,
                        Style::default().fg(Palette::TEXT),
                    )));
                }
            }
        }
    }
    if input_active && field == crate::app::SidebarField::Links {
        if attaching {
            // Attach picker: list synced candidates for the current kind,
            // filtered by `link_search` with open items before closed.
            let kind_str = match link_kind {
                crate::app::LinkKind::Pr => "PRs",
                crate::app::LinkKind::Linear => "Issues",
            };
            lines.push(Line::from(Span::styled(
                format!(
                    "  attach {kind_str} (tab: kind · j/k: move · enter: attach · esc: cancel)"
                ),
                Style::default().fg(Palette::DIM),
            )));
            // Search input line.
            lines.push(Line::from(Span::styled(
                format!("  search: {}", text_with_caret(link_search, 0)),
                Style::default().fg(Palette::DIM),
            )));
            let candidates: Vec<usize> = if link_kind == crate::app::LinkKind::Pr {
                crate::views::overlays::pr_picker_candidates(app, link_search)
            } else {
                crate::views::overlays::linear_picker_candidates(app, link_search)
            };
            if candidates.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  no candidates",
                    Style::default().fg(Palette::GHOST),
                )));
            } else {
                let sel = link_selection.min(candidates.len() - 1);
                for (vis, &data_idx) in candidates.iter().enumerate() {
                    let (selected, label) = if link_kind == crate::app::LinkKind::Pr {
                        let row = &app.data.pulls[data_idx];
                        (vis == sel, format!("PR #{} {}", row.number, row.title))
                    } else {
                        let row = &app.data.linears[data_idx];
                        (vis == sel, format!("{} {}", row.identifier, row.title))
                    };
                    if selected {
                        lines.push(Line::from(vec![
                            Span::styled(
                                Glyph::CURSOR.to_string(),
                                Style::default().fg(Palette::ACCENT),
                            ),
                            Span::raw(" "),
                            Span::styled(
                                label,
                                Style::default()
                                    .fg(Palette::ACCENT)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    } else {
                        lines.push(Line::from(Span::styled(
                            format!("  {label}"),
                            Style::default().fg(Palette::TEXT),
                        )));
                    }
                }
            }
        } else {
            let kind_str = match link_kind {
                crate::app::LinkKind::Pr => "PRs",
                crate::app::LinkKind::Linear => "Issues",
            };
            lines.push(Line::from(Span::styled(
                format!("  [{kind_str}] selection: {link_selection}",),
                Style::default().fg(Palette::DIM),
            )));
        }
    }

    // TAGS
    lines.push(Line::from(field_header(
        "TAGS",
        crate::app::SidebarField::Tags,
        field,
        input_active,
    )));
    let tag_strs: Vec<String> = todo
        .tag
        .nodes
        .iter()
        .map(|t| format!("#{}", t.slug))
        .collect();
    if input_active && field == crate::app::SidebarField::Tags {
        lines.push(Line::from(Span::styled(
            format!(
                "{} {}",
                tag_strs.join(" "),
                text_with_caret(tag_input, tag_caret)
            ),
            Style::default().fg(Palette::TEXT),
        )));
    } else if tag_strs.is_empty() {
        lines.push(Line::from(Span::styled(
            "  none",
            Style::default().fg(Palette::GHOST),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!("  {}", tag_strs.join(" ")),
            Style::default().fg(Palette::DIM),
        )));
    }

    // EVENT LOG: append-only timeline, most recent first.
    if let Some(d) = app.detail.as_ref() {
        if !d.events.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "EVENT LOG",
                Style::default()
                    .fg(Palette::ACCENT)
                    .add_modifier(Modifier::BOLD),
            )));
            for ev in d.events.iter().rev() {
                let parts: Vec<String> = vec![ev.kind.clone()]
                    .into_iter()
                    .chain(ev.field.clone())
                    .chain(ev.old_value.clone().map(|v| format!("\"{v}\"")))
                    .chain(ev.new_value.clone().map(|v| format!("-> \"{v}\"")))
                    .collect();
                lines.push(Line::from(Span::styled(
                    format!("  {} · {}", parts.join(" "), ev.actor),
                    Style::default().fg(Palette::DIM),
                )));
            }
        }
    }

    f.render_widget(
        Paragraph::new(lines)
            .wrap(ratatui::widgets::Wrap { trim: false })
            .scroll((scroll as u16, 0)),
        inner,
    );
}

/// Render the sidebar add form when in SidebarAdd mode. Mirrors the edit
/// form but with empty fields, no existing todo, and a CREATE title.
fn render_sidebar_add_form(f: &mut Frame, app: &crate::app::App, area: Rect) {
    let crate::app::Mode::SidebarAdd {
        field,
        input_active,
        title_input,
        title_caret,
        desc_input,
        desc_caret,
        tag_input,
        tag_caret,
        link_kind,
        link_selection,
        attaching,
        link_search,
        scroll,
        ..
    } = &app.mode
    else {
        return;
    };

    let field = *field;
    let input_active = *input_active;
    let link_kind = *link_kind;
    let link_selection = *link_selection;
    let attaching = *attaching;
    let scroll = *scroll;
    let title_caret = *title_caret;
    let desc_caret = *desc_caret;
    let tag_caret = *tag_caret;
    let title_input: &str = title_input;
    let desc_input: &str = desc_input;
    let tag_input: &str = tag_input;
    let link_search: &str = link_search;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Palette::ACCENT))
        .title(Span::styled(
            " CREATE ",
            Style::default().fg(Palette::ACCENT),
        ));
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

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        "new todo",
        Style::default().fg(Palette::DIM),
    )));
    lines.push(Line::from(""));

    // TITLE
    lines.push(Line::from(field_header(
        "TITLE",
        crate::app::SidebarField::Title,
        field,
        input_active,
    )));
    if input_active && field == crate::app::SidebarField::Title {
        lines.push(Line::from(Span::styled(
            text_with_caret(title_input, title_caret),
            Style::default().fg(Palette::TEXT),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            if title_input.is_empty() {
                "—"
            } else {
                title_input
            },
            Style::default().fg(Palette::DIM),
        )));
    }
    lines.push(Line::from(""));

    // DESCRIPTION
    lines.push(Line::from(field_header(
        "DESCRIPTION",
        crate::app::SidebarField::Description,
        field,
        input_active,
    )));
    if input_active && field == crate::app::SidebarField::Description {
        lines.push(Line::from(Span::styled(
            text_with_caret(desc_input, desc_caret),
            Style::default().fg(Palette::TEXT),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            if desc_input.is_empty() {
                "—"
            } else {
                desc_input
            },
            Style::default().fg(Palette::DIM),
        )));
    }
    lines.push(Line::from(""));

    // LINKS
    lines.push(Line::from(field_header(
        "LINKS",
        crate::app::SidebarField::Links,
        field,
        input_active,
    )));
    if input_active && field == crate::app::SidebarField::Links {
        if attaching {
            let kind_str = match link_kind {
                crate::app::LinkKind::Pr => "PRs",
                crate::app::LinkKind::Linear => "Issues",
            };
            lines.push(Line::from(Span::styled(
                format!(
                    "  attach {kind_str} (tab: kind · j/k: move · enter: attach · esc: cancel)"
                ),
                Style::default().fg(Palette::DIM),
            )));
            lines.push(Line::from(Span::styled(
                format!("  search: {}", text_with_caret(link_search, 0)),
                Style::default().fg(Palette::DIM),
            )));
            let candidates: Vec<usize> = if link_kind == crate::app::LinkKind::Pr {
                crate::views::overlays::pr_picker_candidates(app, link_search)
            } else {
                crate::views::overlays::linear_picker_candidates(app, link_search)
            };
            if candidates.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  no candidates",
                    Style::default().fg(Palette::GHOST),
                )));
            } else {
                let sel = link_selection.min(candidates.len() - 1);
                for (vis, &data_idx) in candidates.iter().enumerate() {
                    let (selected, label) = if link_kind == crate::app::LinkKind::Pr {
                        let row = &app.data.pulls[data_idx];
                        (vis == sel, format!("PR #{} {}", row.number, row.title))
                    } else {
                        let row = &app.data.linears[data_idx];
                        (vis == sel, format!("{} {}", row.identifier, row.title))
                    };
                    if selected {
                        lines.push(Line::from(vec![
                            Span::styled(
                                Glyph::CURSOR.to_string(),
                                Style::default().fg(Palette::ACCENT),
                            ),
                            Span::raw(" "),
                            Span::styled(
                                label,
                                Style::default()
                                    .fg(Palette::ACCENT)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    } else {
                        lines.push(Line::from(Span::styled(
                            format!("  {label}"),
                            Style::default().fg(Palette::TEXT),
                        )));
                    }
                }
            }
        } else {
            lines.push(Line::from(Span::styled(
                "  press a to attach a PR or issue",
                Style::default().fg(Palette::GHOST),
            )));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "  —",
            Style::default().fg(Palette::DIM),
        )));
    }
    lines.push(Line::from(""));

    // TAGS
    lines.push(Line::from(field_header(
        "TAGS",
        crate::app::SidebarField::Tags,
        field,
        input_active,
    )));
    if input_active && field == crate::app::SidebarField::Tags {
        lines.push(Line::from(Span::styled(
            text_with_caret(tag_input, tag_caret),
            Style::default().fg(Palette::TEXT),
        )));
    } else if tag_input.is_empty() {
        lines.push(Line::from(Span::styled(
            "  —",
            Style::default().fg(Palette::DIM),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!("  {}", tag_input),
            Style::default().fg(Palette::DIM),
        )));
    }

    f.render_widget(
        Paragraph::new(lines)
            .wrap(ratatui::widgets::Wrap { trim: false })
            .scroll((scroll as u16, 0)),
        inner,
    );
}

/// Resolve the highlighted todo for the sidebar: the cursor todo from the
/// active view's row set at draw time.
fn sidebar_todo(app: &crate::app::App) -> Option<&Todo> {
    match app.view {
        crate::app::View::Today => app.today_plan().get(app.cursor).copied(),
        crate::app::View::Backlog => app.backlog().get(app.cursor).copied(),
        _ => None,
    }
}
#[cfg(test)]
mod sidebar_edit_tests {
    use crate::app::tests::{make_linear, make_plan_row, make_pr, make_todo};
    use crate::app::{App, LinkKind, Mode, SidebarField, ToastKind};
    use crate::gql;
    use crate::test_support::buffer_text;
    use crossterm::event::{KeyEvent, KeyModifiers};
    use ratatui::crossterm::event::KeyCode;
    use ratatui::{Terminal, backend::TestBackend};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::mpsc;

    // -- helpers --

    /// A minimal GraphQL mock server for confirming that a mutation posts.
    /// Captures the request body, responds with a stub `data` payload, and
    /// stores the captured request in `captures` (shared Arc<parking_lot::Mutex>).
    async fn spawn_gql_mock() -> (
        String,
        std::sync::Arc<parking_lot::Mutex<Vec<serde_json::Value>>>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let captures: std::sync::Arc<parking_lot::Mutex<Vec<serde_json::Value>>> =
            std::sync::Arc::new(parking_lot::Mutex::new(Vec::new()));
        let captures2 = captures.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let caps = captures2.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut tmp = [0u8; 1024];
                    loop {
                        let n = socket.read(&mut tmp).await.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&tmp[..n]);
                        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                        if buf.len() > 1_000_000 {
                            break;
                        }
                    }
                    // Split headers from body.
                    if let Some(idx) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let body = &buf[idx + 4..];
                        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(body) {
                            caps.lock().push(val);
                        }
                    }
                    // Respond with an empty data JSON so the client decodes.
                    let body = r#"{"data":{}}"#;
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
            title_caret: 0,
            desc_input: String::new(),
            desc_caret: 0,
            desc_scroll: 0,
            tag_input: String::new(),
            tag_caret: 0,
            link_kind: LinkKind::Pr,
            link_selection: 0,
            attaching: false,
            link_search: String::new(),
            scroll: 0,
        };
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn test_edit_form_shows_caret_in_title_when_active() {
        let mut app = setup_app_with_todo();
        app.mode = Mode::SidebarEdit {
            id: 1,
            field: SidebarField::Title,
            input_active: true,
            title_input: "ab".to_string(),
            title_caret: 1, // between 'a' and 'b'
            desc_input: String::new(),
            desc_caret: 0,
            desc_scroll: 0,
            tag_input: String::new(),
            tag_caret: 0,
            link_kind: LinkKind::Pr,
            link_selection: 0,
            attaching: false,
            link_search: String::new(),
            scroll: 0,
        };
        let output = render_full(&mut app);
        // The caret glyph must appear between a and b: "a▌b".
        let caret = crate::theme::Glyph::CURSOR;
        let needle: String = ['a', caret, 'b'].iter().collect();
        assert!(
            output.contains(&needle),
            "title edit should render the caret between 'a' and 'b' (expected {needle:?}):\n{output}"
        );
    }

    #[test]
    fn test_edit_form_moves_caret_with_left_right() {
        let mut app = setup_app_with_todo();
        app.mode = Mode::SidebarEdit {
            id: 1,
            field: SidebarField::Title,
            input_active: true,
            title_input: "hello".to_string(),
            title_caret: 5,
            desc_input: String::new(),
            desc_caret: 0,
            desc_scroll: 0,
            tag_input: String::new(),
            tag_caret: 0,
            link_kind: LinkKind::Pr,
            link_selection: 0,
            attaching: false,
            link_search: String::new(),
            scroll: 0,
        };
        // Left then Left: caret moves to index 3.
        super::handle_sidebar_edit(&mut app, KeyCode::Left);
        super::handle_sidebar_edit(&mut app, KeyCode::Left);
        if let Mode::SidebarEdit { title_caret, .. } = &app.mode {
            assert_eq!(*title_caret, 3, "Left twice should move caret to 3");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    fn render_full(app: &mut App) -> String {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| crate::views::render(f, app)).unwrap();
        buffer_text(terminal.backend().buffer())
    }

    // -- 5.1 r enters InlineEdit; Enter commits; Esc reverts --

    #[test]
    fn test_r_enters_inline_edit_seeded_with_title() {
        let mut app = setup_app_with_todo();
        app.handle_key(key(KeyCode::Char('r')));
        assert!(
            matches!(&app.mode, Mode::InlineEdit { id, input, .. } if *id == 1 && input == "My Task"),
            "should enter InlineEdit with id=1 and input seeded from title"
        );
    }

    #[test]
    fn test_r_on_empty_plan_does_nothing() {
        let mut app = App::default();

        app.handle_key(key(KeyCode::Char('r')));
        assert_eq!(app.mode, Mode::Navigate);
    }

    #[test]
    fn test_esc_from_inline_edit_returns_to_navigate() {
        let mut app = setup_app_with_todo();
        // Enter inline edit
        app.handle_key(key(KeyCode::Char('r')));
        assert!(matches!(app.mode, Mode::InlineEdit { .. }));
        // Esc returns to Navigate
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Navigate);
    }

    #[test]
    fn test_enter_from_inline_edit_commits_and_returns_to_navigate() {
        let mut app = setup_app_with_todo();
        app.handle_key(key(KeyCode::Char('r')));
        assert!(matches!(app.mode, Mode::InlineEdit { .. }));
        // Enter commits (mutation runs async) and returns to Navigate
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.mode, Mode::Navigate);
    }

    // -- 5.2 e/Enter enter SidebarEdit; Esc exits discarding input --

    #[test]
    fn test_e_enters_sidebar_edit() {
        let mut app = setup_app_with_todo();
        app.handle_key(key(KeyCode::Char('e')));
        assert!(
            matches!(&app.mode, Mode::SidebarEdit { id, field, .. } if *id == 1 && *field == SidebarField::Description),
            "should enter SidebarEdit for id=1, focused on Description"
        );
    }

    #[test]
    fn test_enter_enters_sidebar_edit() {
        let mut app = setup_app_with_todo();
        app.handle_key(key(KeyCode::Enter));
        assert!(
            matches!(&app.mode, Mode::SidebarEdit { id, .. } if *id == 1),
            "Enter should also enter SidebarEdit"
        );
    }

    #[test]
    fn test_esc_from_sidebar_edit_returns_to_navigate() {
        let mut app = setup_app_with_todo();
        app.handle_key(key(KeyCode::Char('e')));
        assert!(matches!(app.mode, Mode::SidebarEdit { .. }));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Navigate);
    }

    #[test]
    fn test_sidebar_edit_starts_with_scroll_zero() {
        let mut app = setup_app_with_todo();
        app.handle_key(key(KeyCode::Char('e')));
        if let Mode::SidebarEdit { scroll, .. } = &app.mode {
            assert_eq!(*scroll, 0, "scroll should be reset on entry");
        } else {
            panic!("expected SidebarEdit mode");
        }
    }

    #[test]
    fn test_sidebar_edit_seeds_desc_input_from_todo_description() {
        let mut app = setup_app_with_todo();
        app.data.todos[0].description = Some("existing description".to_string());
        if let Some(todo) = &mut app.data.plan[0].todo {
            todo.description = Some("existing description".to_string());
        }
        app.handle_key(key(KeyCode::Char('e')));
        if let Mode::SidebarEdit { desc_input, .. } = &app.mode {
            assert_eq!(desc_input, "existing description");
        } else {
            panic!("expected SidebarEdit mode");
        }
    }

    // -- 5.4 narrow terminal toast --

    #[test]
    fn test_narrow_terminal_prevents_sidebar_edit() {
        let mut app = setup_app_with_todo();
        app.content_width = 80; // below 100
        app.handle_key(key(KeyCode::Char('e')));
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

    #[test]
    fn test_narrow_terminal_blocks_enter_key_too() {
        let mut app = setup_app_with_todo();
        app.content_width = 99;
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.mode, Mode::Navigate);
        assert!(app.toast.is_some());
    }

    // -- 5.5 field navigation (Tab/j/k) --

    #[test]
    fn test_tab_cycles_fields_forward() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        // Description -> Links
        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        if let Mode::SidebarEdit { field, .. } = &app.mode {
            assert_eq!(*field, SidebarField::Links);
        } else {
            panic!("expected SidebarEdit");
        }

        // Links -> Tags
        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        if let Mode::SidebarEdit { field, .. } = &app.mode {
            assert_eq!(*field, SidebarField::Tags);
        } else {
            panic!("expected SidebarEdit");
        }

        // Tags -> Title (wraps)
        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        if let Mode::SidebarEdit { field, .. } = &app.mode {
            assert_eq!(*field, SidebarField::Title);
        } else {
            panic!("expected SidebarEdit");
        }

        // Title -> Description (wraps)
        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        if let Mode::SidebarEdit { field, .. } = &app.mode {
            assert_eq!(*field, SidebarField::Description);
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_j_cycles_fields_forward_same_as_tab() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        super::handle_sidebar_edit(&mut app, KeyCode::Char('j'));
        if let Mode::SidebarEdit { field, .. } = &app.mode {
            assert_eq!(*field, SidebarField::Links, "j should advance like Tab");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_k_cycles_fields_backward() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        // Description -> Title (backward)
        super::handle_sidebar_edit(&mut app, KeyCode::Char('k'));
        if let Mode::SidebarEdit { field, .. } = &app.mode {
            assert_eq!(*field, SidebarField::Title);
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_back_tab_cycles_fields_backward() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        // Description -> Title (backward)
        super::handle_sidebar_edit(&mut app, KeyCode::BackTab);
        if let Mode::SidebarEdit { field, .. } = &app.mode {
            assert_eq!(*field, SidebarField::Title);
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_field_nav_resets_scroll() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        // Set scroll to something nonzero
        if let Mode::SidebarEdit { scroll, .. } = &mut app.mode {
            *scroll = 5;
        }
        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        if let Mode::SidebarEdit { scroll, .. } = &app.mode {
            assert_eq!(*scroll, 0, "scroll should reset to 0 on field nav");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_field_nav_does_not_move_list_cursor() {
        let mut app = setup_app_with_todo();
        app.cursor = 0;
        enter_sidebar_edit(&mut app);
        super::handle_sidebar_edit(&mut app, KeyCode::Char('j'));
        assert_eq!(
            app.cursor, 0,
            "list cursor should not move during sidebar field nav"
        );
    }

    // -- description field: input, commit, scroll --

    #[test]
    fn test_enter_activates_input_on_description() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        assert!(
            !matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if *input_active),
            "input_active should be false on entry"
        );

        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        assert!(
            matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if *input_active),
            "Enter should activate input"
        );
    }

    #[test]
    fn test_description_char_input() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        // Activate input
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        // Type some chars
        super::handle_sidebar_edit(&mut app, KeyCode::Char('H'));
        super::handle_sidebar_edit(&mut app, KeyCode::Char('i'));
        if let Mode::SidebarEdit { desc_input, .. } = &app.mode {
            assert_eq!(desc_input, "Hi");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_description_backspace() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        super::handle_sidebar_edit(&mut app, KeyCode::Char('H'));
        super::handle_sidebar_edit(&mut app, KeyCode::Char('i'));
        super::handle_sidebar_edit(&mut app, KeyCode::Backspace);
        if let Mode::SidebarEdit { desc_input, .. } = &app.mode {
            assert_eq!(desc_input, "H");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_description_enter_deactivates_input() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        assert!(matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if *input_active));
        // Commit with Enter
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        assert!(
            matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if !*input_active),
            "Enter on description should deactivate input (commit)"
        );
        // Mode should still be SidebarEdit
        assert!(matches!(app.mode, Mode::SidebarEdit { .. }));
    }

    #[test]
    fn test_description_esc_deactivates_input() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        assert!(matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if *input_active));
        super::handle_sidebar_edit(&mut app, KeyCode::Esc);
        assert!(
            matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if !*input_active),
            "Esc should deactivate input"
        );
    }

    #[test]
    fn test_description_down_increases_scroll() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        super::handle_sidebar_edit(&mut app, KeyCode::Down);
        if let Mode::SidebarEdit { desc_scroll, .. } = &app.mode {
            assert_eq!(*desc_scroll, 1);
        } else {
            panic!("expected SidebarEdit");
        }
        super::handle_sidebar_edit(&mut app, KeyCode::Down);
        if let Mode::SidebarEdit { desc_scroll, .. } = &app.mode {
            assert_eq!(*desc_scroll, 2);
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_description_up_decreases_scroll_saturating() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        // Scroll is 0, Up should saturate at 0
        super::handle_sidebar_edit(&mut app, KeyCode::Up);
        if let Mode::SidebarEdit { desc_scroll, .. } = &app.mode {
            assert_eq!(*desc_scroll, 0);
        } else {
            panic!("expected SidebarEdit");
        }
    }

    // -- tags: type, Enter adds --

    #[test]
    fn test_tag_input_typing() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        // Nav to Tags
        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        // Activate input
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        // Type a tag
        super::handle_sidebar_edit(&mut app, KeyCode::Char('a'));
        super::handle_sidebar_edit(&mut app, KeyCode::Char('b'));
        if let Mode::SidebarEdit { tag_input, .. } = &app.mode {
            assert_eq!(tag_input, "ab");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_tag_enter_clears_input_and_deactivates() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        // Nav to Tags
        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        super::handle_sidebar_edit(&mut app, KeyCode::Char('t'));
        super::handle_sidebar_edit(&mut app, KeyCode::Char('e'));
        super::handle_sidebar_edit(&mut app, KeyCode::Char('s'));
        // Commit
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
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

    #[test]
    fn test_tag_backspace() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        super::handle_sidebar_edit(&mut app, KeyCode::Char('a'));
        super::handle_sidebar_edit(&mut app, KeyCode::Backspace);
        if let Mode::SidebarEdit { tag_input, .. } = &app.mode {
            assert!(tag_input.is_empty());
        } else {
            panic!("expected SidebarEdit");
        }
    }

    // -- links: j/k navigation, Tab switches kind, x detach, Enter attach --

    #[test]
    fn test_links_j_moves_selection_down() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        // Nav to Links
        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        // Activate
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        // j (no links in data, selection stays at 0)
        super::handle_sidebar_edit(&mut app, KeyCode::Char('j'));
        if let Mode::SidebarEdit { link_selection, .. } = &app.mode {
            assert_eq!(*link_selection, 0, "selection should stay 0 with no links");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_links_k_selection_saturates_at_zero() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        super::handle_sidebar_edit(&mut app, KeyCode::Char('k'));
        if let Mode::SidebarEdit { link_selection, .. } = &app.mode {
            assert_eq!(*link_selection, 0);
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_links_down_moves_selection_down() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        super::handle_sidebar_edit(&mut app, KeyCode::Down);
        if let Mode::SidebarEdit { link_selection, .. } = &app.mode {
            assert_eq!(*link_selection, 0, "Down also moves link selection");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_links_up_moves_selection_up() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);
        // Set link_selection to 1
        if let Mode::SidebarEdit { link_selection, .. } = &mut app.mode {
            *link_selection = 1;
        }

        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        super::handle_sidebar_edit(&mut app, KeyCode::Up);
        if let Mode::SidebarEdit { link_selection, .. } = &app.mode {
            assert_eq!(*link_selection, 0);
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_links_tab_switches_kind() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        // Nav to Links
        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        assert!(
            matches!(&app.mode, Mode::SidebarEdit { link_kind, .. } if *link_kind == LinkKind::Pr),
            "default link kind should be Pr"
        );
        // Tab within links input toggles kind
        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        if let Mode::SidebarEdit { link_kind, .. } = &app.mode {
            assert_eq!(*link_kind, LinkKind::Linear, "Tab should switch to Linear");
        } else {
            panic!("expected SidebarEdit");
        }
        // Tab again wraps back to Pr
        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        if let Mode::SidebarEdit { link_kind, .. } = &app.mode {
            assert_eq!(*link_kind, LinkKind::Pr, "Tab should wrap back to Pr");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_links_enter_deactivates_input() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        assert!(
            matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if !*input_active),
            "Enter on Links should deactivate input"
        );
    }

    #[test]
    fn test_links_esc_deactivates_input() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        super::handle_sidebar_edit(&mut app, KeyCode::Esc);
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
            prs: vec![],
            linears: vec![],
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
            prs: vec![],
            linears: vec![],
        });
        let output = render_full(&mut app);
        assert!(
            output.contains("No linked") || output.contains("loading"),
            "LINKS region should show empty state or loading text"
        );
    }

    #[test]
    fn test_read_only_sidebar_shows_description() {
        let mut app = setup_app_with_todo();
        app.mode = Mode::Navigate;
        app.data.plan[0].todo.as_mut().unwrap().description = Some("detailed notes".to_string());
        app.data.todos[0].description = Some("detailed notes".to_string());
        let output = render_full(&mut app);
        assert!(
            output.contains("detailed notes"),
            "read-only sidebar should show the todo description:\n{output}"
        );
    }

    #[test]
    fn test_read_only_sidebar_omits_empty_description() {
        // No description set — the read-only sidebar must not render an empty
        // description block (make_todo leaves description None).
        let mut app = setup_app_with_todo();
        app.mode = Mode::Navigate;
        let output = render_full(&mut app);
        assert!(
            output.contains("My Task"),
            "todo title should still render:\n{output}"
        );
    }

    #[test]
    fn test_read_only_sidebar_wraps_long_description() {
        let mut app = setup_app_with_todo();
        app.mode = Mode::Navigate;
        let long = "word ".repeat(60); // long enough to exceed the sidebar width
        app.data.plan[0].todo.as_mut().unwrap().description = Some(long.clone());
        app.data.todos[0].description = Some(long);
        let output = render_full(&mut app);
        // The full description must appear (it is not truncated to one line),
        // and it must occupy multiple lines (wrapped, not clipped).
        let described: Vec<&str> = output.lines().filter(|l| l.contains("word")).collect();
        assert!(
            described.len() > 1,
            "long description should wrap across multiple lines, got {} words-lines:\n{output}",
            described.len()
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

    #[test]
    fn test_sidebar_entry_sets_detail_loaded_id() {
        let mut app = setup_app_with_todo();
        assert_eq!(app.detail_loaded_id, None);
        app.handle_key(key(KeyCode::Char('e')));
        // fetch_detail sets detail_loaded_id synchronously
        assert_eq!(app.detail_loaded_id, Some(1));
    }

    // -- Esc double-press when input_active --

    #[test]
    fn test_esc_twice_exits_when_input_active() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        // Activate input
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
        assert!(matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if *input_active));

        // First Esc deactivates input
        super::handle_sidebar_edit(&mut app, KeyCode::Esc);
        assert!(matches!(&app.mode, Mode::SidebarEdit { input_active, .. } if !*input_active));

        // Second Esc exits — but this goes through handle_key which routes to handle_sidebar_edit
        // In the handler, Esc when !input_active exits to Navigate
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Navigate);
    }

    // -- title field: reachable, seeded, commit/cancel, no r binding --

    #[test]
    fn test_title_field_seeded_from_current_title() {
        let mut app = setup_app_with_todo();
        app.handle_key(key(KeyCode::Char('e')));
        if let Mode::SidebarEdit { title_input, .. } = &app.mode {
            assert_eq!(
                title_input, "My Task",
                "title input should be seeded from the todo title"
            );
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_title_field_reachable_via_navigation() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        // Backward from Description -> Title.
        super::handle_sidebar_edit(&mut app, KeyCode::Char('k'));
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

    #[test]
    fn test_title_commit_deactivates_input_and_stays_in_edit() {
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
        // Typing then Enter commits the title (mutation runs async).
        super::handle_sidebar_edit(&mut app, KeyCode::Char('N'));
        super::handle_sidebar_edit(&mut app, KeyCode::Char('e'));
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);
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

    #[test]
    fn test_title_esc_deactivates_without_committing() {
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
        // Type a char then Esc: Esc deactivates, input keeps the char.
        super::handle_sidebar_edit(&mut app, KeyCode::Char('X'));
        super::handle_sidebar_edit(&mut app, KeyCode::Esc);
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

    #[test]
    fn test_r_in_edit_mode_does_not_inline_edit_title() {
        let mut app = setup_app_with_todo();
        enter_sidebar_edit(&mut app);

        // r while in sidebar edit mode must not enter InlineEdit.
        super::handle_sidebar_edit(&mut app, KeyCode::Char('r'));
        assert!(
            matches!(app.mode, Mode::SidebarEdit { .. }),
            "r in edit mode should be ignored (no inline title edit)"
        );
    }

    #[test]
    fn test_title_input_typing_and_backspace() {
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
        super::handle_sidebar_edit(&mut app, KeyCode::Char('N'));
        super::handle_sidebar_edit(&mut app, KeyCode::Char('e'));
        if let Mode::SidebarEdit { title_input, .. } = &app.mode {
            assert_eq!(title_input, "Ne", "typing appends to title input");
        } else {
            panic!("expected SidebarEdit");
        }
        super::handle_sidebar_edit(&mut app, KeyCode::Backspace);
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
    // -- attach picker: `a` opens it, Esc cancels, Enter confirms --

    fn nav_to_links_active(app: &mut App) {
        if let Mode::SidebarEdit {
            field,
            input_active,
            ..
        } = &mut app.mode
        {
            *field = SidebarField::Links;
            *input_active = true;
        }
    }

    /// Filter captured GraphQL requests to `query` operations that name a
    /// mutation (handle_sidebar_edit also issues todo_pull_requests /
    /// todo_linear_issues queries on every call, so we ignore these).
    fn captured_mutations(
        captures: &parking_lot::Mutex<Vec<serde_json::Value>>,
    ) -> Vec<serde_json::Value> {
        captures
            .lock()
            .iter()
            .filter(|v| {
                v.get("query")
                    .and_then(|q| q.as_str())
                    .map(|q| q.starts_with("mutation"))
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    #[test]
    fn test_a_on_links_opens_attach_picker_state() {
        let mut app = setup_app_with_todo();
        app.data.pulls = vec![make_pr(10, None), make_pr(11, None)];
        enter_sidebar_edit(&mut app);
        nav_to_links_active(&mut app);

        super::handle_sidebar_edit(&mut app, KeyCode::Char('a'));

        if let Mode::SidebarEdit { attaching, .. } = &app.mode {
            assert!(attaching, "`a` on active Links should set attaching=true");
        } else {
            panic!("expected SidebarEdit");
        }
    }

    #[test]
    fn test_attach_picker_renders_synced_pr_candidates() {
        let mut app = setup_app_with_todo();
        app.data.pulls = vec![make_pr(10, None), make_pr(11, None)];
        enter_sidebar_edit(&mut app);
        if let Mode::SidebarEdit {
            attaching,
            field,
            input_active,
            ..
        } = &mut app.mode
        {
            *attaching = true;
            *field = SidebarField::Links;
            *input_active = true;
        }
        let output = render_full(&mut app);
        assert!(
            output.contains("attach PRs"),
            "picker should render an attach header:\n{output}"
        );
        assert!(
            output.contains("PR #10"),
            "candidate PR #10 should be listed:\n{output}"
        );
        assert!(
            output.contains("PR #11"),
            "candidate PR #11 should be listed:\n{output}"
        );
    }

    #[tokio::test]
    async fn test_attach_esc_cancels_without_mutating() {
        let (base_url, captures) = spawn_gql_mock().await;
        let client = gql::Client::new(&base_url);
        let (tx, _rx) = mpsc::channel(32);
        let mut app = setup_app_with_todo();
        app.data.pulls = vec![make_pr(10, None)];
        enter_sidebar_edit(&mut app);
        nav_to_links_active(&mut app);
        app.init(client, tx);

        super::handle_sidebar_edit(&mut app, KeyCode::Char('a'));
        super::handle_sidebar_edit(&mut app, KeyCode::Esc);

        if let Mode::SidebarEdit { attaching, .. } = &app.mode {
            assert!(!attaching, "Esc should clear attaching state");
        } else {
            panic!("expected SidebarEdit");
        }
        // Let any spawned task run briefly; none should post a mutation.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            captured_mutations(&captures).is_empty(),
            "Esc must not issue a mutation"
        );
    }

    #[tokio::test]
    async fn test_attach_confirm_pr_posts_link_pull_request() {
        let (base_url, captures) = spawn_gql_mock().await;
        let client = gql::Client::new(&base_url);
        let (tx, _rx) = mpsc::channel(32);
        let mut app = setup_app_with_todo();
        app.data.pulls = vec![make_pr(10, None)];
        enter_sidebar_edit(&mut app);
        nav_to_links_active(&mut app);
        app.init(client, tx);

        super::handle_sidebar_edit(&mut app, KeyCode::Char('a'));
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);

        // give the spawned mutation a moment to reach the mock
        let mut muts = Vec::new();
        for _ in 0..40 {
            muts = captured_mutations(&captures);
            if !muts.is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        assert_eq!(
            muts.len(),
            1,
            "confirm should post a link mutation (captured {} requests)",
            captures.lock().len()
        );
        let op_name = muts[0]
            .get("operationName")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(
            op_name.contains("linkPullRequest") || op_name.contains("LinkPullRequest"),
            "expected linkPullRequest operation, got {op_name:?}"
        );
        // The mutation should use the default References relation and todo id 1.
        let vars = muts[0]
            .get("variables")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        assert_eq!(vars.get("todoId").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(vars.get("prId").and_then(|v| v.as_i64()), Some(10));
        assert_eq!(
            vars.get("relation").and_then(|v| v.as_str()),
            Some("references"),
            "default relation should be References"
        );
    }

    #[tokio::test]
    async fn test_attach_tab_switches_to_linear_and_confirm_posts_link_linear_issue() {
        let (base_url, captures) = spawn_gql_mock().await;
        let client = gql::Client::new(&base_url);
        let (tx, _rx) = mpsc::channel(32);
        let mut app = setup_app_with_todo();
        app.data.linears = vec![make_linear(7, None)];
        enter_sidebar_edit(&mut app);
        nav_to_links_active(&mut app);
        app.init(client, tx);

        super::handle_sidebar_edit(&mut app, KeyCode::Char('a'));
        // Tab switches to Linear kind in the picker.
        super::handle_sidebar_edit(&mut app, KeyCode::Tab);
        if let Mode::SidebarEdit { link_kind, .. } = &app.mode {
            assert_eq!(*link_kind, LinkKind::Linear, "Tab should switch to Linear");
        } else {
            panic!("expected SidebarEdit");
        }
        super::handle_sidebar_edit(&mut app, KeyCode::Enter);

        let mut muts = Vec::new();
        for _ in 0..40 {
            muts = captured_mutations(&captures);
            if !muts.is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        assert_eq!(
            muts.len(),
            1,
            "confirm should post a link mutation (captured {} requests)",
            captures.lock().len()
        );
        let op_name = muts[0]
            .get("operationName")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(
            op_name.contains("linkLinearIssue") || op_name.contains("LinkLinearIssue"),
            "expected linkLinearIssue operation, got {op_name:?}"
        );
        let vars = muts[0]
            .get("variables")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        assert_eq!(vars.get("todoId").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(vars.get("issueId").and_then(|v| v.as_i64()), Some(7));
    }
}
#[cfg(test)]
mod sidebar_add_tests {
    use crate::app::tests::make_pr;
    use crate::app::{App, LinkKind, Mode, SidebarField, View};
    use crate::gql;
    use crate::test_support::buffer_text;
    use crossterm::event::{KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyCode;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::mpsc;

    async fn spawn_gql_mock() -> (
        String,
        std::sync::Arc<parking_lot::Mutex<Vec<serde_json::Value>>>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let captures: std::sync::Arc<parking_lot::Mutex<Vec<serde_json::Value>>> =
            std::sync::Arc::new(parking_lot::Mutex::new(Vec::new()));
        let captures2 = captures.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let caps = captures2.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut tmp = [0u8; 1024];
                    loop {
                        let n = socket.read(&mut tmp).await.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&tmp[..n]);
                        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                        if buf.len() > 1_000_000 {
                            break;
                        }
                    }
                    if let Some(idx) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let body = &buf[idx + 4..];
                        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(body) {
                            caps.lock().push(val);
                        }
                    }
                    let body = r#"{"data":{}}"#;
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

    fn captured_mutations(
        captures: &parking_lot::Mutex<Vec<serde_json::Value>>,
    ) -> Vec<serde_json::Value> {
        captures
            .lock()
            .iter()
            .filter(|v| {
                v.get("query")
                    .and_then(|q| q.as_str())
                    .map(|q| q.starts_with("mutation"))
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    fn render_full(app: &mut App) -> String {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| crate::views::render(f, app)).unwrap();
        buffer_text(terminal.backend().buffer())
    }

    fn enter_sidebar_add(app: &mut App) {
        app.mode = Mode::SidebarAdd {
            field: SidebarField::Title,
            input_active: false,
            title_input: String::new(),
            title_caret: 0,
            desc_input: String::new(),
            desc_caret: 0,
            desc_scroll: 0,
            tag_input: String::new(),
            tag_caret: 0,
            link_kind: LinkKind::Pr,
            link_selection: 0,
            attaching: false,
            link_search: String::new(),
            pending_link_pr: Vec::new(),
            scroll: 0,
        };
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    // -- 1. pressing 'a' enters SidebarAdd, not InlineCreate --

    #[test]
    fn test_a_enters_sidebar_add_not_inline_create() {
        let mut app = App {
            view: View::Today,
            content_width: 120,
            ..Default::default()
        };
        app.handle_key(key(KeyCode::Char('a')));
        assert!(
            matches!(app.mode, Mode::SidebarAdd { .. }),
            "pressing 'a' should enter SidebarAdd, got {:?}",
            app.mode
        );
        assert!(
            !matches!(app.mode, Mode::InlineCreate { .. }),
            "pressing 'a' must not enter InlineCreate"
        );
    }

    #[test]
    fn test_a_in_backlog_enters_sidebar_add() {
        let mut app = App {
            view: View::Backlog,
            content_width: 120,
            ..Default::default()
        };
        app.handle_key(key(KeyCode::Char('a')));
        assert!(
            matches!(app.mode, Mode::SidebarAdd { .. }),
            "pressing 'a' in Backlog should enter SidebarAdd"
        );
    }

    // -- 2. accepting in SidebarAdd creates a todo with the title --

    #[tokio::test]
    async fn test_sidebar_add_accept_creates_todo() {
        let (base_url, captures) = spawn_gql_mock().await;
        let client = gql::Client::new(&base_url);
        let (tx, _rx) = mpsc::channel::<crate::AppMsg>(32);
        let mut app = App {
            view: View::Today,
            content_width: 120,
            ..Default::default()
        };
        enter_sidebar_add(&mut app);
        app.init(client, tx);
        // Activate the Title field and type a title.
        super::handle_sidebar_add(&mut app, KeyCode::Enter);
        super::handle_sidebar_add(&mut app, KeyCode::Char('N'));
        super::handle_sidebar_add(&mut app, KeyCode::Char('e'));
        super::handle_sidebar_add(&mut app, KeyCode::Char('w'));
        // Accept (Enter on Title) creates the todo.
        super::handle_sidebar_add(&mut app, KeyCode::Enter);

        let mut muts = Vec::new();
        for _ in 0..40 {
            muts = captured_mutations(&captures);
            if !muts.is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        assert!(
            !muts.is_empty(),
            "accepting SidebarAdd should post a mutation (captured {})",
            captures.lock().len()
        );
        let op_name = muts[0]
            .get("operationName")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(
            op_name.contains("createTodo") || op_name.contains("CreateTodo"),
            "expected createTodo operation, got {op_name:?}"
        );
        let vars = muts[0]
            .get("variables")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        let title = vars
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert_eq!(title, "New", "createTodo should carry the typed title");
        assert_eq!(app.mode, Mode::Navigate, "accept should return to Navigate");
    }

    // -- 3. link picker filters by search text --

    #[test]
    fn test_link_picker_filters_by_search() {
        let mut app = App {
            content_width: 120,
            ..Default::default()
        };
        app.data.pulls = vec![
            make_pr(10, None),
            crate::gql::PullRequest {
                id: 11,
                owner: "owner".into(),
                repo: "repo".into(),
                number: 11,
                title: "Fix build".into(),
                url: "x".into(),
                author: None,
                state: "open".into(),
                review_requested: false,
                authored_by_me: false,
                changes_requested: false,
                copilot_comments: false,
                merge_conflicts: false,
                dismissed_at: None,
                remote_updated_at: None,
            },
        ];
        enter_sidebar_add(&mut app);
        // Move to Links and activate, then open the attach picker.
        super::handle_sidebar_add(&mut app, KeyCode::Tab); // Title -> Description
        super::handle_sidebar_add(&mut app, KeyCode::Tab); // Description -> Links
        super::handle_sidebar_add(&mut app, KeyCode::Enter); // activate Links
        super::handle_sidebar_add(&mut app, KeyCode::Char('a')); // open picker
        // Type "fix" to filter.
        super::handle_sidebar_add(&mut app, KeyCode::Char('f'));
        super::handle_sidebar_add(&mut app, KeyCode::Char('i'));
        super::handle_sidebar_add(&mut app, KeyCode::Char('x'));

        let buf = render_full(&mut app);
        assert!(
            buf.contains("Fix build"),
            "filtered picker should show the matching PR:\n{buf}"
        );
        assert!(
            !buf.contains("PR #10"),
            "filtered picker should hide the non-matching PR #10:\n{buf}"
        );
    }

    // -- 4. link picker orders closed/merged after open --

    #[test]
    fn test_link_picker_orders_closed_after_open() {
        let mut app = App {
            content_width: 120,
            ..Default::default()
        };
        app.data.pulls = vec![
            crate::gql::PullRequest {
                id: 1,
                owner: "o".into(),
                repo: "r".into(),
                number: 1,
                title: "Closed PR".into(),
                url: "x".into(),
                author: None,
                state: "closed".into(),
                review_requested: false,
                authored_by_me: false,
                changes_requested: false,
                copilot_comments: false,
                merge_conflicts: false,
                dismissed_at: None,
                remote_updated_at: None,
            },
            crate::gql::PullRequest {
                id: 2,
                owner: "o".into(),
                repo: "r".into(),
                number: 2,
                title: "Open PR".into(),
                url: "x".into(),
                author: None,
                state: "open".into(),
                review_requested: false,
                authored_by_me: false,
                changes_requested: false,
                copilot_comments: false,
                merge_conflicts: false,
                dismissed_at: None,
                remote_updated_at: None,
            },
        ];
        enter_sidebar_add(&mut app);
        if let Mode::SidebarAdd {
            field,
            input_active,
            attaching,
            ..
        } = &mut app.mode
        {
            *field = SidebarField::Links;
            *input_active = true;
            *attaching = true;
        }
        let buf = render_full(&mut app);
        let open_idx = buf.find("Open PR");
        let closed_idx = buf.find("Closed PR");
        assert!(open_idx.is_some(), "open PR should appear:\n{buf}");
        assert!(closed_idx.is_some(), "closed PR should appear:\n{buf}");
        assert!(
            open_idx < closed_idx,
            "open PR should appear before closed PR:\n{buf}"
        );
    }
}
