//! **A shell in the editor's window.**
//!
//! `Ctrl+\`` opens a dock across the bottom of the Cauldron: a real login shell (`$SHELL`, so `fish`,
//! `zsh` or `bash` as the person configured it), in a real PTY, parsed by a real VTE parser, drawn in
//! the editor's own ink. `cargo`, `git`, `hs`, the Wiggum driver — everything the person would
//! otherwise alt-tab for — runs in it, against the open project's directory.
//!
//! # Why a terminal, when the editor already has an agent door
//!
//! **An agent never needed a shell to drive this editor.** `bevy_cauldron_mcp` publishes typed tools
//! and BRP carries the same operation registry through `ops::dispatch`, so an agent's reach is
//! already better than a keystroke stream. The terminal's justification is the *person's own* work:
//! reading `cargo`'s error, running `git log`, watching the loop's next pass. Those are not
//! operations, and wrapping each in a typed tool would be inventing a shell badly.
//!
//! # The one research finding that shaped the pane
//!
//! SWE-agent ([arXiv 2405.15793](https://arxiv.org/abs/2405.15793)) measured that terminal-style
//! interactive affordances are *"verbose and costly"* and that environment feedback *"should be
//! informative but concise"*. That is why the pane is a compact dock rather than a workspace-wide
//! view, why it survives a narrow window instead of clipping, and why its scrollback is bounded and
//! stated rather than unbounded.
//!
//! # The engine and the pane
//!
//! | file | what it may touch |
//! |---|---|
//! | [`pty`] | a file descriptor and a child process — no Bevy |
//! | [`grid`] | `alacritty_terminal` and `vte` — no Bevy beyond `Color` |
//! | [`palette`] | 256 indices → [`bevy::prelude::Color`] |
//! | this file | the sessions as one Bevy [`Resource`], and the four operations the pane calls |
//!
//! Above [`Terminal`] nothing knows that a terminal has a parser or a PID; `terminal_pane` and
//! `terminal_view` know about `Node`s and nothing about `alacritty_terminal`. That is the same
//! boundary `project_assets` keeps for the asset server, and it is what makes the engine provable
//! with no window at all ([`tests`]).

mod grid;
pub mod input;
mod palette;
mod pty;

#[cfg(test)]
mod tests;

#[cfg(all(test, unix))]
mod flow_tests;

#[cfg(test)]
mod input_tests;

use std::path::{Path, PathBuf};

use bevy::prelude::*;

pub use grid::{Cell, Cursor, Line, SCROLLBACK};
pub use pty::TerminalSession;

/// **Every shell the editor is running, and which one the pane is looking at.**
///
/// A `Resource` and not a set of entities: a terminal that the door sweep despawned (a `Node` root,
/// as `crate::screen::scene_roots` finds them) must not take its shell with it — the person's `cargo
/// build` is in that PTY. The pane is a `Node` and is swept; this is not, and is not.
///
/// Registered by [`TerminalPlugin`] at `Startup`, so it outlives every door.
#[derive(Resource, Default)]
pub struct Terminal {
    open: Vec<TerminalTab>,
    active: usize,
    /// The last refusal, kept so the pane can draw it after the call that produced it returned.
    problem: Option<String>,
    /// The pane's height in logical pixels, remembered for the session (`recent.ron`'s
    /// `terminal_h`), so the dock opens where the person left it.
    height: f32,
    /// Where the *next* shell starts. Set from the open project and from a `cd` chip.
    next_cwd: Option<PathBuf>,
    /// **The person collapsed the pane** (Primary+J, or the chord that lowers it), and every screen's
    /// pane is built and kept collapsed until they restore it (#315). `false` at launch: the terminal is
    /// open on every screen from the application's first frame (#314, the owner's ask of 2026-11-06).
    /// Kept here, on the session's one resource, because a pane is rebuilt on every screen and the
    /// person's choice must outlive it.
    collapsed: bool,
}

/// One shell, its screen, and what is drawn about it.
pub struct TerminalTab {
    /// The title a program asked for (`OSC 2`), or the shell's own name. The bar shows it.
    title: String,
    /// **The directory this tab's shell has been told to move to**, when the `cd` chip was taken.
    ///
    /// A terminal cannot know where a shell *is*: `cd` is the shell's own state, and reading it back
    /// means OSC 7 (which a shell only emits if it is configured to) or `/proc/<pid>/cwd` (Linux only,
    /// and a second source of truth beside the shell's). So the pane records what it *told* the shell
    /// and offers the chip again only when the project moves somewhere it has not been told about.
    /// `None` means nothing has been offered for this tab yet.
    cd_offered: Option<PathBuf>,
    session: Option<TerminalSession>,
    screen: grid::TerminalScreen,
    /// Why this tab has no shell, when it has none: the pane draws it in red and keeps the tab
    /// usable for a second attempt, which is what makes a missing `/bin/sh` a message rather than a
    /// dead panel.
    problem: Option<String>,
}

impl TerminalTab {
    /// A tab for a shell that could not start.
    fn refused(cols: u16, rows: u16, why: String) -> TerminalTab {
        TerminalTab {
            title: "terminal".to_owned(),
            cd_offered: None,
            session: None,
            screen: grid::TerminalScreen::new(cols, rows),
            problem: Some(why),
        }
    }
}

impl Terminal {
    /// **Open a shell**, or return the one already open.
    ///
    /// The cwd is the open project's directory when there is one — [`crate::project_assets::ProjectRoot`]
    /// is what the asset server reads, so the shell and the editor are looking at the same files.
    pub fn spawn(&mut self, cols: u16, rows: u16, cwd: &Path) -> usize {
        match TerminalSession::spawn(cols, rows, cwd) {
            Ok(session) => {
                let title = shell_title(cwd);
                let mut screen = grid::TerminalScreen::new(cols, rows);
                screen.resize(cols, rows);
                self.open.push(TerminalTab {
                    title,
                    cd_offered: None,
                    session: Some(session),
                    screen,
                    problem: None,
                });
                self.active = self.open.len() - 1;
                self.problem = None;
                self.active
            }
            Err(why) => {
                self.open.push(TerminalTab::refused(cols, rows, why.clone()));
                self.active = self.open.len() - 1;
                self.problem = Some(why);
                self.active
            }
        }
    }

    /// How many shells are in the bar.
    pub fn len(&self) -> usize {
        self.open.len()
    }

    /// Is there nothing open at all?
    pub fn is_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// Which tab the pane draws.
    pub fn active(&self) -> usize {
        self.active
    }

    /// Look at another tab.
    pub fn select(&mut self, index: usize) {
        if index < self.open.len() {
            self.active = index;
            self.problem = None;
        }
    }

    /// The tab the pane draws, if there is one.
    pub fn tab(&self) -> Option<&TerminalTab> {
        self.open.get(self.active)
    }

    /// The tab the pane draws, mutably.
    pub fn tab_mut(&mut self) -> Option<&mut TerminalTab> {
        self.open.get_mut(self.active)
    }

    /// Every tab, for the bar.
    pub fn tabs(&self) -> &[TerminalTab] {
        &self.open
    }

    /// **Close the active tab, and with it the shell.** `Drop` on the session hangs the child up.
    pub fn close_active(&mut self) {
        if self.open.is_empty() {
            return;
        }
        self.open.remove(self.active);
        self.active = self.active.min(self.open.len().saturating_sub(1));
        self.problem = None;
    }

    /// **Close the tab at `index`** — what the bar's `×` on a chip does.
    pub fn close(&mut self, index: usize) {
        if index >= self.open.len() {
            return;
        }
        self.active = index;
        self.close_active();
    }

    /// The last thing that went wrong, for the pane to draw.
    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    /// Record a refusal that is not one tab's — a resize that failed, say.
    pub fn refuse(&mut self, why: String) {
        self.problem = Some(why);
    }

    /// Forget the standing refusal.
    pub fn clear_problem(&mut self) {
        self.problem = None;
    }

    /// The pane's height, logical pixels: the person's, or [`crate::terminal_pane::TERMINAL_DEFAULT_H`].
    pub fn height(&self) -> f32 {
        self.chosen_height().unwrap_or(crate::terminal_pane::TERMINAL_DEFAULT_H)
    }

    /// **The height the person dragged the pane to**, if they have; `None` until they do.
    pub fn chosen_height(&self) -> Option<f32> {
        (self.height > 0.0).then_some(self.height)
    }

    /// **The height a pane opens at in a window `window_h` logical pixels tall** (#314): the person's, or
    /// [`crate::terminal_pane::TERMINAL_WINDOW_SHARE`] of the window clamped to the terminal's range.
    ///
    /// A share and not a fixed number because the terminal is now open on every screen from the first
    /// frame: the fixed 320 px it opened at took more than half of a 1280 x 720 window at the editor's
    /// `UiScale`, and measured in the editor's own tests, that put the home's New project button under the
    /// terminal, shrank the Place palette to 189 px and drew the map's legend over the shell's prompt.
    pub fn opening_height(&self, window_h: Option<f32>) -> f32 {
        use crate::terminal_pane::{TERMINAL_DEFAULT_H, TERMINAL_MIN_H, TERMINAL_WINDOW_SHARE};
        self.chosen_height().unwrap_or_else(|| {
            window_h.map_or(TERMINAL_DEFAULT_H, |h| (h * TERMINAL_WINDOW_SHARE).clamp(TERMINAL_MIN_H, TERMINAL_DEFAULT_H))
        })
    }

    /// Set the pane's height, clamped to what a terminal can be drawn in.
    pub fn set_height(&mut self, px: f32) {
        self.height = px.clamp(crate::terminal_pane::TERMINAL_MIN_H, crate::terminal_pane::TERMINAL_MAX_H);
    }

    /// Has the person collapsed the pane? Every screen's pane is drawn collapsed while this holds.
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Collapse the pane, or restore it, for this screen and every one after it.
    pub fn set_collapsed(&mut self, collapsed: bool) {
        self.collapsed = collapsed;
    }

    /// Where the next shell starts: the open project, or the home directory.
    ///
    /// Re-read every time a tab is added rather than captured once, because a person who opens
    /// another project and presses `+` means the new one.
    pub fn set_next_cwd(&mut self, cwd: Option<PathBuf>) {
        self.next_cwd = cwd;
    }

    /// The directory a new shell should start in.
    pub fn next_cwd(&self) -> PathBuf {
        self.next_cwd.clone().unwrap_or_else(|| {
            std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
        })
    }

    /// **Keystrokes to the active shell**, and the view back to its prompt first (#319).
    ///
    /// Everything a person sends goes through here — a key, a paste, a mouse report, the `cd` chip —
    /// and none of it should land at a prompt scrolled out of sight. The answers the emulator owes a
    /// program (`Terminal::pump`) are not input and do not come this way, so a `tput` question never
    /// moves the view; nor do the pane's own paging keys, which scroll and send nothing.
    pub fn write(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let Some(tab) = self.tab_mut() else {
            return;
        };
        let Some(session) = tab.session.as_mut() else {
            return;
        };
        tab.screen.scroll_to_bottom();
        if let Err(why) = session.write(bytes) {
            self.problem = Some(why);
        }
    }

    /// **Tell the active shell the pane changed size**, and the emulator too.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        let Some(tab) = self.tab_mut() else {
            return;
        };
        tab.screen.resize(cols, rows);
        if let Some(session) = tab.session.as_mut()
            && let Err(why) = session.resize(cols, rows)
        {
            self.problem = Some(why);
        }
    }

    /// **Read every shell that has output, and answer every question one asked.**
    ///
    /// Run once per frame by [`crate::terminal_view`]. Every tab, not just the drawn one: a `cargo
    /// build` in a background tab must not stall on a full pipe because the pane was looking
    /// elsewhere.
    pub fn pump(&mut self) {
        let mut buffer = Vec::new();
        for tab in &mut self.open {
            let Some(session) = tab.session.as_mut() else {
                continue;
            };
            buffer.clear();
            let outcome = session.drain(&mut buffer).and_then(|_| {
                if !buffer.is_empty() {
                    tab.screen.feed(&buffer);
                }
                // **A question the program asked has to be answered or it waits forever.**
                // `tput cols` is the shape of it: it writes a cursor-position request and blocks
                // on the reply.
                let replies = tab.screen.take_replies();
                if replies.is_empty() {
                    return Ok(());
                }
                session.write(&replies)
            });
            if let Err(why) = outcome {
                tab.problem = Some(why);
            }
        }
    }

}

impl TerminalTab {
    /// The shell's screen, for the view to draw and to scroll.
    pub fn screen(&self) -> &grid::TerminalScreen {
        &self.screen
    }

    /// The screen, mutably — a scroll or a selection.
    pub fn screen_mut(&mut self) -> &mut grid::TerminalScreen {
        &mut self.screen
    }

    /// The bar's label for this tab.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Why this tab has no shell, if it has none.
    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    /// Is a shell actually running in this tab?
    pub fn is_live(&self) -> bool {
        self.session.is_some()
    }

    /// The directory this shell was started in — **not** where it is now, which only the shell knows.
    pub fn cwd(&self) -> Option<&Path> {
        self.session.as_ref().map(TerminalSession::cwd)
    }

    /// **Should the bar offer to `cd` this shell to `want`?**
    ///
    /// True when the shell was started somewhere else and has not already been told to move there —
    /// what the pane knows for certain, rather than a guess at the shell's current directory.
    pub fn wants_cd_to(&self, want: &Path) -> bool {
        self.is_live() && self.cwd() != Some(want) && self.cd_offered.as_deref() != Some(want)
    }

    /// Record that this tab has been told to move to `to`, so the chip goes away.
    pub fn offered_cd(&mut self, to: PathBuf) {
        self.cd_offered = Some(to);
    }
}

/// **The last segment of a path**, which is what a bar's chip can afford: `bevy_grimoire`, not
/// `/home/ladvien/bevy_emerge_loop`.
fn shell_title(cwd: &Path) -> String {
    cwd.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| cwd.display().to_string())
}

/// **The shell's directory follows the project**, without restarting anything.
///
/// A shell started while project A was open is still in A's directory after the person opens B — that is
/// right, because the running `cargo build` in it is A's. What must not happen is the *next* shell
/// starting in the wrong place, so this reads the open root every frame it changes and offers the
/// `cd` chip ([`crate::terminal_pane`]) rather than moving a shell out from under a command.
///
/// Reads `PointedAt` — the one resource that says which project the editor is pointed at — because
/// `ProjectRoot` alone does not change when a door is re-opened on the same project.
fn follow_the_project(
    pointed: Option<Res<crate::project_assets::PointedAt>>,
    mut terminal: ResMut<Terminal>,
) {
    let Some(pointed) = pointed else {
        return;
    };
    if !pointed.is_changed() {
        return;
    }
    let root = pointed.root.clone();
    terminal.set_next_cwd(Some(root));
}

/// Registers [`Terminal`] and the systems that keep it fed.
///
/// The engine's own systems are here; the pane's are in [`crate::terminal_pane`] and
/// [`crate::terminal_view`], because the pane is a `Node` tree and this module is not.
pub struct TerminalPlugin;

impl Plugin for TerminalPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Terminal>()
            // **`PreStartup`, so the resource exists before any panel asks for it.** Several
            // systems in the editor take `Res<Terminal>` as an `Option` and one of the pane's own
            // spawners runs at `Startup`; `init_resource` at `Startup` would order by luck.
            .add_systems(PreStartup, seat_the_terminals_cwd)
            .add_systems(Update, follow_the_project)
            .add_plugins((crate::terminal_pane::TerminalPanePlugin, crate::terminal_view::TerminalViewPlugin));
    }
}

/// **A shell opens where the open project is.**
///
/// Before the first `Ctrl+\``, so the very first shell is already in the project rather than in the
/// editor's own working directory — which, for a build launched from a game's folder, is the
/// directory the game's assets live in and would have looked right by accident.
fn seat_the_terminals_cwd(mut terminal: ResMut<Terminal>, root: Option<Res<crate::project_assets::ProjectRoot>>) {
    if let Some(root) = root.as_ref().and_then(|r| r.current()) {
        terminal.set_next_cwd(Some(root));
    }
}
