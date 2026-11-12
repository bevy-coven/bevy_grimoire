//! **The audio panel** (S-01): a project's sound as the crate's own data, with its mix drawn live and every cue
//! auditionable.
//!
//! The file is `<project>/family/bevy_siren.yaml` — `bevy_siren`'s own `SirenSettings`, `CueTable` and `MusicScore`,
//! read and written through their own `serde` derives. There is no second schema: the editor writes the crate's
//! types, so the file a person edits here is the file a game ships.
//!
//! # What it draws
//!
//! - A row per bus: its level from [`SirenMix::db`], the duck on it from `ducked`, and its live voices from `voices`.
//!   The same numbers a game's own meter reads, because it is the same resource.
//! - Every cue with its bus, its mode and its priority, and an **Audition** button on the selected one that writes
//!   [`Play::cue`] — the door a game uses. The log then shows `Played` or `Refused`, so a cue that cannot play says
//!   what it is missing rather than doing nothing.
//! - The tail of [`SirenLog`], so "why is the music quiet" has an answer on the screen.
//!
//! # A file that does not parse
//!
//! Is a problem on the status line, naming the file and what is wrong with it — never a silently defaulted mix. The
//! editor keeps the last good settings in the world while the file is broken, which is what the family layer's hot
//! reload does for every crate.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_grimoire_core::family::settings_dir;

use crate::editor::EditorState;
use crate::project::Project;

/// The crate's settings file, under the project's `family/`.
pub const FILE: &str = "bevy_siren.yaml";

/// The panel's own state: what was read, what is selected, and whether it is open.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct AudioPanel {
    /// Whether the panel is open. A panel, not a viewport overlay: it costs nothing when closed.
    pub open: bool,
    /// The bus list from the open project's file, in the file's order.
    pub buses: Vec<String>,
    /// The cue names, in the file's order: the audition target list.
    pub cues: Vec<String>,
    /// The cue an **Audition** would play.
    pub selected: usize,
    /// The project root the last read came from, so a different project re-reads.
    pub root: Option<std::path::PathBuf>,
    /// The bytes last read, so a frame that changed nothing costs nothing and a hand edit is picked up once.
    pub text: String,
}

impl AudioPanel {
    /// The cue **Audition** would play, if the list has one.
    pub fn selected_cue(&self) -> Option<&str> {
        self.cues.get(self.selected).map(String::as_str)
    }

    /// Moves the selection by `step`, wrapping.
    pub fn step(&mut self, step: isize) {
        if self.cues.is_empty() {
            return;
        }
        let len = self.cues.len() as isize;
        self.selected = (((self.selected as isize + step) % len + len) % len) as usize;
    }
}

/// Audition the selected cue, the top of the panel's one row of buttons.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AuditionButton;

/// The AUDIO section of the controls panel.
pub fn spawn_section(s: &mut ChildSpawnerCommands) {
    use crate::chrome::{Severity, button, section};
    section(s, "AUDIO");
    button(s, AuditionButton, "Audition cue", Severity::Plain);
}

/// **Read the project's sound** when the project changes and when its file changes on disk — once per change, never
/// per frame. A file that does not parse is a problem naming the field, and the previous settings stay in force.
///
/// A missing file is not a problem: a project may ship no sound yet, and the crate's defaults are a real mix. The
/// panel still names the buses and the cues from the crate's own tables, so it has rows to draw.
pub fn read_audio(
    mut panel: ResMut<AudioPanel>,
    project: Option<Res<Project>>,
    mut settings: ResMut<bevy_siren::SirenSettings>,
    mut table: ResMut<bevy_siren::CueTable>,
    mut state: ResMut<EditorState>,
) {
    let Some(project) = project else {
        return;
    };
    if panel.root.as_deref() == Some(project.root.as_path())
        && !project.is_changed()
        && !panel.is_changed()
    {
        return;
    }
    let reopened = panel.root.as_deref() != Some(project.root.as_path());
    panel.root = Some(project.root.clone());
    let file = settings_dir(&project.root).join(FILE);
    let Ok(text) = std::fs::read_to_string(&file) else {
        if reopened {
            panel.buses = settings
                .bus_names()
                .iter()
                .map(|b| (*b).to_owned())
                .collect();
            panel.cues = table.names().map(str::to_owned).collect();
            panel.text.clear();
        }
        return;
    };
    if !reopened && panel.text == text {
        return;
    }
    panel.text.clone_from(&text);
    match read_file(&text) {
        Ok((read, table_read)) => {
            for problem in read
                .problems()
                .into_iter()
                .chain(table_read.problems().into_iter().map(|p| p.to_string()))
            {
                state.status.problem(format!("family/{FILE}: {problem}"));
            }
            panel.buses = read.bus_names().iter().map(|b| (*b).to_owned()).collect();
            panel.cues = table_read.names().map(str::to_owned).collect();
            panel.selected = panel.selected.min(panel.cues.len().saturating_sub(1));
            *settings = read;
            *table = table_read;
        }
        Err(problem) => state.status.problem(problem),
    }
}

/// One file's worth of the crate's own types: the mix table and the cue table, which is what an audio panel edits.
///
/// The file is a RON tuple of the two, so a project ships one sound file and the editor has one file to write. A
/// score is a third section and is optional: a project with no adaptive music leaves it out.
pub fn read_file(text: &str) -> Result<(bevy_siren::SirenSettings, bevy_siren::CueTable), String> {
    #[derive(serde::Deserialize)]
    struct File {
        #[serde(default)]
        settings: bevy_siren::SirenSettings,
        #[serde(default)]
        cues: bevy_siren::CueTable,
    }
    let file: File = bevy_grimoire_core::yaml::from_str(text, &format!("family/{FILE}"))
        .map_err(|p| format!("family/{FILE} does not parse: {}", p.line()))?;
    Ok((file.settings, file.cues))
}

/// Writes the mix table and the cue table back as the same file, so an edit here is an edit a game ships.
pub fn write_file(
    settings: &bevy_siren::SirenSettings,
    table: &bevy_siren::CueTable,
) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct File<'a> {
        settings: &'a bevy_siren::SirenSettings,
        cues: &'a bevy_siren::CueTable,
    }
    bevy_grimoire_core::yaml::to_string(&File {
        settings,
        cues: table,
    })
    .map_err(|p| format!("family/{FILE} does not write: {}", p.line()))
}

/// **Audition** plays the selected cue through the crate's own door, and the panel's log shows what happened.
pub fn on_audition(
    activate: On<Activate>,
    buttons: Query<(), With<AuditionButton>>,
    panel: Option<Res<AudioPanel>>,
    mut plays: MessageWriter<bevy_siren::Play>,
    mut state: ResMut<EditorState>,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let Some(panel) = panel else {
        return;
    };
    let Some(cue) = panel.selected_cue() else {
        state
            .status
            .problem("no cue is selected to audition".to_owned());
        return;
    };
    plays.write(bevy_siren::Play::cue(cue));
    state.status.note(format!("auditioning `{cue}`"));
}

/// The rows the panel draws: one line per bus, from this frame's mix.
pub fn bus_rows(mix: &bevy_siren::SirenMix, buses: &[String]) -> Vec<String> {
    buses
        .iter()
        .map(|bus| {
            let db = mix.bus_db(bus);
            let duck = mix.ducked.get(bus).copied().unwrap_or(0.0);
            let duck = if duck < -0.05 {
                format!("  duck {duck:+.1} dB")
            } else {
                String::new()
            };
            format!(
                "{bus:<10} {db:+6.1} dB{duck}  {} voices",
                mix.voices_on(bus)
            )
        })
        .collect()
}

/// **The panel's plugin** (S-01, #274).
///
/// It exists so `EditorUiPlugin` has one name to add, and it deliberately adds **nothing**: the panel's
/// state, its audition observer and its read system are registered beside the rest of the editor's
/// observers in `editor.rs`, where a reader looking for "what does the editor register" finds them
/// without opening this file. A second registration here would be a second `SirenPlugins` add, which
/// panics.
///
/// **Why this is worth a comment rather than an empty struct.** `editor.rs` named this type from the
/// commit that added the panel and the type was never written, so `--features bevy_siren` did not
/// compile for any caller: the feature was unreachable, and the panel's test could not build either.
pub struct AudioPanelPlugin;

impl bevy::prelude::Plugin for AudioPanelPlugin {
    fn build(&self, _app: &mut bevy::prelude::App) {}
}
