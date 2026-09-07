//! Shared readline editing primitives.
//!
//! Sofka's `edit_chord` (ctrl-u kill line, ctrl-w/alt-backspace kill word)
//! extended with caret tracking: every text input in the TUI uses these
//! primitives for consistent editing behavior.

use crossterm::event::{KeyCode, KeyModifiers};

/// Readline chord: ctrl-u (kill line), ctrl-w / alt-backspace (kill word),
/// ctrl-a (home), ctrl-e (end).
/// Returns true if handled. Used by every text input before mode-specific
/// key handling.
pub(crate) fn edit_chord(
    key: KeyCode,
    mods: KeyModifiers,
    buf: &mut String,
    caret: &mut usize,
) -> bool {
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    let alt = mods.contains(KeyModifiers::ALT);
    match key {
        KeyCode::Char('u') if ctrl => {
            buf.clear();
            *caret = 0;
        }
        KeyCode::Char('w') if ctrl => {
            *caret = pop_word(buf, *caret);
        }
        KeyCode::Char('a') if ctrl => {
            *caret = 0;
        }
        KeyCode::Char('e') if ctrl => {
            *caret = buf.len();
        }
        KeyCode::Backspace if alt || ctrl => {
            *caret = pop_word(buf, *caret);
        }
        _ => return false,
    }
    true
}

/// Delete the trailing word before caret (readline unix-word-rubout).
/// Returns the new caret position.
fn pop_word(buf: &mut String, caret: usize) -> usize {
    let caret = caret.min(buf.len());
    let bytes = buf.as_bytes();
    let mut end = caret;
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    while end > 0 && !bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    buf.replace_range(end..caret, "");
    end
}

/// Insert `ch` at byte `caret` in `buf`; return new caret.
pub(crate) fn insert_char(buf: &mut String, caret: usize, ch: char) -> usize {
    let caret = caret.min(buf.len());
    buf.insert(caret, ch);
    caret + ch.len_utf8()
}

/// Delete the char before `caret`; return new caret.
pub(crate) fn backspace(buf: &mut String, caret: usize) -> usize {
    let caret = caret.min(buf.len());
    if caret == 0 {
        return 0;
    }
    let idx = buf[..caret]
        .char_indices()
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(0);
    buf.replace_range(idx..caret, "");
    idx
}

/// Move caret one char in `dir` (-1 left, +1 right), clamped to [0, len].
pub(crate) fn move_caret(buf: &str, caret: usize, dir: i8) -> usize {
    let caret = caret.min(buf.len());
    if dir < 0 {
        if caret == 0 {
            0
        } else {
            buf[..caret]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0)
        }
    } else {
        if caret >= buf.len() {
            buf.len()
        } else {
            buf[caret..]
                .chars()
                .next()
                .map(|c| caret + c.len_utf8())
                .unwrap_or(buf.len())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_chord_ctrl_u_clears_and_resets_caret() {
        let mut buf = String::from("hello world");
        let mut caret = 5;
        assert!(edit_chord(
            KeyCode::Char('u'),
            KeyModifiers::CONTROL,
            &mut buf,
            &mut caret
        ));
        assert_eq!(buf, "");
        assert_eq!(caret, 0);
    }

    #[test]
    fn test_edit_chord_ctrl_w_pops_word_and_updates_caret() {
        let mut buf = String::from("hello world");
        let mut caret = 11;
        assert!(edit_chord(
            KeyCode::Char('w'),
            KeyModifiers::CONTROL,
            &mut buf,
            &mut caret
        ));
        assert_eq!(buf, "hello ");
        assert_eq!(caret, 6);
    }

    #[test]
    fn test_edit_chord_ctrl_a_sets_caret_to_start() {
        let mut buf = String::from("hello");
        let mut caret = 5;
        assert!(edit_chord(
            KeyCode::Char('a'),
            KeyModifiers::CONTROL,
            &mut buf,
            &mut caret
        ));
        assert_eq!(caret, 0);
        assert_eq!(buf, "hello");
    }

    #[test]
    fn test_edit_chord_ctrl_e_sets_caret_to_end() {
        let mut buf = String::from("hello");
        let mut caret = 0;
        assert!(edit_chord(
            KeyCode::Char('e'),
            KeyModifiers::CONTROL,
            &mut buf,
            &mut caret
        ));
        assert_eq!(caret, 5);
    }

    #[test]
    fn test_edit_chord_alt_backspace_pops_word() {
        let mut buf = String::from("hello world");
        let mut caret = 11;
        assert!(edit_chord(
            KeyCode::Backspace,
            KeyModifiers::ALT,
            &mut buf,
            &mut caret
        ));
        assert_eq!(buf, "hello ");
        assert_eq!(caret, 6);
    }

    #[test]
    fn test_edit_chord_unhandled_returns_false() {
        let mut buf = String::from("hello");
        let mut caret = 0;
        assert!(!edit_chord(
            KeyCode::Char('x'),
            KeyModifiers::NONE,
            &mut buf,
            &mut caret
        ));
        assert_eq!(buf, "hello");
        assert_eq!(caret, 0);
    }

    #[test]
    fn test_pop_word_trailing_whitespace_first() {
        let mut buf = String::from("hello   ");
        let caret = pop_word(&mut buf, 8);
        assert_eq!(buf, "");
        assert_eq!(caret, 0);
    }

    #[test]
    fn test_insert_char_at_middle() {
        let mut buf = String::from("hello");
        let caret = insert_char(&mut buf, 2, 'X');
        assert_eq!(buf, "heXllo");
        assert_eq!(caret, 3);
    }

    #[test]
    fn test_backspace_at_start_is_noop() {
        let mut buf = String::from("hello");
        let caret = backspace(&mut buf, 0);
        assert_eq!(buf, "hello");
        assert_eq!(caret, 0);
    }

    #[test]
    fn test_move_caret_left_at_start_clamps() {
        let caret = move_caret("hello", 0, -1);
        assert_eq!(caret, 0);
    }

    #[test]
    fn test_move_caret_right_at_end_clamps() {
        let caret = move_caret("hello", 5, 1);
        assert_eq!(caret, 5);
    }
}
