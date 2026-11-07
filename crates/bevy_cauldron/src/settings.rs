//! **The editor's settings, in three layers that change the running editor** (UI-70, #248).
//!
//! 1. The built-in defaults ([`EditorSettings::default`]).
//! 2. The user's file, `<config_dir>/Grimoire/settings.ron` ([`user_file`]).
//! 3. The project's file, `<project>/assets/grimoire/editor.ron` (kind `editor_settings` in
//!    `reload::KINDS`).
//!
//! **Precedence is per key, and the later layer wins.** A file is a [`SettingsLayer`]: every key is an
//! `Option`, so a file says only what it sets, and a key a layer leaves out falls through to the layer
//! below. `(version: 1, text_scale: Some(1.5))` in the user's file and `(version: 1, reduced_motion:
//! Some(true))` in the project's give both; the same key in both gives the project's.
//!
//! **Live.** [`sync_settings`] looks at both files while the editor runs (size and modification time
//! each frame, the text once a second regardless, as `oplog_view` does) and re-resolves
//! [`EditorSettings`]. Deleting a key, or the file, falls back to the layer below at once. A file that
//! does not read (not RON, an unknown key, a key of the wrong type, a value out of range, another
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

const USER_FILE: &str = "settings.ron";
/// The project's file, under the project directory (`assets/grimoire`).
pub const PROJECT_FILE: &str = "editor.ron";

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

/// **The agent the terminal's first tab starts**, connected to the editor's MCP server and proved with
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
    /// The map's grid is drawn in the viewport.
    pub show_grid: bool,
    /// How strongly the map's grid is drawn: 1.0 is the skin's grid colour, less is fainter.
    pub grid_strength: f32,
    /// The triangle count is shown at the top right (#302). Off until asked for: it is a number for
    /// someone tuning a scene, and the first thing a newcomer reads at the top of the window.
    pub show_statistics: bool,
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
    /// The agent the terminal's first tab starts, connected to the editor's tools. Off by default: a
    /// shell is what a terminal opens until the person picks an agent.
    pub terminal_agent: TerminalAgent,
    /// The shell line `TerminalAgent::Custom` types. Deliberately not in the settings catalogue: the
    /// Settings screen has no text control (#324), so this key is set in the file only.
    pub terminal_agent_command: Option<String>,
    /// **The shell's arrangement, as a person left it** (UI-33, #227): a `bevy_gooey` `DockLayout` of
    /// this door's panels, written by the editor when a divider or a tab is moved and read back on
    /// the next start. `None` is `ui::shell_layout` — the default, so a person who has never moved
    /// anything has no key in their file.
    pub shell_layout: Option<bevy_gooey::DockLayout>,
}

impl Default for EditorSettings {
    fn default() -> Self {
        EditorSettings {
            theme: Theme::default(),
            text_scale: 1.0,
            interface_scale: 1.0,
            reduced_motion: false,
            show_grid: true,
            grid_strength: 1.0,
            show_statistics: false,
            keymap: BTreeMap::new(),
            terminal_text_scale: 1.0,
            terminal_cursor: TerminalCursor::default(),
            terminal_cursor_blink: true,
            terminal_scrollback: crate::terminal::SCROLLBACK,
            terminal_agent: TerminalAgent::default(),
            terminal_agent_command: None,
            shell_layout: None,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_grid: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_strength: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_statistics: Option<bool>,
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
    /// The shell's arrangement (UI-33, #227). Not in the settings catalogue: it is a thing a person
    /// drags, not one they type, and Window ▸ Reset layout is the way back to the default.
    #[serde(skip_serializing_if = "Option::is_none")]
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
            show_grid: None,
            grid_strength: None,
            show_statistics: None,
            keymap: BTreeMap::new(),
            terminal_text_scale: None,
            terminal_cursor: None,
            terminal_cursor_blink: None,
            terminal_scrollback: None,
            terminal_agent: None,
            terminal_agent_command: None,
            shell_layout: None,
        }
    }
}

impl SettingsLayer {
    /// **Reads a file's text as a layer**, refusing with the file and the field when it does not read,
    /// is another version, or holds a value no editor can use.
    pub fn read(file: &str, text: &str) -> Result<SettingsLayer, Refusal> {
        let layer: SettingsLayer = reload_check::read(KIND, file, text)?;
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
        let mut out = EditorSettings::default();
        for layer in [user, project].into_iter().flatten() {
            out.theme = layer.theme.unwrap_or(out.theme);
            out.text_scale = layer.text_scale.unwrap_or(out.text_scale);
            out.interface_scale = layer.interface_scale.unwrap_or(out.interface_scale);
            out.reduced_motion = layer.reduced_motion.unwrap_or(out.reduced_motion);
            out.show_grid = layer.show_grid.unwrap_or(out.show_grid);
            out.grid_strength = layer.grid_strength.unwrap_or(out.grid_strength);
            out.show_statistics = layer.show_statistics.unwrap_or(out.show_statistics);
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
            // **The whole arrangement, not a key of it**: a layout is one value, and a later layer
            // that states one replaces the earlier's (the same rule as `theme`).
            if let Some(layout) = &layer.shell_layout {
                out.shell_layout = Some(layout.clone());
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

/// **Look at both settings files, decide what changed, and keep [`EditorSettings`] the layers' sum.**
///
/// A file that does not read is refused and its layer stays as it last read; a file that is gone has no
/// layer. A verdict is said once per distinct finding, so a frame in between decides nothing.
pub fn sync_settings(
    time: Res<Time>,
    project: Option<Res<Project>>,
    user_file: Option<Res<UserSettingsFile>>,
    mut view: ResMut<SettingsView>,
    mut settings: ResMut<EditorSettings>,
    mut reloads: Option<ResMut<EditorReloads>>,
    mut state: Option<ResMut<EditorState>>,
) {
    let dt = time.delta_secs();
    let mut looks: Vec<(String, Look)> = Vec::new();
    if let Some(file) = user_file {
        let name = file.0.to_string_lossy().replace('\\', "/");
        if let Some(look) = view.user.look(&file.0, &name, dt) {
            looks.push((name, look));
        }
    }
    if let Some(project) = project {
        let path = project.project_dir.join(PROJECT_FILE);
        let name = crate::map_files::file_name(&project, &path);
        if let Some(look) = view.project.look(&path, &name, dt) {
            looks.push((name, look));
        }
    } else if view.project.path.is_some() {
        view.project = Watch::default();
    }
    for (file, look) in looks {
        let verdict = match look {
            Look::Applied => KindVerdict::applied(KIND, file),
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
    let resolved = EditorSettings::resolve(view.user.good.as_ref(), view.project.good.as_ref());
    if *settings != resolved {
        *settings = resolved;
    }
}
