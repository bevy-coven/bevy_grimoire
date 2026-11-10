//! **A game's reflected schema reaches the file an outside editor reads** (GD-04, #366).
//!
//! `<root>/.dark_factory/schema/game.schema.json` describes the components a game's own files may name, and
//! only the game's registry knows them. `bevy_grimoire` writes the file and never asks the registry itself:
//! Bevy's answer to that question is `registry.schema`, which a game that runs `RemotePlugin` already serves,
//! and this crate does not depend on `bevy_remote` — a shipping game must not gain a listening server for one
//! file written at start.
//!
//! So the plugin takes the document and writes it. What these tests pin: the file lands at the path the
//! editor's own kinds use, the document the game answered is kept as it answered, a game with no remote
//! writes nothing rather than an empty file, and a person's own file at that path is kept.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_grimoire::game_schema::{GameSchemaPlugin, RegistrySchema, write_now};
use bevy_grimoire_core::schema;

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

/// A headless app with the plugin and the document a game's registry would answer.
fn app_with(root: &Path, document: Option<&'static str>) -> App {
    let mut app = App::new();
    let schema: Option<RegistrySchema> =
        document.map(|text| std::sync::Arc::new(move |_: &World| Some(text.to_owned())) as RegistrySchema);
    app.add_plugins(GameSchemaPlugin { root: root.to_path_buf(), schema });
    app
}

/// **The document the registry answered is the file**, and it is stamped so the next start may replace it.
#[test]
fn the_games_document_is_written_where_the_editors_own_kinds_are() {
    let root = scratch("game_schema_plugin");
    let document = "{\"my_game::Health\": {\"type\": \"object\"}, \"my_game::Spin\": {\"type\": \"object\"}}";
    let mut app = app_with(&root, Some(document));
    app.update();

    let path = game_file(&root);
    assert!(path.exists(), "the game's schema is written at start: {}", path.display());
    // **The game's own type paths are the file's content**: this is the registry's answer, not a copy of ours.
    // Read as text because this crate takes no JSON dependency of its own; the write already parsed it.
    let text = std::fs::read_to_string(&path).expect("readable");
    assert!(text.contains("my_game::Health"), "the registry's `Health` is in the file: {text}");
    assert!(text.contains("my_game::Spin"), "the registry's `Spin` is in the file: {text}");
    assert!(
        text.contains("generated"),
        "the file says it is derived, so the next start may replace it: {text}"
    );

    std::fs::remove_dir_all(&root).expect("the test removes its own scratch folder");
}

/// **A game that runs no remote writes no file**, rather than an empty one an outside editor would offer
/// nothing from.
#[test]
fn a_game_with_no_remote_writes_no_schema_file() {
    let root = scratch("game_schema_none");
    let mut app = app_with(&root, None);
    app.update();
    assert!(!game_file(&root).exists(), "a game that answered nothing writes no schema file");

    std::fs::remove_dir_all(&root).expect("the test removes its own scratch folder");
}

/// **A person's own `game.schema.json` is kept**, and the game starts anyway: a derived file that could not be
/// written is a warning, never a game that will not run.
#[test]
fn a_persons_own_game_schema_is_kept_and_the_game_still_starts() {
    let root = scratch("game_schema_theirs");
    std::fs::create_dir_all(root.join(schema::SCHEMA_DIR)).expect("the folder is made");
    let mine = "{\"title\": \"a schema a person maintains\"}";
    std::fs::write(game_file(&root), mine).expect("the person's file is written");

    let mut app = app_with(&root, Some("{\"my_game::Health\": {\"type\": \"object\"}}"));
    app.update();

    assert_eq!(std::fs::read_to_string(game_file(&root)).expect("readable"), mine, "the person's file is untouched");

    std::fs::remove_dir_all(&root).expect("the test removes its own scratch folder");
}

/// **A document that is not JSON is refused rather than written**, because a file an editor cannot parse is
/// worse than no file: the editor would report a broken schema at every open.
#[test]
fn a_document_that_is_not_json_is_refused() {
    let root = scratch("game_schema_bad_json");
    assert!(write_now(&root, "not json").is_err(), "a document an editor cannot read is refused");
    assert!(!game_file(&root).exists(), "and nothing is written in its place");

    std::fs::remove_dir_all(&root).expect("the test removes its own scratch folder");
}
