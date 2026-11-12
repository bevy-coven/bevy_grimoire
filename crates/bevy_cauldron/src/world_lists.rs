//! **What the map and the project hold, drawn as lists in the World and Sound workspaces** (NV-11, #392, #393, #394, #395).
//!
//! `docs/ui/navigation.md` measured four destinations that nothing drew: a project's **sound cues** (behind the `bevy_siren` feature),
//! a map's **lights** (behind `bevy_lights`), and a map's **water** and **forest** (operations, `water_place` and `forest_scatter`, written
//! through the World workspace's terrain line, with no panel). Each is a *list of data the project already has*: a cue is a name in
//! `family/bevy_siren.yaml`, a light is a placement's `bevy_lights` family value, the water plan and the forest are files the operations
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
use bevy_grimoire_core::map::Map;
use bevy_grimoire_core::water::WaterPlan;

use crate::chrome;
use crate::ops_forest::Forest;
use crate::project::{OpenMap, Project};
use crate::screen::Screen;
use crate::workspace::Workspace;

/// How often the lists look at their files again, in seconds.
const READ_SECS: f32 = 0.5;

/// What the WATER list says when the map has none.
pub const NO_WATER: &str = "no water placed";

/// How the terrain line places water, in a line: the heading's one-line note. The line's own refusal spells the whole syntax, and a note that
/// wrapped over three lines pushed the headings below it off the panel's first screen.
pub const WATER_HINT: &str = "set on the TERRAIN line: water ...";

/// The WATER section's list, in the World workspace's panel.
#[derive(Component, Clone, Copy)]
pub struct WaterList;

/// What the FOREST list says when the map has none.
pub const NO_TREES: &str = "no trees planted";

/// How the terrain line plants a forest, in a line (see [`WATER_HINT`]).
pub const FOREST_HINT: &str = "set on the TERRAIN line: forest ...";

/// The FOREST section's list, in the World workspace's panel.
#[derive(Component, Clone, Copy)]
pub struct ForestList;

/// The most characters of a light's settings one row shows before it is cut with an ellipsis. A row is one line of the panel.
const ROW_CHARS: usize = 72;

/// The family key a placement's light fixture is saved under (`bevy_lights`), the same string `fixture::KEY` holds in a build that has the crate.
pub const LIGHTS_KEY: &str = "bevy_lights";

/// What the LIGHTS list says when the map has none.
pub const NO_LIGHTS: &str = "no light fixtures on this map";

/// Where a light is set: not here.
pub const LIGHTS_HINT: &str = "set per piece with family_set";

/// The LIGHTS section's list, in the World workspace's panel.
#[derive(Component, Clone, Copy)]
pub struct LightsList;

/// The crate's sound file under the project's `family/`, the same name `audio_panel::FILE` holds in a build that has the crate.
pub const SIREN_FILE: &str = "bevy_siren.yaml";

/// What the CUES list says when the project has none.
pub const NO_CUES: &str = "no cues in this project";

/// Where a cue is edited: not here.
pub const CUES_HINT: &str = "cues live in family/bevy_siren.yaml";

/// The CUES list, in the Sound workspace's panel.
#[derive(Component, Clone, Copy)]
pub struct CueList;

pub struct WorldListsPlugin;

impl Plugin for WorldListsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                refresh_water.run_if(Workspace::world_is_open),
                refresh_forest.run_if(Workspace::world_is_open),
                refresh_lights.run_if(Workspace::world_is_open),
                refresh_cues.run_if(Workspace::sound_is_open),
            )
                .run_if(in_state(Screen::Editor)),
        );
    }
}

/// **The WATER section**: a heading, how to place some, and what the map's plan holds.
pub fn spawn_water_section(p: &mut ChildSpawnerCommands) {
    chrome::section(p, "WATER");
    p.spawn(bevy_gooey::label(WATER_HINT));
    p.spawn((WaterList, list_node()));
}

/// **The FOREST section**: a heading, how to plant one, and what the map's forest holds.
pub fn spawn_forest_section(p: &mut ChildSpawnerCommands) {
    chrome::section(p, "FOREST");
    p.spawn(bevy_gooey::label(FOREST_HINT));
    p.spawn((ForestList, list_node()));
}

/// **What reading a map's file found**: none, the value, or why it did not read.
enum Read<T> {
    Absent,
    Value(T),
    Broken(String),
}

/// **Read one YAML file a map's operation writes**, naming it (`water/m.yaml`) when it exists and does not parse.
///
/// The project is YAML since the cutover (GD-03, #365): this read a `.ron` and parsed with `ron::from_str`, so the lists were
/// empty for every project the editor can open.
fn read_file<T: serde::de::DeserializeOwned>(path: &std::path::Path, shown_as: &str) -> Read<T> {
    match std::fs::read_to_string(path) {
        Ok(text) => match bevy_grimoire_core::yaml::from_str::<T>(&text, shown_as) {
            Ok(value) => Read::Value(value),
            Err(problem) => Read::Broken(problem.line()),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Read::Absent,
        Err(e) => Read::Broken(format!("{shown_as} cannot be read: {e}")),
    }
}

/// **The water plan in words**: a row per spring, sink and lake, in the order the plan lists them (springs, sinks, lakes).
pub fn water_rows(plan: &WaterPlan) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    for s in &plan.springs {
        rows.push(format!("spring at ({}, {}): {} m3/s, radius {} m", s.x, s.z, s.rate_m3_s, s.radius_m));
    }
    for s in &plan.sinks {
        rows.push(format!("sink at ({}, {}): {} m3/s, radius {} m", s.x, s.z, s.rate_m3_s, s.radius_m));
    }
    for l in &plan.lakes {
        rows.push(format!("lake at ({}, {}): radius {} m, level {} m", l.x, l.z, l.radius_m, l.level_m));
    }
    if rows.is_empty() {
        rows.push(NO_WATER.to_owned());
    }
    rows
}

/// **The forest in words**: how many trees, then a row per species with its count, in alphabetical order.
pub fn forest_rows(forest: &Forest) -> Vec<String> {
    if forest.trees.is_empty() {
        return vec![NO_TREES.to_owned()];
    }
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for tree in &forest.trees {
        *counts.entry(tree.species.as_str()).or_default() += 1;
    }
    let mut rows = vec![format!("{} tree(s)", forest.trees.len())];
    rows.extend(counts.into_iter().map(|(species, n)| format!("{species}: {n}")));
    rows
}

/// **The fixtures list of the LIGHTS section**: the heading is the caller's (the family crate's section draws its own, with the pool
/// button), the rows are drawn in every build.
pub fn spawn_lights_list(p: &mut ChildSpawnerCommands) {
    p.spawn(bevy_gooey::label(LIGHTS_HINT));
    p.spawn((LightsList, list_node()));
}

/// **A map's lights in words**: a row per placement that carries `bevy_lights` settings, `<id>: <settings>`, the settings on one line and cut
/// at [`ROW_CHARS`]. A build without the crate reads the same value as text: it cannot tell a flicker from a fire, and does not need to.
pub fn light_rows(map: &Map) -> Vec<String> {
    let mut rows: Vec<String> = map
        .placements
        .iter()
        .filter_map(|p| p.family.get(LIGHTS_KEY).map(|settings| format!("{}: {}", p.id, one_line(settings))))
        .collect();
    if rows.is_empty() {
        rows.push(NO_LIGHTS.to_owned());
    }
    rows
}

/// A settings value on one line: whitespace collapsed, cut to [`ROW_CHARS`] with an ellipsis.
fn one_line(text: &str) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.chars().count() <= ROW_CHARS {
        return joined;
    }
    let cut: String = joined.chars().take(ROW_CHARS.saturating_sub(1)).collect();
    format!("{cut}…")
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

/// **The cue names in `family/bevy_siren.yaml`**, read as a plain value rather than as the crate's `CueTable`, so a build without
/// `bevy_siren` lists them too. The file is a mapping of `settings:` (whatever the crate keeps) and `cues: {cues: [{name: ..}, ..]}`;
/// a file that does not read is an error naming what is wrong.
pub fn cue_names(text: &str) -> Result<Vec<String>, String> {
    // **The one YAML reader**, so this file is held to the rules every project file is, and a refusal names the field it stopped at.
    let file = bevy_grimoire_core::yaml::to_json(text, &format!("family/{SIREN_FILE}")).map_err(|p| p.line())?;
    let field = |value: &serde_json::Value, name: &str| -> Option<serde_json::Value> {
        value.as_object().and_then(|fields| fields.get(name)).cloned()
    };
    // A project with no cues table has none: the crate's own default is an empty table.
    let Some(table) = field(&file, "cues") else { return Ok(Vec::new()) };
    let Some(cues) = field(&table, "cues").and_then(|c| c.as_array().cloned()) else { return Ok(Vec::new()) };
    Ok(cues
        .iter()
        .filter_map(|cue| field(cue, "name").and_then(|n| n.as_str().map(str::to_owned)))
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

fn refresh_lights(
    mut commands: Commands,
    time: Res<Time>,
    open: Option<Res<OpenMap>>,
    lists: Query<(Entity, Has<Children>), With<LightsList>>,
    mut shown: Local<(Option<Vec<String>>, f32)>,
) {
    keep(&mut commands, &time, &mut shown, &lists, || Some(light_rows(&open.as_deref()?.map)));
}

fn refresh_water(
    mut commands: Commands,
    time: Res<Time>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    lists: Query<(Entity, Has<Children>), With<WaterList>>,
    mut shown: Local<(Option<Vec<String>>, f32)>,
) {
    keep(&mut commands, &time, &mut shown, &lists, || {
        let (project, open) = (project.as_deref()?, open.as_deref()?);
        let path = crate::ops_water::plan_path_of(project, open)?;
        let shown_as = format!("water/{}", path.file_name()?.to_string_lossy());
        Some(match read_file::<WaterPlan>(&path, &shown_as) {
            Read::Value(plan) => water_rows(&plan),
            Read::Absent => vec![NO_WATER.to_owned()],
            Read::Broken(why) => vec![why],
        })
    });
}

fn refresh_forest(
    mut commands: Commands,
    time: Res<Time>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    lists: Query<(Entity, Has<Children>), With<ForestList>>,
    mut shown: Local<(Option<Vec<String>>, f32)>,
) {
    keep(&mut commands, &time, &mut shown, &lists, || {
        let (project, open) = (project.as_deref()?, open.as_deref()?);
        let name = &open.map.name;
        let path = project.grimoire_dir.join("forest").join(format!("{name}.yaml"));
        Some(match read_file::<Forest>(&path, &format!("forest/{name}.yaml")) {
            Read::Value(forest) => forest_rows(&forest),
            Read::Absent => vec![NO_TREES.to_owned()],
            Read::Broken(why) => vec![why],
        })
    });
}
