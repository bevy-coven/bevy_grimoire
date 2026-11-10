//! **Opening a project writes every kind's schema where an outside editor looks** (GD-04, #366).
//!
//! `<project>/.dark_factory/schema/<kind>.schema.json` is derived from the Rust types the reader reads, so an
//! outside editor's YAML extension completes the fields the type declares and a field's doc comment is its
//! hover. It is written on **the one path an open takes** ([`bevy_cauldron::args::open`], which the command
//! line and the launcher both call), so a project opened by either finds the folder filled and a project
//! opened before this change needs no re-save of anything.
//!
//! **The refusal that matters** is a person's own file at a schema path: it is never replaced, and the project
//! still opens. Losing it to a regenerated file would be a deletion nobody asked for. The core test beside
//! this one (`crates/bevy_grimoire_core/tests/a_generated_schema_is_replaced_and_a_hand_written_one_is_not.rs`)
//! pins that the write refuses such a file; this file pins that the editor's open calls that write at all.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::kits::KITS_VERSION;
use bevy_grimoire_core::schema;
use serde_json::Value;

use crate::fixtures;

/// The schema file a kind is written to, under `root`.
fn kind_file(root: &Path, kind: &str) -> PathBuf {
    root.join(schema::SCHEMA_DIR).join(format!("{kind}{}", schema::SCHEMA_EXT))
}

#[test]
fn opening_a_project_writes_a_schema_for_every_kind() {
    let root = fixtures::Fixture::new("schemas_on_open").descriptor("site/wall", "alpha").build("m");
    // A project opened before this change has its files and no schema folder: this is the upgrade, and it
    // must not need a re-save of anything.
    let folder = root.join(schema::SCHEMA_DIR);
    assert!(!folder.exists(), "the fixture is a project from before the schema folder");

    bevy_cauldron::args::open(&[root.to_string_lossy().into_owned(), "m".to_owned()])
        .unwrap_or_else(|e| panic!("the fixture project opens: {e}"));

    for kind in schema::KINDS {
        let path = kind_file(&root, kind);
        assert!(path.exists(), "`{kind}`'s schema is written when the project opens: {}", path.display());
        let value: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("readable"))
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(value["x-dark-factory"], Value::from(*kind), "{} names its kind", path.display());
        assert!(
            value["$comment"].as_str().is_some_and(|c| c.contains("generated")),
            "{} says it is derived, so a person knows a hand edit is overwritten",
            path.display()
        );
    }
}

#[test]
fn the_schema_written_on_open_describes_the_type_the_project_writes() {
    let root = fixtures::Fixture::new("schemas_describe_the_type").descriptor("site/wall", "alpha").build("m");
    bevy_cauldron::args::open(&[root.to_string_lossy().into_owned(), "m".to_owned()])
        .unwrap_or_else(|e| panic!("the fixture project opens: {e}"));

    // The map's schema is the type's, not a stale copy from another release.
    let map: Value = serde_json::from_str(&std::fs::read_to_string(kind_file(&root, "map")).expect("readable")).expect("JSON");
    assert!(map["properties"]["placements"].is_object(), "a map's schema describes the placements a map holds: {}", map["properties"]);
    // A field's doc comment is what hover shows, which is the point of generating from the type.
    assert!(
        map["properties"]["name"]["description"].as_str().is_some(),
        "the map's `name` carries its doc comment as a description: {}",
        map["properties"]["name"]
    );
    // And a kind that states a version has it described, at the version the code writes.
    let kits: Value = serde_json::from_str(&std::fs::read_to_string(kind_file(&root, "kits")).expect("readable")).expect("JSON");
    assert!(kits["properties"]["version"].is_object(), "the kits schema describes the version {KITS_VERSION} a kits file states");
}

#[test]
fn a_persons_own_schema_file_is_kept_and_the_project_still_opens() {
    let root = fixtures::Fixture::new("schemas_a_persons_file").descriptor("site/wall", "alpha").build("m");
    let folder = root.join(schema::SCHEMA_DIR);
    std::fs::create_dir_all(&folder).expect("the schema folder is made");
    let mine = "{\"title\": \"a schema a person maintains\"}";
    let path = kind_file(&root, "map");
    std::fs::write(&path, mine).expect("the person's file is written");

    // **The open still succeeds.** A derived file that could not be written is a report, never an editor
    // that will not start: the schema is what completion is built from, not what the project is.
    bevy_cauldron::args::open(&[root.to_string_lossy().into_owned(), "m".to_owned()])
        .unwrap_or_else(|e| panic!("a refused schema write must not refuse the open: {e}"));

    assert_eq!(
        std::fs::read_to_string(&path).expect("readable"),
        mine,
        "a file the editor did not generate is byte-identical after an open"
    );
}

/// **A hand edit to a generated schema is overwritten when the project is opened again**, which is what the
/// `$comment` inside it warns about — while a person's own file at a schema path never is. Both on the open
/// path, because that is the seam the editor owns; the core test beside this pins the write itself.
#[test]
fn opening_again_overwrites_a_hand_edit_and_never_a_persons_own_file() {
    let root = fixtures::Fixture::new("schemas_hand_edit_on_open").descriptor("site/wall", "alpha").build("m");
    let argv = [root.to_string_lossy().into_owned(), "m".to_owned()];
    bevy_cauldron::args::open(&argv).unwrap_or_else(|e| panic!("the fixture project opens: {e}"));

    // A person edits a generated file, removing the warning comment as they do it.
    let generated = kind_file(&root, "map");
    assert!(std::fs::read_to_string(&generated).expect("readable").contains("placements"), "the first write is the type's own");
    std::fs::write(&generated, "{\"hand\": \"edit\"}").expect("the edit is written");

    // And puts a file of their own where another kind's schema goes, in a folder that has never been written.
    let theirs_root = fixtures::Fixture::new("schemas_theirs_on_open").descriptor("site/wall", "alpha").build("m");
    let theirs_argv = [theirs_root.to_string_lossy().into_owned(), "m".to_owned()];
    std::fs::create_dir_all(theirs_root.join(schema::SCHEMA_DIR)).expect("the folder is made");
    let mine = "{\"title\": \"a schema a person maintains\"}";
    let theirs = kind_file(&theirs_root, "map");
    std::fs::write(&theirs, mine).expect("the person's file is written");

    bevy_cauldron::args::open(&argv).unwrap_or_else(|e| panic!("reopening still opens: {e}"));
    bevy_cauldron::args::open(&theirs_argv).unwrap_or_else(|e| panic!("a refused schema write must not refuse the open: {e}"));

    let after = std::fs::read_to_string(&generated).expect("readable");
    assert_ne!(after, "{\"hand\": \"edit\"}", "a hand edit to a generated schema is overwritten on the next open");
    assert!(after.contains("placements"), "and the type's own schema is back: {after}");
    assert_eq!(std::fs::read_to_string(&theirs).expect("readable"), mine, "a person's own file is kept byte-identical");
}

/// **A new project does not commit the generated folder.** The schemas are rewritten at every open and every
/// game start, so one committed would diff a file nobody edited each time a type moved. The guide and the
/// journal under `.dark_factory/` are the project's own, so the ignore names the one folder and not the parent.
#[test]
fn a_new_project_ignores_the_generated_folder() {
    let place = std::env::temp_dir().join(format!("schemas_gitignore_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&place);
    std::fs::create_dir_all(&place).expect("the folder the project goes in is made");
    let form = bevy_cauldron::new_project::NewProject {
        name: "ignored".to_owned(),
        location: place.to_string_lossy().into_owned(),
        template: bevy_cauldron::new_project::Template::Empty,
    };
    let root = bevy_cauldron::new_project::create(&form).unwrap_or_else(|e| panic!("{e}"));

    let path = root.join(bevy_cauldron::new_project::GITIGNORE);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(
        text.lines().any(|line| line.trim() == ".dark_factory/schema/"),
        "the generated folder is ignored, and only it: {text}"
    );
    assert!(!text.lines().any(|line| line.trim() == ".dark_factory/" ), "the guide and journal are the project's own: {text}");

    let _ = std::fs::remove_dir_all(&place);
}
