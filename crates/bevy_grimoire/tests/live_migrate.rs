//! **One migration hook per kind of state, not one per call site** (#201). A reload draws a piece
//! again when its definition changed (a move, a turn, a new kind), and the entity that stood for it is
//! despawned. State a game hung on that entity survives only if the game says, once, how it moves from
//! the old entity to the new one: `carry_across_respawn::<C>()` for a component that is copied as it
//! is, `migrate_on_respawn` for anything that needs more. These tests prove the hook runs for exactly
//! the pieces a reload draws again, with the old entity still alive and the new one fully drawn.
//!
//! Headless: footprint-only descriptors draw as marker slabs, so no glTF and no GPU is needed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_grimoire::live::{GrimoireLiveApp, GrimoireLivePlugin};
use bevy_grimoire::{GrimoirePlugin, GrimoireWorld, OfDescriptor, Placement};
use bevy_grimoire_core::descriptor::{Descriptor, Extent};
use bevy_grimoire_core::kits::{Bind, KITS_VERSION, Kits, Lattice};
use bevy_grimoire_core::library::{LIBRARY_VERSION, Library};
use bevy_grimoire_core::map::{MAP_VERSION, Map, Placed};
use bevy_grimoire_core::project_data::ProjectData;

/// State a game hung on a placement's entity, and registered to be carried.
#[derive(Component, Clone, Debug, PartialEq)]
struct Booked(u32);

/// State a game hung on a placement's entity and did not register: it is not carried.
#[derive(Component, Clone, Debug, PartialEq)]
struct Mood(u32);

fn piece(id: &str, footprint: (f32, f32)) -> Descriptor {
    Descriptor {
        id: id.to_owned(),
        extent: Extent { footprint: Some(footprint), height: Some(1.0) },
        ..Descriptor::default()
    }
}

fn row(id: &str, descriptor: &str, at: (f32, f32)) -> Placed {
    Placed { id: id.to_owned(), descriptor: descriptor.to_owned(), at, ..Placed::default() }
}

fn map(placements: Vec<Placed>) -> Map {
    Map {
        version: MAP_VERSION,
        name: "yard".to_owned(),
        bounds: (100.0, 10.0, 100.0),
        placements,
        ..Map::default()
    }
}

struct Project {
    grimoire: PathBuf,
}

impl Project {
    fn new(tag: &str) -> Project {
        let dir = std::env::temp_dir().join(format!("grimoire_live_migrate_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let grimoire = dir.join("assets/grimoire");
        std::fs::create_dir_all(grimoire.join("kit")).unwrap();
        std::fs::create_dir_all(grimoire.join("maps")).unwrap();
        let kits = Kits {
            version: KITS_VERSION,
            note: None,
            lattice: Lattice::default(),
            bind: vec![Bind { namespace: "kit".to_owned(), dir: "kit".to_owned() }],
            bash: Vec::new(),
            authoring: Some("kit".to_owned()),
        };
        std::fs::write(grimoire.join("kits.ron"), kits.to_ron().unwrap()).unwrap();
        std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/grimoire/vocab.ron"), grimoire.join("vocab.ron"))
            .unwrap();
        std::fs::write(grimoire.join("kit/project.ron"), "(\n    version: 2,\n    note: None,\n    patches: [],\n)").unwrap();
        let library = Library {
            version: LIBRARY_VERSION,
            note: None,
            descriptors: vec![piece("kit/crate", (1.0, 1.0)), piece("kit/table", (2.0, 1.0))],
        };
        std::fs::write(grimoire.join("kit/library.ron"), library.to_ron().unwrap()).unwrap();
        Project { grimoire }
    }

    /// Writes with a modification time later than anything written before, so a fast test is
    /// never defeated by a coarse filesystem clock.
    fn write_map(&self, m: &Map) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static TICK: AtomicU64 = AtomicU64::new(1);
        let path = self.grimoire.join("maps/yard.map.ron");
        std::fs::write(&path, m.to_ron().unwrap()).unwrap();
        let t = SystemTime::now() + Duration::from_secs(TICK.fetch_add(10, Ordering::SeqCst));
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(t).unwrap();
    }
}

/// An app on `project`'s `yard` map. `hooks` runs after the plugin is added and before the first
/// frame, where a game registers what it carries.
fn app(project: &Project, hooks: impl FnOnce(&mut App)) -> App {
    let data = ProjectData::load(&project.grimoire, &["yard"]).unwrap_or_else(|e| panic!("{e}"));
    let world = GrimoireWorld::with_compositions(
        data.bound.library.clone(),
        data.maps["yard"].clone(),
        data.vocab.clone(),
        &[],
        data.kits.lattice,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .add_plugins(GrimoirePlugin)
        .add_plugins(GrimoireLivePlugin { dir: project.grimoire.clone(), maps: vec!["yard".to_owned()] })
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(600)))
        .insert_resource(world);
    hooks(&mut app);
    app.update();
    app
}

fn entities(app: &mut App) -> HashMap<String, Entity> {
    app.world_mut().query::<(Entity, &Placement)>().iter(app.world()).map(|(e, p)| (p.0.clone(), e)).collect()
}

/// Steps frames until the watcher has had time to notice and settle.
fn settle(app: &mut App) {
    for _ in 0..8 {
        app.update();
    }
}

#[test]
fn a_registered_component_follows_a_piece_that_is_drawn_again() {
    let project = Project::new("move");
    project.write_map(&map(vec![
        row("crate_a", "kit/crate", (1.0, 1.0)),
        row("crate_b", "kit/crate", (4.0, 1.0)),
    ]));
    let mut app = app(&project, |app| {
        app.carry_across_respawn::<Booked>();
    });
    let before = entities(&mut app);
    for id in ["crate_a", "crate_b"] {
        app.world_mut().entity_mut(before[id]).insert((Booked(7), Mood(3)));
    }

    project.write_map(&map(vec![
        row("crate_a", "kit/crate", (1.0, 1.0)),
        row("crate_b", "kit/crate", (6.0, 1.0)),
    ]));
    settle(&mut app);

    let after = entities(&mut app);
    assert_ne!(after["crate_b"], before["crate_b"], "a moved piece is drawn again");
    assert_eq!(app.world().get::<Booked>(after["crate_b"]), Some(&Booked(7)), "the registered state moved to the new entity");
    assert_eq!(app.world().get::<Mood>(after["crate_b"]), None, "a component nobody registered is not carried");
    assert_eq!(after["crate_a"], before["crate_a"], "an untouched piece keeps its entity");
    assert_eq!(app.world().get::<Mood>(after["crate_a"]), Some(&Mood(3)), "and everything on it");
}

#[test]
fn a_changed_kind_carries_state_to_the_new_kind() {
    let project = Project::new("kind");
    project.write_map(&map(vec![row("crate_b", "kit/crate", (4.0, 1.0))]));
    let mut app = app(&project, |app| {
        app.carry_across_respawn::<Booked>();
    });
    let before = entities(&mut app);
    app.world_mut().entity_mut(before["crate_b"]).insert(Booked(11));

    project.write_map(&map(vec![row("crate_b", "kit/table", (4.0, 1.0))]));
    settle(&mut app);

    let after = entities(&mut app);
    assert_ne!(after["crate_b"], before["crate_b"]);
    assert_eq!(app.world().get::<OfDescriptor>(after["crate_b"]).map(|d| d.0.as_str()), Some("kit/table"));
    assert_eq!(app.world().get::<Booked>(after["crate_b"]), Some(&Booked(11)));
}

#[test]
fn a_hook_runs_once_per_piece_drawn_again_with_the_old_entity_alive_and_the_new_one_drawn() {
    let project = Project::new("hook");
    project.write_map(&map(vec![
        row("crate_a", "kit/crate", (1.0, 1.0)),
        row("crate_b", "kit/crate", (4.0, 1.0)),
        row("crate_c", "kit/crate", (8.0, 1.0)),
    ]));
    // Each call logs (id of the old entity, kind of the new entity) and puts the old state doubled on the new one.
    let seen: Arc<Mutex<Vec<(String, Option<String>)>>> = Arc::default();
    let log = Arc::clone(&seen);
    let mut app = app(&project, move |app| {
        app.migrate_on_respawn(move |world, old, new| {
            let was = world.get::<Placement>(old).map(|p| p.0.clone()).expect("the old entity is still alive");
            let kind = world.get::<OfDescriptor>(new).map(|d| d.0.clone());
            if let Some(Booked(n)) = world.get::<Booked>(old).cloned() {
                world.entity_mut(new).insert(Booked(n * 2));
            }
            log.lock().unwrap().push((was, kind));
        });
    });
    let before = entities(&mut app);
    app.world_mut().entity_mut(before["crate_b"]).insert(Booked(5));

    project.write_map(&map(vec![
        row("crate_a", "kit/crate", (1.0, 1.0)),
        row("crate_b", "kit/table", (4.0, 1.0)),
        row("crate_c", "kit/crate", (8.0, 1.0)),
    ]));
    settle(&mut app);

    assert_eq!(*seen.lock().unwrap(), vec![("crate_b".to_owned(), Some("kit/table".to_owned()))], "one call, for the one piece drawn again");
    let after = entities(&mut app);
    assert_eq!(app.world().get::<Booked>(after["crate_b"]), Some(&Booked(10)), "the hook decides what arrives");
    assert!(app.world().get_entity(before["crate_b"]).is_err(), "the old entity is gone once the hook has run");
}

#[test]
fn a_rider_drawn_again_with_its_host_carries_its_state_too() {
    let project = Project::new("rider");
    let mut lamp = row("lamp", "kit/crate", (5.0, 1.0));
    lamp.on = Some("table".to_owned());
    project.write_map(&map(vec![row("table", "kit/table", (5.0, 1.0)), lamp.clone()]));
    let mut app = app(&project, |app| {
        app.carry_across_respawn::<Booked>();
    });
    let before = entities(&mut app);
    app.world_mut().entity_mut(before["table"]).insert(Booked(1));
    app.world_mut().entity_mut(before["lamp"]).insert(Booked(2));

    project.write_map(&map(vec![row("table", "kit/table", (5.5, 1.0)), lamp]));
    settle(&mut app);

    let after = entities(&mut app);
    assert_ne!(after["table"], before["table"]);
    assert_ne!(after["lamp"], before["lamp"], "the rider follows its host");
    assert_eq!(app.world().get::<Booked>(after["table"]), Some(&Booked(1)));
    assert_eq!(app.world().get::<Booked>(after["lamp"]), Some(&Booked(2)));
}

#[test]
fn a_reload_that_draws_nothing_again_runs_no_hook_and_a_removed_piece_has_nothing_to_carry_to() {
    let project = Project::new("quiet");
    let rows = vec![row("crate_a", "kit/crate", (1.0, 1.0)), row("crate_b", "kit/crate", (4.0, 1.0))];
    project.write_map(&map(rows.clone()));
    let calls: Arc<Mutex<Vec<(Entity, Entity)>>> = Arc::default();
    let log = Arc::clone(&calls);
    let mut app = app(&project, move |app| {
        app.migrate_on_respawn(move |_, old, new| log.lock().unwrap().push((old, new)));
    });

    project.write_map(&map(rows.clone()));
    settle(&mut app);
    assert!(calls.lock().unwrap().is_empty(), "the same content again draws nothing, so nothing migrates");

    project.write_map(&map(vec![rows[0].clone()]));
    settle(&mut app);
    assert!(calls.lock().unwrap().is_empty(), "a removed piece is despawned with nothing to migrate to");
    assert_eq!(entities(&mut app).len(), 1);
}
