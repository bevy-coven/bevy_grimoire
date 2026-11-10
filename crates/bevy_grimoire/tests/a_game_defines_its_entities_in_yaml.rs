//! **A game's entities are defined in YAML, and a save changes the running game** (GD-05, #367).
//!
//! The bridge is **reflection and nothing else**: a type the game registered (`register_type::<T>()` with
//! `#[reflect(Component)]`) is usable from a file; one it did not register is refused, naming the type. Each
//! test below builds a headless game that registers `Health` and `Spin`, writes the file, and asks the world
//! what happened — the `Entity` id included, because "a changed field keeps the entity" is the whole point of
//! applying in place.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_grimoire::entities::{self, Fault, GameEntities, GameEntitiesPlugin};

/// **A component the test game registers**, with the two fields the acceptance names.
#[derive(Component, Reflect, Default, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize)]
struct Health {
    current: i32,
    max: i32,
}

/// A second one, so "a file sets a component" is not satisfied by one lucky type.
#[derive(Component, Reflect, Default, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Component, Deserialize, Serialize)]
struct Spin {
    degrees_per_second: f32,
}

/// A resource, to prove `resources:` goes through the same door.
#[derive(Resource, Reflect, Default, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Resource, Deserialize, Serialize)]
struct Clock {
    elapsed: f64,
}

/// A scratch project of this test's own: its name and the process id.
fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(entities::ENTITIES_DIR)).expect("the entity folder is made");
    dir
}

fn write(root: &Path, text: &str) {
    let path = root.join(entities::ENTITIES_DIR).join(format!("main{}", entities::ENTITIES_EXT));
    std::fs::write(path, text).expect("the entity file is written");
}

/// **A headless game with the three types registered and the entity reader on.**
fn game(root: &Path) -> App {
    let mut app = App::new();
    app.init_resource::<AppTypeRegistry>();
    app.register_type::<Health>().register_type::<Spin>().register_type::<Clock>();
    // **A reflected resource needs its `ComponentId` to exist**, which the engine makes when the type is used
    // as a `Resource` — the same requirement `registry.schema`'s own `world.insert_resources` has. A game
    // already inserts its resources; this test's game inserts the one it registers.
    app.init_resource::<Clock>();
    app.add_plugins(GameEntitiesPlugin { root: root.to_path_buf() });
    app
}

/// **One frame, with the commands a `Startup` system queued applied**: `update` runs `Startup` and the
/// schedule's systems, and the queued reads reach the world at `finish`/`cleanup`.
fn frame(app: &mut App) {
    app.update();
    app.finish();
    app.cleanup();
}

/// The same read, done directly so a test can see the fault instead of an empty world.
fn read(root: &Path) -> entities::Applied {
    let mut app = game(root);
    let path = root.join(entities::ENTITIES_DIR).join(format!("main{}", entities::ENTITIES_EXT));
    let document = entities::load(&path).unwrap_or_else(|e| panic!("{}", e.line()));
    entities::apply(app.world_mut(), &path.display().to_string(), &path.display().to_string(), &document)
        .unwrap_or_else(|e| panic!("{}", e.line()))
}

/// The `Entity` the file's `door_north` stands for.
fn door(app: &App, root: &Path) -> Entity {
    let key = root.join(entities::ENTITIES_DIR).join(format!("main{}", entities::ENTITIES_EXT));
    app.world()
        .resource::<GameEntities>()
        .spawned
        .get(&key.display().to_string())
        .and_then(|by_name| by_name.get("door_north").copied())
        .unwrap_or_else(|| panic!("`door_north` was spawned for {}", key.display()))
}

const FIRST: &str = "entities:\n  door_north:\n    Health: { current: 10, max: 10 }\n    Spin: { degrees_per_second: 45.0 }\nresources:\n  Clock: { elapsed: 1.0 }\n";

#[test]
fn a_file_spawns_its_entities_and_sets_its_resources() {
    let root = project("entities_spawn");
    write(&root, FIRST);
    let applied = read(&root);
    assert_eq!(applied.spawned, vec!["door_north".to_owned()], "the file spawns its entity");
    let mut app = game(&root);
    frame(&mut app);

    let entity = door(&app, &root);
    assert_eq!(app.world().get::<Health>(entity), Some(&Health { current: 10, max: 10 }), "the file's `Health` is on the entity");
    assert_eq!(
        app.world().get::<Spin>(entity),
        Some(&Spin { degrees_per_second: 45.0 }),
        "and its `Spin`"
    );
    assert_eq!(app.world().get_resource::<Clock>(), Some(&Clock { elapsed: 1.0 }), "`resources:` sets the game's resource");

    let _ = std::fs::remove_dir_all(&root);
}

/// **The acceptance's central claim**: `Health.max` changed on disk, and after the reload the new value is on
/// **the same `Entity`**.
#[test]
fn changing_a_field_on_disk_changes_it_in_place_on_the_same_entity() {
    let root = project("entities_in_place");
    write(&root, FIRST);
    let mut app = game(&root);
    frame(&mut app);
    let before = door(&app, &root);
    assert_eq!(app.world().get::<Health>(before).map(|h| h.max), Some(10));

    // **Only `max` moves.** `current` and the `Spin` beside it must survive, which is what "applied in place"
    // means and what a respawn would lose.
    write(&root, "entities:\n  door_north:\n    Health: { current: 10, max: 25 }\n    Spin: { degrees_per_second: 45.0 }\nresources:\n  Clock: { elapsed: 1.0 }\n");
    frame(&mut app);

    let after = door(&app, &root);
    assert_eq!(after, before, "the entity is the same one, not a respawn");
    assert_eq!(app.world().get::<Health>(after).map(|h| h.max), Some(25), "the changed field is the new value");
    assert_eq!(app.world().get::<Health>(after).map(|h| h.current), Some(10), "a field the file did not change keeps its value");

    let _ = std::fs::remove_dir_all(&root);
}

/// **Removing an entity from the file despawns it, and adding one spawns it.**
#[test]
fn removing_an_entity_despawns_it_and_adding_one_spawns_it() {
    let root = project("entities_add_remove");
    write(&root, FIRST);
    let mut app = game(&root);
    frame(&mut app);
    let gone = door(&app, &root);
    assert!(app.world().get_entity(gone).is_ok());

    // The file drops `door_north` and adds `lamp_hall`.
    write(&root, "entities:\n  lamp_hall:\n    Health: { current: 5, max: 5 }\nresources:\n  Clock: { elapsed: 1.0 }\n");
    frame(&mut app);

    assert!(app.world().get_entity(gone).is_err(), "an entity the file no longer names is despawned");
    let key = root.join(entities::ENTITIES_DIR).join(format!("main{}", entities::ENTITIES_EXT));
    let added = app
        .world()
        .resource::<GameEntities>()
        .spawned
        .get(&key.display().to_string())
        .and_then(|by_name| by_name.get("lamp_hall").copied())
        .unwrap_or_else(|| panic!("`lamp_hall` was spawned"));
    assert_eq!(app.world().get::<Health>(added).map(|h| h.current), Some(5), "and the new one carries its components");

    let _ = std::fs::remove_dir_all(&root);
}

/// **An unregistered type is refused, naming the type and the line**, and the last good state stays live.
#[test]
fn an_unregistered_type_is_refused_naming_the_type_and_the_line() {
    let root = project("entities_unregistered");
    write(&root, "entities:\n  door_north:\n    Mana: { current: 3 }\n");
    let mut app = game(&root);
    frame(&mut app);

    // Nothing was spawned from a refused document, so the names are empty.
    let key = root.join(entities::ENTITIES_DIR).join(format!("main{}", entities::ENTITIES_EXT));
    let spawned = app.world().resource::<GameEntities>().spawned.get(&key.display().to_string()).cloned().unwrap_or_default();
    assert!(spawned.is_empty(), "a refused file makes no entity: {spawned:?}");

    // And the fault says what is wrong, by name and at a line.
    let document = entities::load(&key).expect("the file reads as YAML");
    let err = entities::apply(app.world_mut(), &key.display().to_string(), &key.display().to_string(), &document)
        .expect_err("an unregistered type is refused");
    let line = err.line();
    assert!(line.contains("Mana"), "the refusal names the type: {line}");
    assert!(line.contains("line "), "and states a line: {line}");
    assert!(line.contains("register_type"), "and says how to fix it: {line}");

    let _ = std::fs::remove_dir_all(&root);
}

/// **An ambiguous short path is refused, listing the full paths**, so a person is not left to guess which of
/// two types the file meant.
#[test]
fn an_ambiguous_short_path_is_refused_listing_the_full_paths() {
    // Two registered types whose short paths collide, which is what a game with a `Health` in two modules has.
    let root = project("entities_ambiguous");
    let mut app = App::new();
    app.init_resource::<AppTypeRegistry>();
    app.register_type::<first::Mode>().register_type::<second::Mode>();
    app.update();

    let document = entities::read_document("a.entities.yaml", "entities:\n  a:\n    Mode: { value: 1 }\n").expect("reads");
    let err = entities::apply(app.world_mut(), "a.entities.yaml", "a.entities.yaml", &document)
        .expect_err("an ambiguous short path is refused");
    let line = err.line();
    assert!(line.contains("more than one"), "the refusal says it is ambiguous: {line}");
    assert!(line.contains("first::Mode") && line.contains("second::Mode"), "and lists the full paths: {line}");

    let _ = std::fs::remove_dir_all(&root);
}

/// **A document that does not read keeps the last good state live**: nothing is despawned from a refused file.
#[test]
fn a_refused_document_leaves_the_last_good_state_live() {
    let root = project("entities_refused_keeps_state");
    write(&root, FIRST);
    let mut app = game(&root);
    frame(&mut app);
    let kept = door(&app, &root);
    assert!(app.world().get_entity(kept).is_ok());

    // The file now names a type the game never registered — a typo, say.
    write(&root, "entities:\n  door_north:\n    Mana: { current: 3 }\n");
    frame(&mut app);

    assert!(app.world().get_entity(kept).is_ok(), "an entity is not despawned because a file was refused");
    assert_eq!(app.world().get::<Health>(kept).map(|h| h.max), Some(10), "and its values are the last good ones");

    let _ = std::fs::remove_dir_all(&root);
}

/// **Children are spawned under their parent**, which is what `children:` means.
#[test]
fn a_child_is_spawned_under_its_parent() {
    let root = project("entities_children");
    write(&root, "entities:\n  door_north:\n    Health: { current: 10, max: 10 }\n    children:\n      hinge:\n        Spin: { degrees_per_second: 90.0 }\n");
    let mut app = game(&root);
    frame(&mut app);

    let parent = door(&app, &root);
    // **A `ChildOf` relationship is the engine's own index**, so the child is found by asking the world for
    // the entity whose parent is the door, not by the order the document listed them in.
    let mut query = app.world_mut().query::<(&ChildOf, &Spin)>();
    let child = query
        .iter(app.world())
        .find(|(child_of, spin)| child_of.parent() == parent && spin.degrees_per_second == 90.0)
        .map(|(child_of, _)| child_of.parent());
    assert_eq!(child, Some(parent), "the hinge is a child of the door");

    let _ = std::fs::remove_dir_all(&root);
}

/// **A file that is not a document says so**, rather than reading as an empty one that despawns everything.
#[test]
fn a_file_that_is_not_a_document_is_refused() {
    let root = project("entities_bad_document");
    let err = entities::read_document("a.entities.yaml", "- just\n- a list\n").expect_err("a list is not a document");
    assert!(err.line().contains("a.entities.yaml"), "the refusal names the file: {}", err.line());
    let _ = std::fs::remove_dir_all(&root);
}

/// Two types with one short path, for the ambiguity case.
mod first {
    use super::*;
    #[derive(Component, Reflect, Default, Debug)]
    #[reflect(Component)]
    pub struct Mode {
        pub value: i32,
    }
}

mod second {
    use super::*;
    #[derive(Component, Reflect, Default, Debug)]
    #[reflect(Component)]
    pub struct Mode {
        pub value: i32,
    }
}

/// A fault's own line names the file, the line and the path, so a person goes to a place.
#[test]
fn a_fault_names_the_file_the_line_and_the_path() {
    let fault = Fault { file: "a.entities.yaml".to_owned(), line: 4, at: "entities.door.Health".to_owned(), why: "no".to_owned() };
    assert_eq!(fault.line(), "a.entities.yaml: line 4: entities.door.Health: no");
}
