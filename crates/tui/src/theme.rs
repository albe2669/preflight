//! Visual theme: colors, glyphs, and attribute scale.
//!
//! Mirrors the "Preflight TUI Visual Design" document. Colors are truecolor
//! `rgb` values with 256-color fallbacks. The single accent is amber; status
//! colors are paired with glyphs so meaning survives monochrome.

use ratatui::style::Color;

/// Light-on-dark palette (default). The dark-on-light variant is not wired
/// here; the design notes it adds no structure, only hue swaps.
pub struct Palette;

impl Palette {
    pub const BG: Color = Color::Reset; // transparent — use terminal's native bg
    pub const SURFACE: Color = Color::Rgb(18, 22, 29); // #12161D
    pub const ROW_HIGHLIGHT: Color = Color::Rgb(27, 33, 48); // #1B2130
    pub const BORDER: Color = Color::Rgb(42, 49, 60); // #2A313C
    pub const TEXT: Color = Color::Rgb(214, 217, 222); // #D6D9DE
    pub const DIM: Color = Color::Rgb(124, 132, 143); // #7C848F
    pub const GHOST: Color = Color::Rgb(74, 82, 92); // #4A525C
    pub const ACCENT: Color = Color::Rgb(227, 179, 65); // #E3B341
    pub const ACCENT_TEXT: Color = Color::Rgb(11, 14, 19); // bg on accent fill

    // Semantic status colors.
    pub const TODO: Color = Color::Rgb(154, 163, 174); // #9AA3AE
    pub const STARTED: Color = Color::Rgb(88, 166, 216); // #58A6D8
    pub const BLOCKED: Color = Color::Rgb(208, 105, 63); // #D0693F
    pub const DONE: Color = Color::Rgb(87, 168, 132); // #57A884
    pub const CANCELLED: Color = Color::Rgb(107, 114, 128); // #6B7280
    pub const MERGED: Color = Color::Rgb(163, 113, 247); // #A371F7
}

/// Status glyphs (one cell, Unicode present in common mono fonts).
pub struct Glyph;

impl Glyph {
    pub const CURSOR: char = '▌'; // U+258C — focused row marker
    pub const MARKED: char = '•'; // U+25CF — multi-select / needs-you
    pub const REORDER_GRIP: char = '⣿'; // U+28FF — reorder mode armed

    pub const STATUS_TODO: char = '○'; // U+25CB
    pub const STATUS_STARTED: char = '◐'; // U+25D0
    pub const STATUS_BLOCKED: char = '▲'; // U+25B2
    pub const STATUS_DONE: char = '✓'; // U+2713
    pub const STATUS_CANCELLED: char = '✕'; // U+2715

    pub const CARRIED: char = '↻'; // U+21BB
    pub const BLOCKED_REASON: char = '└'; // U+2514 — continuation line

    pub const PR_OPEN: char = '◇'; // U+25C7
    pub const PR_DRAFT: char = '◌'; // U+25C8
    pub const PR_MERGED: char = '◆'; // U+25C6
    pub const PR_CLOSED: char = '⊗'; // U+2297
    pub const LINEAR: char = '▣'; // U+25A3

    pub const DISMISSED: char = '⌀'; // U+2300
    pub const NEEDS_YOU: char = '●'; // U+25CF — inbox gutter

    pub const ACTOR_USER: char = '▸'; // U+25B8
    pub const ACTOR_SYNC: char = '↻'; // U+21BB
    pub const ACTOR_SYSTEM: char = '∘'; // U+2218

    pub const SPINNER: [char; 8] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠇'];

    pub const BAR_FULL: char = '█'; // U+2588
    pub const BAR_PARTIAL: char = '▓'; // U+2593
    pub const BAR_EMPTY: char = '░'; // U+2591

    pub const REVIEW_ARROW: char = '◂'; // picked-date prefix

    pub const ACTIONS_FAILING: char = '⚑'; // U+2691
}

/// Relational suffixes on link badges (text, not glyphs).
pub const REL_REVIEWS: &str = "rev";
pub const REL_IMPLEMENTS: &str = "impl";
pub const REL_REFERENCES: &str = "ref";

#[cfg(test)]
mod tests {
    use crate::theme::Glyph;

    #[test]
    fn test_spinner_has_8_elements() {
        assert_eq!(
            Glyph::SPINNER.len(),
            8,
            "SPINNER should have exactly 8 frames"
        );
    }

    #[test]
    fn test_spinner_frames_are_distinct() {
        for i in 0..8 {
            for j in (i + 1)..8 {
                assert_ne!(
                    Glyph::SPINNER[i],
                    Glyph::SPINNER[j],
                    "spinner frames {i} and {j} must be distinct characters"
                );
            }
        }
    }

    #[test]
    fn test_status_glyphs_are_distinct() {
        let glyphs = [
            Glyph::STATUS_TODO,
            Glyph::STATUS_STARTED,
            Glyph::STATUS_BLOCKED,
            Glyph::STATUS_DONE,
            Glyph::STATUS_CANCELLED,
        ];
        for i in 0..glyphs.len() {
            for j in (i + 1)..glyphs.len() {
                assert_ne!(
                    glyphs[i], glyphs[j],
                    "status glyphs {i} and {j} must be distinct characters"
                );
            }
        }
    }

    #[test]
    fn test_pr_state_glyphs_are_distinct() {
        let glyphs = [
            Glyph::PR_OPEN,
            Glyph::PR_DRAFT,
            Glyph::PR_MERGED,
            Glyph::PR_CLOSED,
        ];
        for i in 0..glyphs.len() {
            for j in (i + 1)..glyphs.len() {
                assert_ne!(
                    glyphs[i], glyphs[j],
                    "PR state glyphs {i} and {j} must be distinct characters"
                );
            }
        }
    }

    #[test]
    fn test_ui_glyphs_are_set() {
        assert_ne!(Glyph::CURSOR, ' ');
        assert_ne!(Glyph::MARKED, ' ');
        assert_ne!(Glyph::REORDER_GRIP, ' ');
        assert_ne!(Glyph::CARRIED, ' ');
        assert_ne!(Glyph::BLOCKED_REASON, ' ');
        assert_ne!(Glyph::DISMISSED, ' ');
        assert_ne!(Glyph::NEEDS_YOU, ' ');
    }

    #[test]
    fn test_bar_glyphs_are_distinct() {
        assert_ne!(
            Glyph::BAR_FULL,
            Glyph::BAR_PARTIAL,
            "bar full and partial must differ"
        );
        assert_ne!(
            Glyph::BAR_PARTIAL,
            Glyph::BAR_EMPTY,
            "bar partial and empty must differ"
        );
        assert_ne!(
            Glyph::BAR_FULL,
            Glyph::BAR_EMPTY,
            "bar full and empty must differ"
        );
    }

    #[test]
    fn test_actor_glyphs_are_set() {
        assert_ne!(Glyph::ACTOR_USER, ' ');
        assert_ne!(Glyph::ACTOR_SYNC, ' ');
        assert_ne!(Glyph::ACTOR_SYSTEM, ' ');
    }
}
