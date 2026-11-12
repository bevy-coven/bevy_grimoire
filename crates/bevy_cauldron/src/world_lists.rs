//! **What the map and the project hold, drawn as lists in the World and Sound workspaces** (NV-11, #392, #393, #394, #395).
//!
//! `docs/ui/navigation.md` measured four destinations that nothing drew: a project's **sound cues** (behind the `bevy_siren` feature),
//! a map's **lights** (behind `bevy_lights`), and a map's **water** and **forest** (operations, `water_place` and `forest_scatter`, written
//! through the World workspace's terrain line, with no panel). Each is a *list of data the project already has*: a cue is a name in
//! `family/bevy_siren.ron`, a light is a placement's `bevy_lights` family value, the water plan and the forest are files the operations
//! write. None needs the family crate to be **read**; only to be edited or auditioned. So each list is drawn in every build, from the
//! file, and the family crate's own controls (the audition button, the pool budget) stay behind their feature.
//!
//! # Read-only on purpose
//!
//! The lists do not write. A second way to change what a list shows would be a second path to the same operation; what a list adds is the
//! thing nothing drew before, which is **what is there now**.
//!
//! # Filled when the file changes
//!
//! Each section is spawned with its workspace's panel (`workspace_panels`) and filled by a system that reads the file twice a second and
//! rebuilds the rows only when what they say changed, as `problems_view` does for the PROBLEMS list. A file that does not read is a row that
//! names it, never a list that quietly says there is nothing.

use bevy::prelude::*;

use crate::chrome;
use crate::project::Project;
use crate::screen::Screen;
use crate::workspace::Workspace;

/// How often the lists look at their files again, in seconds.
const READ_SECS: f32 = 0.5;

/// The crate's sound file under the project's `family/`, the same name `audio_panel::FILE` holds in a build that has the crate.
pub const SIREN_FILE: &str = "bevy_siren.ron";

/// What the CUES list says when the project has none.
pub const NO_CUES: &str = "no cues in this project";

/// Where a cue is edited: not here.
pub const CUES_HINT: &str = "cues live in family/bevy_siren.ron";

/// The CUES list, in the Sound workspace's panel.
#[derive(Component, Clone, Copy)]
pub struct CueList;

pub struct WorldListsPlugin;

impl Plugin for WorldListsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, refresh_cues.run_if(Workspace::sound_is_open).run_if(in_state(Screen::Editor)));
    }
}

/// **The cues list of the AUDIO section**, drawn in every build under the heading the caller draws.
pub fn spawn_cue_list(p: &mut ChildSpawnerCommands) {
    p.spawn(bevy_gooey::label(CUES_HINT));
    p.spawn((CueList, list_node()));
}

/// A list's column.
fn list_node() -> Node {
    Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(chrome::GAP_TIGHT), ..default() }
}

/// **The cue names in `family/bevy_siren.ron`**, read as RON text rather than as the crate's `CueTable`, so a build without `bevy_siren`
/// lists them too. The file is `(settings: .., cues: (cues: [(name: "..", ..), ..]))`; a file that does not read is an error naming what is wrong.
pub fn cue_names(text: &str) -> Result<Vec<String>, String> {
    let file: ron::Value = ron::from_str(text).map_err(|e| format!("family/{SIREN_FILE} does not parse: {e}"))?;
    let field = |value: &ron::Value, name: &str| -> Option<ron::Value> {
        match value {
            ron::Value::Map(fields) => fields.iter().find(|(k, _)| matches!(k, ron::Value::String(s) if s == name)).map(|(_, v)| v.clone()),
            _ => None,
        }
    };
    // A project with no cues table has none: the crate's own default is an empty table.
    let Some(table) = field(&file, "cues") else { return Ok(Vec::new()) };
    let Some(ron::Value::Seq(cues)) = field(&table, "cues") else { return Ok(Vec::new()) };
    Ok(cues
        .iter()
        .filter_map(|cue| match field(cue, "name") {
            Some(ron::Value::String(name)) => Some(name),
            _ => None,
        })
        .collect())
}

/// **The sound cues of a project in words**: a row per cue, or the line that says there are none. A file that does not read is its own row.
pub fn cue_rows(project: &Project) -> Vec<String> {
    let file = bevy_grimoire_core::family::settings_dir(&project.root).join(SIREN_FILE);
    let text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return vec![NO_CUES.to_owned()],
        Err(e) => return vec![format!("family/{SIREN_FILE} cannot be read: {e}")],
    };
    match cue_names(&text) {
        Ok(names) if names.is_empty() => vec![NO_CUES.to_owned()],
        Ok(names) => names,
        Err(why) => vec![why],
    }
}

/// **Keep one list equal to its rows.** The rows are rebuilt only when what they say changed, or when a door change respawned the list
/// and left it without children.
fn keep<L: Component>(
    commands: &mut Commands,
    time: &Time,
    shown: &mut (Option<Vec<String>>, f32),
    lists: &Query<(Entity, Has<Children>), With<L>>,
    now: impl FnOnce() -> Option<Vec<String>>,
) {
    let (held, since) = shown;
    *since += time.delta_secs();
    let unfilled = lists.iter().any(|(_, filled)| !filled);
    if held.is_some() && *since < READ_SECS && !unfilled {
        return;
    }
    *since = 0.0;
    let Some(rows) = now() else { return };
    if held.as_ref() == Some(&rows) && !unfilled {
        return;
    }
    for (list, _) in lists {
        commands.entity(list).despawn_related::<Children>();
        commands.entity(list).with_children(|p| {
            for row in &rows {
                p.spawn(bevy_gooey::label(row.clone()));
            }
        });
    }
    *held = Some(rows);
}

fn refresh_cues(
    mut commands: Commands,
    time: Res<Time>,
    project: Option<Res<Project>>,
    lists: Query<(Entity, Has<Children>), With<CueList>>,
    mut shown: Local<(Option<Vec<String>>, f32)>,
) {
    keep(&mut commands, &time, &mut shown, &lists, || Some(cue_rows(project.as_deref()?)));
}
