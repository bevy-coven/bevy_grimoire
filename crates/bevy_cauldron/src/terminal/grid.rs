//! **The emulator: bytes in, cells out.**
//!
//! `alacritty_terminal::Term` is the VTE state machine and the grid behind it; `vte::Parser` is what
//! drives it. Both are private to this file. What leaves is [`TerminalScreen::lines`] — a `Vec` of
//! rows of [`Cell`]s whose colours are already `bevy::Color`s, so nothing above here has ever heard
//! of a palette index, a `NamedColor`, or an escape sequence.
//!
//! # Why the boundary is here and not higher
//!
//! A drawing system that iterated `Term` directly would have to know that a cell's `fg` may be
//! `Named(Background)` and means "the default", that a wide character occupies two cells, that
//! `display_offset` moves the viewport through scrollback, and that a wrapped row is marked on its
//! *last* cell. That is four pieces of terminal lore in a UI system. Here they are four lines.
//!
//! # What is drawn and what is not
//!
//! Bevy's text has no underline, no strikethrough and no italic face for this editor's one shipped
//! weight, so those three attributes are **not** carried out of this file — carrying a flag nothing
//! can draw is a promise the screen does not keep. What *is* carried, because it is what a shell
//! actually uses to be readable: colour, the inverse attribute (baked into a swapped `fg`/`bg`), the
//! dim attribute (baked as a mix toward the background, which is what `ESC[2m` means), and bold on a
//! named colour (which becomes that colour's bright slot, the standard's own rule for `ESC[1m31m`).
//!
//! # Scrollback is bounded, and the bound is stated
//!
//! [`SCROLLBACK`] lines. A `cargo build` with `-v` can produce megabytes; a terminal that kept them
//! all would be a slow leak in a program that is meant to run for a working day. Five thousand lines
//! is about eight of the panes' worth at its tallest.

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line as TermLine, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor, Processor};
use bevy::prelude::*;

use super::palette;

/// How many lines of history a session keeps above its viewport.
pub const SCROLLBACK: usize = 5_000;

/// One row of the screen, as the pane draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// The row's cells, left to right, **with the trailing blanks of a default background removed**.
    ///
    /// Trimming is not cosmetic: a 200-column row of spaces is 200 entities if the view draws them,
    /// and most of a terminal's rows are empty. A cell that paints a background is kept even when it
    /// is a space, because that block is visible.
    pub cells: Vec<Cell>,
    /// **This row continues onto the next one.**
    ///
    /// True when the child's output wrapped at the right edge rather than starting a new line, which
    /// is what makes a 120-character command line one logical line drawn as two rows. The emulator
    /// already re-wraps the scrollback when the pane narrows ([`TerminalScreen::resize`]), so this
    /// flag's job in the pane is to say *why* the next row starts mid-sentence — no left padding, no
    /// prompt — and to join them when the pane asks for its text.
    pub wraps: bool,
}

/// One cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
}

impl Cell {
    /// A blank on the pane's own ground — what a row is padded with, and what trimming looks for.
    fn is_default_blank(&self) -> bool {
        self.ch == ' ' && self.bg == palette::default_bg() && self.fg == palette::default_fg()
    }
}

/// Where the caret is, in the pane's own row/column terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    /// Row from the top of the drawn viewport.
    pub row: usize,
    pub column: usize,
    /// The child hid the caret (`ESC[?25l`), which a full-screen program does while it paints.
    pub visible: bool,
}

/// **What the child wrote to the terminal**, collected rather than acted on here.
///
/// `Term` answers some escape sequences by *writing back* — a cursor-position report (`ESC[6n`), a
/// device-attributes reply, an `OSC 52` clipboard store. Those replies must reach the shell or a
/// program that asked will wait forever: `tput cols` is exactly this shape. So the listener collects
/// them and [`TerminalScreen::take_replies`] hands them to whoever owns the write end.
///
/// `Event::ClipboardLoad` carries a closure this type deliberately never calls: a paste request from
/// the shell would need the pane's clipboard, and the pane's paste is the person's `Ctrl+Shift+V`.
/// Leaving the closure uncalled means the shell is told nothing, which is the honest answer for a
/// terminal that does not offer the shell its clipboard.
#[derive(Clone, Default)]
pub struct TerminalListener {
    replies: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}

impl EventListener for TerminalListener {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event {
            if let Ok(mut replies) = self.replies.lock() {
                replies.extend_from_slice(text.as_bytes());
            }
        }
    }
}

/// The size the emulator is told it has.
struct Size {
    cols: usize,
    rows: usize,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

/// **The emulator.** One per session; `feed` bytes in, read `lines` out.
pub struct TerminalScreen {
    term: Term<TerminalListener>,
    parser: Processor,
    listener: TerminalListener,
    /// Set by [`feed`](Self::feed), [`resize`](Self::resize) and [`scroll`](Self::scroll), cleared by
    /// [`take_dirty`](Self::take_dirty). A settled prompt therefore costs one `bool` read per frame
    /// rather than a rebuild of two hundred rows.
    dirty: bool,
    /// What [`lines`](Self::lines) last built, so a rebuild happens once per change rather than once
    /// per frame — and so a caller can hold a row across frames without re-asking.
    rows: Vec<Line>,
    /// The caret's place, cached beside `rows` for the same reason.
    caret: Cursor,
    cols: u16,
    rows_count: u16,
}

impl TerminalScreen {
    /// A screen of `cols` × `rows` cells that has never been fed.
    ///
    /// Zero is refused by [`crate::terminal::Terminal::spawn`] before it gets here, but the fields
    /// are clamped anyway because a zero in `Term::new` divides by zero deep inside the grid's
    /// resize when the first wide character arrives, and a panic three frames later is worse than a
    /// terminal that is one cell wide.
    pub fn new(cols: u16, rows: u16) -> TerminalScreen {
        let cols = cols.max(2);
        let rows = rows.max(1);
        let listener = TerminalListener::default();
        let config = Config {
            scrolling_history: SCROLLBACK,
            ..Default::default()
        };
        let term = Term::new(config, &Size { cols: cols as usize, rows: rows as usize }, listener.clone());
        TerminalScreen {
            term,
            parser: Processor::new(),
            listener,
            dirty: true,
            rows: Vec::new(),
            caret: Cursor { row: 0, column: 0, visible: true },
            cols,
            rows_count: rows,
        }
    }

    /// The emulator's current width in cells.
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// The emulator's current height in cells.
    pub fn rows(&self) -> u16 {
        self.rows_count
    }

    /// **Feed the child's bytes to the parser.**
    ///
    /// The only path into the grid: a caller that fed `term` directly would skip the parser and draw
    /// the escape sequences as text.
    pub fn feed(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let mut term = std::mem::replace(
            &mut self.term,
            Term::new(
                Config { scrolling_history: 0, ..Default::default() },
                &Size { cols: self.cols as usize, rows: self.rows_count as usize },
                self.listener.clone(),
            ),
        );
        self.parser.advance(&mut term, bytes);
        self.term = term;
        self.dirty = true;
    }

    /// **Tell the emulator the pane changed size.**
    ///
    /// This is also the reflow: `Term::resize` re-wraps the scrollback against the new width using
    /// the wrap marks the child's own output left, so a 120-character line that was one row at 120
    /// columns becomes two rows at 60 — and stays one logical line. Nothing in the pane has to
    /// re-split anything.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        let cols = cols.max(2);
        let rows = rows.max(1);
        if cols == self.cols && rows == self.rows_count {
            return;
        }
        self.cols = cols;
        self.rows_count = rows;
        self.term.resize(Size { cols: cols as usize, rows: rows as usize });
        self.dirty = true;
    }

    /// Move the viewport through the scrollback. Positive `delta` scrolls up (toward older lines).
    pub fn scroll(&mut self, delta: i32) {
        if delta == 0 {
            return;
        }
        let before = self.term.grid().display_offset();
        self.term.scroll_display(Scroll::Delta(delta));
        if self.term.grid().display_offset() != before {
            self.dirty = true;
        }
    }

    /// `PageUp`/`PageDown`, in whole viewports.
    pub fn scroll_page(&mut self, up: bool) {
        self.term
            .scroll_display(if up { Scroll::PageUp } else { Scroll::PageDown });
        self.dirty = true;
    }

    /// Was anything drawn since the last time the pane asked?
    pub fn take_dirty(&mut self) -> bool {
        std::mem::replace(&mut self.dirty, false)
    }

    /// **The rows the pane draws**, rebuilt if anything changed.
    ///
    /// Returned by reference so a still terminal hands back the same `Vec` sixty times a second.
    pub fn lines(&mut self) -> &[Line] {
        if self.take_dirty() {
            self.rebuild();
        }
        &self.rows
    }

    /// The caret, in the same terms as [`lines`](Self::lines).
    pub fn cursor(&self) -> Cursor {
        self.caret
    }

    /// Bytes the emulator owes the shell — see [`TerminalListener`]. Taken, not read, so a reply is
    /// written once.
    pub fn take_replies(&mut self) -> Vec<u8> {
        self.listener
            .replies
            .lock()
            .map(|mut r| std::mem::take(&mut *r))
            .unwrap_or_default()
    }

    /// Select from `from` to `to`, both in viewport cell coordinates — what a drag over the pane does.
    pub fn select(&mut self, from: (usize, usize), to: (usize, usize)) {
        use alacritty_terminal::index::Side;
        use alacritty_terminal::selection::{Selection, SelectionType};
        let (start, end) = if from <= to { (from, to) } else { (to, from) };
        let anchor = self.point_of(start.0, start.1);
        let mut selection = Selection::new(SelectionType::Simple, anchor, Side::Left);
        selection.update(self.point_of(end.0, end.1), Side::Right);
        self.term.selection = Some(selection);
        self.dirty = true;
    }

    /// **The selected text**, or `None` when nothing is selected.
    ///
    /// Trailing blank columns trimmed per line, so copying a block out of a terminal does not paste
    /// forty spaces at each line's end — which is what every text editor then has to have stripped.
    pub fn selection_text(&self) -> Option<String> {
        let text = self.term.selection_to_string()?;
        if text.is_empty() {
            return None;
        }
        let trimmed: Vec<&str> = text.split('\n').map(str::trim_end).collect();
        Some(trimmed.join("\n"))
    }

    /// **Forget the scrollback**, keeping what is on screen.
    ///
    /// `Term::clear_screen` clears the viewport; the history above it is what a long session pays for,
    /// so this is the menu's Clear: the live output stays, the thousand lines above it go. Nothing the
    /// shell said is lost from the *screen* — an author clearing history is the same gesture as
    /// `clear` in a shell, and it is not an undoable edit because it is not an edit.
    pub fn clear_scrollback(&mut self) {
        // **Drop what is held, then put the limit back.** `update_history(0)` shrinks the held lines
        // *and* the maximum, so leaving it there would make the session keep no history ever again —
        // the Clear menu item would quietly turn into a setting. The second call raises the ceiling
        // and shrinks nothing, because there is nothing left to shrink.
        self.term.grid_mut().update_history(0);
        self.term.grid_mut().update_history(SCROLLBACK);
        self.dirty = true;
    }

    /// **Did the child ask for bracketed paste?** (`ESC[?2004h`.)
    ///
    /// Read by the paste path: a multi-line paste wrapped in `ESC[200~`/`ESC[201~` is an insertion, an
    /// unwrapped one is a series of `Enter`s — and only the child knows which it wants.
    pub fn bracketed_paste(&self) -> bool {
        self.term
            .mode()
            .contains(alacritty_terminal::term::TermMode::BRACKETED_PASTE)
    }

    /// Viewport cell coordinates to an absolute grid point.
    fn point_of(&self, row: usize, column: usize) -> Point {
        let offset = self.term.grid().display_offset() as i32;
        Point::new(
            TermLine(row as i32 - offset),
            Column(column.min(self.cols.saturating_sub(1) as usize)),
        )
    }

    /// Rebuild `rows` and `caret` from the grid.
    fn rebuild(&mut self) {
        let offset = self.term.grid().display_offset() as i32;
        let cols = self.cols as usize;
        let fg_default = palette::default_fg();
        let bg_default = palette::default_bg();

        let mut rows: Vec<Line> = Vec::with_capacity(self.rows_count as usize);
        let mut row: Vec<Cell> = Vec::with_capacity(cols);
        let mut wrapped = false;
        let mut open_line = i32::MIN;

        // **One pass over the visible grid**, which is what `display_iter` is for: it starts at the
        // top-left cell of the viewport and walks to its bottom-right, skipping the scrollback.
        for indexed in self.term.grid().display_iter() {
            let line = indexed.point.line.0;
            if line != open_line {
                if open_line != i32::MIN {
                    rows.push(finish_row(std::mem::take(&mut row), wrapped));
                }
                open_line = line;
                wrapped = false;
            }
            let cell = indexed.cell;
            // **The wrap mark is read before anything else touches the cell**, because it lives on
            // the row's last cell and a row whose last cells are spaces would lose it to the trim.
            if cell.flags.contains(Flags::WRAPLINE) {
                wrapped = true;
            }
            // A wide character's second cell is a spacer: drawing it would push the rest of the row
            // one column right, so it is skipped rather than drawn as a blank.
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) || cell.flags.contains(Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }
            let (mut fg, mut bg) = (colour_of(cell.fg, fg_default), colour_of(cell.bg, bg_default));
            if cell.flags.contains(Flags::BOLD) {
                // The standard's rule for a bold named colour: it becomes the bright slot. This is
                // what makes `ls`'s directories and `grep`'s matches stand out.
                fg = brightened(cell.fg, fg);
            }
            if cell.flags.contains(Flags::DIM) {
                fg = fg.mix(&bg, 0.4);
            }
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            row.push(Cell { ch: cell.c, fg, bg });
        }
        if !row.is_empty() || !rows.is_empty() {
            rows.push(finish_row(row, wrapped));
        }
        // `display_iter` yields exactly `screen_lines` rows unless the grid is empty; pad so the
        // pane's row count and the emulator's agree even on a grid that has never been written.
        while rows.len() < self.rows_count as usize {
            rows.push(Line { cells: Vec::new(), wraps: false });
        }

        // The caret, in viewport terms.
        let content = self.term.renderable_content();
        let point = content.cursor.point;
        let row = (point.line.0 + offset).clamp(0, self.rows_count as i32 - 1) as usize;
        self.caret = Cursor {
            row,
            column: point.column.0.min(cols.saturating_sub(1)),
            visible: content.cursor.shape != alacritty_terminal::vte::ansi::CursorShape::Hidden,
        };
        self.rows = rows;
    }
}

/// Trim the trailing blanks off one row.
fn finish_row(mut cells: Vec<Cell>, wraps: bool) -> Line {
    while cells.last().is_some_and(Cell::is_default_blank) {
        cells.pop();
    }
    Line { cells, wraps }
}

/// The `Color` a cell's ANSI colour means here.
fn colour_of(colour: AnsiColor, default: Color) -> Color {
    match colour {
        AnsiColor::Named(named) => named_colour(named, default),
        AnsiColor::Spec(rgb) => Color::srgb_u8(rgb.r, rgb.g, rgb.b),
        AnsiColor::Indexed(index) => palette::resolve(index),
    }
}

/// A named colour, through the same palette table as an index — which is what makes the shell's
/// `ESC[31m` and `ESC[38;5;1m` the same red, as they are in every terminal.
fn named_colour(named: NamedColor, default: Color) -> Color {
    match named {
        NamedColor::Foreground | NamedColor::BrightForeground => palette::default_fg(),
        NamedColor::Background => palette::default_bg(),
        NamedColor::Cursor => default,
        // The dim ramp is the normal sixteen at half strength, which is what `ESC[2m` asks for; the
        // pane mixes rather than storing it, so this is only reached by a program that named it.
        NamedColor::DimBlack => palette::resolve(0).mix(&palette::default_bg(), 0.4),
        NamedColor::DimRed => palette::resolve(1).mix(&palette::default_bg(), 0.4),
        NamedColor::DimGreen => palette::resolve(2).mix(&palette::default_bg(), 0.4),
        NamedColor::DimYellow => palette::resolve(3).mix(&palette::default_bg(), 0.4),
        NamedColor::DimBlue => palette::resolve(4).mix(&palette::default_bg(), 0.4),
        NamedColor::DimMagenta => palette::resolve(5).mix(&palette::default_bg(), 0.4),
        NamedColor::DimCyan => palette::resolve(6).mix(&palette::default_bg(), 0.4),
        NamedColor::DimWhite => palette::resolve(7).mix(&palette::default_bg(), 0.4),
        NamedColor::DimForeground => palette::default_fg().mix(&palette::default_bg(), 0.4),
        // The sixteen named slots, in the enum's own order: `NamedColor`'s discriminants 0–15 are
        // exactly the ANSI indices, which `palette::resolve` takes.
        other => palette::resolve(other as u8),
    }
}

/// A named colour's bright slot, for `ESC[1m31m`.
fn brightened(colour: AnsiColor, otherwise: Color) -> Color {
    match colour {
        AnsiColor::Named(NamedColor::Black) => palette::resolve(8),
        AnsiColor::Named(NamedColor::Red) => palette::resolve(9),
        AnsiColor::Named(NamedColor::Green) => palette::resolve(10),
        AnsiColor::Named(NamedColor::Yellow) => palette::resolve(11),
        AnsiColor::Named(NamedColor::Blue) => palette::resolve(12),
        AnsiColor::Named(NamedColor::Magenta) => palette::resolve(13),
        AnsiColor::Named(NamedColor::Cyan) => palette::resolve(14),
        AnsiColor::Named(NamedColor::White) => palette::resolve(15),
        // An indexed or 24-bit colour has no bright slot of its own; the standard says bold is a
        // no-op there, and inventing a lighter version would make `ESC[1m` change a colour the
        // program chose deliberately.
        _ => otherwise,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A blank screen is `rows` rows of nothing, and it says so rather than being empty.
    #[test]
    fn a_fresh_screen_has_a_row_per_line() {
        let mut screen = TerminalScreen::new(20, 5);
        assert_eq!(screen.lines().len(), 5);
        assert!(screen.lines().iter().all(|l| l.cells.is_empty()));
        assert_eq!(screen.cursor().row, 0);
    }

    /// **The colour path, end to end**: an escape sequence becomes a `Color`, and the pane never sees
    /// an index.
    #[test]
    fn an_escape_sequence_becomes_a_bevy_colour() {
        let mut screen = TerminalScreen::new(40, 3);
        screen.feed(b"\x1b[31mR\x1b[0mP");
        let row = &screen.lines()[0];
        assert_eq!(row.cells.len(), 2, "two characters were written: {:?}", row.cells);
        assert_eq!(row.cells[0].ch, 'R');
        assert_eq!(row.cells[0].fg, palette::resolve(1), "ESC[31m is ANSI red");
        assert_eq!(row.cells[1].fg, palette::default_fg(), "ESC[0m is the default again");
    }

    /// Bold on a named colour is its bright slot, which is what makes `ls`'s directories readable.
    #[test]
    fn bold_lifts_a_named_colour_to_its_bright_slot() {
        let mut screen = TerminalScreen::new(20, 2);
        screen.feed(b"\x1b[1;34mB");
        assert_eq!(screen.lines()[0].cells[0].fg, palette::resolve(12), "bold blue is bright blue");
    }

    /// Inverse swaps, which is how a selection or a TUI's highlighted row is drawn.
    #[test]
    fn the_inverse_attribute_swaps_the_two_colours() {
        let mut screen = TerminalScreen::new(20, 2);
        screen.feed(b"\x1b[7mI\x1b[0m");
        let cell = screen.lines()[0].cells[0];
        assert_eq!(cell.fg, palette::default_bg());
        assert_eq!(cell.bg, palette::default_fg());
    }

    /// A row of spaces is trimmed to nothing, so a still screen is a handful of entities.
    #[test]
    fn trailing_blanks_are_trimmed() {
        let mut screen = TerminalScreen::new(80, 2);
        screen.feed(b"hi");
        assert_eq!(screen.lines()[0].cells.len(), 2);
    }

    /// A program that asks the terminal a question gets an answer, or it waits forever.
    #[test]
    fn a_device_status_request_is_answered() {
        let mut screen = TerminalScreen::new(80, 5);
        screen.feed(b"\x1b[6n");
        let replies = screen.take_replies();
        assert!(!replies.is_empty(), "ESC[6n (report cursor position) must produce a reply");
        assert_eq!(replies.first(), Some(&0x1b), "the reply is itself an escape sequence");
        assert!(screen.take_replies().is_empty(), "a reply is written once");
    }
}
