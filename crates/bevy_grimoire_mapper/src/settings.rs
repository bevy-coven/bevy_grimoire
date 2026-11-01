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

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use bevy::prelude::*;
use bevy_grimoire_core::live::{KindVerdict, Outcome};
use bevy_grimoire_core::reload_check::{self, Refusal};
use serde::{Deserialize, Serialize};

use crate::editor::EditorState;
use crate::live::EditorReloads;
use crate::project::Project;

pub const SETTINGS_VERSION: u32 = 1;

/// The id of the reload-contract kind both layers' verdicts carry.
pub const KIND: &str = "editor_settings";

/// The directory under the platform's config directory that holds the editor's own files.
const APP_DIR: &str = "Grimoire";
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
    dirs::config_dir().map(|d| d.join(APP_DIR).join(USER_FILE))
}

/// The look of the editor's chrome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    ToolDark,
    Parchment,
    HighContrast,
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
    /// Animation is skipped: a change lands without easing.
    pub reduced_motion: bool,
}

impl Default for EditorSettings {
    fn default() -> Self {
        EditorSettings { theme: Theme::default(), text_scale: 1.0, interface_scale: 1.0, reduced_motion: false }
    }
}

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
}

impl Default for SettingsLayer {
    fn default() -> Self {
        SettingsLayer { version: SETTINGS_VERSION, theme: None, text_scale: None, interface_scale: None, reduced_motion: None }
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
        for (field, value) in [("text_scale", layer.text_scale), ("interface_scale", layer.interface_scale)] {
            if let Some(v) = value
                && !(SCALE_RANGE.0..=SCALE_RANGE.1).contains(&v)
            {
                return Err(refuse(field, format!("{v} is outside {} to {}", SCALE_RANGE.0, SCALE_RANGE.1)));
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
