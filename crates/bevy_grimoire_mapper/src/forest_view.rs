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
//!
//! **A tree file that does not read keeps its trees** (#200, H3). Each edit of a `.tree.ron` is decided
//! once, with the crate's own reader, and the view holds the last good value in memory: a refusal
//! leaves the trees standing as that value drew them and is reported in `grimoire_state.reloads` as
//! kind `trees`, with the file and the field. A file that is gone is not a refusal; its trees are not
//! drawn.

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

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// What the view holds of one `.tree.ron` a drawn species is read from.
struct HeldTree {
    /// The modification time the file had when it was last decided, so an edit is decided once.
    read: Option<SystemTime>,
    /// The last good value, with the modification time of the file it came from: what the trees are
    /// drawn from while the file is refused. `None` before a good read and once the file is gone.
    good: Option<(TreeParams, Option<SystemTime>)>,
    /// What became of the file as it stands: `None` when there is no file.
    verdict: Option<KindVerdict>,
}

/// **Decide the tree file at `path`, once per edit**: read it again only when its modification time
/// moved since the last decision. A file that reads becomes the held value; one that does not is
/// refused and the held value stays. Whether this was a new decision.
fn decide_tree(project: &Project, path: &Path, held: &mut HashMap<PathBuf, HeldTree>) -> bool {
    let stamp = modified(path);
    if held.get(path).is_some_and(|h| h.read == stamp) {
        return false;
    }
    let file = crate::map_files::file_name(project, path);
    let entry = held.entry(path.to_owned()).or_insert(HeldTree { read: None, good: None, verdict: None });
    entry.read = stamp;
    match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            entry.good = None;
            entry.verdict = None;
        }
        Err(e) => {
            warn!("forest view: {file} cannot be read: {e}; the trees drawn from it are kept");
            entry.verdict = Some(KindVerdict::refused("trees", file, None, e.to_string()));
        }
        Ok(text) => match reload_check::read::<TreeParams>("trees", &file, &text) {
            Ok(params) => {
                entry.good = Some((params, stamp));
                entry.verdict = Some(KindVerdict::applied("trees", file));
            }
            Err(refusal) => {
                warn!("forest view: {}; the trees drawn from it are kept", refusal.sentence());
                entry.verdict = Some(KindVerdict::refused("trees", refusal.file, refusal.field, refusal.reason));
            }
        },
    }
    true
}

/// What the drawn trees were spawned from; a change in any of it respawns them.
#[derive(PartialEq)]
struct Seen {
    map_path: PathBuf,
    forest: Option<SystemTime>,
    /// Each `.tree.ron` the drawn forest's species were read from, with the modification time of the
    /// last good read (`None`: no good read). A refused edit does not move it, so it redraws nothing.
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
    /// The tree files the drawn forest reads, as last decided.
    trees: HashMap<PathBuf, HeldTree>,
}

/// **The verdicts one look of the view hands to the editor's record, as one decision**, so an edit that
/// touches the forest and a tree file is one number in `grimoire_state.reloads.seq`.
#[derive(Default)]
struct Verdicts {
    forget: Vec<&'static str>,
    decided: Vec<KindVerdict>,
}

impl Verdicts {
    /// The verdict of every tree file in `files`, in place of the earlier ones.
    fn trees(&mut self, held: &HashMap<PathBuf, HeldTree>, files: &[(PathBuf, Option<SystemTime>)]) {
        self.forget.push("trees");
        self.decided.extend(files.iter().filter_map(|(path, _)| held.get(path)?.verdict.clone()));
    }

    fn commit(self, reloads: &mut Option<ResMut<crate::live::EditorReloads>>) {
        let Some(reloads) = reloads.as_mut() else { return };
        if self.forget.is_empty() && self.decided.is_empty() {
            return;
        }
        reloads.forget(&self.forget);
        if !self.decided.is_empty() {
            reloads.decide(self.decided);
        }
    }
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
    // Each tree file the drawn forest reads is decided once per edit whether or not the trees are
    // drawn again: a refused file leaves them standing, from the last good value.
    let used: Vec<PathBuf> = state.seen.as_ref().map(|s| s.files.iter().map(|(p, _)| p.clone()).collect()).unwrap_or_default();
    let mut trees_decided = false;
    for path in &used {
        trees_decided |= decide_tree(&project, path, &mut state.trees);
    }
    let files_now = used.iter().map(|p| (p.clone(), state.trees.get(p).and_then(|h| h.good.as_ref()).and_then(|g| g.1))).collect();
    let mut now = Seen {
        map_path: open.map_path.clone(),
        forest: modified(&forest_path),
        files: files_now,
        origin: [origin.0.to_bits(), origin.1.to_bits(), origin.2.to_bits()],
        drop: drop.to_bits(),
    };
    let mut verdicts = Verdicts::default();
    if state.seen.as_ref() == Some(&now) {
        if trees_decided {
            verdicts.trees(&state.trees, &now.files);
        }
        verdicts.commit(&mut reloads);
        return;
    }
    // No file is an empty forest. A file that does not parse (broken by hand) keeps what is drawn, and
    // is not read again until its modification time moves. Each edit of the file is one verdict, in
    // `grimoire_state.reloads`, with the file and the field; a redraw because a species or the origin
    // changed re-reads the file but does not decide it again.
    let edited = state.seen.as_ref().is_none_or(|s| s.forest != now.forest || s.map_path != now.map_path);
    let file = crate::map_files::file_name(&project, &forest_path);
    let forest: Forest = match std::fs::read_to_string(&forest_path) {
        Err(_) => {
            if edited {
                verdicts.forget.push("forest");
            }
            Forest::default()
        }
        Ok(text) => match reload_check::read::<Forest>("forest", &file, &text) {
            Ok(forest) => {
                if edited {
                    verdicts.forget.push("forest");
                    verdicts.decided.push(KindVerdict::applied("forest", file));
                }
                forest
            }
            Err(refusal) => {
                warn!("forest view: {}; the trees drawn are kept", refusal.sentence());
                if edited {
                    verdicts.forget.push("forest");
                    verdicts.decided.push(KindVerdict::refused("forest", refusal.file, refusal.field, refusal.reason));
                }
                if trees_decided {
                    verdicts.trees(&state.trees, &now.files);
                }
                verdicts.commit(&mut reloads);
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
            if let Some(preset) = TreePreset::ALL.iter().find(|p| p.name() == tree.species) {
                return Some((preset.params(), None));
            }
            let Some(file) = species_file(&project, &tree.species) else {
                warn!("forest view: `{}` is neither a bevy_flora preset nor a .tree.ron path in the project; its trees are not drawn", tree.species);
                return None;
            };
            trees_decided |= decide_tree(&project, &file, &mut state.trees);
            let good = state.trees.get(&file).and_then(|h| h.good.clone());
            if good.is_none() {
                warn!("forest view: {} has no tree that reads; its trees are not drawn", file.display());
            }
            now.files.push((file, good.as_ref().and_then(|g| g.1)));
            good
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
    state.trees.retain(|path, _| now.files.iter().any(|(file, _)| file == path));
    // The tree verdicts are said again when a file was decided, or the set of files changed (a species
    // came or went with the forest file's edit), so none names a file nothing draws from.
    if trees_decided || used.iter().ne(now.files.iter().map(|(p, _)| p)) {
        verdicts.trees(&state.trees, &now.files);
    }
    verdicts.commit(&mut reloads);
    state.seen = Some(now);
}
