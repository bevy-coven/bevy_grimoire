//! **A reference to a piece follows the piece when a reload draws it again** (#201). A person is not a
//! placement: a reload never touches them, so their position and ties survive an edit to any piece. But
//! a person points at the pieces of the map (a home, a workplace), and a piece an edit draws again has a
//! new `Entity`, so without a hook every such reference would name an entity that is gone.
//! `follow_respawn::<C>` says, once per kind of reference, how `C` reads the entity it names, and a reload
//! rewrites each `C` that names a piece drawn again. The components here stand in for a society's
//! `Home(Entity)` and a list of named places; they are not those crates' types, which a game cannot give
//! a `MapEntities` impl (orphan rule), and that is why the hook takes a closure.
//!
//! Headless: footprint-only descriptors draw as marker slabs, so no glTF and no GPU is needed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_grimoire::live::{GrimoireLiveApp, GrimoireLivePlugin};
use bevy_grimoire::{GrimoirePlugin, GrimoireWorld, Placement};
use bevy_grimoire_core::descriptor::{Descriptor, Extent};
use bevy_grimoire_core::kits::{Bind, KITS_VERSION, Kits, Lattice};
use bevy_grimoire_core::library::{LIBRARY_VERSION, Library};
use bevy_grimoire_core::map::{MAP_VERSION, Map, Placed};
use bevy_grimoire_core::project_data::ProjectData;

/// Where a person lives: one entity, as a society's `Home` is.
#[derive(Component, Debug, PartialEq)]
struct Home(Entity);

/// The places bound to a person by name: several entities, as a society's `BoundPlaces` is.
#[derive(Component, Debug, PartialEq)]
struct Places(Vec<(String, Entity)>);

/// The people a person knows: other people, never placements.
#[derive(Component, Debug, PartialEq)]
struct Ties(Vec<Entity>);

/// The people whose `Home` the engine flagged as changed, per frame, after the log was last cleared.
#[derive(Resource, Clone, Default)]
struct Flagged(Arc<Mutex<Vec<Entity>>>);

fn log_changed_homes(changed: Query<Entity, Changed<Home>>, flagged: Res<Flagged>) {
    flagged.0.lock().unwrap().extend(changed.iter());
}

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
    Map { version: MAP_VERSION, name: "yard".to_owned(), bounds: (100.0, 10.0, 100.0), placements, ..Map::default() }
}

struct Project {
    grimoire: PathBuf,
}

impl Project {
    fn new(tag: &str) -> Project {
        let dir = std::env::temp_dir().join(format!("grimoire_live_references_{tag}_{}", std::process::id()));
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
/// frame, where a game registers how it follows a respawn.
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
        .insert_resource(Flagged::default())
        .add_systems(Update, log_changed_homes)
        .insert_resource(world);
    hooks(&mut app);
    app.update();
    app
}

fn follow_homes_and_places(app: &mut App) {
    app.follow_respawn::<Home>(|home, respawned| home.0 = respawned.follow(home.0));
    app.follow_respawn::<Places>(|places, respawned| {
        for (_, place) in &mut places.0 {
            *place = respawned.follow(*place);
        }
    });
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

/// One frame, so what the spawns flagged is logged, then the log is emptied: what is flagged after this
/// is the reload's doing.
fn forget_flags(app: &mut App) {
    app.update();
    app.world().resource::<Flagged>().0.lock().unwrap().clear();
}

struct People {
    anna: Entity,
    ben: Entity,
}

/// Two people who know each other, at their own places, living in `house_a` and `house_b`.
fn spawn_people(app: &mut App, at: &HashMap<String, Entity>) -> People {
    let anna = app.world_mut().spawn((Transform::from_xyz(1.5, 0.0, 1.5), Home(at["house_a"]))).id();
    let ben = app.world_mut().spawn((Transform::from_xyz(8.5, 0.0, 1.5), Home(at["house_b"]))).id();
    app.world_mut().entity_mut(anna).insert((Ties(vec![ben]), Places(vec![("lamp".to_owned(), at["lamp"])])));
    app.world_mut().entity_mut(ben).insert(Ties(vec![anna]));
    People { anna, ben }
}

fn yard(house_a_at: (f32, f32)) -> Map {
    let mut lamp = row("lamp", "kit/crate", (house_a_at.0, 1.0));
    lamp.on = Some("house_a".to_owned());
    map(vec![
        row("house_a", "kit/table", house_a_at),
        lamp,
        row("house_b", "kit/table", (8.0, 1.0)),
        row("well", "kit/crate", (4.0, 8.0)),
    ])
}

#[test]
fn an_edit_to_an_unrelated_piece_leaves_every_person_and_what_they_point_at_alone() {
    let project = Project::new("unrelated");
    project.write_map(&yard((1.0, 1.0)));
    let mut app = app(&project, |app| follow_homes_and_places(app));
    let at = entities(&mut app);
    let people = spawn_people(&mut app, &at);
    forget_flags(&mut app);

    let mut edited = yard((1.0, 1.0));
    edited.placements[3].at = (5.0, 8.0);
    project.write_map(&edited);
    settle(&mut app);

    let after = entities(&mut app);
    assert_eq!(after, at, "only the well was edited, so no piece has a new entity");
    assert_eq!(app.world().get::<Transform>(people.anna).map(|t| t.translation), Some(Vec3::new(1.5, 0.0, 1.5)));
    assert_eq!(app.world().get::<Home>(people.anna), Some(&Home(at["house_a"])));
    assert_eq!(app.world().get::<Home>(people.ben), Some(&Home(at["house_b"])));
    assert_eq!(app.world().get::<Ties>(people.anna), Some(&Ties(vec![people.ben])));
    assert_eq!(app.world().get::<Ties>(people.ben), Some(&Ties(vec![people.anna])));
    assert!(app.world().resource::<Flagged>().0.lock().unwrap().is_empty(), "no reference was rewritten, so none is flagged as changed");
}

#[test]
fn a_reference_follows_the_piece_it_names_when_a_reload_draws_it_again() {
    let project = Project::new("follow");
    project.write_map(&yard((1.0, 1.0)));
    let mut app = app(&project, |app| follow_homes_and_places(app));
    let at = entities(&mut app);
    let people = spawn_people(&mut app, &at);
    forget_flags(&mut app);

    // House A moves (and the lamp on it is drawn again with it); House B is untouched.
    project.write_map(&yard((2.0, 1.0)));
    settle(&mut app);

    let after = entities(&mut app);
    assert_ne!(after["house_a"], at["house_a"], "the moved house is drawn again");
    assert_ne!(after["lamp"], at["lamp"], "and so is what rests on it");
    assert_eq!(after["house_b"], at["house_b"]);

    assert_eq!(app.world().get::<Home>(people.anna), Some(&Home(after["house_a"])), "Anna's home is the new entity");
    assert_eq!(
        app.world().get::<Places>(people.anna),
        Some(&Places(vec![("lamp".to_owned(), after["lamp"])])),
        "a list of places is followed entry by entry, and the rider follows with its host"
    );
    assert_eq!(app.world().get::<Home>(people.ben), Some(&Home(at["house_b"])), "Ben's house was not drawn again");
    assert_eq!(
        app.world().get::<Placement>(app.world().get::<Home>(people.anna).unwrap().0).map(|p| p.0.as_str()),
        Some("house_a"),
        "the reference names a live entity that stands for the same piece"
    );

    assert_eq!(app.world().get::<Transform>(people.anna).map(|t| t.translation), Some(Vec3::new(1.5, 0.0, 1.5)), "the person did not move");
    assert_eq!(app.world().get::<Ties>(people.anna), Some(&Ties(vec![people.ben])));
    assert_eq!(app.world().get::<Ties>(people.ben), Some(&Ties(vec![people.anna])));
    assert_eq!(
        *app.world().resource::<Flagged>().0.lock().unwrap(),
        vec![people.anna],
        "only the reference that was rewritten is flagged as changed"
    );
}

#[test]
fn a_reference_follows_a_piece_through_a_rename_and_a_new_kind_in_separate_saves() {
    let project = Project::new("kinds");
    project.write_map(&yard((1.0, 1.0)));
    let mut app = app(&project, |app| follow_homes_and_places(app));
    let at = entities(&mut app);
    let people = spawn_people(&mut app, &at);

    // Save one: House B is renamed. Its entity is kept, so the reference needs no rewrite.
    let mut renamed = yard((1.0, 1.0));
    renamed.placements[2].id = "cottage".to_owned();
    project.write_map(&renamed);
    settle(&mut app);
    let after = entities(&mut app);
    assert_eq!(after["cottage"], at["house_b"], "a rename keeps the entity");
    assert_eq!(app.world().get::<Home>(people.ben), Some(&Home(at["house_b"])));

    // Save two: the cottage becomes another kind of piece, which is drawn again.
    renamed.placements[2].descriptor = "kit/crate".to_owned();
    project.write_map(&renamed);
    settle(&mut app);
    let after = entities(&mut app);
    assert_ne!(after["cottage"], at["house_b"]);
    assert_eq!(app.world().get::<Home>(people.ben), Some(&Home(after["cottage"])));
}

#[test]
fn a_reference_to_a_removed_piece_is_left_as_it_was_and_one_nobody_registered_is_not_touched() {
    let project = Project::new("removed");
    project.write_map(&yard((1.0, 1.0)));
    // Only `Home` is registered; `Places` is not.
    let mut app = app(&project, |app| {
        app.follow_respawn::<Home>(|home, respawned| home.0 = respawned.follow(home.0));
    });
    let at = entities(&mut app);
    let people = spawn_people(&mut app, &at);

    // House B is removed; House A moves, so the lamp and the house are drawn again.
    let mut edited = yard((2.0, 1.0));
    edited.placements.remove(2);
    project.write_map(&edited);
    settle(&mut app);

    let after = entities(&mut app);
    assert!(!after.contains_key("house_b"));
    assert_eq!(app.world().get::<Home>(people.ben), Some(&Home(at["house_b"])), "nothing stands for a removed piece, so the reference is not invented");
    assert!(app.world().get_entity(at["house_b"]).is_err(), "and it names an entity that is gone: the game's to handle");
    assert_eq!(app.world().get::<Home>(people.anna), Some(&Home(after["house_a"])));
    assert_eq!(
        app.world().get::<Places>(people.anna),
        Some(&Places(vec![("lamp".to_owned(), at["lamp"])])),
        "a kind of reference the game did not register is left as it was"
    );
}
