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

/// **An immutable component**, which `ReflectComponent::apply` panics on: the engine refuses to change one
/// through reflection, so a file's second save must insert rather than apply.
#[derive(Component, Reflect, Default, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[component(immutable)]
#[reflect(Component, Deserialize, Serialize)]
struct Stamped {
    value: i32,
}

/// **A resource the game registers and never inserts**, which has no `ComponentId` until it is used as a
/// resource: the shape whose write is refused.
#[derive(Resource, Reflect, Default, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[reflect(Resource, Deserialize, Serialize)]
struct Never {
    count: i32,
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
    // A bare `App` has no `Time`; the reload polls on one, so the test's game has a clock it can advance.
    app.init_resource::<Time>();
    app.register_type::<Health>().register_type::<Spin>().register_type::<Clock>().register_type::<Stamped>().register_type::<Never>();
    // **A reflected resource needs its `ComponentId` to exist**, which the engine makes when the type is used
    // as a `Resource` — the same requirement `registry.schema`'s own `world.insert_resources` has. A game
    // already inserts its resources; this test's game inserts the one it registers.
    app.init_resource::<Clock>();
    app.add_plugins(GameEntitiesPlugin { root: root.to_path_buf() });
    app
}

/// **One frame, with the commands a `Startup` system queued applied**: `update` runs `Startup` and the
/// schedule's systems, and the queued reads reach the world at `finish`/`cleanup`.
///
/// **The clock moves a second per frame**, which is what the reload's `FileWatch` needs: it polls at the
/// project's half second and waits out its settle time, so a write is read by the second frame after it and
/// never on the frame it happens — a half-written save is not read.
fn frame(app: &mut App) {
    let mut time = app.world_mut().resource_mut::<Time>();
    time.advance_by(std::time::Duration::from_secs(1));
    app.update();
    app.finish();
    app.cleanup();
}

/// **One save: the watch sees the new bytes, then settles on them, then the read runs.** Two frames, because
/// that is what the watch's contract is — one poll notices the change, the next confirms it has stopped.
fn save(app: &mut App) {
    frame(app);
    frame(app);
    frame(app);
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
    save(&mut app);

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
    save(&mut app);

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
    save(&mut app);

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

// ---------------------------------------------------------------------------------------------------
// Manager review of #367: M1 to M5. Each of these failed before the fix below.
// ---------------------------------------------------------------------------------------------------

/// **M1: a fault names the line the fault is on**, not `line 0`. The core walk already knows where every
/// path stands; this is the number a person needs to go to.
#[test]
fn a_fault_names_the_actual_line_it_is_on() {
    // `Mana` is on line 3 of this file, and `Health` on line 2.
    let text = "entities:\n  door_north:\n    Mana: { current: 3 }\n";
    let document = entities::read_document("a.entities.yaml", text).expect("the file reads as YAML");
    let mut app = game(&project("entities_m1"));
    let err = entities::apply(app.world_mut(), "a.entities.yaml", "a.entities.yaml", &document)
        .expect_err("an unregistered type is refused");
    assert!(err.at.ends_with("Mana"), "the fault is at the component: {}", err.at);
    assert_eq!(err.line, 3, "the fault is on the line `Mana` stands on: {}", err.line());

    let _ = std::fs::remove_dir_all(std::env::temp_dir().join(format!("entities_m1-{}", std::process::id())));
}

/// **M2: a refused document changes nothing.** A component written before the bad one must not stay, and no
/// entity the refusal left behind may be spawned.
#[test]
fn a_refused_document_holds_no_changed_value_and_no_extra_entity() {
    let root = project("entities_m2");
    // `Health` is valid and comes first in BTreeMap order; `Mana` is the refusal after it.
    write(&root, "entities:\n  door_north:\n    Health: { current: 99, max: 99 }\n    Mana: { current: 3 }\n");
    let mut app = game(&root);
    frame(&mut app);

    let key = root.join(entities::ENTITIES_DIR).join(format!("main{}", entities::ENTITIES_EXT));
    let held = app.world().resource::<GameEntities>().spawned.clone();
    assert!(held.get(&key.display().to_string()).is_none_or(|by_name| by_name.is_empty()), "a refused file spawns nothing: {held:?}");
    // **No entity at all was left behind**, so nothing can be duplicated by the next good save.
    let mut query = app.world_mut().query::<&Health>();
    assert_eq!(query.iter(app.world()).count(), 0, "a refused document leaves no entity");

    // And the next good save makes exactly one.
    write(&root, FIRST);
    save(&mut app);
    let spawned = app.world().resource::<GameEntities>().spawned.get(&key.display().to_string()).cloned().unwrap_or_default();
    assert_eq!(spawned.len(), 1, "one entity, not one plus a leftover: {spawned:?}");
    let mut query = app.world_mut().query::<&Health>();
    assert_eq!(query.iter(app.world()).count(), 1, "and the world holds one `Health`, not two");

    let _ = std::fs::remove_dir_all(&root);
}

/// **M3: children are tracked like top-level entities**, so five saves leave one child and removing it from
/// the file removes it.
#[test]
fn a_child_does_not_multiply_on_every_save() {
    let root = project("entities_m3");
    write(&root, "entities:\n  door_north:\n    Health: { current: 10, max: 10 }\n    children:\n      hinge:\n        Spin: { degrees_per_second: 90.0 }\n");
    let mut app = game(&root);
    frame(&mut app);
    for _ in 0..4 {
        // Each save carries the same child, so a fifth read must still find one.
        write(&root, "entities:\n  door_north:\n    Health: { current: 10, max: 10 }\n    children:\n      hinge:\n        Spin: { degrees_per_second: 90.0 }\n");
        save(&mut app);
    }
    let mut query = app.world_mut().query::<&Spin>();
    assert_eq!(query.iter(app.world()).count(), 1, "five saves leave one child, not five");

    // Removing the child from the file removes it.
    write(&root, "entities:\n  door_north:\n    Health: { current: 10, max: 10 }\n");
    save(&mut app);
    let mut query = app.world_mut().query::<&Spin>();
    assert_eq!(query.iter(app.world()).count(), 0, "a child the file dropped is despawned");

    let _ = std::fs::remove_dir_all(&root);
}

/// **M4: an immutable component inserted on the first read must not panic on the next save.**
///
/// `Stamped` is immutable (`#[component(immutable)]`) and registered with `ReflectComponent`, which is the
/// shape that panicked: `ReflectComponent::apply` is a `panic!` on an immutable component, and a second save of
/// the same file reached it.
#[test]
fn an_immutable_component_is_inserted_rather_than_panicked_over() {
    let root = project("entities_m4");
    write(&root, "entities:\n  door_north:\n    Stamped: { value: 7 }\n");
    let mut app = game(&root);
    save(&mut app);
    // **The second save is the one that panics**, because the component is now on the entity and the value is
    // there to be applied.
    write(&root, "entities:\n  door_north:\n    Stamped: { value: 7 }\n");
    save(&mut app);

    let key = root.join(entities::ENTITIES_DIR).join(format!("main{}", entities::ENTITIES_EXT));
    let spawned = app.world().resource::<GameEntities>().spawned.get(&key.display().to_string()).cloned().unwrap_or_default();
    let entity = *spawned.get("door_north").unwrap_or_else(|| panic!("the entity is tracked: {spawned:?}"));
    assert_eq!(app.world().get::<Stamped>(entity).map(|s| s.value), Some(7), "the immutable value is on the entity");

    let _ = std::fs::remove_dir_all(&root);
}

/// **NIT: removing a component from an entity in the file removes it from the entity.**
#[test]
fn a_component_the_file_drops_is_removed_from_the_entity() {
    let root = project("entities_drop_component");
    write(&root, FIRST);
    let mut app = game(&root);
    frame(&mut app);
    let entity = door(&app, &root);
    assert!(app.world().get::<Spin>(entity).is_some(), "the file sets `Spin`");

    // The file keeps the entity and drops `Spin`.
    write(&root, "entities:\n  door_north:\n    Health: { current: 10, max: 10 }\nresources:\n  Clock: { elapsed: 1.0 }\n");
    save(&mut app);

    assert_eq!(door(&app, &root), entity, "and it is still the same entity");
    assert!(app.world().get::<Spin>(entity).is_none(), "a component the file no longer names is taken off the entity");

    let _ = std::fs::remove_dir_all(&root);
}

/// **NIT: deleting a whole file despawns what it made.**
#[test]
fn deleting_a_file_despawns_its_entities() {
    let root = project("entities_delete_file");
    write(&root, FIRST);
    let mut app = game(&root);
    frame(&mut app);
    let entity = door(&app, &root);

    std::fs::remove_file(root.join(entities::ENTITIES_DIR).join(format!("main{}", entities::ENTITIES_EXT))).expect("the file is deleted");
    save(&mut app);
    assert!(app.world().get_entity(entity).is_err(), "a deleted file's entities are despawned");

    let _ = std::fs::remove_dir_all(&root);
}

/// **NIT: a reload reports only what changed**, so a save nobody altered does not claim every entity did.
#[test]
fn a_reload_of_an_unchanged_document_reports_no_change() {
    let root = project("entities_no_change");
    write(&root, FIRST);
    let mut app = game(&root);
    frame(&mut app);

    let path = root.join(entities::ENTITIES_DIR).join(format!("main{}", entities::ENTITIES_EXT));
    let document = entities::load(&path).expect("the file reads");
    let applied = entities::apply(app.world_mut(), &path.display().to_string(), &path.display().to_string(), &document)
        .expect("the same document applies");
    assert!(applied.spawned.is_empty(), "nothing spawned: {applied:?}");
    assert!(applied.changed.is_empty(), "nothing changed, because nothing differs: {applied:?}");

    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------------------------------
// Manager review round 3.
// ---------------------------------------------------------------------------------------------------

/// **A resource refused AFTER the entities would have been applied changes nothing** (M2, round 3).
///
/// A `Resource` the game registered and never inserted has no `ComponentId`, so the write is refused — and that
/// refusal used to happen inside `apply`, after the entities were spawned, which left them untracked and made
/// the next good save spawn them a second time.
#[test]
fn a_resource_that_is_never_inserted_refuses_before_any_entity_is_spawned() {
    let root = project("entities_m2_round3");
    // `Never` is registered as a `Resource` and never inserted into the world, which is the shape that has no
    // `ComponentId` at all.
    write(&root, "entities:\n  door_north:\n    Health: { current: 10, max: 10 }\nresources:\n  Never: { count: 1 }\n");
    let mut app = game(&root);
    let path = root.join(entities::ENTITIES_DIR).join(format!("main{}", entities::ENTITIES_EXT));
    let document = entities::load(&path).expect("the file reads");
    let err = entities::apply(app.world_mut(), &path.display().to_string(), &path.display().to_string(), &document)
        .expect_err("a resource with no component id is refused");
    assert!(err.line().contains("Never"), "the refusal names the resource: {}", err.line());

    // **No entity exists at all**, so nothing leaked untracked.
    let mut query = app.world_mut().query::<&Health>();
    assert_eq!(query.iter(app.world()).count(), 0, "a refused document leaves no entity");

    // And the next good save makes exactly one.
    write(&root, "entities:\n  door_north:\n    Health: { current: 10, max: 10 }\n");
    save(&mut app);
    let mut query = app.world_mut().query::<&Health>();
    assert_eq!(query.iter(app.world()).count(), 1, "the next good save makes one entity, not two");

    let _ = std::fs::remove_dir_all(&root);
}

/// **`.nan` and `.inf` are refused, not written as `null`** (NIT, round 3): a number JSON cannot hold is a
/// value the reader cannot carry, and a `null` in its place would be a value the file does not say.
#[test]
fn a_number_json_cannot_hold_is_refused() {
    let err = entities::read_document("a.entities.yaml", "entities:\n  door_north:\n    Health: { current: .nan }\n")
        .expect_err("`.nan` is not a value JSON holds");
    let line = err.line();
    assert!(line.contains("nan") || line.contains("NaN") || line.contains("not a number"), "the refusal names the number: {line}");
    assert!(line.contains("line 3"), "and the line it is on: {line}");
}

/// **A document whose shape is wrong names a line too** (NIT, round 3): `entities` holding a list is refused at
/// the line `entities` stands on, not at line 0.
#[test]
fn a_shape_fault_names_the_line_it_is_on() {
    let err = entities::read_document("a.entities.yaml", "entities:\n  - one\n  - two\n")
        .expect_err("a list is not a map of entities");
    assert_eq!(err.line, 2, "the refusal is at the line the list starts on: {}", err.line());
}

/// **An entity name with a dot in it is refused** (NIT, round 3): the key a child is tracked under is
/// `parent.children.name`, so a top-level name containing a dot could collide with a child's key.
#[test]
fn an_entity_name_with_a_dot_is_refused() {
    let err = entities::read_document("a.entities.yaml", "entities:\n  door.children.hinge:\n    Health: { current: 1, max: 1 }\n")
        .expect_err("a name with a dot could collide with a child's key");
    assert!(err.line().contains("dot"), "the refusal says why: {}", err.line());
}
