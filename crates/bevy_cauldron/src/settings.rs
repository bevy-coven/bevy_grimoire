//! **The editor's settings, in three layers that change the running editor** (UI-70, #248).
//!
//! 1. The built-in defaults ([`EditorSettings::default`]).
//! 2. The user's file, `<config_dir>/Grimoire/settings.yaml` ([`user_file`]).
//! 3. The project's file, `<project>/assets/grimoire/editor.yaml` (kind `editor_settings` in
//!    `reload::KINDS`).
//!
//! **Precedence is per key, and the later layer wins.** A file is a [`SettingsLayer`]: every key is an
//! `Option`, so a file says only what it sets, and a key a layer leaves out falls through to the layer
//! below. `version: 1` and `text_scale: 1.5` in the user's file and `version: 1` and `reduced_motion:
//! true` in the project's give both; the same key in both gives the project's.
//!
//! **Live.** [`sync_settings`] looks at both files while the editor runs (size and modification time
//! each frame, the text once a second regardless, as `oplog_view` does) and re-resolves
//! [`EditorSettings`]. Deleting a key, or the file, falls back to the layer below at once. A file that
//! does not read (not YAML, an unknown key, a key of the wrong type, a value out of range, another
//! version) **keeps that layer's last good content**, is refused in `grimoire_state.reloads` with the
//! file and the field, and is a problem on the status line, once per distinct fault.
//!
//! The project layer is a kind of the reload contract, so the guide's table lists it. The user's file
//! belongs to no project, so it is not a kind; its verdicts name its full path.
//!
//! The settings screen (UI-71) writes these layers, and the keymap (UI-72) and the theme (UI-73) add
//! their keys to [`SettingsLayer`]; a new key is an `Option` there with its default in
//! [`EditorSettings`], and an old file still reads.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use bevy::prelude::*;
use bevy_grimoire_core::live::{KindVerdict, Outcome};
use bevy_grimoire_core::reload_check::{self, Refusal};
use serde::{Deserialize, Serialize};
use bevy_gooey::Shortcut;

use crate::editor::EditorState;
use crate::live::EditorReloads;
use crate::project::Project;

pub const SETTINGS_VERSION: u32 = 1;

/// The id of the reload-contract kind both layers' verdicts carry.
pub const KIND: &str = "editor_settings";

const USER_FILE: &str = "settings.yaml";
/// The project's file, under the project directory (`assets/grimoire`).
pub const PROJECT_FILE: &str = "editor.yaml";

/// How long a file whose size and modification time did not move goes unread.
const RECHECK_SECS: f32 = 1.0;

/// The smallest and largest a scale may be: below the first nothing is legible, above the second no
/// panel fits.
pub const SCALE_RANGE: (f32, f32) = (0.5, 3.0);

/// Where the user's settings live, or `None` on a platform with no config directory.
pub fn user_file() -> Option<PathBuf> {
    bevy_grimoire_core::user_dirs::config_dir().map(|d| d.join(USER_FILE))
}

/// The look of the editor's chrome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    ToolDark,
    Parchment,
    HighContrast,
}

/// **The terminal's caret when a program asks for none** (#324): VS Code's
/// `terminal.integrated.cursorStyle`. A program that asks for a shape (vim's DECSCUSR) still gets it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalCursor {
    #[default]
    Block,
    Underline,
    Bar,
}

impl TerminalCursor {
    /// The shape the terminal draws for this setting.
    pub fn shape(self) -> crate::terminal::CaretShape {
        match self {
            TerminalCursor::Block => crate::terminal::CaretShape::Block,
            TerminalCursor::Underline => crate::terminal::CaretShape::Underline,
            TerminalCursor::Bar => crate::terminal::CaretShape::Bar,
        }
    }
}

/// **The agent the terminal's `Invoke Agent` button starts**, connected to the editor's MCP server and proved with
/// a power-on self test (`terminal_agent`). `Custom` runs `terminal_agent_command` from the file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalAgent {
    #[default]
    Off,
    OhMyPi,
    Custom,
}

impl TerminalAgent {
    /// The word chips and messages call this agent by.
    pub fn name(self) -> &'static str {
        match self {
            TerminalAgent::OhMyPi => "omp",
            TerminalAgent::Off | TerminalAgent::Custom => "agent",
        }
    }
}

/// **The program a machine must have for the terminal to start with the agent chosen** (#341).
///
/// It is what an `Invoke Agent` press types, so a machine that has it can start an agent and a machine
/// that does not would only fail its self test.
pub const AGENT_PROGRAM: &str = "omp";

/// **How long the login shell is given to answer, in seconds** (#341).
///
/// A login shell reads the person's rc files, so it is slower than a plain one and one of them can
/// hang. Past this the machine is read as having no agent: a window that never opens is worse than a
/// terminal that offers a shell.
pub const LOGIN_ASK_PATIENCE_S: u64 = 5;

/// **Is the agent installed on this machine?** — the executable [`AGENT_PROGRAM`] in any folder of the
/// `PATH` it is given.
///
/// Off the environment as a parameter rather than read inside, so the answer for any folder list can be
/// tested without touching the process's own. A file that is not executable is not a program: a machine
/// with one has no agent, and its terminal keeps its shell.
pub fn agent_in_path(path: Option<&str>) -> TerminalAgent {
    let Some(path) = path else { return TerminalAgent::Off };
    // An empty entry is "here" to a shell, so a test run from a folder holding an `omp` would answer
    // for a machine whose `PATH` does not name one; it is left out rather than read as a folder.
    let found = std::env::split_paths(path)
        .filter(|folder| !folder.as_os_str().is_empty())
        .any(|folder| is_runnable(&folder.join(AGENT_PROGRAM)));
    if found { TerminalAgent::OhMyPi } else { TerminalAgent::Off }
}

/// Is `program` a file this machine can run? A directory named `omp` is not, and neither is a file
/// whose mode has no execute bit on it.
fn is_runnable(program: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(program) else { return false };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// How often a shell that has answered is looked at for its exit, in milliseconds.
const LOGIN_ASK_EXIT_POLL_MS: u64 = 5;

/// **What this machine has, asked once** and carried for the whole session (#341).
///
/// A `PATH` that gains the agent while the editor runs does not change the editor under the person
/// using it: the answer is taken at start-up, which is also what lets Settings show the effective value
/// as a value rather than as a thing that moves. Inserted by `main`; absent in a test, whose editor
/// resolves from the built-in default alone.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MachineAgent(pub TerminalAgent);

impl MachineAgent {
    /// **Asks the machine the way the press runs the agent** (#341).
    ///
    /// It is `SHELL -lc 'command -v omp'`, not this process's `PATH`. A macOS app opened from Finder or
    /// the Dock inherits launchd's environment — `/usr/bin:/bin:/usr/sbin:/sbin` — so the owner's
    /// `~/.local/bin/omp` is not on it and the machine would answer `Off` while `Invoke Agent` works,
    /// because that press types the command into a *login* shell (`terminal_agent.rs` says why). The
    /// question is therefore put to the same login shell, so the pane's offer and the button's work
    /// come from one answer.
    ///
    /// **The shell is the pane's own** — `crate::terminal`'s `shell_program_of_environment`, the one a
    /// tab runs when no `terminal_shell` is chosen (`terminal_shell` is a setting read on the frames
    /// after this, so it is not the shell an answer could be put to here). One function decides which
    /// shell is this person's, so the question and the tab cannot disagree about `$SHELL` or about a
    /// machine that has neither `$SHELL` nor `/bin/sh`.
    ///
    /// Bounded: a login shell whose rc files hang answers nothing, and [`LOGIN_ASK_PATIENCE_S`] later
    /// the machine is read as having no agent rather than the window waiting.
    pub fn ask() -> MachineAgent {
        match crate::terminal::shell_program_of_environment() {
            Ok(shell) => MachineAgent(MachineAgent::ask_the_login_shell(&shell, LOGIN_ASK_PATIENCE_S)),
            // No shell at all: there is nothing to ask, and nothing the press could run either.
            Err(_) => MachineAgent(TerminalAgent::Off),
        }
    }

    /// [`MachineAgent::ask`] with the shell and its patience named, so a test can give both.
    ///
    /// `None` from `command -v` — the shell exits non-zero — is a machine without the agent; so is a
    /// shell that cannot be started, or one whose output does not arrive inside `patience_s`.
    ///
    /// **One deadline covers the whole question, the answer included.** The shell's exit alone is not
    /// the answer: the text is read on its own thread and sent down a channel, so a login shell that
    /// backgrounded a process which holds the pipe (`( sleep 30 & ) ; command -v omp ; exit 0`) cannot
    /// keep the editor waiting for an EOF that arrives 30 s later. Past `patience_s`, the child is
    /// killed and reaped and the machine reads `Off`.
    pub fn ask_the_login_shell(shell: &str, patience_s: u64) -> TerminalAgent {
        let script = format!("command -v {AGENT_PROGRAM}");
        let started = std::process::Command::new(shell)
            .arg("-lc")
            .arg(&script)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn();
        let Ok(mut child) = started else { return TerminalAgent::Off };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(patience_s);
        // **The text arrives on a channel, not from `join`.** A reader thread that is still blocked on
        // the pipe at the deadline is left holding it: it writes into a channel nobody reads and its
        // thread ends with the process, so the editor never waits on it.
        let (saying, heard) = std::sync::mpsc::channel();
        if let Some(mut out) = child.stdout.take() {
            std::thread::spawn(move || {
                use std::io::Read as _;
                let mut text = String::new();
                let _ = out.read_to_string(&mut text);
                let _ = saying.send(text);
            });
        }
        let text = {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            heard.recv_timeout(left).ok()
        };
        // **A shell that never closed its output says nothing worth reading**, and outlived its
        // patience: killed and reaped, so it does not stay a zombie for the hours this editor runs.
        let Some(text) = text else {
            let _ = child.kill();
            let _ = child.wait();
            return TerminalAgent::Off;
        };
        // **The exit, then the answer.** Output arriving is not the exit arriving: the shell may still
        // be in the microseconds before `wait` can see it, so the status is looked for inside the same
        // patience rather than assumed. A shell whose status never comes is one that did not finish.
        let failed_or_unfinished = loop {
            match child.try_wait() {
                Ok(Some(status)) => break !status.success(),
                Ok(None) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(LOGIN_ASK_EXIT_POLL_MS));
                }
                _ => break true,
            }
        };
        if failed_or_unfinished || text.trim().is_empty() { TerminalAgent::Off } else { TerminalAgent::OhMyPi }
    }

    /// **The agent a machine starts its terminal with**, from what the binary asked it.
    ///
    /// [`None`] is a process that never asked — every test's editor — and reads `Off`, the built-in
    /// default. One place, because the terminal's resolution and the Settings screen must not answer
    /// this question two ways.
    pub fn resolved(machine: Option<&MachineAgent>) -> TerminalAgent {
        machine.map_or(TerminalAgent::Off, |machine| machine.0)
    }
}

/// **What the running editor uses**: the defaults with the user's file and then the project's laid over
/// them, key by key.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct EditorSettings {
    pub theme: Theme,
    /// Text size as a multiple of the skin's: 1.0 is as designed, 2.0 is 200%.
    pub text_scale: f32,
    /// Interface size (spacing and targets) as a multiple of the skin's.
    pub interface_scale: f32,
    /// Reduced motion (gooey's `MotionSettings::reduced`): what travels lands at once, fades and colours still ease.
    pub reduced_motion: bool,
    /// **Whether the view follows the agent's changes** (AF-04, #360): View ▸ Follow the agent.
    ///
    /// On by default, because the follow is the point of the AGENT series: an agent works faster than a
    /// person reads, and a view that stayed put would mean the person never saw the work at all. Off, the
    /// camera never moves for an agent's change and the look chip still offers *Look*, so a person who
    /// turned it off has not lost the ability to see — they have taken away only the moving.
    ///
    /// A settings key rather than a session switch for `reduced_motion`'s reason: it is how a person
    /// wants to watch an agent, not a fact about one project. **Its tests and its changelog line are
    /// AF-04's** (`tests/the_follow_waits_for_a_person_using_the_view.rs`, `af-04-pace`), which is
    /// where the behaviour it switches was built.
    pub follow_agent: bool,
    /// The map's grid is drawn in the viewport.
    pub show_grid: bool,
    /// How strongly the map's grid is drawn: 1.0 is the skin's grid colour, less is fainter.
    pub grid_strength: f32,
    /// The triangle count is shown at the top right (#302). Off until asked for: it is a number for
    /// someone tuning a scene, and the first thing a newcomer reads at the top of the window.
    pub show_statistics: bool,
    /// **View ▸ Show activity bar** (NV-03, #380): the vertical strip of workspace icons at the
    /// window's far left. On by default, because it is the one place the workspaces are all in view;
    /// off takes it away entirely and the choice is saved.
    pub show_activity_bar: bool,
    /// The rebound commands of every layer, by command id: what the keymap screen shows against each
    /// command's default chord.
    pub keymap: BTreeMap<String, Shortcut>,
    /// The terminal's text size as a multiple of the shipped size (#324): VS Code's
    /// `terminal.integrated.fontSize`, as a scale so it reads like the editor's own text size.
    pub terminal_text_scale: f32,
    /// The terminal's caret when a program asks for none.
    pub terminal_cursor: TerminalCursor,
    /// The terminal's caret blinks. On by default, as it has been since the terminal shipped.
    pub terminal_cursor_blink: bool,
    /// How many lines of output each terminal keeps above its screen (#324): VS Code's
    /// `terminal.integrated.scrollback`.
    pub terminal_scrollback: usize,
    /// The agent the terminal's `Invoke Agent` button starts, connected to the editor's tools.
    ///
    /// **The built-in default is `Off`**, a pure value with no machine behind it. What a person meets is
    /// the resolved settings, and that asks this machine once at start-up (`MachineAgent`, #341): a
    /// machine whose login shell finds `omp` resolves to `OhMyPi`, so its terminal opens on the empty pane that
    /// offers the agent; a machine without one resolves to `Off` and opens a shell. A `terminal_agent`
    /// key in either settings file wins over both.
    ///
    /// Either way nothing starts by itself — a session costs tokens to warm up — until the button is
    /// pressed.
    pub terminal_agent: TerminalAgent,
    /// The shell line `TerminalAgent::Custom` types. Deliberately not in the settings catalogue: the
    /// Settings screen has no text control (#324), so this key is set in the file only.
    pub terminal_agent_command: Option<String>,
    /// **The program a new terminal tab runs** (#324): VS Code's `terminal.integrated.defaultProfile`.
    /// `None` is `$SHELL`, then `/bin/sh`. Tabs already open keep the shell they started with. Like
    /// `terminal_agent_command`, not in the settings catalogue: the Settings screen has no text control.
    pub terminal_shell: Option<String>,
    /// The arguments `terminal_shell` is started with (VS Code's profile `args`); empty when none.
    pub terminal_shell_args: Vec<String>,
    /// **Each workspace's arrangement, as a person left it** (UI-33, #227; per workspace since
    /// NV-02, #379): the workspace's id and the `bevy_gooey` `DockLayout` it shows, written by the
    /// editor when a divider or a tab is moved and read back on the next start.
    ///
    /// **A map, not one layout.** A door was one arrangement because a door was one place; with nine
    /// workspaces a single layout would move Map's inspector when somebody dragged Kit's, which is
    /// the thing per-workspace layouts exist to stop. A workspace with no entry is
    /// [`Workspace::default_layout`](crate::workspace::Workspace::default_layout) — its own default,
    /// so a person who has never moved anything has no key in their file.
    pub workspace_layout: BTreeMap<String, bevy_gooey::DockLayout>,
}

impl Default for EditorSettings {
    fn default() -> Self {
        EditorSettings {
            theme: Theme::default(),
            text_scale: 1.0,
            interface_scale: 1.0,
            reduced_motion: false,
            follow_agent: true,
            show_grid: true,
            grid_strength: 1.0,
            show_statistics: false,
            show_activity_bar: true,
            keymap: BTreeMap::new(),
            terminal_text_scale: 1.0,
            terminal_cursor: TerminalCursor::default(),
            terminal_cursor_blink: true,
            terminal_scrollback: crate::terminal::SCROLLBACK,
            terminal_agent: TerminalAgent::default(),
            terminal_agent_command: None,
            terminal_shell: None,
            terminal_shell_args: Vec::new(),
            workspace_layout: BTreeMap::new(),
        }
    }
}

/// **The fewest and the most lines of history a terminal may keep** (#324): under the first, a build's
/// error is gone before it can be scrolled to; over the second, a long day's output is a slow leak in
/// the editor's memory (each line is a row of cells).
pub const SCROLLBACK_RANGE: (usize, usize) = (100, 100_000);

/// The faintest and fullest the grid may be drawn: below the first it cannot be seen.
pub const GRID_STRENGTH_RANGE: (f32, f32) = (0.1, 1.0);

/// **One file's worth of settings**: only the keys it sets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SettingsLayer {
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_scale: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interface_scale: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduced_motion: Option<bool>,
    /// **View ▸ Follow the agent** (AF-04, #360). `None` means the layer leaves it alone, so an
    /// `editor.yaml` written before this key existed reads as on — the default, not a stored `false`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub follow_agent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_grid: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_strength: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_statistics: Option<bool>,
    /// **View ▸ Show activity bar** (NV-03, #380). `None` leaves the layer below to decide, so a file
    /// written before this key existed reads as on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_activity_bar: Option<bool>,
    /// **Rebound commands**: a command's id (`file.save`, `commands::Command::id`) and the chord that
    /// now runs it. A command the file leaves out keeps its default chord (UI-72, #250).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub keymap: BTreeMap<String, Shortcut>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_text_scale: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_cursor: Option<TerminalCursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_cursor_blink: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_scrollback: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_agent: Option<TerminalAgent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_agent_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_shell: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_shell_args: Option<Vec<String>>,
    /// Each workspace's arrangement (UI-33, #227; per workspace since NV-02, #379). Not in the
    /// settings catalogue: it is a thing a person drags, not one they type, and Window ▸ Reset
    /// layout is the way back to the current workspace's default.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub workspace_layout: BTreeMap<String, bevy_gooey::DockLayout>,
    /// **The one arrangement files had before the workspaces** (UI-33, #227), read so no existing
    /// settings file is refused for it: [`SettingsLayer::read`] moves it into
    /// `workspace_layout["map"]` (the arrangement was the Map's) unless that entry exists. Never
    /// written back.
    #[serde(default, skip_serializing)]
    pub shell_layout: Option<bevy_gooey::DockLayout>,
}

impl Default for SettingsLayer {
    fn default() -> Self {
        SettingsLayer {
            version: SETTINGS_VERSION,
            theme: None,
            text_scale: None,
            interface_scale: None,
            reduced_motion: None,
            follow_agent: None,
            show_grid: None,
            grid_strength: None,
            show_statistics: None,
            show_activity_bar: None,
            keymap: BTreeMap::new(),
            terminal_text_scale: None,
            terminal_cursor: None,
            terminal_cursor_blink: None,
            terminal_scrollback: None,
            terminal_agent: None,
            terminal_agent_command: None,
            terminal_shell: None,
            terminal_shell_args: None,
            workspace_layout: BTreeMap::new(),
            shell_layout: None,
        }
    }
}

impl SettingsLayer {
    /// **Reads a file's text as a layer**, refusing with the file and the field when it does not read,
    /// is another version, or holds a value no editor can use.
    pub fn read(file: &str, text: &str) -> Result<SettingsLayer, Refusal> {
        let mut layer: SettingsLayer = reload_check::read(KIND, file, text, |t| bevy_grimoire_core::yaml::from_str(t, "settings").map_err(|p| p.line()))?;
        if let Some(old) = layer.shell_layout.take() {
            layer.workspace_layout.entry(crate::workspace::Workspace::Map.id().to_owned()).or_insert(old);
        }
        // **A keymap that rebound a command the workspaces replaced still reads** (#379): the entry
        // binds nothing now, so it is dropped here and the next save writes the file without it.
        layer.keymap.retain(|id, _| !crate::commands::is_retired(id));
        let refuse = |field: &str, reason: String| Refusal { kind: KIND, file: file.to_owned(), field: Some(field.to_owned()), reason };
        if layer.version != SETTINGS_VERSION {
            return Err(refuse(
                "version",
                format!("version {} is not the {SETTINGS_VERSION} this build reads", layer.version),
            ));
        }
        for (field, value) in [
            ("text_scale", layer.text_scale),
            ("interface_scale", layer.interface_scale),
            ("terminal_text_scale", layer.terminal_text_scale),
        ] {
            if let Some(v) = value
                && !(SCALE_RANGE.0..=SCALE_RANGE.1).contains(&v)
            {
                return Err(refuse(field, format!("{v} is outside {} to {}", SCALE_RANGE.0, SCALE_RANGE.1)));
            }
        }
        if let Some(v) = layer.terminal_scrollback
            && !(SCROLLBACK_RANGE.0..=SCROLLBACK_RANGE.1).contains(&v)
        {
            return Err(refuse("terminal_scrollback", format!("{v} lines is outside {} to {}", SCROLLBACK_RANGE.0, SCROLLBACK_RANGE.1)));
        }
        if let Some(v) = layer.grid_strength
            && !(GRID_STRENGTH_RANGE.0..=GRID_STRENGTH_RANGE.1).contains(&v)
        {
            return Err(refuse("grid_strength", format!("{v} is outside {} to {}", GRID_STRENGTH_RANGE.0, GRID_STRENGTH_RANGE.1)));
        }
        if let Some(command) = &layer.terminal_agent_command
            && command.trim().is_empty()
        {
            return Err(refuse("terminal_agent_command", "an empty command runs nothing: leave the key out or name a program".to_owned()));
        }
        if let Some(shell) = &layer.terminal_shell
            && shell.trim().is_empty()
        {
            return Err(refuse("terminal_shell", "an empty shell runs nothing: leave the key out or name a program".to_owned()));
        }
        if layer.terminal_shell_args.is_some() && layer.terminal_shell.is_none() {
            return Err(refuse("terminal_shell_args", "arguments for a shell that is not named: set `terminal_shell` too, or leave the key out".to_owned()));
        }
        for (id, chord) in &layer.keymap {
            let field = format!("keymap.{id}");
            if crate::commands::by_id(id).is_none() {
                return Err(refuse(&field, format!("`{id}` is not a command the editor has")));
            }
            if !chord.is_valid() {
                return Err(refuse(&field, "this chord has no key (a modifier alone is not a shortcut)".to_owned()));
            }
        }
        Ok(layer)
    }
}

impl EditorSettings {
    /// **The defaults with `user` and then `project` laid over them, key by key.**
    pub fn resolve(user: Option<&SettingsLayer>, project: Option<&SettingsLayer>) -> EditorSettings {
        EditorSettings::resolve_with(user, project, EditorSettings::default())
    }

    /// **The same, on this machine** (#341): the built-in defaults with the one key a machine answers
    /// filled in first — [`TerminalAgent`], which is `Off` unless `machine` says the agent is installed.
    ///
    /// **A file still wins, both ways.** `machine` fills the default, it does not overrule a layer, so a
    /// person on a machine with `omp` who wants a plain shell names `Off` in their file and gets it, and
    /// a person whose agent is somewhere their login shell does not reach names it too. Only the key nobody
    /// stated comes from the machine.
    pub fn resolve_on_this_machine(
        user: Option<&SettingsLayer>,
        project: Option<&SettingsLayer>,
        machine: TerminalAgent,
    ) -> EditorSettings {
        EditorSettings::resolve_with(user, project, EditorSettings { terminal_agent: machine, ..EditorSettings::default() })
    }

    /// **The one resolution**, from `base`. Every caller goes through here, so a key added to the
    /// defaults cannot be resolved one way by the editor and another by whatever asked a machine about
    /// it.
    fn resolve_with(user: Option<&SettingsLayer>, project: Option<&SettingsLayer>, mut out: EditorSettings) -> EditorSettings {
        for layer in [user, project].into_iter().flatten() {
            out.theme = layer.theme.unwrap_or(out.theme);
            out.text_scale = layer.text_scale.unwrap_or(out.text_scale);
            out.interface_scale = layer.interface_scale.unwrap_or(out.interface_scale);
            out.reduced_motion = layer.reduced_motion.unwrap_or(out.reduced_motion);
            out.follow_agent = layer.follow_agent.unwrap_or(out.follow_agent);
            out.show_grid = layer.show_grid.unwrap_or(out.show_grid);
            out.grid_strength = layer.grid_strength.unwrap_or(out.grid_strength);
            out.show_statistics = layer.show_statistics.unwrap_or(out.show_statistics);
            out.show_activity_bar = layer.show_activity_bar.unwrap_or(out.show_activity_bar);
            // Per command: a later layer's binding replaces the earlier one's for that command alone.
            out.keymap.extend(layer.keymap.iter().map(|(id, chord)| (id.clone(), *chord)));
            out.terminal_text_scale = layer.terminal_text_scale.unwrap_or(out.terminal_text_scale);
            out.terminal_cursor = layer.terminal_cursor.unwrap_or(out.terminal_cursor);
            out.terminal_cursor_blink = layer.terminal_cursor_blink.unwrap_or(out.terminal_cursor_blink);
            out.terminal_scrollback = layer.terminal_scrollback.unwrap_or(out.terminal_scrollback);
            out.terminal_agent = layer.terminal_agent.unwrap_or(out.terminal_agent);
            if let Some(command) = &layer.terminal_agent_command {
                out.terminal_agent_command = Some(command.clone());
            }
            // **A shell and its arguments are one value**: a layer that names a shell brings its own
            // arguments (none when it gives none), never the earlier layer's, which were for another program.
            if let Some(shell) = &layer.terminal_shell {
                out.terminal_shell = Some(shell.clone());
                out.terminal_shell_args = layer.terminal_shell_args.clone().unwrap_or_default();
            }
            // **The whole arrangement, not a key of it**: a layout is one value, and a later layer
            // that states one replaces the earlier's (the same rule as `theme`).
            for (workspace, layout) in &layer.workspace_layout {
                out.workspace_layout.insert(workspace.clone(), layout.clone());
            }
        }
        out
    }
}

/// Names the user's file. Inserted by `main`; absent in a test, so no test reads the real config.
#[derive(Resource, Clone, Debug)]
pub struct UserSettingsFile(pub PathBuf);

/// What one look at a file found.
#[derive(Clone, Debug, PartialEq)]
enum Look {
    Applied,
    Refused { field: Option<String>, reason: String },
    /// No file: not an error, the layer is simply empty.
    Gone,
}

/// One layer's file, and what the editor last made of it.
#[derive(Default)]
struct Watch {
    path: Option<PathBuf>,
    stat: Option<(SystemTime, u64)>,
    since: f32,
    /// A hash of the text last parsed, so the same text is not parsed again.
    hash: Option<u64>,
    /// **The last content that read.** Kept across a refusal.
    good: Option<SettingsLayer>,
    last: Option<Look>,
}

fn hash_of(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

impl Watch {
    /// Looks at `path` when it changed; the look it decided, or `None` when there is nothing new to say.
    fn look(&mut self, path: &Path, file: &str, dt: f32) -> Option<Look> {
        self.since += dt;
        let stat = std::fs::metadata(path).ok().and_then(|m| Some((m.modified().ok()?, m.len())));
        let same_file = self.path.as_deref() == Some(path);
        if same_file && self.stat == stat && self.since < RECHECK_SECS && self.last.is_some() {
            return None;
        }
        self.since = 0.0;
        if !same_file {
            *self = Watch { path: Some(path.to_owned()), ..Watch::default() };
        }
        self.stat = stat;
        let look = match std::fs::read_to_string(path) {
            Ok(text) => {
                let hash = hash_of(&text);
                match self.last.clone().filter(|_| self.hash == Some(hash)) {
                    Some(last) => last,
                    None => {
                        self.hash = Some(hash);
                        match SettingsLayer::read(file, &text) {
                            Ok(layer) => {
                                self.good = Some(layer);
                                Look::Applied
                            }
                            Err(r) => Look::Refused { field: r.field, reason: r.reason },
                        }
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.hash = None;
                self.good = None;
                Look::Gone
            }
            Err(e) => {
                self.hash = None;
                Look::Refused { field: None, reason: e.to_string() }
            }
        };
        if self.last.as_ref() == Some(&look) {
            return None;
        }
        self.last = Some(look.clone());
        Some(look)
    }
}

/// **What the editor last made of the two files.**
#[derive(Resource, Default)]
pub struct SettingsView {
    user: Watch,
    project: Watch,
}

impl SettingsView {
    /// Has the open project's own file been looked at? Until it has, [`EditorSettings`] is the user's layer alone,
    /// and a setting the project makes (the terminal's agent, say) is not yet in it.
    pub fn project_looked(&self) -> bool {
        self.project.path.is_some()
    }
}

/// **Everything [`sync_settings`] resolves from**, grouped so its argument list stays readable and
/// under clippy's limit as a source is added.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SettingsSources<'w> {
    project: Option<Res<'w, Project>>,
    user_file: Option<Res<'w, UserSettingsFile>>,
    /// **What this machine has** (#341), asked once at start-up by the binary. Absent in a test, whose
    /// editor resolves from the built-in default alone and so opens the shell it always did.
    machine: Option<Res<'w, MachineAgent>>,
}

impl SettingsSources<'_> {
    /// The agent this machine's terminal starts with when no file states one.
    fn machine_agent(&self) -> TerminalAgent {
        MachineAgent::resolved(self.machine.as_deref())
    }
}

/// **Which settings file a finding is about** (#425): the person's own, or the open project's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentFile {
    User,
    Project,
}

/// **What a settings file that chooses the Custom agent without a command says, and how to fix it** (#425). The Settings screen
/// offers the fix in the Terminal agent row; this is the same sentence in the file's own verdict, which the status band counts.
pub const CUSTOM_WITHOUT_COMMAND: &str = "terminal_agent is Custom but terminal_agent_command is not set, so no agent can start. Fix: choose Oh My Pi (or Off) for Terminal agent in Settings, or add terminal_agent_command to this file";

/// **The file that chose `Custom` where no layer gives it a command** (#425), or `None` when the agent is anything else or the command
/// is set somewhere. The project's layer is the later one, so it is the culprit when it chose Custom itself; otherwise the user's.
pub fn custom_agent_without_command(user: Option<&SettingsLayer>, project: Option<&SettingsLayer>) -> Option<AgentFile> {
    let chosen = [user, project].into_iter().flatten().filter_map(|layer| layer.terminal_agent).next_back();
    if chosen != Some(TerminalAgent::Custom) {
        return None;
    }
    if [user, project].into_iter().flatten().any(|layer| layer.terminal_agent_command.is_some()) {
        return None;
    }
    if project.is_some_and(|layer| layer.terminal_agent == Some(TerminalAgent::Custom)) {
        Some(AgentFile::Project)
    } else {
        Some(AgentFile::User)
    }
}

/// One file that read: applied, or refused for the Custom agent it chose without a command.
fn agent_verdict(file: String, source: AgentFile, culprit: Option<AgentFile>) -> KindVerdict {
    if culprit == Some(source) {
        KindVerdict::refused(KIND, file, Some("terminal_agent".to_owned()), CUSTOM_WITHOUT_COMMAND)
    } else {
        KindVerdict::applied(KIND, file)
    }
}

/// **Look at both settings files, decide what changed, and keep [`EditorSettings`] the layers' sum.**
///
/// A file that does not read is refused and its layer stays as it last read; a file that is gone has no
/// layer. A verdict is said once per distinct finding, so a frame in between decides nothing.
pub fn sync_settings(
    time: Res<Time>,
    sources: SettingsSources,
    mut view: ResMut<SettingsView>,
    mut settings: ResMut<EditorSettings>,
    mut reloads: Option<ResMut<EditorReloads>>,
    mut state: Option<ResMut<EditorState>>,
    mut last_culprit: Local<Option<AgentFile>>,
) {
    let dt = time.delta_secs();
    let mut looks: Vec<(String, Look, AgentFile)> = Vec::new();
    // Every file this frame can name, looked at or not, so a verdict that depends on the *other* file can be said again.
    let mut named: Vec<(String, AgentFile)> = Vec::new();
    if let Some(file) = &sources.user_file {
        let name = file.0.to_string_lossy().replace('\\', "/");
        named.push((name.clone(), AgentFile::User));
        if let Some(look) = view.user.look(&file.0, &name, dt) {
            looks.push((name, look, AgentFile::User));
        }
    }
    if let Some(project) = &sources.project {
        let path = project.project_dir.join(PROJECT_FILE);
        let name = crate::map_files::file_name(project, &path);
        named.push((name.clone(), AgentFile::Project));
        if let Some(look) = view.project.look(&path, &name, dt) {
            looks.push((name, look, AgentFile::Project));
        }
    } else if view.project.path.is_some() {
        view.project = Watch::default();
    }
    // **The file that chose a Custom agent and gave it no command** (#425), judged on both layers together: a project that
    // chooses Custom is fine when the user's file holds the command.
    let culprit = custom_agent_without_command(view.user.good.as_ref(), view.project.good.as_ref());
    let looked: Vec<AgentFile> = looks.iter().map(|(_, _, source)| *source).collect();
    for (file, look, source) in looks {
        let verdict = match look {
            Look::Applied => agent_verdict(file, source, culprit),
            Look::Gone => {
                if let Some(reloads) = reloads.as_deref_mut() {
                    reloads.forget_file(KIND, &file);
                }
                continue;
            }
            Look::Refused { field, reason } => {
                KindVerdict::refused(KIND, file, field, format!("{reason}; the last good settings stay"))
            }
        };
        if verdict.outcome != Outcome::Applied
            && let Some(state) = state.as_deref_mut()
        {
            state.status.problem(format!("{}: refused", verdict.sentence()));
        }
        if let Some(reloads) = reloads.as_deref_mut() {
            reloads.decide(vec![verdict]);
        }
    }
    // **The other file's verdict follows**: a project file that now sets the command clears the user file's problem, and one that
    // chooses Custom without it raises it, though that other file did not change. A file whose own text changed but read as `Applied`
    // again (the fix a person pressed) says nothing new as a look, so the verdict is also decided again whenever the culprit moved.
    let moved = culprit != *last_culprit;
    *last_culprit = culprit;
    if !looked.is_empty() || moved {
        for (file, source) in named {
            let good = match source {
                AgentFile::User => view.user.good.is_some(),
                AgentFile::Project => view.project.good.is_some(),
            };
            if !good || looked.contains(&source) {
                continue;
            }
            if let Some(reloads) = reloads.as_deref_mut() {
                reloads.decide(vec![agent_verdict(file, source, culprit)]);
            }
        }
    }
    let resolved = EditorSettings::resolve_on_this_machine(
        view.user.good.as_ref(),
        view.project.good.as_ref(),
        sources.machine_agent(),
    );
    if *settings != resolved {
        *settings = resolved;
    }
}
