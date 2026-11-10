//! **The editor owns the schema files it generates, and only those** (GD-04, #366).
//!
//! `<root>/.dark_factory/schema/<kind>.schema.json` is derived: written on every open and again whenever a
//! kind's file is written, so a hand edit to one is overwritten and the file's own `$comment` says so.
//!
//! **The exception is the one that matters.** A person who put a file of their own at that path — an
//! extension they maintain, a schema from somewhere else — must not lose it to a derived write. So a file
//! that is *not* marked as generated is refused, with a problem that names it, and the write of the other
//! kinds still happens. Losing a person's file to a regenerated one is the failure this test exists for; a
//! schema that silently replaced it would be a deletion nobody asked for.
//!
//! The scratch paths carry this test's name and the process id, because the core tests share a process with
//! other files' tests.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::schema;
use serde_json::Value;

/// A scratch folder of this test's own: its name and the process id, never a fixed one.
fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{name}-{}", std::process::id()))
}

/// The schema file a kind is written to, under `root`.
fn kind_file(root: &Path, kind: &str) -> PathBuf {
    root.join(schema::SCHEMA_DIR).join(format!("{kind}{}", schema::SCHEMA_EXT))
}

/// **A file a person wrote at a schema path is refused, not replaced**, the problem names it, and no other
/// schema file moves either: a batch that replaced half the project's schemas and stopped at one would be a
/// state nobody could read off the disk.
#[test]
fn a_hand_written_schema_is_refused_by_name_and_nothing_is_written() {
    let root = scratch("schema_hand_written");
    let _ = std::fs::remove_dir_all(&root);
    let dir = root.join(schema::SCHEMA_DIR);
    std::fs::create_dir_all(&dir).expect("the scratch folder is made");
    let hand = kind_file(&root, "map");
    let mine = "{\"title\": \"a schema a person maintains\", \"properties\": {\"only\": {\"type\": \"string\"}}}";
    std::fs::write(&hand, mine).expect("the hand-written file is written");

    let err = schema::write_all(&root).expect_err("a hand-written schema file is not replaced by a generated one");
    let line = err.line();
    assert!(
        line.contains(&hand.display().to_string()),
        "the problem names the file it left alone, so a person knows which one blocked the write: {line}"
    );

    let kept = std::fs::read_to_string(&hand).expect("the hand-written file is still there");
    assert_eq!(kept, mine, "a file that is not generated is byte-identical after a refused write");

    // **Not one kind is written.** The check is every path before any write, so the refusal leaves the
    // project's schema folder as it was.
    let others: Vec<&str> = schema::KINDS
        .iter()
        .filter(|kind| **kind != "map" && kind_file(&root, kind).exists())
        .copied()
        .collect();
    assert!(others.is_empty(), "a refused batch wrote {} anyway, so the folder is half replaced", others.join(", "));

    std::fs::remove_dir_all(&root).expect("the test removes its own scratch folder");
}

/// **A generated file is replaced on the next write**, which is what "derived" means, and a second write of
/// the same kind does not refuse.
#[test]
fn a_generated_schema_is_rewritten_on_the_next_write() {
    let root = scratch("schema_generated_rewrite");
    let _ = std::fs::remove_dir_all(&root);
    schema::write_all(&root).expect("a project with no schema files gets all of them");

    // A person's hand edit to a generated file is overwritten, and the file says that will happen.
    let path = kind_file(&root, "map");
    let generated = std::fs::read_to_string(&path).expect("the generated file reads");
    let value: Value = serde_json::from_str(&generated).expect("the generated file is JSON");
    let comment = value["$comment"].as_str().expect("a generated schema says it is generated");
    assert!(
        comment.contains("overwritten"),
        "the file itself warns that a hand edit is overwritten: {comment}"
    );
    // **A hand edit to a generated file is overwritten, and the record is why.** The edit here removes the
    // `$comment` as well, which is the case that must still be replaced: ownership is what the editor
    // recorded writing, not a claim read out of the file.
    std::fs::write(&path, "{\"a\": \"hand edit\"}").expect("the hand edit is written");

    let written = schema::write_all(&root).expect("a generated file is written again rather than refused");
    assert!(written.iter().any(|p| p.ends_with(format!("map{}", schema::SCHEMA_EXT))), "the map's schema is one of the files written");

    std::fs::remove_dir_all(&root).expect("the test removes its own scratch folder");
}

/// **The game's own schema file obeys the same rule**, and the two cases are told apart:
///
/// - a file *this editor wrote* stays its own, so a hand edit to it is overwritten on the next start — which
///   is what the `$comment` in it warns about;
/// - a file a *person put there*, which the editor never wrote, is refused and kept byte-identical.
#[test]
fn the_games_schema_is_derived_and_a_persons_own_file_is_kept() {
    let root = scratch("schema_game_derived");
    let _ = std::fs::remove_dir_all(&root);
    let path = schema::write_game(&root, "{\"title\": \"the game's components\"}").expect("the game's schema writes");
    assert_eq!(path, kind_file(&root, "game"), "the game's schema is `game.schema.json`");

    let written: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("readable")).expect("JSON");
    assert_eq!(written["title"], Value::from("the game's components"), "the registry's own document is kept as it answered");
    assert!(
        written["$comment"].as_str().is_some_and(|c| c.contains("generated")),
        "the file says it is generated, so the next start may replace it: {written}"
    );

    // **A hand edit to a generated file is overwritten**: the edit here drops the `$comment` too, which is
    // the case that must still be replaced, because ownership is what the editor recorded writing.
    std::fs::write(&path, "{\"title\": \"hand edit\"}").expect("the hand edit is written");
    schema::write_game(&root, "{\"title\": \"the game's components\"}").expect("a file the editor wrote is written again");
    let again: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("readable")).expect("JSON");
    assert_eq!(again["title"], Value::from("the game's components"), "a hand edit to a derived file is replaced: {again}");

    // **A person's file, at a path this editor never wrote, is kept.** A fresh folder, so nothing recorded
    // it as generated — which is exactly the state of a project whose owner maintains their own schema.
    let theirs = scratch("schema_game_hand_written");
    let _ = std::fs::remove_dir_all(&theirs);
    let path = kind_file(&theirs, "game");
    std::fs::create_dir_all(path.parent().expect("under the schema folder")).expect("the folder is made");
    std::fs::write(&path, "{\"title\": \"mine\"}").expect("the person's file is written");

    let err = schema::write_game(&theirs, "{\"title\": \"the game's components\"}").expect_err("a person's own game schema is refused");
    assert!(err.line().contains(&path.display().to_string()), "the problem names the file: {}", err.line());
    assert_eq!(std::fs::read_to_string(&path).expect("readable"), "{\"title\": \"mine\"}", "the person's file is untouched");

    std::fs::remove_dir_all(&root).expect("the test removes its own scratch folder");
    std::fs::remove_dir_all(&theirs).expect("the test removes its own scratch folder");
}
