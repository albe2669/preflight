//! View switching and cursor movement.

use crate::app::{App, View};

impl App {
    pub(crate) fn next_view(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        let idx = View::ALL.iter().position(|v| *v == self.view).unwrap_or(0);
        self.view = View::ALL[(idx + 1) % View::ALL.len()];
        self.cursor = 0;
        self.clamp_cursor_to_active();
        if self.view == View::Review {
            self.spawn_fetch_review();
        }
    }

    pub(crate) fn prev_view(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        let idx = View::ALL.iter().position(|v| *v == self.view).unwrap_or(0);
        self.view = View::ALL[(idx + View::ALL.len() - 1) % View::ALL.len()];
        self.cursor = 0;
        self.clamp_cursor_to_active();
        if self.view == View::Review {
            self.spawn_fetch_review();
        }
    }
    pub(crate) fn switch_view(&mut self, view: View) {
        self.generation = self.generation.wrapping_add(1);
        self.view = view;
        self.cursor = 0;
        self.clamp_cursor_to_active();
        if self.view == View::Review {
            self.spawn_fetch_review();
        }
    }
}
