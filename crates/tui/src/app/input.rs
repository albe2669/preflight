//! Centralized key dispatch.

use crossterm::event::{KeyCode, KeyEvent};

use crate::app::edit;
use crate::app::{App, Mode, SidebarField};
impl App {
    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') if self.mode == Mode::Navigate => return true,
            KeyCode::Tab if self.mode == Mode::Navigate => {
                self.next_view();
                return false;
            }
            KeyCode::BackTab if self.mode == Mode::Navigate => {
                self.prev_view();
                return false;
            }
            KeyCode::Char('?') if self.mode == Mode::Navigate => {
                self.mode = Mode::Help {
                    filter: String::new(),
                };
                self.help_scroll = 0;
                return false;
            }
            KeyCode::Esc => match self.mode {
                Mode::Navigate => {
                    self.toast = None;
                    return false;
                }
                Mode::SidebarEdit { .. } | Mode::SidebarAdd { .. } => {}
                _ => {
                    self.mode = Mode::Navigate;
                    self.toast = None;
                    return false;
                }
            },
            _ => {}
        }

        match self.mode.clone() {
            Mode::Navigate => self.key_navigate(key.code),
            Mode::InlineCreate { .. } => self.key_inline_create(key),
            Mode::InlineEdit { .. } => self.key_inline_edit(key),
            Mode::Search { .. } => self.key_search(key),
            Mode::Reorder { source_id } => self.key_reorder(key.code, source_id),
            Mode::Confirm { action } => self.key_confirm(key.code, &action),
            Mode::Help { filter } => self.key_help(key.code, &filter),
            Mode::StatusSelect {
                id,
                selection,
                reason,
            } => self.key_status_select(key.code, id, selection, reason),
            Mode::LinkTodo { .. } => self.key_link_todo(key.code),
            Mode::SidebarEdit { .. } => self.key_sidebar_edit(key),
            Mode::SidebarAdd { .. } => self.key_sidebar_add(key),
        }
        false
    }

    fn key_navigate(&mut self, key: KeyCode) {
        match self.view {
            crate::app::View::Today => crate::views::today::handle_navigate(self, key),
            crate::app::View::Backlog => crate::views::backlog::handle_navigate(self, key),
            crate::app::View::Inbox => crate::views::inbox::handle_navigate(self, key),
            crate::app::View::Review => crate::views::review::handle_navigate(self, key),
            crate::app::View::Sync => crate::views::sync::handle_navigate(self, key),
        }
    }

    fn key_inline_create(&mut self, key: KeyEvent) {
        if let Mode::InlineCreate { input, caret } = &mut self.mode {
            if edit::edit_chord(key.code, key.modifiers, input, caret) {
                return;
            }
        }
        crate::views::inline::handle_inline_create(self, key.code);
    }

    fn key_inline_edit(&mut self, key: KeyEvent) {
        if let Mode::InlineEdit { input, caret, .. } = &mut self.mode {
            if edit::edit_chord(key.code, key.modifiers, input, caret) {
                return;
            }
        }
        crate::views::inline::handle_inline_edit(self, key.code);
    }

    fn key_search(&mut self, key: KeyEvent) {
        if let Mode::Search { input, caret } = &mut self.mode {
            if edit::edit_chord(key.code, key.modifiers, input, caret) {
                return;
            }
        }
        crate::views::inline::handle_search(self, key.code);
    }

    fn key_reorder(&mut self, key: KeyCode, _source_id: i32) {
        crate::views::inline::handle_reorder(self, key);
    }

    fn key_confirm(&mut self, key: KeyCode, action: &crate::app::ConfirmAction) {
        crate::views::overlays::handle_confirm(self, key, action);
    }

    fn key_help(&mut self, key: KeyCode, filter: &str) {
        crate::views::handle_help(self, key, filter);
    }

    fn key_status_select(
        &mut self,
        key: KeyCode,
        _id: i32,
        _selection: usize,
        _reason: Option<String>,
    ) {
        crate::views::overlays::handle_status_select(self, key);
    }

    fn key_link_todo(&mut self, key: KeyCode) {
        crate::views::overlays::handle_link_todo(self, key);
    }

    fn key_sidebar_edit(&mut self, key: KeyEvent) {
        if let Mode::SidebarEdit {
            input_active: true,
            field,
            ..
        } = &self.mode
        {
            if !matches!(field, SidebarField::Links) {
                let (buf, caret) = sidebar_edit_active_buf_caret_mut(self);
                if edit::edit_chord(key.code, key.modifiers, buf, caret) {
                    return;
                }
            }
        }
        crate::views::sidebar::handle_sidebar_edit(self, key.code);
    }

    fn key_sidebar_add(&mut self, key: KeyEvent) {
        if let Mode::SidebarAdd {
            input_active: true,
            field,
            ..
        } = &self.mode
        {
            if !matches!(field, SidebarField::Links) {
                let (buf, caret) = sidebar_add_active_buf_caret_mut(self);
                if edit::edit_chord(key.code, key.modifiers, buf, caret) {
                    return;
                }
            }
        }
        crate::views::sidebar::handle_sidebar_add(self, key.code);
    }
}

fn sidebar_edit_active_buf_caret_mut(app: &mut App) -> (&mut String, &mut usize) {
    match &mut app.mode {
        Mode::SidebarEdit {
            field,
            title_input,
            title_caret,
            desc_input,
            desc_caret,
            tag_input,
            tag_caret,
            ..
        } => match field {
            SidebarField::Title => (title_input, title_caret),
            SidebarField::Description => (desc_input, desc_caret),
            SidebarField::Tags => (tag_input, tag_caret),
            SidebarField::Links => unreachable!(),
        },
        _ => unreachable!(),
    }
}

fn sidebar_add_active_buf_caret_mut(app: &mut App) -> (&mut String, &mut usize) {
    match &mut app.mode {
        Mode::SidebarAdd {
            field,
            title_input,
            title_caret,
            desc_input,
            desc_caret,
            tag_input,
            tag_caret,
            ..
        } => match field {
            SidebarField::Title => (title_input, title_caret),
            SidebarField::Description => (desc_input, desc_caret),
            SidebarField::Tags => (tag_input, tag_caret),
            SidebarField::Links => unreachable!(),
        },
        _ => unreachable!(),
    }
}
