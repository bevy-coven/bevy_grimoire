//! **The planted forest, drawn in the viewport with the crate's own trees** (#102).
//!
//! `forest_scatter` writes `<grimoire_dir>/forest/<map>.ron` and `tree_edit` writes
//! `<grimoire_dir>/trees/<name>.tree.ron`; until now nothing showed either, so an author planted blind.
//! This draws the forest from those files, the way a game reads them, and follows them when they
//! change on disk (a stroke, a placed tree, a tree edit, an undo, an agent's edit): they are polled
//! twice a second, like [`crate::terrain_view`], and the trees respawned only when a file's
//! modification time moved.
//!
//! Every tree is `bevy_flora`'s, never a mesh the editor builds:
//! - a **scattered** tree (no seed of its own) is a [`PooledTree`] of its species' [`TreePool`], built
//!   in the background, so a forest costs the crate's instanced draws and levels of detail;
//! - a tree **placed by hand** (an age and a seed) is a [`ProceduralTree`] grown from its species with
//!   that seed, at that [`TreeAge`]: the same seed grows the same tree here and in the game.
//!
//! A species is a preset name (`oak`) or a `.tree.ron` path under the project's grimoire folder
//! (`trees/old_oak.tree.ron`, what `tree_edit` writes); editing that file regrows every tree of it,
//! placed or pooled. A tree stands on the drawn ground, which sits [`crate::editor::ground_drop`]
//! under the datum.

use std::collections::HashMap;
use std::path::{Component as PathPart, Path, PathBuf};
use std::time::SystemTime;

use bevy::prelude::*;
use bevy_flora::{BackgroundPools, LodSettings, PooledTree, ProceduralTree, TreeAge, TreeParams, TreePool, TreePreset};
use bevy_grimoire_core::live::KindVerdict;
use bevy_grimoire_core::reload_check;

use crate::ops_forest::Forest;
use crate::project::{OpenMap, Project};

/// Variants in a species' preview pool; a scattered tree draws `variant % POOL_VARIANTS`.
pub const POOL_VARIANTS: u32 = 4;

/// Seconds between looks at the forest's files.
const POLL_S: f32 = 0.5;

/// A drawn tree, by its id in the forest file.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub struct ForestView {
    pub id: u32,
}

/// Draws the open map's forest with `bevy_flora`'s plugin, on the Map door.
pub struct ForestViewPlugin;

impl Plugin for ForestViewPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<bevy_flora::FloraPlugin>() {
            app.add_plugins(bevy_flora::FloraPlugin);
        }
        app.add_systems(
            Update,
            sync_forest_view.run_if(in_state(crate::screen::Screen::Editor)).run_if(crate::tiles::Door::map_door_is_open),
        );
    }
}

/// The `.tree.ron` a species names, under the grimoire folder: `None` for a name that is not such a
/// path, or one that would leave the folder.
fn species_file(project: &Project, species: &str) -> Option<PathBuf> {
    let rel = Path::new(species);
    (species.ends_with(".tree.ron") && rel.components().all(|c| matches!(c, PathPart::Normal(_)))).then(|| project.grimoire_dir.join(rel))
}

/// **The tree a species names**: a preset by its name (`oak`), or a `.tree.ron` under the project's
/// grimoire folder (`trees/old_oak.tree.ron`), read with the crate's own reader.
pub fn species_params(project: &Project, species: &str) -> Result<TreeParams, String> {
    if let Some(preset) = TreePreset::ALL.iter().find(|p| p.name() == species) {
        return Ok(preset.params());
    }
    let path = species_file(project, species).ok_or_else(|| format!("`{species}` is neither a bevy_flora preset nor a .tree.ron path in the project"))?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    TreeParams::from_ron(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// What the drawn trees were spawned from; a change in any of it respawns them.
#[derive(PartialEq)]
struct Seen {
    map_path: PathBuf,
    forest: Option<SystemTime>,
    /// Each `.tree.ron` the drawn forest's species were read from, with its modification time.
    files: Vec<(PathBuf, Option<SystemTime>)>,
    origin: [u32; 3],
    drop: u32,
}

#[derive(Default)]
pub struct ForestState {
    since: f32,
    seen: Option<Seen>,
    drawn: Vec<Entity>,
    /// A species' preview pool, with the modification time of the file it was built from.
    pools: HashMap<String, (Option<SystemTime>, Handle<TreePool>)>,
}

/// **Draw the open map's forest**, and redraw when its file, or a tree file it uses, changes.
pub fn sync_forest_view(
    time: Res<Time>,
    project: Res<Project>,
    open: Res<OpenMap>,
    mut commands: Commands,
    mut background: BackgroundPools,
    mut reloads: Option<ResMut<crate::live::EditorReloads>>,
    mut state: Local<ForestState>,
) {
    let state = &mut *state;
    state.since += time.delta_secs();
    if state.seen.is_some() && state.since < POLL_S {
        return;
    }
    state.since = 0.0;
    let name = open.map_path.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".map.ron")).unwrap_or_default();
    let forest_path = project.grimoire_dir.join("forest").join(format!("{name}.ron"));
    let drop = crate::editor::ground_drop(&project);
    let origin = open.map.origin;
    let files_now = state.seen.as_ref().map(|s| s.files.iter().map(|(p, _)| (p.clone(), modified(p))).collect()).unwrap_or_default();
    let mut now = Seen {
        map_path: open.map_path.clone(),
        forest: modified(&forest_path),
        files: files_now,
        origin: [origin.0.to_bits(), origin.1.to_bits(), origin.2.to_bits()],
        drop: drop.to_bits(),
    };
    if state.seen.as_ref() == Some(&now) {
        return;
    }
    // No file is an empty forest. A file that does not parse (broken by hand) keeps what is drawn, and
    // is not read again until its modification time moves. Each edit of the file is one verdict, in
    // `grimoire_state.reloads`, with the file and the field; a redraw because a species or the origin
    // changed re-reads the file but does not decide it again.
    let edited = state.seen.as_ref().is_none_or(|s| s.forest != now.forest);
    let file = forest_path.strip_prefix(&project.project_dir).unwrap_or(&forest_path).to_string_lossy().replace('\\', "/");
    let forest: Forest = match std::fs::read_to_string(&forest_path) {
        Err(_) => {
            if let Some(reloads) = reloads.as_mut().filter(|_| edited) {
                reloads.forget(&["forest"]);
            }
            Forest::default()
        }
        Ok(text) => match reload_check::read::<Forest>("forest", &file, &text) {
            Ok(forest) => {
                if let Some(reloads) = reloads.as_mut().filter(|_| edited) {
                    reloads.forget(&["forest"]);
                    reloads.decide(vec![KindVerdict::applied("forest", file)]);
                }
                forest
            }
            Err(refusal) => {
                warn!("forest view: {}; the trees drawn are kept", refusal.sentence());
                if let Some(reloads) = reloads.as_mut().filter(|_| edited) {
                    reloads.forget(&["forest"]);
                    reloads.decide(vec![KindVerdict::refused("forest", refusal.file, refusal.field, refusal.reason)]);
                }
                state.seen = Some(now);
                return;
            }
        },
    };
    for e in state.drawn.drain(..) {
        commands.entity(e).despawn();
    }
    now.files.clear();
    let mut species: HashMap<&str, Option<(TreeParams, Option<SystemTime>)>> = HashMap::new();
    for tree in &forest.trees {
        let kind = species.entry(tree.species.as_str()).or_insert_with(|| {
            let stamp = species_file(&project, &tree.species).map(|file| {
                let stamp = modified(&file);
                now.files.push((file, stamp));
                stamp
            });
            match species_params(&project, &tree.species) {
                Ok(params) => Some((params, stamp.flatten())),
                Err(e) => {
                    warn!("forest view: {e}; its trees are not drawn");
                    None
                }
            }
        });
        let Some((params, stamp)) = &*kind else { continue };
        let at = Transform::from_xyz(origin.0 + tree.at.0, origin.1 + tree.ground_m - drop, origin.2 + tree.at.1)
            .with_rotation(Quat::from_rotation_y(tree.yaw))
            .with_scale(Vec3::splat(tree.scale));
        let drawn = match tree.seed {
            Some(seed) => {
                let mut grown = params.clone();
                grown.seed = seed;
                commands.spawn((ForestView { id: tree.id }, ProceduralTree(grown), TreeAge(tree.age_years.unwrap_or(f32::INFINITY)), at))
            }
            None => {
                let pool = match state.pools.get(&tree.species) {
                    Some((built_from, pool)) if built_from == stamp => pool.clone(),
                    _ => {
                        let pool = background.build(params, POOL_VARIANTS, f32::INFINITY, &LodSettings::default());
                        state.pools.insert(tree.species.clone(), (*stamp, pool.clone()));
                        pool
                    }
                };
                commands.spawn((ForestView { id: tree.id }, PooledTree { pool, variant: tree.variant }, at))
            }
        };
        state.drawn.push(drawn.id());
    }
    state.pools.retain(|name, _| species.contains_key(name.as_str()));
    state.seen = Some(now);
}
