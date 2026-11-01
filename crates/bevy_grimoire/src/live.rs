//! **A running game follows its project's files.** [`GrimoireLivePlugin`] watches the project
//! directory, and about a second after the last write reloads it and respawns *only* the placements
//! that changed. An unchanged placement keeps its `Entity`, so whatever a game hung on it (an agent's
//! booking, a camera target, an animation) survives an edit to the next table over.
//!
//! A piece an edit does change (a move, a turn, a new kind) is drawn again: its entity is despawned and a
//! new one stands for it. What a game hung on the old entity moves across through one hook per kind of
//! state, registered on the app with [`GrimoireLiveApp::carry_across_respawn`] (a component copied as it
//! is) or [`GrimoireLiveApp::migrate_on_respawn`] (anything else), never at the call site that edits.
//! What *points at* a piece (a person's home, a list of places) is rewritten the same way, once per kind
//! of reference, with [`GrimoireLiveApp::follow_respawn`].
//!
//! The reload is `bevy_grimoire_core::project_data::load` plus `GrimoireWorld::with_compositions`, the
//! same two doors a game's first load goes through. A reload either of them refuses leaves every
//! entity and the [`GrimoireWorld`] as they were and writes no [`ProjectReloaded`].

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use bevy::ecs::component::Mutable;
use bevy::prelude::*;
use bevy_grimoire_core::live::{FileWatch, MapDiff, Verdict, diff_maps_renaming, same_but_id};
use bevy_grimoire_core::map::Map;
use bevy_grimoire_core::project_data::{self, ProjectData};

use crate::{GrimoireWorld, Placement, spawn_placements};

/// Watches `dir` (the project directory, the one holding `kits.ron`) and keeps the world in step.
///
/// `maps` is every map the game reads; [`ProjectReloaded::diffs`] has one entry per name. The map in
/// the [`GrimoireWorld`] resource is the one that is respawned, and is read from the files by its own
/// name, so it need not be listed.
pub struct GrimoireLivePlugin {
    pub dir: PathBuf,
    pub maps: Vec<String>,
}

/// One successful reload.
#[derive(Message, Clone, Debug)]
pub struct ProjectReloaded {
    /// What changed in each map, by name, expanded maps compared (a stamp's rows count).
    pub diffs: BTreeMap<String, MapDiff>,
    /// A descriptor (or its resolved masks) differs: every placement of it was respawned.
    pub library_changed: bool,
}

#[derive(Resource)]
struct Live {
    dir: PathBuf,
    maps: Vec<String>,
    watch: FileWatch,
    /// The last good version of every listed map, the baseline the next diff is against.
    last: BTreeMap<String, Map>,
    /// How many reload decisions this process has written, so a verdict is one edit's, once.
    seq: u64,
}

/// A hook run for one piece a reload drew again: the world, the entity that stood for it, the entity
/// that stands for it now.
type Migration = Arc<dyn Fn(&mut World, Entity, Entity) + Send + Sync>;

/// A hook run once for a reload that drew pieces again: the world, and where each of them stands now.
type Reference = Arc<dyn Fn(&mut World, &Respawned) + Send + Sync>;

/// The hooks a game registered, each kind in the order it registered them.
#[derive(Resource, Default)]
struct ReloadMigrations {
    pieces: Vec<Migration>,
    references: Vec<Reference>,
}

/// **Where each piece a reload drew again stands now** (#201): the entity that stood for it is gone and
/// a new one stands for it. A hook registered with [`GrimoireLiveApp::follow_respawn`] asks it.
pub struct Respawned {
    now: HashMap<Entity, Entity>,
    /// Set when [`Respawned::follow`] answered with another entity, so a rewrite that changed nothing
    /// is not flagged as a change.
    answered: Cell<bool>,
}

impl Respawned {
    /// The entity that stands for `entity` now: the new one when a reload drew `entity` again, and
    /// `entity` itself for everything else (a piece the reload kept, a piece it removed, an entity
    /// that is not a placement at all).
    pub fn follow(&self, entity: Entity) -> Entity {
        match self.now.get(&entity) {
            Some(&now) => {
                self.answered.set(true);
                now
            }
            None => entity,
        }
    }
}

/// **How a game's state follows a piece a reload draws again** (#201). A reload keeps the entity of every
/// placement whose definition did not change, and draws the rest again: a moved, turned or retyped piece
/// gets a new entity and the old one is despawned, and so does whatever rests on it. State a game hung on
/// the old entity (a booking, an animation phase, a camera target) is lost unless the game says how it
/// moves, and it says so here, once per kind of state, and not at every place that edits a piece.
///
/// A hook runs for each piece drawn again, **after** the new entity is drawn (its `Placement`,
/// `OfDescriptor`, `Transform` and `MountedOn` are in place) and **before** the old one is despawned. It
/// does not run for a piece an edit left alone (that piece keeps its entity and everything on it), for one
/// that is removed (nothing stands for it any more), or for one that is renamed and edited in the same
/// save (that is a new piece: its meaning changed).
pub trait GrimoireLiveApp {
    /// Run `migrate(world, old, new)` for each piece a reload draws again. Hooks run in registration
    /// order, and the old entity is still alive, so one can read it and write the new one.
    fn migrate_on_respawn(&mut self, migrate: impl Fn(&mut World, Entity, Entity) + Send + Sync + 'static) -> &mut Self;

    /// Copy component `C` from the old entity to the new one, when the old one has it.
    fn carry_across_respawn<C: Component + Clone>(&mut self) -> &mut Self {
        self.migrate_on_respawn(|world, old, new| {
            let Some(state) = world.get::<C>(old).cloned() else { return };
            if let Ok(mut entity) = world.get_entity_mut(new) {
                entity.insert(state);
            }
        })
    }

    /// **Rewrite the entities a component names** when a reload draws them again (#201). A person is
    /// not a placement, so a reload leaves them alone; but a person's `Home(entity)` names a placement,
    /// and a placement an edit draws again has a new entity. After each reload that drew a piece again,
    /// `follow` is called for every `C` in the world with the component and a [`Respawned`] to ask where
    /// an entity stands now:
    ///
    /// ```ignore
    /// app.follow_respawn::<Home>(|home, respawned| home.0 = respawned.follow(home.0));
    /// ```
    ///
    /// It is a closure, not `MapEntities`, because a game cannot implement that for another crate's
    /// component. A `C` is marked changed only when `follow` was answered with another entity. A
    /// reference to a piece the reload removed is left as it was: nothing stands for it.
    fn follow_respawn<C: Component<Mutability = Mutable>>(
        &mut self,
        follow: impl Fn(&mut C, &Respawned) + Send + Sync + 'static,
    ) -> &mut Self;
}

impl GrimoireLiveApp for App {
    fn migrate_on_respawn(&mut self, migrate: impl Fn(&mut World, Entity, Entity) + Send + Sync + 'static) -> &mut Self {
        self.init_resource::<ReloadMigrations>();
        self.world_mut().resource_mut::<ReloadMigrations>().pieces.push(Arc::new(migrate));
        self
    }

    fn follow_respawn<C: Component<Mutability = Mutable>>(
        &mut self,
        follow: impl Fn(&mut C, &Respawned) + Send + Sync + 'static,
    ) -> &mut Self {
        let hook = move |world: &mut World, respawned: &Respawned| {
            let mut query = world.query::<&mut C>();
            for mut component in query.iter_mut(world) {
                respawned.answered.set(false);
                follow(component.bypass_change_detection(), respawned);
                if respawned.answered.get() {
                    component.set_changed();
                }
            }
        };
        self.init_resource::<ReloadMigrations>();
        self.world_mut().resource_mut::<ReloadMigrations>().references.push(Arc::new(hook));
        self
    }
}

impl Plugin for GrimoireLivePlugin {
    fn build(&self, app: &mut App) {
        // Stat BEFORE loading: a write between the two is then a change the first poll sees.
        let watch = FileWatch::new(&self.dir, &["ron"]);
        let names: Vec<&str> = self.maps.iter().map(String::as_str).collect();
        let last = match ProjectData::load(&self.dir, &names) {
            Ok(data) => data.maps,
            Err(e) => {
                warn!("grimoire live: the project does not load yet, diffs start from nothing: {e}");
                BTreeMap::new()
            }
        };
        app.add_message::<ProjectReloaded>()
            .init_resource::<ReloadMigrations>()
            .insert_resource(Live {
                dir: self.dir.clone(),
                maps: self.maps.clone(),
                watch,
                last,
                seq: 0,
            })
            .add_systems(Update, reload);
    }
}

/// What to do to the entities: draw these (new) ids, and despawn the entities that stood for these (old)
/// ids. A reload draws first, runs the migration hooks, then despawns.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Plan {
    pub despawn: BTreeSet<String>,
    pub spawn: BTreeSet<String>,
    /// Old id to new id (#201): the entity that stood for the old id is kept and stands for the
    /// new one, so whatever a game hung on it follows the rename.
    pub renamed: BTreeMap<String, String>,
    pub library_changed: bool,
}

/// **Which placements must be drawn again to turn `old` into `new`.**
///
/// A placement is respawned when it is new, when its row differs, when its resolved height differs
/// (a table moved, so the lamp on it did not change but stands elsewhere), when its descriptor or the
/// descriptor's masks differ, or when what it rests on is respawned (a relationship points at an
/// entity, and a despawned host takes it away). A placement that only changed its id (`renamed`,
/// from [`diff_maps_renaming`]) is compared with its old row and kept when nothing else differs.
pub(crate) fn plan(old: &GrimoireWorld, new: &GrimoireWorld, renamed: &BTreeMap<String, String>) -> Plan {
    let before: HashMap<&str, usize> = old
        .map
        .placements
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id.as_str(), i))
        .collect();
    let was_of: HashMap<&str, &str> = renamed.iter().map(|(was, now)| (now.as_str(), was.as_str())).collect();
    let mut out = Plan {
        library_changed: old.library != new.library || old.masks != new.masks,
        renamed: renamed.clone(),
        ..Plan::default()
    };
    for (i, p) in new.map.placements.iter().enumerate() {
        let was = was_of.get(p.id.as_str()).copied().unwrap_or(p.id.as_str());
        let same = before.get(was).is_some_and(|&j| {
            same_but_id(&old.map.placements[j], p, renamed)
                && old.y.get(j) == new.y.get(i)
                && old.entry(&p.descriptor) == new.entry(&p.descriptor)
                && old.map.origin == new.map.origin
        });
        if !same {
            out.spawn.insert(p.id.clone());
        }
    }
    // Riders follow their host, transitively.
    loop {
        let before_len = out.spawn.len();
        for p in &new.map.placements {
            if p.on.as_ref().is_some_and(|h| out.spawn.contains(h)) {
                out.spawn.insert(p.id.clone());
            }
        }
        if out.spawn.len() == before_len {
            break;
        }
    }
    let now: BTreeSet<&str> = new.map.placements.iter().map(|p| p.id.as_str()).collect();
    for p in &old.map.placements {
        let current = renamed.get(&p.id).unwrap_or(&p.id);
        if !now.contains(current.as_str()) || out.spawn.contains(current) {
            out.despawn.insert(p.id.clone());
        }
    }
    out
}

/// Every map a reload reads: the listed ones, and the world's own map.
fn map_names(live: &Live, current: &GrimoireWorld) -> Vec<String> {
    let mut names = live.maps.clone();
    if !names.contains(&current.map.name) {
        names.push(current.map.name.clone());
    }
    names
}

/// **Write the decision on this edit**, once: the per-file verdicts (every file the load reads, with
/// the field of each one it refused) and the message, under the next sequence number.
fn decide(live: &mut Live, names: &[String], error: Option<&str>, message: String) {
    live.seq += 1;
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let kinds = project_data::verdicts(&live.dir, &names, error);
    if let Err(e) = Verdict::with_kinds(live.seq, message, kinds).write(&live.dir) {
        warn!("grimoire live: could not write the verdict: {e}");
    }
}

fn load_world(live: &Live, current: &GrimoireWorld) -> Result<(GrimoireWorld, BTreeMap<String, Map>), String> {
    let names = map_names(live, current);
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut data = ProjectData::load(&live.dir, &names)?;
    let map = data
        .maps
        .get(&current.map.name)
        .cloned()
        .ok_or_else(|| format!("map `{}` was not loaded", current.map.name))?;
    // The loader returns the expanded map with its stamps still listed; the world must not expand
    // them a second time.
    let mut map = map;
    map.stamps.clear();
    let world = GrimoireWorld::with_compositions(
        data.bound.library.clone(),
        map,
        data.vocab.clone(),
        &[],
        data.kits.lattice,
    )?;
    data.maps.retain(|name, _| live.maps.contains(name));
    Ok((world, data.maps))
}

#[allow(clippy::too_many_arguments)]
fn reload(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    world: Option<ResMut<GrimoireWorld>>,
    mut live: ResMut<Live>,
    placed: Query<(Entity, &Placement)>,
    migrations: Res<ReloadMigrations>,
    mut reloaded: MessageWriter<ProjectReloaded>,
) {
    if !live.watch.poll(time.elapsed_secs_f64()) {
        return;
    }
    let Some(mut world) = world else { return };
    let names = map_names(&live, &world);
    let (new, maps) = match load_world(&live, &world) {
        Ok(loaded) => loaded,
        Err(e) => {
            warn!("grimoire live: reload refused, keeping the last good project: {e}");
            decide(&mut live, &names, Some(&e), e.clone());
            return;
        }
    };

    // The world's own map is diffed against what is on screen; the others against their last load.
    let empty = Map::default();
    let mut diffs = BTreeMap::new();
    for (name, map) in &maps {
        let old = if *name == new.map.name { &world.map } else { live.last.get(name).unwrap_or(&empty) };
        diffs.insert(name.clone(), diff_maps_renaming(old, map));
    }
    if !diffs.contains_key(&new.map.name) {
        diffs.insert(new.map.name.clone(), diff_maps_renaming(&world.map, &new.map));
    }
    let renamed: BTreeMap<String, String> = diffs[&new.map.name].renamed.iter().cloned().collect();
    let plan = plan(&world, &new, &renamed);

    let mut kept: HashMap<String, Entity> = HashMap::new();
    // The entities that stood for a piece drawn again, by the id they had.
    let mut replaced: Vec<(&str, Entity)> = Vec::new();
    for (entity, Placement(id)) in &placed {
        if plan.despawn.contains(id) {
            replaced.push((id.as_str(), entity));
        } else if let Some(now) = plan.renamed.get(id) {
            commands.entity(entity).insert(Placement(now.clone()));
            kept.insert(now.clone(), entity);
        } else {
            kept.insert(id.clone(), entity);
        }
    }
    // A placement the old world never drew (no mesh, no footprint) has no entity to despawn, and one
    // whose entity is missing for any other reason is simply drawn.
    let drawn = spawn_placements(&mut commands, &assets, &new, Some(&plan.spawn), &kept);
    let spawned = drawn.len();

    // **One hook per kind of state** (#201): an old entity and the new entity for the same piece are
    // paired here, once, so state a game registered follows it. The hooks run after the new entity is
    // spawned and before the old one is despawned, both in this command queue's order. A piece that was
    // removed, or renamed and edited together, has no new entity under its name and is not paired.
    // What points at a piece is rewritten after the per-piece hooks, once for the whole reload.
    if !migrations.pieces.is_empty() || !migrations.references.is_empty() {
        let pairs: Vec<(Entity, Entity)> = replaced
            .iter()
            .filter_map(|&(id, old)| {
                let now = plan.renamed.get(id).map_or(id, String::as_str);
                drawn.get(now).map(|&new| (old, new))
            })
            .collect();
        if !pairs.is_empty() {
            let pieces = migrations.pieces.clone();
            let references = migrations.references.clone();
            commands.queue(move |world: &mut World| {
                for &(old, new) in &pairs {
                    for hook in &pieces {
                        hook(world, old, new);
                    }
                }
                if !references.is_empty() {
                    let respawned = Respawned { now: pairs.into_iter().collect(), answered: Cell::new(false) };
                    for hook in &references {
                        hook(world, &respawned);
                    }
                }
            });
        }
    }
    for (_, old) in replaced {
        commands.entity(old).despawn();
    }

    let summary = diffs
        .iter()
        .map(|(name, d)| format!("{name}: {}", d.summary()))
        .collect::<Vec<_>>()
        .join("; ");
    info!("grimoire live: reloaded ({summary}); respawned {spawned}");
    decide(&mut live, &names, None, summary);
    *world = new;
    live.last = maps;
    reloaded.write(ProjectReloaded {
        diffs,
        library_changed: plan.library_changed,
    });
}
