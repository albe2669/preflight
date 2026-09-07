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
            KeyCode::Char(':') if self.mode == Mode::Navigate => {
                self.mode = Mode::Command {
                    input: String::new(),
                    selection: 0,
                    caret: 0,
                };
                return false;
            }
            KeyCode::Esc => match &self.mode {
                Mode::Navigate => {
                    self.toast = None;
                    return false;
                }
                Mode::SidebarEdit { .. } | Mode::SidebarAdd { .. } => {}
                Mode::Filter { input, .. } => {
                    if !input.is_empty() {
                        self.filter = String::new();
                    }
                    self.mode = Mode::Navigate;
                    self.toast = None;
                    return false;
                }
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
            Mode::Filter { .. } => self.key_filter(key),
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
            Mode::Command { .. } => self.key_command(key),
        }
        if self.quit_requested {
            return true;
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

    fn key_filter(&mut self, key: KeyEvent) {
        if let Mode::Filter { input, caret } = &mut self.mode {
            if edit::edit_chord(key.code, key.modifiers, input, caret) {
                return;
            }
        }
        use crossterm::event::KeyCode::*;
        match key.code {
            Char(c) if c.is_alphanumeric() || c == ' ' || c == '#' || c == '-' || c == '_' => {
                if let Mode::Filter { input, caret } = &mut self.mode {
                    *caret = edit::insert_char(input, *caret, c);
                }
            }
            Backspace => {
                if let Mode::Filter { input, caret } = &mut self.mode {
                    *caret = edit::backspace(input, *caret);
                }
            }
            Left => {
                if let Mode::Filter { input, caret } = &mut self.mode {
                    *caret = edit::move_caret(input, *caret, -1);
                }
            }
            Right => {
                if let Mode::Filter { input, caret } = &mut self.mode {
                    *caret = edit::move_caret(input, *caret, 1);
                }
            }
            Home => {
                if let Mode::Filter { caret, .. } = &mut self.mode {
                    *caret = 0;
                }
            }
            End => {
                if let Mode::Filter { input, caret } = &mut self.mode {
                    *caret = input.len();
                }
            }
            Enter => {
                if let Mode::Filter { input, .. } = &self.mode {
                    self.filter = input.clone();
                }
                self.mode = Mode::Navigate;
            }

            _ => {}
        }
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

    fn key_command(&mut self, key: KeyEvent) {
        let (input, selection, caret) = match &mut self.mode {
            Mode::Command {
                input,
                selection,
                caret,
            } => (input, selection, caret),
            _ => unreachable!(),
        };
        if edit::edit_chord(key.code, key.modifiers, input, caret) {
            return;
        }
        use crossterm::event::KeyCode::*;
        match key.code {
            Char('j') | Down => {
                let filtered = filter_commands(input);
                if !filtered.is_empty() {
                    *selection = (*selection + 1) % filtered.len();
                }
            }
            Char('k') | Up => {
                let filtered = filter_commands(input);
                if !filtered.is_empty() {
                    *selection = selection.checked_sub(1).unwrap_or(filtered.len() - 1);
                }
            }
            Tab => {
                let filtered = filter_commands(input);
                if !filtered.is_empty() {
                    *selection = (*selection + 1) % filtered.len();
                }
            }
            Enter => {
                let filtered = filter_commands(input);
                if let Some(cmd) = filtered.get(*selection) {
                    let action = cmd.action;
                    self.mode = Mode::Navigate;
                    action(self);
                } else {
                    self.mode = Mode::Navigate;
                }
            }
            Char(c) if c.is_alphanumeric() || c == ' ' => {
                *caret = edit::insert_char(input, *caret, c);
                *selection = 0;
            }
            Backspace => {
                *caret = edit::backspace(input, *caret);
                *selection = 0;
            }
            Left => {
                *caret = edit::move_caret(input, *caret, -1);
            }
            Right => {
                *caret = edit::move_caret(input, *caret, 1);
            }
            Home => {
                *caret = 0;
            }
            End => {
                *caret = input.len();
            }
            _ => {}
        }
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

pub(crate) struct Command {
    pub(crate) label: &'static str,
    action: fn(&mut App),
}

const COMMANDS: &[Command] = &[
    Command {
        label: "today",
        action: |app| {
            app.switch_view(crate::app::View::Today);
        },
    },
    Command {
        label: "backlog",
        action: |app| {
            app.switch_view(crate::app::View::Backlog);
        },
    },
    Command {
        label: "inbox",
        action: |app| {
            app.switch_view(crate::app::View::Inbox);
        },
    },
    Command {
        label: "review",
        action: |app| {
            app.switch_view(crate::app::View::Review);
        },
    },
    Command {
        label: "sync",
        action: |app| {
            app.switch_view(crate::app::View::Sync);
        },
    },
    Command {
        label: "sync github",
        action: |app| {
            app.spawn_sync_github();
        },
    },
    Command {
        label: "sync linear",
        action: |app| {
            app.spawn_sync_linear();
        },
    },
    Command {
        label: "carry over",
        action: |app| {
            let from = (app.logical_date - chrono::Duration::days(1))
                .format("%Y-%m-%d")
                .to_string();
            let to = app.logical_date.format("%Y-%m-%d").to_string();
            app.mode = Mode::Confirm {
                action: crate::app::ConfirmAction::CarryOver { from, to },
            };
        },
    },
    Command {
        label: "refresh",
        action: |app| {
            app.spawn_refresh();
        },
    },
    Command {
        label: "toggle done",
        action: |app| {
            app.show_done = !app.show_done;
        },
    },
    Command {
        label: "toggle dismissed",
        action: |app| {
            app.show_dismissed = !app.show_dismissed;
        },
    },
    Command {
        label: "help",
        action: |app| {
            app.mode = Mode::Help {
                filter: String::new(),
            };
            app.help_scroll = 0;
        },
    },
    Command {
        label: "quit",
        action: |app| {
            app.quit_requested = true;
        },
    },
];

pub(crate) fn filter_commands(input: &str) -> Vec<&'static Command> {
    if input.is_empty() {
        return COMMANDS.iter().collect();
    }
    let needle = input.to_lowercase();
    COMMANDS
        .iter()
        .filter(|c| c.label.to_lowercase().contains(&needle))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, Mode, View};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn test_colon_enters_command_mode() {
        let mut app = App::default();
        app.handle_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE));
        assert!(matches!(app.mode, Mode::Command { .. }));
    }

    #[test]
    fn test_command_palette_filters_by_substring() {
        let filtered = filter_commands("sync");
        assert!(filtered.iter().any(|c| c.label == "sync github"));
        assert!(filtered.iter().any(|c| c.label == "sync linear"));
    }

    #[test]
    fn test_command_enter_executes_selection() {
        let mut app = App {
            mode: Mode::Command {
                input: "today".into(),
                selection: 0,
                caret: 5,
            },
            ..Default::default()
        };
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.view, View::Today);
        assert_eq!(app.mode, Mode::Navigate);
    }

    #[test]
    fn test_command_esc_returns_to_navigate() {
        let mut app = App {
            mode: Mode::Command {
                input: "sync".into(),
                selection: 0,
                caret: 4,
            },
            ..Default::default()
        };
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.mode, Mode::Navigate);
    }

    #[test]
    fn test_command_j_moves_selection_down() {
        let mut app = App {
            mode: Mode::Command {
                input: String::new(),
                selection: 0,
                caret: 0,
            },
            ..Default::default()
        };
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        if let Mode::Command { selection, .. } = &app.mode {
            assert!(*selection > 0);
        } else {
            panic!("expected Command mode");
        }
    }

    #[test]
    fn test_command_k_moves_selection_up() {
        let mut app = App {
            mode: Mode::Command {
                input: String::new(),
                selection: 1,
                caret: 0,
            },
            ..Default::default()
        };
        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        if let Mode::Command { selection, .. } = &app.mode {
            assert_eq!(*selection, 0);
        } else {
            panic!("expected Command mode");
        }
    }

    #[test]
    fn test_slash_enters_filter_mode() {
        let mut app = App {
            view: View::Today,
            ..Default::default()
        };
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        assert!(matches!(app.mode, Mode::Filter { .. }));
    }

    #[test]
    fn test_filter_enter_commits_filter() {
        let mut app = App {
            mode: Mode::Filter {
                input: "hello".into(),
                caret: 5,
            },
            ..Default::default()
        };
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.mode, Mode::Navigate);
        assert_eq!(app.filter, "hello");
    }

    #[test]
    fn test_filter_esc_clears_filter() {
        let mut app = App {
            mode: Mode::Filter {
                input: "hello".into(),
                caret: 5,
            },
            ..Default::default()
        };
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.mode, Mode::Navigate);
        assert_eq!(app.filter, "");
    }
}
