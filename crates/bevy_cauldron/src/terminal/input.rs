//! **What a terminal sends a program for a cursor key and for the mouse** — bytes only, no Bevy.
//!
//! Both vocabularies are xterm's, because `TERM=xterm-256color` is what the session tells every program
//! ([`super::pty`]), and a program reads input against the terminfo entry that name selects. The
//! sources are xterm's *ctlseqs* ("PC-Style Function Keys", "Mouse Tracking") — the same tables VS
//! Code's xterm.js and Alacritty implement.

/// The modifiers held with a key or a click.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

impl Mods {
    fn any(self) -> bool {
        self.shift || self.alt || self.ctrl
    }
}

/// The keys whose spelling depends on the cursor-key mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorKey {
    Up,
    Down,
    Right,
    Left,
    Home,
    End,
}

impl CursorKey {
    fn final_byte(self) -> u8 {
        match self {
            CursorKey::Up => b'A',
            CursorKey::Down => b'B',
            CursorKey::Right => b'C',
            CursorKey::Left => b'D',
            CursorKey::Home => b'H',
            CursorKey::End => b'F',
        }
    }
}

/// **A cursor key, as xterm spells it** (#311).
///
/// - With a modifier: `CSI 1 ; m x`, where `m` is 1 + Shift + 2·Alt + 4·Ctrl — whatever the mode, which
///   is how Ctrl+Right reaches a shell as "a word right" (`ESC [ 1 ; 5 C`).
/// - Without one: `SS3 x` (`ESC O x`) once the program has asked for application cursor keys (DECCKM,
///   which ncurses' `keypad()` turns on), else `CSI x` (`ESC [ x`). `xterm-256color`'s terminfo names
///   the application form (`kcuu1=\EOA`), so an ncurses program in keypad mode reads `ESC [ A` as
///   Escape and `[` — measured with a Python curses program in a PTY: `[27, 91]` against `[259]`.
pub fn cursor_key(key: CursorKey, mods: Mods, app_cursor: bool) -> Vec<u8> {
    let last = key.final_byte();
    if mods.any() {
        let m = 1 + u8::from(mods.shift) + 2 * u8::from(mods.alt) + 4 * u8::from(mods.ctrl);
        return format!("\x1b[1;{m}{}", last as char).into_bytes();
    }
    if app_cursor {
        vec![0x1b, b'O', last]
    } else {
        vec![0x1b, b'[', last]
    }
}

/// The mouse buttons a terminal reports, with their xterm codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
}

impl MouseButton {
    fn code(self) -> u32 {
        match self {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
            MouseButton::WheelUp => 64,
            MouseButton::WheelDown => 65,
        }
    }
}

/// What the button did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseAction {
    Press,
    Release,
    /// Moved with the button held (reported under modes 1002 and 1003).
    Drag,
}

/// How the program asked for coordinates to be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseEncoding {
    /// Mode 1006: decimal parameters, and a release that names its button.
    Sgr,
    /// Mode 1005: the legacy bytes, with each coordinate a UTF-8 character (up to 2015).
    Utf8,
    /// The legacy X10/1000 form: one byte per value, so a coordinate past 223 cannot be written.
    Legacy,
}

/// Which mouse events the program asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseTracking {
    /// Mode 1000: presses, releases and the wheel.
    Click,
    /// Mode 1002: those, and motion while a button is held.
    Drag,
    /// Mode 1003: every motion too. This pane reports the held-button part of it.
    Motion,
}

impl MouseTracking {
    /// Does this mode want motion with a button held?
    pub fn reports_drag(self) -> bool {
        self != MouseTracking::Click
    }
}

/// **Where a wheel notch goes** (#312, #313).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WheelTarget {
    /// The pane's own scrollback: the normal screen, with no mouse reporting.
    Scrollback,
    /// The program, as cursor keys: the alternate screen has no scrollback, so xterm's *alternate
    /// scroll* (mode 1007, on by default) turns the wheel into Up and Down for `less`, `man`, `vim`.
    Arrows,
    /// The program, as a mouse report: it asked for the mouse.
    Report(MouseEncoding),
}

/// Lines one wheel notch moves, wherever it goes — the cadence the editor's lists use.
pub const LINES_PER_NOTCH: i32 = 3;

/// The modifier bits xterm adds to a button code: Shift 4, Meta 8, Control 16.
const SHIFT_BIT: u32 = 4;
const META_BIT: u32 = 8;
const CTRL_BIT: u32 = 16;
/// Added to a button code for motion with the button held.
const MOTION_BIT: u32 = 32;
/// The legacy code for "a button was released", which does not say which.
const LEGACY_RELEASE: u32 = 3;
/// The legacy encodings offset every value by a space, so a value is printable.
const LEGACY_OFFSET: u32 = 32;
/// The largest value one legacy byte can carry.
const LEGACY_MAX: u32 = 255;
/// The largest value one UTF-8 coordinate can carry under mode 1005 (xterm's own limit).
const UTF8_MAX: u32 = 2047;

/// **One mouse event, as the program asked to hear it** (#313), at the 0-based `(row, column)` cell.
///
/// `None` when the event cannot be written in the program's encoding — a cell past column 223 in the
/// legacy form — because a wrong byte would put the click somewhere the person did not click.
pub fn mouse_report(
    button: MouseButton,
    action: MouseAction,
    cell: (usize, usize),
    mods: Mods,
    encoding: MouseEncoding,
) -> Option<Vec<u8>> {
    let x = u32::try_from(cell.1).ok()?.checked_add(1)?;
    let y = u32::try_from(cell.0).ok()?.checked_add(1)?;
    let mut code = button.code();
    if action == MouseAction::Drag {
        code += MOTION_BIT;
    }
    if mods.shift {
        code += SHIFT_BIT;
    }
    if mods.alt {
        code += META_BIT;
    }
    if mods.ctrl {
        code += CTRL_BIT;
    }
    match encoding {
        MouseEncoding::Sgr => {
            let end = if action == MouseAction::Release { 'm' } else { 'M' };
            Some(format!("\x1b[<{code};{x};{y}{end}").into_bytes())
        }
        MouseEncoding::Legacy | MouseEncoding::Utf8 => {
            // A legacy release does not name its button; the modifier bits stay.
            if action == MouseAction::Release {
                code = (code & !0b11 & !MOTION_BIT) | LEGACY_RELEASE;
            }
            let mut out = vec![0x1b, b'[', b'M'];
            push_legacy(&mut out, code + LEGACY_OFFSET, LEGACY_MAX)?;
            let limit = if encoding == MouseEncoding::Utf8 { UTF8_MAX } else { LEGACY_MAX };
            push_legacy(&mut out, x + LEGACY_OFFSET, limit)?;
            push_legacy(&mut out, y + LEGACY_OFFSET, limit)?;
            Some(out)
        }
    }
}

/// One legacy value: a byte, or under mode 1005 a UTF-8 character, refused past `limit`.
fn push_legacy(out: &mut Vec<u8>, value: u32, limit: u32) -> Option<()> {
    if value > limit {
        return None;
    }
    if value < 0x80 {
        out.push(value as u8);
    } else if limit > LEGACY_MAX {
        let mut utf8 = [0u8; 4];
        out.extend_from_slice(char::from_u32(value)?.encode_utf8(&mut utf8).as_bytes());
    } else {
        out.push(value as u8);
    }
    Some(())
}
