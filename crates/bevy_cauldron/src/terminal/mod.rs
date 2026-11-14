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
mod links;
mod palette;
mod pty;

#[cfg(test)]
mod tests;

#[cfg(all(test, unix))]
mod flow_tests;

#[cfg(test)]
mod input_tests;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::settings::TerminalAgent;
use crate::terminal_agent::{AgentLaunch, AgentPost, LAUNCH_CAP_MS, LAUNCH_QUIET_MS};

use bevy::prelude::*;

pub use grid::{CaretShape, Cell, Cursor, FindStatus, Line, SCROLLBACK, TerminalScreen};
pub use pty::{HANGUP_GRACE_MS, ShellChoice, TERM_PROGRAM, TerminalSession, session_env};
pub(crate) use pty::shell_program_of_environment;

/// **The variable that names the open project to a shell** (#324): its root, as the editor holds it.
pub const PROJECT_ENV: &str = "CAULDRON_PROJECT";

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
    /// The pane's height in logical pixels, remembered for the session (`recent.yaml`'s
    /// `terminal_h`), so the dock opens where the person left it.
    height: f32,
    /// Where the *next* shell starts. Set from the open project and from a `cd` chip.
    next_cwd: Option<PathBuf>,
    /// **The open project's root** (#324): `CAULDRON_PROJECT` in every shell opened while a project is open, so
    /// a script or an agent run in the terminal knows which project the editor holds. `None` is no project.
    project_root: Option<PathBuf>,
    /// **The person has raised the pane** (Primary+J, the status bar's `Terminal` label, or the chord that raises it), and every
    /// screen's pane is built and kept as they left it (#315). `false` at launch: the pane is on every screen but **starts
    /// collapsed** (the owner, 2026-11-11, replacing #314's raised default), and a raise is kept across a relaunch
    /// (`settings::EditorSettings::terminal_collapsed`). Kept here, on the session's one resource, because a pane is rebuilt on
    /// every screen and the person's choice must outlive it. Stored as *raised* so `Terminal::default()` is the collapsed one.
    raised: bool,
    /// **The caret a shell gets when its program asks for none** (#324): the person's
    /// `terminal_cursor` setting, given to every open screen and to each one spawned after.
    default_caret: CaretShape,
    /// **The history each shell keeps** (#324), the person's `terminal_scrollback`; `None` until a setting
    /// names one, which is [`SCROLLBACK`].
    scrollback: Option<usize>,
    /// **The agent an `Invoke Agent` press starts** (`terminal_agent`), and a custom harness's command
    /// line: the person's settings, given by `terminal_view::take_the_terminal_settings`.
    agent: TerminalAgent,
    agent_command: Option<String>,
    /// **The shell new tabs run**, the person's `terminal_shell` and its arguments; `None` is `$SHELL`.
    shell: Option<ShellChoice>,
    /// **The settings have been given at least once.** No shell opens before: a first tab that opened
    /// a frame before the settings were read would be a plain shell for a person who chose an agent.
    settings_taken: bool,
    /// **The projects whose agent tab was closed or whose shell left**, until `terminal_agent::keep_the_resume_record`
    /// takes them: the person ended that agent, so its resume record goes. Quitting the editor adds nothing here.
    ended_agents: Vec<PathBuf>,
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
    /// **This tab's agent and its self test**, when it was opened as an agent tab.
    agent: Option<AgentPost>,
    /// The launch line, typed once the shell has gone quiet at its prompt.
    launch: Option<PendingLaunch>,
    /// **Has a person sent this shell anything?** A key, a paste, a mouse report or the `cd` chip
    /// (`Terminal::write`). A shell that leaves after that was told to (`exit`, Ctrl+D) and its tab
    /// closes; one that leaves with nothing sent never got going, and its tab stays to say so.
    typed: bool,
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
            agent: None,
            launch: None,
            typed: false,
        }
    }

    /// **A shell left on its own, before a person sent it anything**: a shell that could not stay up (a
    /// bad `terminal_shell`, an rc file that ends the session). The tab stays, with the reason in red and
    /// no session, so the bar can close it and the person can open another. Closing it here would open a
    /// fresh shell the next frame, and a shell that cannot stay up would be opened for ever.
    fn ended_on_its_own(&mut self) {
        let why = "the shell ended before anything was typed to it; close this tab and open another".to_owned();
        if self.launch.take().is_some()
            && let Some(post) = self.agent.as_mut()
        {
            post.not_started(format!("the shell stopped before the agent was typed: {why}"));
        }
        self.problem = Some(why);
        self.session = None;
    }
}

/// **An agent tab's launch line, waiting for its shell's prompt.** Typed once the shell printed
/// something and then nothing for [`LAUNCH_QUIET_MS`], or [`LAUNCH_CAP_MS`] after the tab opened
/// whatever it printed: a login shell that loads a slow rc file is still read from when it is ready,
/// because the typed bytes wait in the PTY.
struct PendingLaunch {
    bytes: Vec<u8>,
    opened: Instant,
    last_output: Option<Instant>,
}

impl PendingLaunch {
    fn is_due(&self, now: Instant) -> bool {
        now.duration_since(self.opened) >= Duration::from_millis(LAUNCH_CAP_MS)
            || self.last_output.is_some_and(|at| now.duration_since(at) >= Duration::from_millis(LAUNCH_QUIET_MS))
    }
}

impl Terminal {
    /// **Open a shell**, or return the one already open.
    ///
    /// The cwd is the open project's directory when there is one — [`crate::project_assets::ProjectRoot`]
    /// is what the asset server reads, so the shell and the editor are looking at the same files.
    pub fn spawn(&mut self, cols: u16, rows: u16, cwd: &Path) -> usize {
        self.open_tab(cols, rows, cwd, &[], None)
    }

    /// **Open an agent tab**: a shell with the launch's environment, which types the launch line at
    /// its first prompt and keeps the self-test record. A shell that does not start keeps the record
    /// too, so the card says why.
    pub fn spawn_agent(&mut self, cols: u16, rows: u16, cwd: &Path, launch: AgentLaunch) -> usize {
        let AgentLaunch { line, env, mut post } = launch;
        let index = self.open_tab(cols, rows, cwd, &env, Some(post.harness.name().to_owned()));
        let tab = &mut self.open[index];
        if !line.is_empty() {
            if tab.session.is_some() {
                tab.launch = Some(PendingLaunch { bytes: line, opened: Instant::now(), last_output: None });
            } else if let Some(why) = &tab.problem {
                post.not_started(why.clone());
            }
        }
        tab.agent = Some(post);
        index
    }

    /// The one place a tab is opened: a shell with `env` laid over the session's own, titled `title`
    /// or after its directory.
    fn open_tab(&mut self, cols: u16, rows: u16, cwd: &Path, env: &[(String, String)], title: Option<String>) -> usize {
        let mut env = env.to_vec();
        if let Some(root) = &self.project_root {
            env.push((PROJECT_ENV.to_owned(), root.to_string_lossy().into_owned()));
        }
        match TerminalSession::spawn_with(cols, rows, cwd, &env, self.shell.as_ref()) {
            Ok(session) => {
                let title = title.unwrap_or_else(|| shell_title(cwd));
                let mut screen = grid::TerminalScreen::new(cols, rows);
                screen.resize(cols, rows);
                screen.set_default_caret(self.default_caret);
                screen.set_scrollback(self.scrollback.unwrap_or(SCROLLBACK));
                self.open.push(TerminalTab {
                    title,
                    cd_offered: None,
                    session: Some(session),
                    screen,
                    problem: None,
                    agent: None,
                    launch: None,
                    typed: false,
                });
                self.active = self.open.len() - 1;
                self.problem = None;
                self.active
            }
            Err(why) => {
                let mut tab = TerminalTab::refused(cols, rows, why.clone());
                if let Some(title) = title {
                    tab.title = title;
                }
                self.open.push(tab);
                self.active = self.open.len() - 1;
                self.problem = Some(why);
                self.active
            }
        }
    }

    /// **The shell every tab opened from now on runs** (`terminal_shell`, `terminal_shell_args`, #324):
    /// `None` is the environment's. A tab already open keeps the shell it started with.
    pub fn set_shell(&mut self, shell: Option<ShellChoice>) {
        self.shell = shell;
    }

    /// The shell the settings choose, when they choose one.
    pub fn shell(&self) -> Option<&ShellChoice> {
        self.shell.as_ref()
    }

    /// **The person's agent settings** (`terminal_agent`, `terminal_agent_command`), which also
    /// mark the settings as given, so the first shell may open.
    pub fn set_agent(&mut self, agent: TerminalAgent, command: Option<String>) {
        self.agent = agent;
        self.agent_command = command;
        self.settings_taken = true;
    }

    /// The agent the settings choose.
    pub fn agent(&self) -> TerminalAgent {
        self.agent
    }

    /// The custom harness's command line, when the settings set one.
    pub fn agent_command(&self) -> Option<&str> {
        self.agent_command.as_deref()
    }

    /// Have the person's settings been given yet? No shell opens before they are.
    pub fn settings_taken(&self) -> bool {
        self.settings_taken
    }

    /// **Does the pane offer the agent instead of a shell?** While the settings choose an agent and
    /// nothing is open. An agent session costs tokens to warm up, so none starts by itself: the pane
    /// shows an `Invoke Agent` button, a person who is only looking around pays nothing, and the press
    /// opens the agent tab (`terminal_agent::open_agent_tab`). The `+` chip still opens a plain shell.
    pub fn invites_the_agent(&self) -> bool {
        self.settings_taken && self.agent != TerminalAgent::Off && self.open.is_empty()
    }

    /// The agent tab whose launch minted `token`.
    pub fn agent_with_token_mut(&mut self, token: &str) -> Option<&mut AgentPost> {
        self.agent_posts_mut().find(|post| post.token == token)
    }

    /// Every agent tab's self-test record.
    pub fn agent_posts_mut(&mut self) -> impl Iterator<Item = &mut AgentPost> {
        self.open.iter_mut().filter_map(|tab| tab.agent.as_mut())
    }

    /// **Take the projects whose agent tab has been taken out** since the last call ([`Self::remove_tab`]).
    pub fn take_ended_agents(&mut self) -> Vec<PathBuf> {
        std::mem::take(&mut self.ended_agents)
    }

    /// **The caret every shell gets when its program asks for none** (#324), now and for each shell
    /// opened after. A program that asked for a shape keeps it.
    pub fn set_default_caret(&mut self, shape: CaretShape) {
        self.default_caret = shape;
        for tab in &mut self.open {
            tab.screen.set_default_caret(shape);
        }
    }

    /// **The history every shell keeps** (#324), now and for each shell opened after.
    pub fn set_scrollback(&mut self, lines: usize) {
        self.scrollback = Some(lines);
        for tab in &mut self.open {
            tab.screen.set_scrollback(lines);
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
        self.remove_tab(self.active);
    }

    /// **Take the tab at `index` out**, with its shell (`Drop` hangs the child up), keeping the same tab
    /// drawn when it is another one that goes.
    fn remove_tab(&mut self, index: usize) {
        if let Some(project) = self.open[index].agent.as_ref().and_then(|post| post.project.clone()) {
            self.ended_agents.push(project);
        }
        self.open.remove(index);
        if index < self.active {
            self.active -= 1;
        }
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

    /// Is the pane collapsed? Every screen's pane is drawn collapsed while this holds, which is the state it starts in (#424).
    pub fn is_collapsed(&self) -> bool {
        !self.raised
    }

    /// Collapse the pane, or restore it, for this screen and every one after it.
    pub fn set_collapsed(&mut self, collapsed: bool) {
        self.raised = !collapsed;
    }

    /// **The project the editor holds** (#324): `CAULDRON_PROJECT` in each shell opened from now on.
    pub fn set_project_root(&mut self, root: Option<PathBuf>) {
        self.project_root = root;
    }

    /// The project the editor holds, when one is open.
    pub fn project_root(&self) -> Option<&Path> {
        self.project_root.as_deref()
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
        tab.typed = true;
        tab.screen.scroll_to_bottom();
        if let Err(why) = session.write(bytes) {
            self.problem = Some(why);
        }
    }

    /// **Close every session's search**, which is what closing the find box means (#321): a search left
    /// open on a tab the person switched away from would light matches nobody asked for when they
    /// came back.
    pub fn clear_searches(&mut self) {
        for tab in &mut self.open {
            tab.screen.find_clear();
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

    /// **Read every shell that has output, answer every question one asked, and close the tab of a shell
    /// that has left.**
    ///
    /// Run once per frame by [`crate::terminal_view`]. Every tab, not just the drawn one: a `cargo
    /// build` in a background tab must not stall on a full pipe because the pane was looking
    /// elsewhere.
    ///
    /// **A shell that was told to leave (`exit`, Ctrl+D) takes its tab with it**, the way every terminal
    /// does (feedback-1794112080244: the pane stayed up over a dead shell, which a person met as the
    /// terminal freezing). What follows is the `×` path's: a fresh shell for a pane with no agent chosen,
    /// the `Invoke Agent` button for one with an agent chosen, and either way `+` opens another.
    pub fn pump(&mut self) {
        let mut buffer = Vec::new();
        let now = Instant::now();
        let mut left = Vec::new();
        for (index, tab) in self.open.iter_mut().enumerate() {
            let Some(session) = tab.session.as_mut() else {
                continue;
            };
            // **Asked before the drain**, so everything the shell wrote before it went is already in the
            // pipe when the answer is yes, and the drain below reads all of it.
            let exited = session.has_exited();
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
            if !buffer.is_empty()
                && let Some(launch) = tab.launch.as_mut()
            {
                launch.last_output = Some(now);
            }
            // **A shell that left is handled before a failed write to it is**: the answer to a question it
            // asked on its way out has nowhere to go, and that error would otherwise hold the tab for ever.
            if exited {
                if tab.typed {
                    left.push(index);
                } else {
                    tab.ended_on_its_own();
                }
                continue;
            }
            if let Err(why) = outcome {
                if tab.launch.take().is_some()
                    && let Some(post) = tab.agent.as_mut()
                {
                    post.not_started(format!("the shell stopped before the agent was typed: {why}"));
                }
                tab.problem = Some(why);
                continue;
            }
            // **The agent is typed at the shell's prompt**, so the person's login PATH and rc files
            // apply, and leaving the agent leaves them at a shell.
            if tab.launch.as_ref().is_some_and(|launch| launch.is_due(now))
                && let Some(launch) = tab.launch.take()
            {
                let typed = session.write(&launch.bytes);
                if let Some(post) = tab.agent.as_mut() {
                    match &typed {
                        Ok(()) => post.started(),
                        Err(why) => post.not_started(why.clone()),
                    }
                }
                if let Err(why) = typed {
                    tab.problem = Some(why);
                }
            }
        }
        for index in left.into_iter().rev() {
            self.remove_tab(index);
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

    /// This tab's agent and its self test, when it is an agent tab.
    pub fn agent(&self) -> Option<&AgentPost> {
        self.agent.as_ref()
    }

    /// This tab's agent record, mutably: a click on its status chip.
    pub fn agent_mut(&mut self) -> Option<&mut AgentPost> {
        self.agent.as_mut()
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
    terminal.set_project_root(Some(root.clone()));
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
            .add_plugins((
                crate::terminal_pane::TerminalPanePlugin,
                crate::terminal_view::TerminalViewPlugin,
                crate::terminal_agent::TerminalAgentPlugin,
            ));
    }
}

/// **A shell opens where the open project is.**
///
/// Before the first `Ctrl+\``, so the very first shell is already in the project rather than in the
/// editor's own working directory — which, for a build launched from a game's folder, is the
/// directory the game's assets live in and would have looked right by accident.
fn seat_the_terminals_cwd(mut terminal: ResMut<Terminal>, root: Option<Res<crate::project_assets::ProjectRoot>>) {
    if let Some(root) = root.as_ref().and_then(|r| r.current()) {
        terminal.set_project_root(Some(root.clone()));
        terminal.set_next_cwd(Some(root));
    }
}
