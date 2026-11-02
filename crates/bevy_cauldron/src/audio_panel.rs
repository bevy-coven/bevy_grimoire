//! **The audio panel** (#274): the project's sound, read from `bevy_siren`'s own data, with a cue you can
//! hear.
//!
//! The editor hosts `bevy_siren` the way it hosts every family crate — its settings are the crate's own
//! types, saved in `<project>/family/bevy_siren.ron`, never a second schema here. This module draws what
//! those types hold, and puts one **Audition** control on the first cue, because a cue table you cannot
//! hear is a file you are reading rather than a mix you are tuning.
//!
//! **Two files, one panel.** `bevy_siren.ron` is the *mix* ([`SirenSettings`]: the master, the voice
//! budget, the buses and their ducks). `bevy_siren_cues.ron` is the *cue table* ([`CueTable`]): what can
//! be played, by name. Either one failing to parse is a problem on the status line naming the file, never
//! a silently defaulted mix — a panel that quietly showed an empty cue list for a broken file would look
//! exactly like a project with no sounds.
//!
//! **The audition is the crate's own path.** The button writes a [`Play`] for the cue's name, which is
//! what a game does; the crate answers with [`Played`] or [`Refused`], and the panel shows whichever it
//! got. So a refusal here is the same refusal a game would meet, and there is no second playback path.
//!
//! **What this does not do yet**, recorded on the issue rather than hidden: edit a bus or a cue here and
//! write it back (the crate's `to_ron` is ready for it), and a per-voice meter. Both are on #274's "what
//! the editor may need next" list.

use bevy::prelude::*;
use bevy_siren::{CueTable, Play, Played, Refused, SirenSettings, SirenSounds};

use crate::chrome::{self, Side};
use crate::project::Project;

/// The family key these files are saved under, as `family_host` names it.
pub const CRATE: &str = "bevy_siren";

/// The cue table's file, beside the mix.
pub const CUES: &str = "bevy_siren_cues.ron";

/// **The panel's own state**: what the mix holds, which cues exist, and what the last audition did.
///
/// A resource rather than fields on `EditorState`, because `EditorState` is rebuilt on every door crossing
/// and this is about one panel's contents.
#[derive(Resource, Default)]
pub struct AudioPanel {
    /// One line per bus, as it reads in the file: `music  -6.0 dB, up to 4 voices`.
    pub buses: Vec<String>,
    /// The cue names, in the table's order.
    pub cues: Vec<String>,
    /// The cue the Audition control plays: the first in the table, or empty when there is none.
    pub auditioning: String,
    /// What the last audition did, in a sentence, or empty before the first.
    pub said: String,
    /// The table as read, so its recordings can be loaded and an audition has a handle to play.
    pub table: Option<CueTable>,
}

pub struct AudioPanelPlugin;

impl Plugin for AudioPanelPlugin {
    fn build(&self, app: &mut App) {
        // **The crate, then the panel.** `SirenPlugin` owns `CueTable`, `SirenSounds` and the `Play`
        // message; adding it here rather than in `main` keeps the feature honest — a build without
        // `bevy_siren` has no audio panel and no audio systems.
        if !app.is_plugin_added::<bevy_siren::SirenPlugin>() {
            app.add_plugins(bevy_siren::SirenPlugin);
        }
        app.init_resource::<AudioPanel>()
            .add_systems(OnEnter(crate::screen::Screen::Editor), spawn_panel.after(chrome::FrameSystems))
            .add_systems(
                Update,
                (read_the_files, load_the_sounds.after(read_the_files), show_it, hear_the_answer),
            )
            .add_observer(on_audition);
    }
}

/// **Read the two files and say what they hold**, once per project.
///
/// The **family layer** is what watches for edits (`family_host::reload_settings` refuses a bad file and
/// keeps the last good one), so this reads the files it accepted rather than a second watcher: `done`
/// keys on the root and is cleared by [`forget`] when the layer reports a change. That is the same shape
/// `environment` and `ui_skin` use.
pub fn read_the_files(
    project: Option<Res<Project>>,
    settings: Res<crate::family_host::FamilySettings>,
    mut panel: ResMut<AudioPanel>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    mut done: Local<Option<(std::path::PathBuf, usize)>>,
) {
    let Some(project) = project else { return };
    // The layer's own text is the last *good* one: a file that does not parse never reaches it.
    let stamp = (project.root.clone(), settings.text.len());
    if done.as_ref() == Some(&stamp) {
        return;
    }
    *done = Some(stamp);

    let mut buses = Vec::new();
    if let Some(text) = settings.text.get(CRATE) {
        match SirenSettings::from_ron(text) {
            Ok(mix) => {
                // **The master first**, because a bus level means nothing without it: `-6 dB` on a bus
                // under a master of `-3` is `-9` out of the speaker.
                buses.push(format!("master {:+.1} dB, up to {} voices", mix.master_db, mix.max_voices));
                for bus in &mix.buses {
                    let mute = if bus.mute { ", muted" } else { "" };
                    let cap = if bus.max_voices == 0 {
                        "the table's budget".to_owned()
                    } else {
                        format!("up to {} voices", bus.max_voices)
                    };
                    buses.push(format!("{} {:+.1} dB, {cap}{mute}", bus.name, bus.gain_db));
                }
            }
            Err(e) => {
                if let Some(state) = state.as_deref_mut() {
                    state.status.problem(format!("family/{CRATE}.ron: {e}; the previous mix stays"));
                }
            }
        }
    }

    let mut cues = Vec::new();
    let mut table = None;
    // No cue table is not a fault: a project may have a mix and no sounds yet.
    if let Ok(text) = std::fs::read_to_string(project.root.join("family").join(CUES)) {
        match CueTable::from_ron(&text) {
            Ok(read) => {
                cues.extend(read.names().map(str::to_owned));
                table = Some(read);
            }
            Err(e) => {
                if let Some(state) = state.as_deref_mut() {
                    state.status.problem(format!("family/{CUES}: {e}"));
                }
            }
        }
    }

    let auditioning = cues.first().cloned().unwrap_or_default();
    if panel.buses != buses || panel.cues != cues || panel.auditioning != auditioning {
        panel.buses = buses;
        panel.cues = cues;
        panel.auditioning = auditioning;
        panel.table = table;
    }
}

/// **Spawn the panel**, in the left dock beside the Meshes pane: it is about the project's data, which is
/// what that door is for.
pub fn spawn_panel(mut commands: Commands, frame: Option<Res<chrome::Frame>>) {
    let Some(frame) = frame else { return };
    let root = chrome::panel_root(&mut commands, &frame, Side::Left, chrome::CONTROLS_W, true, true)
        .insert(AudioRoot)
        .id();
    commands.entity(root).with_children(|p| {
        chrome::title(p, "AUDIO");
        chrome::scroll_list(p, AudioBody).with_children(|p| {
            p.spawn((
                AudioMix,
                Text::new(String::new()),
                TextColor(chrome::DIM),
                chrome::font(chrome::text::LABEL),
            ));
            p.spawn((
                AudioCues,
                Text::new(String::new()),
                TextColor(chrome::DIM),
                chrome::font(chrome::text::LABEL),
            ));
            // **A gooey button**, which spawns its own label child; `show_it` writes that child's
            // text. Adding a second `children!` here is a duplicate `Children` and a panic at spawn.
            p.spawn((AuditionRow, bevy_gooey::button(String::new())));
            p.spawn((
                AudioSaid,
                Text::new(String::new()),
                TextColor(chrome::DIM),
                chrome::font(chrome::text::LABEL),
            ));
        });
    });
}

/// The panel's own root, so a test can find it without knowing the dock's shape.
#[derive(Component)]
pub struct AudioRoot;

/// The scrolling body.
#[derive(Component)]
pub struct AudioBody;

/// The bus lines, as [`read_the_files`] wrote them.
#[derive(Component)]
pub struct AudioMix;

/// The cue list, one name per line.
#[derive(Component)]
pub struct AudioCues;

/// The one control that auditions: a line naming the cue it will play, with its key.
#[derive(Component)]
pub struct AuditionRow;

/// What the last audition did.
#[derive(Component)]
pub struct AudioSaid;

/// **Draw what was read.** Every write is compared before it lands: these are `Text` nodes, and
/// `no_system_writes_every_frame` is the rule the editor's panels are held to.
pub fn show_it(
    panel: Res<AudioPanel>,
    mut mix: MixLines,
    mut cues: CueLines,
    mut said: SaidLine,
    buttons: Query<&Children, With<AuditionRow>>,
    mut labels: ButtonLabel,
) {
    if !panel.is_changed() {
        return;
    }
    let mix_text = if panel.buses.is_empty() {
        "no family/bevy_siren.ron: the crate's own defaults apply".to_owned()
    } else {
        panel.buses.join("\n")
    };
    let cues_text = if panel.cues.is_empty() {
        "no cues: family/bevy_siren_cues.ron names none".to_owned()
    } else {
        panel.cues.join("\n")
    };
    let audition = if panel.auditioning.is_empty() {
        "Audition: no cue to play".to_owned()
    } else {
        format!("Audition {}", panel.auditioning)
    };

    write(mix.iter_mut(), &mix_text);
    write(cues.iter_mut(), &cues_text);
    write(said.iter_mut(), &panel.said);
    // **The button's own label is its child**: `bevy_gooey::button` spawns exactly one, and this is the
    // only text on it.
    for children in &buttons {
        for child in children.iter() {
            if let Ok(mut text) = labels.get_mut(child)
                && text.0 != audition
            {
                text.0 = audition.clone();
            }
        }
    }
}

/// The three readouts, each its own narrow query. Written as aliases because four `&mut Text` queries
/// in one system are `B0001` against each other unless their filters are disjoint, and the disjointness
/// is the thing worth naming.
type MixLines<'w, 's> = Query<'w, 's, &'static mut Text, (With<AudioMix>, Without<AudioCues>, Without<AudioSaid>)>;
type CueLines<'w, 's> = Query<'w, 's, &'static mut Text, (With<AudioCues>, Without<AudioMix>, Without<AudioSaid>)>;
type SaidLine<'w, 's> = Query<'w, 's, &'static mut Text, (With<AudioSaid>, Without<AudioMix>, Without<AudioCues>)>;
type ButtonLabel<'w, 's> =
    Query<'w, 's, &'static mut Text, (Without<AudioMix>, Without<AudioCues>, Without<AudioSaid>)>;

/// Compare before writing, which is `no_system_writes_every_frame`'s rule: these are `Text` nodes and
/// everything downstream reads their change detection.
fn write<'a>(texts: impl Iterator<Item = Mut<'a, Text>>, want: &str) {
    for mut text in texts {
        if text.0 != want {
            text.0 = want.to_owned();
        }
    }
}

/// **An audition is the crate's own `Play`**, so what a person hears from this panel is the path a game
/// takes. A refusal is reported in the panel's last line, naming what the crate said was missing.
pub fn on_audition(
    activate: On<bevy_gooey::Activate>,
    rows: Query<&AuditionRow>,
    panel: Res<AudioPanel>,
    mut play: MessageWriter<Play>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
) {
    if rows.get(activate.entity).is_err() || panel.auditioning.is_empty() {
        return;
    }
    play.write(Play::cue(panel.auditioning.clone()));
    if let Some(state) = state.as_deref_mut() {
        state.status.note(format!("auditioning {}", panel.auditioning));
    }
}

/// **Report what became of the audition**, in the panel's own line: the variant that played, or the
/// crate's reason it did not.
pub fn hear_the_answer(
    mut panel: ResMut<AudioPanel>,
    mut played: MessageReader<Played>,
    mut refused: MessageReader<Refused>,
) {
    let mut said = None;
    for answer in played.read() {
        said = Some(format!("played {} on {} ({} dB)", answer.sound, answer.bus, answer.gain_db));
    }
    for answer in refused.read() {
        said = Some(format!("{} refused: {:?}", answer.cue, answer.reason));
    }
    if let Some(said) = said
        && panel.said != said
    {
        panel.said = said;
    }
}

/// **Load the table's recordings**, so an audition has a handle to play. Runs after the file is read and
/// before the panel draws, and asks the `AssetServer` only for keys it has not loaded yet — the crate's
/// own `load_table` skips what it already holds, so a second read costs nothing.
pub fn load_the_sounds(
    panel: Res<AudioPanel>,
    server: Option<Res<AssetServer>>,
    mut sounds: Option<ResMut<SirenSounds>>,
) {
    let (Some(server), Some(sounds), Some(table)) = (server, sounds.as_mut(), panel.table.as_ref()) else {
        return;
    };
    if !panel.is_changed() {
        return;
    }
    // The project's recordings live under `assets/audio`. A project that keeps them elsewhere names a
    // path in its cue file; this is the one place the editor guesses, and it says so.
    sounds.load_table(&server, table, "audio");
}
