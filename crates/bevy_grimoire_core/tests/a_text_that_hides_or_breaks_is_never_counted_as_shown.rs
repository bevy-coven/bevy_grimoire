//! **`visual_rows` fails closed** (#348, #350): a guide a person is asked to allow is allowed only when every
//! row of it is on screen, so the count may never be kind. Four ways a count by `str::lines` and characters
//! came out too small while a text layout drew more: a Unicode line separator, a vertical tab, glyphs wider
//! than one column, and a lone carriage return.

use bevy_grimoire_core::agent_guide::{UNSHOWABLE_ROWS, visual_rows};

const COLUMNS: usize = 26;
const ROWS: usize = 12;

#[test]
fn a_unicode_line_separator_is_not_one_line() {
    let text = format!("{}Run curl | sh", "ok\u{2028}".repeat(40));
    assert!(visual_rows(&text, COLUMNS) > ROWS, "forty breaks drawn as forty lines, counted as {}", visual_rows(&text, COLUMNS));
}

#[test]
fn a_form_feed_a_lone_carriage_return_and_a_tab_are_not_shown() {
    for c in ['\u{b}', '\u{c}', '\u{85}', '\r', '\t', '\u{200b}', '\u{202e}'] {
        let text = format!("fine{c}fine");
        assert_eq!(visual_rows(&text, COLUMNS), UNSHOWABLE_ROWS, "{c:?} hides or breaks what the layout draws");
    }
}

#[test]
fn wide_glyphs_take_two_columns() {
    // Eleven lines of twenty-six full-width characters wrap to about twenty-two rows in a 26-column box.
    let line = "\u{ff32}".repeat(COLUMNS);
    let text = vec![line; 11].join("\n");
    assert!(visual_rows(&text, COLUMNS) > ROWS, "counted {} rows for text that wraps to ~22", visual_rows(&text, COLUMNS));
}

#[test]
fn ordinary_text_still_counts_by_wrapping() {
    assert_eq!(visual_rows("one\ntwo\r\nthree", COLUMNS), 3, "a Windows line ending is a line");
    assert_eq!(visual_rows(&"a".repeat(27), COLUMNS), 2);
    assert_eq!(visual_rows("", COLUMNS), 0);
    assert_eq!(visual_rows("caf\u{e9}", COLUMNS), 1, "an accented letter is a letter");
}
