//! Centralized key dispatch.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
            KeyCode::Char('p')
                if key.modifiers.contains(KeyModifiers::CONTROL) && self.mode == Mode::Navigate =>
            {
                self.mode = Mode::Command {
                    input: String::new(),
                    selection: 0,
                    caret: 0,
                };
                return false;
            }
            KeyCode::Char('k')
                if key.modifiers.contains(KeyModifiers::CONTROL) && self.mode == Mode::Navigate =>
            {
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
        let (input_clone, selection_val) = match &self.mode {
            Mode::Command {
                input, selection, ..
            } => (input.clone(), *selection),
            _ => unreachable!(),
        };
        if let Mode::Command { input, caret, .. } = &mut self.mode {
            if edit::edit_chord(key.code, key.modifiers, input, caret) {
                return;
            }
        }
        use crossterm::event::KeyCode::*;
        match key.code {
            Char('j') | Down => {
                let filtered = filter_commands(self, &input_clone);
                if !filtered.is_empty() {
                    let new_sel = (selection_val + 1) % filtered.len();
                    if let Mode::Command { selection, .. } = &mut self.mode {
                        *selection = new_sel;
                    }
                }
            }
            Char('k') | Up => {
                let filtered = filter_commands(self, &input_clone);
                if !filtered.is_empty() {
                    let new_sel = selection_val.checked_sub(1).unwrap_or(filtered.len() - 1);
                    if let Mode::Command { selection, .. } = &mut self.mode {
                        *selection = new_sel;
                    }
                }
            }
            Tab => {
                let filtered = filter_commands(self, &input_clone);
                if !filtered.is_empty() {
                    let new_sel = (selection_val + 1) % filtered.len();
                    if let Mode::Command { selection, .. } = &mut self.mode {
                        *selection = new_sel;
                    }
                }
            }
            Enter => {
                let filtered = filter_commands(self, &input_clone);
                if let Some(cmd) = filtered.get(selection_val) {
                    let action = cmd.action;
                    self.mode = Mode::Navigate;
                    action(self);
                } else {
                    self.mode = Mode::Navigate;
                }
            }
            Char(c) if c.is_alphanumeric() || c == ' ' => {
                if let Mode::Command {
                    input,
                    caret,
                    selection,
                    ..
                } = &mut self.mode
                {
                    *caret = edit::insert_char(input, *caret, c);
                    *selection = 0;
                }
            }
            Backspace => {
                if let Mode::Command {
                    input,
                    caret,
                    selection,
                    ..
                } = &mut self.mode
                {
                    *caret = edit::backspace(input, *caret);
                    *selection = 0;
                }
            }
            Left => {
                if let Mode::Command { caret, input, .. } = &mut self.mode {
                    *caret = edit::move_caret(input, *caret, -1);
                }
            }
            Right => {
                if let Mode::Command { caret, input, .. } = &mut self.mode {
                    *caret = edit::move_caret(input, *caret, 1);
                }
            }
            Home => {
                if let Mode::Command { caret, .. } = &mut self.mode {
                    *caret = 0;
                }
            }
            End => {
                if let Mode::Command { caret, input, .. } = &mut self.mode {
                    *caret = input.len();
                }
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
    pub(crate) label: String,
    action: fn(&mut App),
}

type StaticCommand = (&'static str, fn(&mut App));

static COMMANDS: &[StaticCommand] = &[
    ("today", |app| app.switch_view(crate::app::View::Today)),
    ("backlog", |app| app.switch_view(crate::app::View::Backlog)),
    ("inbox", |app| app.switch_view(crate::app::View::Inbox)),
    ("review", |app| app.switch_view(crate::app::View::Review)),
    ("sync", |app| app.switch_view(crate::app::View::Sync)),
    ("sync github", |app| app.spawn_sync_github()),
    ("sync linear", |app| app.spawn_sync_linear()),
    ("carry over", |app| {
        let from = (app.logical_date - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        let to = app.logical_date.format("%Y-%m-%d").to_string();
        app.mode = Mode::Confirm {
            action: crate::app::ConfirmAction::CarryOver { from, to },
        };
    }),
    ("refresh", |app| app.spawn_refresh()),
    ("toggle done", |app| app.show_done = !app.show_done),
    ("toggle dismissed", |app| {
        app.show_dismissed = !app.show_dismissed
    }),
    ("toggle closed", |app| app.show_closed = !app.show_closed),
    ("help", |app| {
        app.mode = Mode::Help {
            filter: String::new(),
        };
        app.help_scroll = 0;
    }),
    ("quit", |app| app.quit_requested = true),
];

fn static_commands() -> Vec<Command> {
    COMMANDS
        .iter()
        .map(|(label, action)| Command {
            label: (*label).to_string(),
            action: *action,
        })
        .collect()
}

fn cursor_todo_info(app: &App) -> Option<(i32, String, Option<String>, String)> {
    match app.view {
        crate::app::View::Today => app.today_plan().get(app.cursor).map(|t| {
            (
                t.id,
                t.title.clone(),
                t.description.clone(),
                t.status.clone(),
            )
        }),
        crate::app::View::Backlog => app.backlog().get(app.cursor).map(|t| {
            (
                t.id,
                t.title.clone(),
                t.description.clone(),
                t.status.clone(),
            )
        }),
        _ => None,
    }
}

fn cursor_commands(app: &App) -> Vec<Command> {
    if cursor_todo_info(app).is_none() {
        return Vec::new();
    }
    let is_today = app.view == crate::app::View::Today;
    let mut cmds = Vec::new();

    cmds.push(Command {
        label: "set status".into(),
        action: |app| {
            if let Some((id, _, _, status)) = cursor_todo_info(app) {
                let selection = crate::app::status_index(&status).unwrap_or(0);
                app.mode = Mode::StatusSelect {
                    id,
                    selection,
                    reason: None,
                };
            }
        },
    });

    if is_today {
        cmds.push(Command {
            label: "unplan for today".into(),
            action: |app| {
                if let Some((id, _, _, _)) = cursor_todo_info(app) {
                    app.mode = Mode::Confirm {
                        action: crate::app::ConfirmAction::Unplan { id },
                    };
                }
            },
        });
    }

    cmds.push(Command {
        label: "cancel todo".into(),
        action: |app| {
            if let Some((id, _, _, _)) = cursor_todo_info(app) {
                app.mode = Mode::Confirm {
                    action: crate::app::ConfirmAction::Cancel { id },
                };
            }
        },
    });

    cmds.push(Command {
        label: "edit title".into(),
        action: |app| {
            if let Some((id, title, _, _)) = cursor_todo_info(app) {
                let caret = title.len();
                app.mode = Mode::InlineEdit {
                    id,
                    input: title,
                    caret,
                };
            }
        },
    });

    cmds.push(Command {
        label: "open sidebar".into(),
        action: |app| {
            if let Some((id, title, desc, _)) = cursor_todo_info(app) {
                if app.content_width < 100 {
                    app.set_error("terminal too narrow for sidebar");
                } else {
                    let title_caret = title.len();
                    let desc = desc.unwrap_or_default();
                    app.mode = Mode::SidebarEdit {
                        id,
                        field: SidebarField::Description,
                        input_active: false,
                        title_input: title,
                        title_caret,
                        desc_input: desc,
                        desc_caret: 0,
                        desc_scroll: 0,
                        tag_input: String::new(),
                        tag_caret: 0,
                        link_kind: crate::app::LinkKind::Pr,
                        link_selection: 0,
                        attaching: false,
                        link_search: String::new(),
                        scroll: 0,
                    };
                    app.spawn_fetch_detail(id);
                }
            }
        },
    });

    cmds
}

pub(crate) fn filter_commands(app: &App, input: &str) -> Vec<Command> {
    let mut all = static_commands();
    all.extend(cursor_commands(app));
    if input.is_empty() {
        return all;
    }
    let needle = input.to_lowercase();
    all.into_iter()
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
        let app = App::default();
        let filtered = filter_commands(&app, "sync");
        assert!(filtered.iter().any(|c| c.label == "sync github"));
        assert!(filtered.iter().any(|c| c.label == "sync linear"));
    }

    #[test]
    fn test_command_palette_shows_todo_commands_on_today() {
        let mut app = App {
            view: View::Today,
            content_width: 120,
            ..Default::default()
        };
        app.data.plan.push(crate::app::tests::make_plan_row(
            1,
            0,
            crate::app::tests::make_todo(10, "write docs", "todo"),
            None,
        ));
        let filtered = filter_commands(&app, "");
        assert!(
            filtered.iter().any(|c| c.label == "set status"),
            "todo commands should appear when cursor is on a todo"
        );
        assert!(
            filtered.iter().any(|c| c.label == "edit title"),
            "edit title command should appear"
        );
        assert!(
            filtered.iter().any(|c| c.label == "open sidebar"),
            "open sidebar command should appear"
        );
        assert!(
            filtered.iter().any(|c| c.label == "cancel todo"),
            "cancel todo command should appear"
        );
        assert!(
            filtered.iter().any(|c| c.label == "unplan for today"),
            "unplan command should appear on Today view"
        );
    }

    #[test]
    fn test_command_palette_shows_todo_commands_on_backlog() {
        let mut app = App {
            view: View::Backlog,
            content_width: 120,
            ..Default::default()
        };
        app.data
            .todos
            .push(crate::app::tests::make_todo(10, "write docs", "todo"));
        let filtered = filter_commands(&app, "");
        assert!(filtered.iter().any(|c| c.label == "set status"));
        assert!(filtered.iter().any(|c| c.label == "edit title"));
        assert!(filtered.iter().any(|c| c.label == "open sidebar"));
        assert!(filtered.iter().any(|c| c.label == "cancel todo"));
        assert!(
            !filtered.iter().any(|c| c.label == "unplan for today"),
            "unplan command should not appear on Backlog view"
        );
    }

    #[test]
    fn test_command_palette_no_todo_commands_without_cursor_todo() {
        let app = App::default();
        let filtered = filter_commands(&app, "");
        assert!(
            !filtered.iter().any(|c| c.label == "set status"),
            "todo commands should not appear without a cursor todo"
        );
        assert!(!filtered.iter().any(|c| c.label == "edit title"));
        assert!(!filtered.iter().any(|c| c.label == "open sidebar"));
        assert!(!filtered.iter().any(|c| c.label == "cancel todo"));
    }

    #[test]
    fn test_command_palette_no_todo_commands_in_inbox_view() {
        let app = App {
            view: View::Inbox,
            ..Default::default()
        };
        let filtered = filter_commands(&app, "");
        assert!(
            !filtered.iter().any(|c| c.label == "set status"),
            "todo commands should not appear in inbox view"
        );
        assert!(!filtered.iter().any(|c| c.label == "edit title"));
    }

    #[test]
    fn test_command_palette_executes_set_status() {
        let mut app = App {
            view: View::Today,
            content_width: 120,
            mode: Mode::Command {
                input: "set status".into(),
                selection: 0,
                caret: 9,
            },
            ..Default::default()
        };
        app.data.plan.push(crate::app::tests::make_plan_row(
            1,
            0,
            crate::app::tests::make_todo(10, "write docs", "todo"),
            None,
        ));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(app.mode, Mode::StatusSelect { id: 10, .. }));
    }

    #[test]
    fn test_command_palette_executes_edit_title() {
        let mut app = App {
            view: View::Today,
            content_width: 120,
            mode: Mode::Command {
                input: "edit title".into(),
                selection: 0,
                caret: 10,
            },
            ..Default::default()
        };
        app.data.plan.push(crate::app::tests::make_plan_row(
            1,
            0,
            crate::app::tests::make_todo(10, "write docs", "todo"),
            None,
        ));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match &app.mode {
            Mode::InlineEdit { id, input, .. } => {
                assert_eq!(*id, 10);
                assert_eq!(input, "write docs");
            }
            _ => panic!("expected InlineEdit mode"),
        }
    }

    #[test]
    fn test_command_palette_executes_unplan() {
        let mut app = App {
            view: View::Today,
            content_width: 120,
            mode: Mode::Command {
                input: "unplan".into(),
                selection: 0,
                caret: 6,
            },
            ..Default::default()
        };
        app.data.plan.push(crate::app::tests::make_plan_row(
            1,
            0,
            crate::app::tests::make_todo(10, "write docs", "todo"),
            None,
        ));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match &app.mode {
            Mode::Confirm {
                action: crate::app::ConfirmAction::Unplan { id },
            } => {
                assert_eq!(*id, 10);
            }
            _ => panic!("expected Confirm mode with Unplan action"),
        }
    }

    #[test]
    fn test_command_palette_executes_cancel_todo() {
        let mut app = App {
            view: View::Today,
            content_width: 120,
            mode: Mode::Command {
                input: "cancel".into(),
                selection: 0,
                caret: 6,
            },
            ..Default::default()
        };
        app.data.plan.push(crate::app::tests::make_plan_row(
            1,
            0,
            crate::app::tests::make_todo(10, "write docs", "todo"),
            None,
        ));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match &app.mode {
            Mode::Confirm {
                action: crate::app::ConfirmAction::Cancel { id },
            } => {
                assert_eq!(*id, 10);
            }
            _ => panic!("expected Confirm mode with Cancel action"),
        }
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

    #[test]
    fn test_ctrl_p_enters_command_mode() {
        let mut app = App::default();
        app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
        assert!(matches!(app.mode, Mode::Command { .. }));
    }

    #[test]
    fn test_ctrl_k_enters_command_mode() {
        let mut app = App::default();
        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
        assert!(matches!(app.mode, Mode::Command { .. }));
    }

    #[test]
    fn test_filter_single_result_still_matchable() {
        let app = App::default();
        let filtered = filter_commands(&app, "backlog");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].label, "backlog");
    }

    #[test]
    fn test_filter_bac_matches_backlog() {
        let app = App::default();
        let filtered = filter_commands(&app, "bac");
        assert!(
            filtered.iter().any(|c| c.label == "backlog"),
            "partial substring 'bac' must match 'backlog'"
        );
    }
}
