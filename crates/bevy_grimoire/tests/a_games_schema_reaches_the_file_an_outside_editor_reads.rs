//! **A game's registered components are in `game.schema.json`** (GD-04, #366, acceptance 3).
//!
//! The file is worth having only if it is **the registry's own answer**: an outside editor completes a
//! component's fields from it, so a document written by hand — or a document of ours that merely looks like
//! one — would offer fields the game does not have. So this test builds a headless game, registers `Health`
//! and `Spin`, adds the plugin, and asks the file what it says.
//!
//! **Read through a JSON parser, not a substring search**: a file that contains the word `Health` somewhere
//! says nothing about whether `Health` is a described type with its fields.
//!
//! `serde_json` is a test dependency of this crate and nothing more: the write already parses the document,
//! and a game gains no JSON crate of its own from having the file.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_grimoire::game_schema::{GameSchemaPlugin, registry_schema, write_now};
use bevy_grimoire_core::schema;

/// A component a test game registers, so the file has a type that is unmistakably the game's own.
#[derive(Component, Reflect, Default, Debug)]
#[reflect(Component)]
struct Health {
    current: i32,
    max: i32,
}

/// A second one, so "lists the game's components" is not satisfied by one lucky entry.
#[derive(Component, Reflect, Default, Debug)]
#[reflect(Component)]
struct Spin {
    degrees_per_second: f32,
}

/// A scratch folder of this test's own: its name and the process id.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch folder is made");
    dir
}

fn game_file(root: &Path) -> PathBuf {
    root.join(schema::SCHEMA_DIR).join(format!("game{}", schema::SCHEMA_EXT))
}

/// **A headless game with the two types registered and the plugin added.**
fn game(root: &Path) -> App {
    let mut app = App::new();
    // `AppTypeRegistry` is what the export reads; a bare `App` does not have one until `TypeRegistryPlugin`
    // or any Bevy plugin that adds it. Adding it directly keeps this a headless test with no renderer.
    app.init_resource::<AppTypeRegistry>();
    app.register_type::<Health>().register_type::<Spin>();
    app.add_plugins(GameSchemaPlugin { root: root.to_path_buf() });
    app
}

/// **The document the registry answers already carries the types**, which is what the plugin writes and what
/// the remote method would serve.
#[test]
fn the_registry_export_names_the_types_the_game_registered() {
    let root = scratch("game_schema_export");
    let mut app = game(&root);
    app.finish();
    app.cleanup();

    let document = registry_schema(app.world_mut());
    let value: serde_json::Value = serde_json::from_str(&document).expect("the export is JSON");
    let entries = value.as_object().expect("the export is a map of type path to schema");
    let names: Vec<&String> = entries.keys().collect();
    assert!(
        names.iter().any(|n| n.ends_with("::Health") || *n == "Health"),
        "the registry's answer names `Health`: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.ends_with("::Spin") || *n == "Spin"),
        "the registry's answer names `Spin`: {names:?}"
    );

    std::fs::remove_dir_all(&root).expect("the test removes its own scratch folder");
}

/// **The plugin writes that document into the project at start**, and the file describes the game's types
/// with their fields — not a placeholder, and not a hand-written shape.
#[test]
fn the_plugin_writes_the_games_components_into_the_project() {
    let root = scratch("game_schema_plugin");
    let mut app = game(&root);
    app.update();

    let path = game_file(&root);
    assert!(path.exists(), "the game's schema is written at start: {}", path.display());
    let text = std::fs::read_to_string(&path).expect("readable");
    let value: serde_json::Value = serde_json::from_str(&text).expect("the file is JSON");

    let health = value
        .as_object()
        .and_then(|entries| entries.iter().find(|(name, _)| name.ends_with("::Health") || *name == "Health"))
        .map(|(_, schema)| schema)
        .unwrap_or_else(|| panic!("`Health` is in the game's schema: {text}"));
    // **The component's fields are described**, which is what an outside editor completes from.
    let described = health.to_string();
    assert!(described.contains("current"), "`Health`'s `current` is described: {described}");
    assert!(described.contains("max"), "`Health`'s `max` is described: {described}");
    // And the file says which reflect data made it a component.
    assert!(described.contains("Component"), "`Health` is named a Component: {described}");
    assert!(
        value["$comment"].as_str().is_some_and(|c| c.contains("generated")),
        "the file says it is derived, so the next start may replace it"
    );

    std::fs::remove_dir_all(&root).expect("the test removes its own scratch folder");
}

/// **A person's own `game.schema.json` is kept**, and the game still starts: a derived file that could not be
/// written is a warning, never a game that will not run.
#[test]
fn a_persons_own_game_schema_is_kept_and_the_game_still_starts() {
    let root = scratch("game_schema_theirs");
    std::fs::create_dir_all(root.join(schema::SCHEMA_DIR)).expect("the folder is made");
    let mine = "{\"title\": \"a schema a person maintains\"}";
    std::fs::write(game_file(&root), mine).expect("the person's file is written");

    let mut app = game(&root);
    app.update();

    assert_eq!(std::fs::read_to_string(game_file(&root)).expect("readable"), mine, "the person's file is untouched");

    std::fs::remove_dir_all(&root).expect("the test removes its own scratch folder");
}

/// **A hand edit to a generated file is overwritten on the next write**, which is what "derived" means.
#[test]
fn a_hand_edit_to_a_generated_game_schema_is_overwritten() {
    let root = scratch("game_schema_hand_edit");
    let mut app = game(&root);
    app.update();
    let first = std::fs::read_to_string(game_file(&root)).expect("readable");
    assert!(first.contains("Health"), "the first write describes the game's types");

    // A person edits the file and removes the warning comment as they do it.
    std::fs::write(game_file(&root), "{\"hand\": \"edit\"}").expect("the edit is written");

    let mut again = game(&root);
    again.update();
    let second = std::fs::read_to_string(game_file(&root)).expect("readable");
    assert_ne!(second, "{\"hand\": \"edit\"}", "a hand edit to a derived file is replaced");
    assert!(second.contains("Health"), "and the game's own types are back: {second}");

    std::fs::remove_dir_all(&root).expect("the test removes its own scratch folder");
}

/// **A document that is not JSON is refused rather than written**, because a file an editor cannot parse is
/// worse than no file: the editor would report a broken schema at every open.
#[test]
fn a_document_that_is_not_json_is_refused() {
    let root = scratch("game_schema_bad_json");
    assert!(
        bevy_grimoire_core::schema::write_game(&root, "not json").is_err(),
        "a document an editor cannot read is refused"
    );
    assert!(!game_file(&root).exists(), "and nothing is written in its place");

    std::fs::remove_dir_all(&root).expect("the test removes its own scratch folder");
}

/// **`write_now` is the same write at a moment the game chooses**, not a second path.
#[test]
fn write_now_answers_the_file_it_wrote() {
    let root = scratch("game_schema_write_now");
    let mut app = game(&root);
    app.finish();
    app.cleanup();
    let path = write_now(&root, app.world_mut()).unwrap_or_else(|e| panic!("{}", e.line()));
    assert_eq!(path, game_file(&root), "the path is the one `.dark_factory/schema/` holds");
    assert!(std::fs::read_to_string(&path).expect("readable").contains("Health"), "and it describes the game's types");

    std::fs::remove_dir_all(&root).expect("the test removes its own scratch folder");
}
