//! Inline create, inline edit, search, and reorder handlers.
//!
//! These are the lightweight single-line text inputs and the J/K reorder
//! handler. Text editing uses the shared primitives from `app::edit`.
use crate::app::edit;
use crate::app::{App, Mode};

pub(crate) fn handle_inline_create(app: &mut App, key: ratatui::crossterm::event::KeyCode) {
    use ratatui::crossterm::event::KeyCode::*;
    match key {
        Char(c) if c.is_alphanumeric() || c == ' ' || c == '#' || c == '-' || c == '_' => {
            let Mode::InlineCreate { input, caret } = &mut app.mode else {
                return;
            };
            *caret = edit::insert_char(input, *caret, c);
        }
        Backspace => {
            let Mode::InlineCreate { input, caret } = &mut app.mode else {
                return;
            };
            *caret = edit::backspace(input, *caret);
        }
        Left => {
            let Mode::InlineCreate { input, caret } = &mut app.mode else {
                return;
            };
            *caret = edit::move_caret(input, *caret, -1);
        }
        Right => {
            let Mode::InlineCreate { input, caret } = &mut app.mode else {
                return;
            };
            *caret = edit::move_caret(input, *caret, 1);
        }
        Home => {
            let Mode::InlineCreate { caret, .. } = &mut app.mode else {
                return;
            };
            *caret = 0;
        }
        End => {
            let Mode::InlineCreate { input, caret } = &mut app.mode else {
                return;
            };
            *caret = input.len();
        }
        Enter => {
            let Mode::InlineCreate { input, .. } = &app.mode else {
                return;
            };
            let title = input.trim().to_string();
            if title.is_empty() {
                app.set_error("a title is required");
                return;
            }
            let plan_after = crate::app::should_plan_after_create(app.view);
            app.spawn_create_todo(&title, plan_after);
            app.mode = Mode::Navigate;
        }
        _ => {}
    }
}

pub(crate) fn handle_inline_edit(app: &mut App, key: ratatui::crossterm::event::KeyCode) {
    use ratatui::crossterm::event::KeyCode::*;
    match key {
        Char(c) if c.is_alphanumeric() || c == ' ' || c == '#' || c == '-' || c == '_' => {
            if let Mode::InlineEdit { input, caret, .. } = &mut app.mode {
                *caret = edit::insert_char(input, *caret, c);
            }
        }
        Backspace => {
            if let Mode::InlineEdit { input, caret, .. } = &mut app.mode {
                *caret = edit::backspace(input, *caret);
            }
        }
        Left => {
            if let Mode::InlineEdit { input, caret, .. } = &mut app.mode {
                *caret = edit::move_caret(input, *caret, -1);
            }
        }
        Right => {
            if let Mode::InlineEdit { input, caret, .. } = &mut app.mode {
                *caret = edit::move_caret(input, *caret, 1);
            }
        }
        Home => {
            if let Mode::InlineEdit { caret, .. } = &mut app.mode {
                *caret = 0;
            }
        }
        End => {
            if let Mode::InlineEdit { input, caret, .. } = &mut app.mode {
                *caret = input.len();
            }
        }
        Enter => {
            let (id, title) = if let Mode::InlineEdit { id, input, .. } = &app.mode {
                (*id, input.trim().to_string())
            } else {
                return;
            };
            if title.is_empty() {
                app.set_error("a title is required");
                return;
            }
            app.spawn_update_todo(id, Some(&title), None);
            app.mode = Mode::Navigate;
        }
        _ => {}
    }
}

pub(crate) fn handle_search(app: &mut App, key: ratatui::crossterm::event::KeyCode) {
    use ratatui::crossterm::event::KeyCode::*;
    match key {
        Char(c) if c.is_alphanumeric() || c == ' ' || c == '#' || c == '-' || c == '_' => {
            if let Mode::Search { input, caret } = &mut app.mode {
                *caret = edit::insert_char(input, *caret, c);
            }
        }
        Backspace => {
            if let Mode::Search { input, caret } = &mut app.mode {
                *caret = edit::backspace(input, *caret);
            }
        }
        Left => {
            if let Mode::Search { input, caret } = &mut app.mode {
                *caret = edit::move_caret(input, *caret, -1);
            }
        }
        Right => {
            if let Mode::Search { input, caret } = &mut app.mode {
                *caret = edit::move_caret(input, *caret, 1);
            }
        }
        Home => {
            if let Mode::Search { caret, .. } = &mut app.mode {
                *caret = 0;
            }
        }
        End => {
            if let Mode::Search { input, caret } = &mut app.mode {
                *caret = input.len();
            }
        }
        _ => {}
    }
}

pub(crate) fn handle_reorder(app: &mut App, key: ratatui::crossterm::event::KeyCode) {
    use ratatui::crossterm::event::KeyCode::*;
    let source_id = match &app.mode {
        Mode::Reorder { source_id } => *source_id,
        _ => return,
    };
    match key {
        Char('J') | Down => {
            let mut ids: Vec<i32> = app
                .data
                .plan
                .iter()
                .filter(|p| p.removed_at.is_none())
                .filter_map(|p| p.todo.as_ref().map(|t| t.id))
                .collect();
            if let Some(pos) = ids.iter().position(|&i| i == source_id) {
                if pos + 1 < ids.len() {
                    ids.swap(pos, pos + 1);
                }
            }
            let date = app.logical_date.format("%Y-%m-%d").to_string();
            app.spawn_reorder(&date, &ids);
        }
        Char('K') | Up => {
            let mut ids: Vec<i32> = app
                .data
                .plan
                .iter()
                .filter(|p| p.removed_at.is_none())
                .filter_map(|p| p.todo.as_ref().map(|t| t.id))
                .collect();
            if let Some(pos) = ids.iter().position(|&i| i == source_id) {
                if pos > 0 {
                    ids.swap(pos, pos - 1);
                }
            }
            let date = app.logical_date.format("%Y-%m-%d").to_string();
            app.spawn_reorder(&date, &ids);
        }
        Enter => {
            app.mode = Mode::Navigate;
        }
        _ => {}
    }
}
