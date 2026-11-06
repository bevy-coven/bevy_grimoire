//! **What the terminal sends for a cursor key and for the mouse, in each mode a program can set**
//! (#311, #312, #313). The modes come from feeding a screen the very escapes a program writes, so the
//! readers are tested against `alacritty_terminal`'s own parse, not a hand-set flag.

use super::grid::TerminalScreen;
use super::input::{
    CursorKey, Mods, MouseAction, MouseButton, MouseEncoding, MouseTracking, WheelTarget, cursor_key, mouse_report,
};

const CTRL: Mods = Mods { shift: false, alt: false, ctrl: true };
const SHIFT: Mods = Mods { shift: true, alt: false, ctrl: false };
const NONE: Mods = Mods { shift: false, alt: false, ctrl: false };

#[test]
fn a_cursor_key_is_spelled_for_the_programs_mode() {
    assert_eq!(cursor_key(CursorKey::Up, NONE, false), b"\x1b[A", "normal mode is CSI");
    assert_eq!(cursor_key(CursorKey::Up, NONE, true), b"\x1bOA", "application mode is SS3 (terminfo kcuu1)");
    assert_eq!(cursor_key(CursorKey::Home, NONE, true), b"\x1bOH", "terminfo khome");
    assert_eq!(cursor_key(CursorKey::End, NONE, false), b"\x1b[F");
    assert_eq!(cursor_key(CursorKey::Right, CTRL, false), b"\x1b[1;5C", "Ctrl+Right is a word right");
    assert_eq!(
        cursor_key(CursorKey::Left, CTRL, true),
        b"\x1b[1;5D",
        "a modifier is always the CSI parameter form, whatever the mode"
    );
    let all = Mods { shift: true, alt: true, ctrl: true };
    assert_eq!(cursor_key(CursorKey::Down, all, false), b"\x1b[1;8B", "1 + Shift 1 + Alt 2 + Ctrl 4");
}

#[test]
fn the_screen_reports_the_cursor_mode_a_program_sets() {
    let mut screen = TerminalScreen::new(20, 5);
    assert!(!screen.app_cursor(), "a fresh screen is in normal cursor mode");
    screen.feed(b"\x1b[?1h");
    assert!(screen.app_cursor(), "DECCKM set (ncurses smkx)");
    screen.feed(b"\x1b[?1l");
    assert!(!screen.app_cursor(), "DECCKM reset (rmkx)");
}

#[test]
fn the_wheel_goes_to_the_scrollback_the_arrows_or_the_program() {
    let mut screen = TerminalScreen::new(20, 5);
    assert_eq!(screen.wheel_target(), WheelTarget::Scrollback, "the normal screen has a scrollback");
    screen.feed(b"\x1b[?1049h");
    assert_eq!(screen.wheel_target(), WheelTarget::Arrows, "the alternate screen has none: alternate scroll");
    screen.feed(b"\x1b[?1007l");
    assert_eq!(screen.wheel_target(), WheelTarget::Scrollback, "a program may turn alternate scroll off");
    screen.feed(b"\x1b[?1007h\x1b[?1000h\x1b[?1006h");
    assert_eq!(screen.wheel_target(), WheelTarget::Report(MouseEncoding::Sgr), "it asked for the mouse");
    assert_eq!(screen.mouse_reporting(), Some((MouseTracking::Click, MouseEncoding::Sgr)));
    screen.feed(b"\x1b[?1002h\x1b[?1006l");
    assert_eq!(screen.mouse_reporting(), Some((MouseTracking::Drag, MouseEncoding::Legacy)));
    screen.feed(b"\x1b[?1003h\x1b[?1005h");
    assert_eq!(screen.mouse_reporting(), Some((MouseTracking::Motion, MouseEncoding::Utf8)));
    screen.feed(b"\x1b[?1003l");
    assert_eq!(screen.mouse_reporting(), None, "turning the mode off gives the mouse back");
}

#[test]
fn sgr_reports_name_the_button_the_cell_and_a_release() {
    let at = (4, 9); // row 4, column 9, 0-based: x 10, y 5 on the wire
    let report = |button, action, mods| mouse_report(button, action, at, mods, MouseEncoding::Sgr).expect("SGR writes any cell");
    assert_eq!(report(MouseButton::Left, MouseAction::Press, NONE), b"\x1b[<0;10;5M");
    assert_eq!(report(MouseButton::Left, MouseAction::Release, NONE), b"\x1b[<0;10;5m");
    assert_eq!(report(MouseButton::Middle, MouseAction::Drag, NONE), b"\x1b[<33;10;5M", "motion adds 32");
    assert_eq!(report(MouseButton::WheelUp, MouseAction::Press, NONE), b"\x1b[<64;10;5M");
    assert_eq!(report(MouseButton::WheelDown, MouseAction::Press, CTRL), b"\x1b[<81;10;5M", "Ctrl adds 16");
    assert_eq!(report(MouseButton::Left, MouseAction::Press, SHIFT), b"\x1b[<4;10;5M", "Shift adds 4");
    assert_eq!(
        mouse_report(MouseButton::Left, MouseAction::Press, (0, 999), NONE, MouseEncoding::Sgr).unwrap(),
        b"\x1b[<0;1000;1M",
        "SGR has no column limit"
    );
}

#[test]
fn legacy_reports_are_bytes_and_refuse_what_they_cannot_write() {
    let press = mouse_report(MouseButton::Left, MouseAction::Press, (4, 9), NONE, MouseEncoding::Legacy).unwrap();
    assert_eq!(press, [0x1b, b'[', b'M', 32, 32 + 10, 32 + 5]);
    let release = mouse_report(MouseButton::Middle, MouseAction::Release, (4, 9), CTRL, MouseEncoding::Legacy).unwrap();
    assert_eq!(release, [0x1b, b'[', b'M', 32 + 3 + 16, 32 + 10, 32 + 5], "release is 3, modifiers kept");
    assert_eq!(
        mouse_report(MouseButton::Left, MouseAction::Press, (0, 222), NONE, MouseEncoding::Legacy).unwrap()[4],
        255,
        "column 223 is the last one a byte can carry"
    );
    assert_eq!(
        mouse_report(MouseButton::Left, MouseAction::Press, (0, 223), NONE, MouseEncoding::Legacy),
        None,
        "past column 223 a byte would name the wrong cell, so nothing is sent"
    );
    let wide = mouse_report(MouseButton::Left, MouseAction::Press, (0, 299), NONE, MouseEncoding::Utf8).unwrap();
    assert_eq!(&wide[4..], "\u{14C}!".as_bytes(), "mode 1005 writes 300 + 32 as a UTF-8 character");
}
