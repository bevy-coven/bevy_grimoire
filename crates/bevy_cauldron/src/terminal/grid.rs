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
use alacritty_terminal::index::{Column, Direction, Line as TermLine, Point};
use alacritty_terminal::term::search::{Match, RegexIter, RegexSearch};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape as AnsiCursorShape, CursorStyle, NamedColor, Processor};
use bevy::prelude::*;

use super::input::{MouseEncoding, MouseTracking, WheelTarget};
use super::palette;
use crate::chrome;

/// How many lines of history a session keeps above its viewport.
pub const SCROLLBACK: usize = 5_000;

/// **The most matches a search keeps: the newest this many** (#321). VS Code's terminal stops counting
/// at a thousand too. Past that, `n of 1000+` is the honest answer, and a ground on every `e` in five
/// thousand lines is noise rather than help.
pub const FIND_MATCH_CAP: usize = 1_000;

/// **What a search says about itself**, for the find box's `n of m`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FindStatus {
    /// The current match, counted from the oldest and from 1; 0 when nothing matched.
    pub current: usize,
    /// How many matches are kept: all of them, or the newest [`FIND_MATCH_CAP`].
    pub total: usize,
    /// More matched than [`FIND_MATCH_CAP`], so the oldest are not counted.
    pub capped: bool,
}

/// **An open search** over the whole grid, scrollback and screen.
///
/// Matches are the grid's own points, and a point names a line by its distance from the top of the
/// screen, so output that scrolls the screen moves every match by a line. Rather than patch them,
/// new output marks the search `stale` and the next rebuild searches again — one pass of
/// alacritty's DFA over the grid, and only while a search is open.
struct Search {
    /// `None` when the query could not be made a pattern, which was said when it happened.
    regex: Option<RegexSearch>,
    /// Oldest first.
    matches: Vec<Match>,
    capped: bool,
    current: Option<usize>,
    stale: bool,
}

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

    /// **Does this cell paint a ground of its own?** False for the pane's default background, which
    /// the pane itself already is; true for a coloured background, an inverse cell and a selected one
    /// — the cells the view must draw a ground behind (#309).
    pub fn has_ground(&self) -> bool {
        self.bg != palette::default_bg()
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
    /// The shape the child asked for (#320).
    pub shape: CaretShape,
}

/// **The caret's shape**, as a program asks for it with DECSCUSR (`ESC[<n> q`): 1 and 2 a block, 3 and
/// 4 an underline, 5 and 6 a bar; 0 the terminal's default, which is a block, as VS Code's and xterm's
/// are (#320). vim asks for a block in normal mode and a bar in insert mode, which is how its user
/// knows which mode they are in. Whether the shape blinks is not carried: the pane blinks every caret.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CaretShape {
    #[default]
    Block,
    Underline,
    Bar,
}

/// **What the child wrote to the terminal**, collected rather than acted on here.
///
/// `Term` answers some escape sequences by *writing back* — a cursor-position report (`ESC[6n`), a
/// device-attributes reply, an `OSC 52` clipboard store. Those replies must reach the shell or a
/// program that asked will wait forever: `tput cols` is exactly this shape. So the listener collects
/// them and [`TerminalScreen::take_replies`] hands them to whoever owns the write end.
///
/// `Event::ClipboardLoad` carries a closure this type deliberately never calls: a paste request from
/// the shell would need the pane's clipboard, and the pane's paste is the person's Cmd+V (Ctrl+Shift+V).
/// Leaving the closure uncalled means the shell is told nothing, which is the honest answer for a
/// terminal that does not offer the shell its clipboard.
#[derive(Clone, Default)]
pub struct TerminalListener {
    replies: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}

impl EventListener for TerminalListener {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event
            && let Ok(mut replies) = self.replies.lock()
        {
            replies.extend_from_slice(text.as_bytes());
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
    /// **The ink the rows were resolved in** ([`chrome::ink_generation`], #428). A cell stores the concrete
    /// colour a program's "default" or "bright black" meant when it was built, so a theme change leaves the
    /// rows in the old theme's colours until something redraws them; [`lines`](Self::lines) rebuilds when this
    /// is not the ink's own.
    ink_generation: u64,
    /// What [`lines`](Self::lines) last built, so a rebuild happens once per change rather than once
    /// per frame — and so a caller can hold a row across frames without re-asking.
    rows: Vec<Line>,
    /// **Which build of `rows` this is**, unique across every screen in the process — see
    /// [`stamp`](Self::stamp).
    stamp: u64,
    /// The caret's place, cached beside `rows` for the same reason.
    caret: Cursor,
    cols: u16,
    rows_count: u16,
    /// The search the find box has open, if any (#321).
    search: Option<Search>,
    /// The caret the person's setting gives when the program asks for none (#324).
    default_caret: CaretShape,
    /// How many lines of history this screen keeps: [`SCROLLBACK`] unless the person set another.
    scrollback: usize,
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
        let config = screen_config(CaretShape::Block, SCROLLBACK);
        let term = Term::new(config, &Size { cols: cols as usize, rows: rows as usize }, listener.clone());
        TerminalScreen {
            term,
            parser: Processor::new(),
            listener,
            dirty: true,
            ink_generation: chrome::ink_generation(),
            rows: Vec::new(),
            stamp: 0,
            caret: Cursor { row: 0, column: 0, visible: true, shape: CaretShape::Block },
            cols,
            rows_count: rows,
            search: None,
            default_caret: CaretShape::Block,
            scrollback: SCROLLBACK,
        }
    }

    /// **The caret when the program asks for none** (#324): the person's setting, handed to the
    /// emulator as its default style, so `ESC[0 q` (a program putting the default back) is this too.
    /// A shape a program asked for is the program's and stays.
    pub fn set_default_caret(&mut self, shape: CaretShape) {
        if self.default_caret == shape {
            return;
        }
        self.default_caret = shape;
        self.term.set_options(screen_config(shape, self.scrollback));
        self.dirty = true;
    }

    /// **How many lines of history this screen keeps** (#324, the person's `terminal_scrollback`). Fewer
    /// than it holds drops the oldest at once; more keeps everything and grows from here.
    pub fn set_scrollback(&mut self, lines: usize) {
        if self.scrollback == lines {
            return;
        }
        self.scrollback = lines;
        self.term.set_options(screen_config(self.default_caret, lines));
        self.dirty = true;
        self.search_again();
    }

    /// How many lines of history this screen keeps at most.
    pub fn scrollback(&self) -> usize {
        self.scrollback
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
    ///
    /// **The parser drives the grid in place.** `parser` and `term` are two fields, so borrowing both
    /// at once is allowed; an earlier version swapped `term` out for a freshly built `Term` around the
    /// call, which allocated a whole grid per feed — measured 2026-11-06 on big at 35 µs a feed on a
    /// 200 × 50 pane, a hundred times the cost of the one echoed byte a keystroke feeds (#308).
    pub fn feed(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.parser.advance(&mut self.term, bytes);
        self.dirty = true;
        self.search_again();
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
        self.search_again();
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

    /// **Back to the prompt**: the viewport at the foot of the scrollback, where the shell is writing.
    ///
    /// Called before input reaches the shell (#319), because a person scrolled up to read who then
    /// types is typing at a prompt they cannot see. VS Code does the same
    /// (`terminal.integrated.scrollOnUserInput`). A view already at the foot is left alone and not
    /// marked dirty, so typing costs no rebuild of its own.
    pub fn scroll_to_bottom(&mut self) {
        if self.term.grid().display_offset() != 0 {
            self.term.scroll_display(Scroll::Bottom);
            self.dirty = true;
        }
    }

    /// **Search the whole grid for `query`, as plain text** (#321), make the newest match current and
    /// scroll it into view, which is where VS Code's terminal starts: what is wanted is usually the
    /// last error, not the first.
    ///
    /// Plain text, because a person typing `a.b` or `f(x` is looking for those characters: every
    /// pattern character is escaped before alacritty's regex search sees it. **Smart case** comes with
    /// that search: a query in lower case matches either case, one with a capital only its own. An
    /// empty query closes the search.
    pub fn find(&mut self, query: &str) {
        if query.is_empty() {
            self.find_clear();
            return;
        }
        let regex = match RegexSearch::new(&literal_pattern(query)) {
            Ok(regex) => Some(regex),
            Err(why) => {
                warn!("the terminal cannot search for {query:?}: {why}");
                None
            }
        };
        self.search = Some(Search { regex, matches: Vec::new(), capped: false, current: None, stale: true });
        self.research();
        if let Some(search) = self.search.as_mut() {
            search.current = search.matches.len().checked_sub(1);
        }
        self.reveal_current();
        self.dirty = true;
    }

    /// **The next match**: `older` walks up into older output (Enter in the find box), otherwise down
    /// (Shift+Enter). Past the last match either way it wraps round, as VS Code's find does.
    pub fn find_next(&mut self, older: bool) {
        if self.search.as_ref().is_some_and(|search| search.stale) {
            self.research();
        }
        let Some(search) = self.search.as_mut() else {
            return;
        };
        let total = search.matches.len();
        if total == 0 {
            return;
        }
        let at = search.current.unwrap_or(total - 1);
        search.current = Some(if older { (at + total - 1) % total } else { (at + 1) % total });
        self.reveal_current();
        self.dirty = true;
    }

    /// Close the search: no match is drawn and nothing is counted.
    pub fn find_clear(&mut self) {
        if self.search.take().is_some() {
            self.dirty = true;
        }
    }

    /// Which match is current and how many there are, or `None` with no search open.
    pub fn find_status(&self) -> Option<FindStatus> {
        let search = self.search.as_ref()?;
        Some(FindStatus {
            current: search.current.map_or(0, |at| at + 1),
            total: search.matches.len(),
            capped: search.capped,
        })
    }

    /// Output arrived or the grid re-wrapped: the matches' points no longer name their text.
    fn search_again(&mut self) {
        if let Some(search) = self.search.as_mut() {
            search.stale = true;
        }
    }

    /// **Find every match again**, oldest first, keeping the newest [`FIND_MATCH_CAP`]. The current
    /// match keeps its place in the list, which is the same text while output only adds lines below.
    fn research(&mut self) {
        let Some(search) = self.search.as_mut() else {
            return;
        };
        search.stale = false;
        search.matches.clear();
        search.capped = false;
        let Some(regex) = search.regex.as_mut() else {
            search.current = None;
            return;
        };
        let start = Point::new(self.term.topmost_line(), Column(0));
        let end = Point::new(self.term.bottommost_line(), self.term.last_column());
        for found in RegexIter::new(start, end, Direction::Right, &self.term, regex) {
            search.matches.push(found);
            // Dropped a block at a time rather than one per match, so a query that matches every line
            // of a full scrollback does not shift the list five thousand times.
            if search.matches.len() == 2 * FIND_MATCH_CAP {
                search.matches.drain(..FIND_MATCH_CAP);
                search.capped = true;
            }
        }
        if search.matches.len() > FIND_MATCH_CAP {
            let over = search.matches.len() - FIND_MATCH_CAP;
            search.matches.drain(..over);
            search.capped = true;
        }
        search.current = search.current.and_then(|at| search.matches.len().checked_sub(1).map(|last| at.min(last)));
    }

    /// Scroll the current match into view.
    fn reveal_current(&mut self) {
        let point = self
            .search
            .as_ref()
            .and_then(|search| search.current.and_then(|at| search.matches.get(at)))
            .map(|found| *found.start());
        if let Some(point) = point {
            self.term.scroll_to_point(point);
        }
    }

    /// Was anything drawn since the last time the pane asked?
    pub fn take_dirty(&mut self) -> bool {
        std::mem::replace(&mut self.dirty, false)
    }

    /// **The rows the pane draws**, rebuilt if anything changed.
    ///
    /// Returned by reference so a still terminal hands back the same `Vec` sixty times a second.
    pub fn lines(&mut self) -> &[Line] {
        let ink = chrome::ink_generation();
        if self.ink_generation != ink {
            self.ink_generation = ink;
            self.dirty = true;
        }
        if self.take_dirty() {
            self.rebuild();
        }
        &self.rows
    }

    /// **The rows as [`lines`](Self::lines) last built them**, without rebuilding — for a reader that
    /// already asked `lines` this frame and needs the rows beside other borrows of the screen.
    pub fn built_lines(&self) -> &[Line] {
        &self.rows
    }

    /// **Which build of the rows [`built_lines`](Self::built_lines) holds.**
    ///
    /// A new number on every rebuild, and never the same number on two screens, so a view that
    /// remembers the stamp it drew knows — with one comparison, and across a switch of tabs — that the
    /// rows have not changed and that there is nothing to copy or compare (#308).
    pub fn stamp(&self) -> u64 {
        self.stamp
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

    /// **Select everything the shell drew** (feedback-1794024642593): every line of the scrollback and the
    /// screen, from the first down to the last one that holds anything, which is what Cmd+A selects in a
    /// Mac terminal. The blank rows under the prompt are left out, so a copy has no tail of empty lines. A
    /// blank terminal has nothing to select.
    pub fn select_all(&mut self) {
        use alacritty_terminal::index::Side;
        use alacritty_terminal::selection::{Selection, SelectionType};
        let grid = self.term.grid();
        let top = grid.topmost_line();
        let mut bottom = grid.bottommost_line();
        while bottom > top && grid[bottom].is_clear() {
            bottom = TermLine(bottom.0 - 1);
        }
        let last_column = grid.last_column();
        let selection = (!grid[bottom].is_clear()).then(|| {
            let mut selection = Selection::new(SelectionType::Simple, Point::new(top, Column(0)), Side::Left);
            selection.update(Point::new(bottom, last_column), Side::Right);
            selection
        });
        self.term.selection = selection;
        self.dirty = true;
    }

    /// **The address under a cell** of the rows the pane draws (#325), or `None`.
    pub fn link_at(&self, row: usize, col: usize) -> Option<String> {
        super::links::link_at(&self.rows, self.cols as usize, row, col)
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
        self.term.grid_mut().update_history(self.scrollback);
        self.dirty = true;
        self.search_again();
    }

    /// **Clear the terminal, keeping the prompt** (#326): what xterm.js's `clear()` does for VS Code's
    /// Cmd+K. The cursor's line, where the shell's prompt is, becomes the first line, and every line
    /// above it, on screen or in the scrollback, goes. Done in the emulator rather than by asking the
    /// shell: a shell asked to clear (Ctrl+L) clears with `ESC[2J`, which alacritty, like every terminal
    /// that keeps history, moves into the scrollback it was meant to empty.
    pub fn clear_keeping_prompt(&mut self) {
        self.scroll_to_bottom();
        let row = self.term.grid().cursor.point.line.0;
        if row > 0 {
            // Scroll the screen up by the cursor's row and move the cursor with it: the prompt's line
            // is the top one and the cursor is still on it.
            let keep_prompt = format!("\x1b[{row}S\x1b[{row}A");
            self.parser.advance(&mut self.term, keep_prompt.as_bytes());
        }
        self.clear_scrollback();
    }

    /// **How many lines of history the session is holding above its viewport.**
    ///
    /// The measurable fact that a scrollback exists and how much of it there is — used by the menu's
    /// Clear to prove it cleared something, and by nothing else. Zero on the alternate screen, which
    /// has no history.
    pub fn scrolled_lines(&self) -> usize {
        self.term.grid().history_size()
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

    /// **Did the child ask for application cursor keys?** (DECCKM, `ESC[?1h`; ncurses' `keypad()`.)
    /// The key path spells Up as `ESC O A` while it holds (#311).
    pub fn app_cursor(&self) -> bool {
        self.term.mode().contains(TermMode::APP_CURSOR)
    }

    /// **Which mouse events the child asked for, and in what encoding**, or `None` when it asked for
    /// none — the pane then keeps the mouse for selecting and scrolling (#313).
    pub fn mouse_reporting(&self) -> Option<(MouseTracking, MouseEncoding)> {
        let mode = self.term.mode();
        let tracking = if mode.contains(TermMode::MOUSE_MOTION) {
            MouseTracking::Motion
        } else if mode.contains(TermMode::MOUSE_DRAG) {
            MouseTracking::Drag
        } else if mode.contains(TermMode::MOUSE_REPORT_CLICK) {
            MouseTracking::Click
        } else {
            return None;
        };
        let encoding = if mode.contains(TermMode::SGR_MOUSE) {
            MouseEncoding::Sgr
        } else if mode.contains(TermMode::UTF8_MOUSE) {
            MouseEncoding::Utf8
        } else {
            MouseEncoding::Legacy
        };
        Some((tracking, encoding))
    }

    /// **Where a wheel notch over this screen goes** (#312, #313): to the child as a mouse report if
    /// it asked for the mouse; as cursor keys on the alternate screen, which has no scrollback, while
    /// alternate scroll (mode 1007, on by default) holds; else to the pane's own scrollback.
    pub fn wheel_target(&self) -> WheelTarget {
        if let Some((_, encoding)) = self.mouse_reporting() {
            return WheelTarget::Report(encoding);
        }
        let mode = self.term.mode();
        if mode.contains(TermMode::ALT_SCREEN) && mode.contains(TermMode::ALTERNATE_SCROLL) {
            WheelTarget::Arrows
        } else {
            WheelTarget::Scrollback
        }
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
        if self.search.as_ref().is_some_and(|search| search.stale) {
            self.research();
        }
        let offset = self.term.grid().display_offset() as i32;
        let cols = self.cols as usize;
        let fg_default = palette::default_fg();
        let bg_default = palette::default_bg();

        let mut rows: Vec<Line> = Vec::with_capacity(self.rows_count as usize);
        let mut row: Vec<Cell> = Vec::with_capacity(cols);
        let mut wrapped = false;
        let mut open_line = i32::MIN;

        // **The selection, in the grid's own points**, so a selected cell is known in the same walk.
        // A selection is drawn as its cells' ground ([`palette::selection_bg`]) over their own ink —
        // the drag already set it on `term`, and Copy already reads it; until #309 the screen never
        // showed it, so a person selected blind.
        let selection = self.term.selection.as_ref().and_then(|s| s.to_range(&self.term));
        // **The search's matches that touch the viewport**, and which one is current (#321). A few at
        // most, so each cell asks a short list; with no search open this is an empty `Vec`, which does
        // not allocate.
        let (top, bottom) = (-offset, self.rows_count as i32 - 1 - offset);
        let found: Vec<(Match, bool)> = self
            .search
            .as_ref()
            .map(|search| {
                search
                    .matches
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| m.end().line.0 >= top && m.start().line.0 <= bottom)
                    .map(|(at, m)| (m.clone(), search.current == Some(at)))
                    .collect()
            })
            .unwrap_or_default();
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
            // **A wide character's second column is a cell too, drawn blank.** The terminal's face is
            // monospace and holds no glyph wider than one cell (a CJK character or an emoji draws as its
            // one-cell missing-glyph box), so the blank is what fills the character's two columns. Leaving
            // it out drew everything after a wide character one cell left of its column, so the caret and
            // a press stood one cell off per wide character (terminal screenshots on mac_air, 2026-11-06).
            // The blank takes the cell's own colours below, so an inverse or selected wide character is
            // grounded across both of its columns.
            let spacer = cell.flags.contains(Flags::WIDE_CHAR_SPACER) || cell.flags.contains(Flags::LEADING_WIDE_CHAR_SPACER);
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
            if let Some((_, current)) = found.iter().find(|(m, _)| m.contains(&indexed.point)) {
                bg = if *current { palette::find_current_bg() } else { palette::find_bg() };
            }
            if selection.as_ref().is_some_and(|range| range.contains(indexed.point)) {
                bg = palette::selection_bg();
            }
            // **A tab's cell is drawn blank** (feedback-1794062374181). The emulator writes the tab character
            // itself into the cell a tab starts from (`alacritty_terminal`'s `put_tab`), so that a copy keeps
            // the tab. The face has no glyph for it, so drawn as it is it was a missing-glyph box after every
            // name `ls` lined up with tabs. Alacritty's own renderer draws that cell as a space for the same
            // reason (alacritty/alacritty#991). The grid keeps the tab, so a copy still holds it.
            let blank = spacer || cell.c == '\t';
            row.push(Cell { ch: if blank { ' ' } else { cell.c }, fg, bg });
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
        // **The shape is the style the program asked for**, read apart from visibility: a program
        // that hides the caret while it paints keeps the shape it chose for when it shows it again.
        let shape = match self.term.cursor_style().shape {
            AnsiCursorShape::Underline => CaretShape::Underline,
            AnsiCursorShape::Beam => CaretShape::Bar,
            AnsiCursorShape::Block | AnsiCursorShape::HollowBlock | AnsiCursorShape::Hidden => CaretShape::Block,
        };
        self.caret = Cursor {
            row,
            column: point.column.0.min(cols.saturating_sub(1)),
            visible: content.cursor.shape != AnsiCursorShape::Hidden,
            shape,
        };
        self.rows = rows;
        self.stamp = NEXT_STAMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The next build's [`TerminalScreen::stamp`]. Starts at 1, so a view that has drawn nothing (stamp 0)
/// never matches a built screen.
static NEXT_STAMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// **The emulator's settings for one screen**: the bounded scrollback, and the caret it draws when the
/// program asks for none (#324). One place, so a screen built and a screen re-set agree.
fn screen_config(caret: CaretShape, scrollback: usize) -> Config {
    let shape = match caret {
        CaretShape::Block => AnsiCursorShape::Block,
        CaretShape::Underline => AnsiCursorShape::Underline,
        CaretShape::Bar => AnsiCursorShape::Beam,
    };
    Config {
        scrolling_history: scrollback,
        default_cursor_style: CursorStyle { shape, blinking: false },
        ..Default::default()
    }
}

/// **A query as a pattern that matches only its own characters.** Every character regex syntax gives a
/// meaning is escaped — the set `regex_syntax::is_meta_character` names, which is also every escape
/// that syntax accepts — so `a.b` finds a dot and `f(x` an unbalanced bracket rather than an error.
fn literal_pattern(query: &str) -> String {
    let mut pattern = String::with_capacity(query.len() * 2);
    for c in query.chars() {
        if matches!(
            c,
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' | '#' | '&' | '-' | '~'
        ) {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    pattern
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

    /// **A row holds one cell per column, wide characters included** (terminal screenshots, 2026-11-06).
    /// `日` takes two columns in the grid, and its second is a spacer. The terminal's face has no glyph for
    /// it, so it draws one cell wide; leaving the spacer out drew everything after it one cell left of its
    /// column, and the caret after `日本語 テスト 🎉 end` stood seven cells right of the text.
    #[test]
    fn a_wide_character_keeps_its_second_column() {
        let mut screen = TerminalScreen::new(40, 2);
        screen.feed("日本 ok".as_bytes());
        let row = &screen.lines()[0];
        let text: String = row.cells.iter().map(|c| c.ch).collect();
        assert_eq!(text, "日 本  ok", "two wide characters are four columns, then a space and `ok`: {text:?}");
        assert_eq!(row.cells[5].ch, 'o', "the `o` is in column 5, where the shell's cursor arithmetic put it");
        assert_eq!(screen.cursor().column, 7, "and the cursor is after the `k`, in column 7");
    }

    /// **A tab's gap is blank** (feedback-1794062374181). The emulator writes the tab character itself into the
    /// cell a tab starts from, so that a copy keeps the tab; drawn, that character is the face's missing-glyph
    /// box, which put a `?` after every name `ls` lined up with tabs. The drawn row holds blanks there, and a
    /// copy of it still holds the tabs the program wrote.
    #[test]
    fn a_tab_draws_blank_columns_and_copies_as_a_tab() {
        let mut screen = TerminalScreen::new(40, 2);
        screen.feed(b"a\tb\tc|");
        let text: String = screen.lines()[0].cells.iter().map(|c| c.ch).collect();
        assert_eq!(text, "a       b       c|", "`b` stands at the tab stop in column 8 and `c` at 16, blanks between: {text:?}");
        screen.select((0, 0), (0, 17));
        assert_eq!(screen.selection_text().as_deref(), Some("a\tb\tc|"), "a copy keeps the tabs the program wrote");
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
