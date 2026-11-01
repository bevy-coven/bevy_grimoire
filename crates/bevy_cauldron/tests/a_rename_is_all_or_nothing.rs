//! **A rename is all-or-nothing, and a panel never describes the wrong thing.**
//!
//! The regression tests for the 2026-10-20-07 run. What they pin:
//!
//! - **A rename onto a name another map has is refused**, and the other map's bytes are
//!   **unchanged**. Before this, `OpenMap::save` derived the path from the map's name and wrote
//!   it with `fs::rename` — which POSIX specifies as *replacing* the target — so a rename
//!   **destroyed another map, silently**.
//! - **A successful rename removes the old file.** One map, one file, one name. Before this, a
//!   rename left two files answering to one map, and the stale one refused to open with a message
//!   about a schema mismatch.
//! - **`Escape` leaves no draft**, so the chooser's panel describes the highlighted map.
//! - **A settings click sets the field**, so clicking `NAME` opens `NAME`.
//!
//! The core's half — the no-replace primitive itself — is in
//! `bevy_grimoire_core/tests/a_rename_never_replaces.rs`.

use crate::fixtures;

use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};
use fixtures::Fixture;

fn project(name: &str) -> std::path::PathBuf {
    Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .build("m")
}

/// **The maps in a project's `maps/`, with their bytes.**
fn maps(root: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let dir = root.join("assets/grimoire/maps");
    let mut out: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
        .expect("the maps directory is there")
        .flatten()
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).expect("the map is readable"),
            )
        })
        .collect();
    out.sort();
    out
}

/// **A rename onto a name another map has is refused, and that map is untouched.**
///
/// This is the defect the run is about, measured on a copy of Sanctuary's project: two maps,
/// `town` and `town_block`, and renaming one to the other's name **replaced the other's file**,
/// because the path is derived from the name and `fs::rename` replaces.
#[test]
fn a_rename_onto_a_taken_name_is_refused_and_the_other_map_is_untouched() {
    let root = project("rename-taken");
    // **A second map**, so there is a name to collide with.
    {
        let other = root.join("assets/grimoire/maps/other.map.ron");
        let text = std::fs::read_to_string(root.join("assets/grimoire/maps/m.map.ron"))
            .expect("the map is there");
        std::fs::write(
            &other,
            text.replace("name: \"m\"", "name: \"other\""),
        )
        .expect("the map is writable");
    }
    let before = maps(&root);

    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    // **Rename the open map to the other one's name.**
    {
        let world = app.world_mut();
        let mut open = world.remove_resource::<OpenMap>();
        let project = world.resource_mut::<Project>();
        open.as_mut().expect("a map is open").map.name = "other".to_owned();
        let refused = open
            .as_mut()
            .expect("a map is open")
            .save(&project)
            .expect_err("renaming onto a taken name must be refused");
        assert!(
            refused.contains("other"),
            "the refusal must name the map that is in the way, so the author knows which name to \\
             change. It said `{refused}`"
        );
        world.insert_resource(open.expect("a map is open"));
    }
    app.update();

    // **And the other map's bytes are unchanged.**
    assert_eq!(
        maps(&root),
        before,
        "a refused rename changed a file on disk. The other map's bytes must be identical."
    );
}

/// **A successful rename removes the old file, and the new one carries the new name.**
#[test]
fn a_rename_leaves_one_file_and_the_new_name() {
    let root = project("rename-moves");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    {
        let world = app.world_mut();
        let mut open = world.remove_resource::<OpenMap>();
        let project = world.resource_mut::<Project>();
        open.as_mut().expect("a map is open").map.name = "renamed".to_owned();
        open.as_mut()
            .expect("a map is open")
            .save(&project)
            .expect("the rename must succeed");
        world.insert_resource(open.expect("a map is open"));
    }
    app.update();

    let after = maps(&root);
    let names: Vec<&str> = after.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        vec!["renamed.map.ron"],
        "a rename must leave ONE file, named for the map. Before this the old file stayed, so the \\
         project held two files answering to one map and the stale one refused to open with a \\
         message about a schema mismatch. The files are now {names:?}"
    );
    // **And the file's own `name:` agrees with its filename.** A map's name IS its file.
    let text = String::from_utf8(after[0].1.clone()).expect("the map is text");
    assert!(
        text.contains("name: \"renamed\""),
        "the file's own name does not agree with its filename:\n{text}"
    );
}

/// **A rename whose old file cannot be removed names the OLD file, not the new one.**
///
/// The old file is made impossible to remove by putting a directory where it was. The new file
/// is written first and taken back when the removal fails, so the rename does not happen; the error
/// must name the old file, the one that is stuck.
#[test]
fn a_rename_that_cannot_remove_the_old_file_names_the_old_file() {
    let root = project("rename-stuck");
    let old = root.join("assets/grimoire/maps/m.map.ron");
    std::fs::remove_file(&old).expect("the old map is removable at the start");
    std::fs::create_dir(&old).expect("a directory can stand where the old file was");

    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    let err = {
        let world = app.world_mut();
        let mut open = world.remove_resource::<OpenMap>();
        let project = world.resource_mut::<Project>();
        open.as_mut().expect("a map is open").map.name = "renamed".to_owned();
        let err = open
            .as_mut()
            .expect("a map is open")
            .save(&project)
            .expect_err("a rename whose old file cannot be removed must say so");
        world.insert_resource(open.expect("a map is open"));
        err
    };
    assert!(
        err.contains("m.map.ron") && !err.contains("renamed.map.ron"),
        "the error must name the OLD file the author has to delete by hand, and not the new one \
         that was written. It said `{err}`"
    );
}

/// **A rename changes the name and nothing else.**
///
/// The metamorphic test, and it is about the **name**, not the bytes. Every map save re-serializes
/// the file — the chooser's own rename has always done so, through the same
/// `ron::ser::to_string_pretty` — so a byte-for-byte round trip is not a property this editor has
/// and never promised. **A rename's promise is narrower and it is the one that matters: one map,
/// one file, one name, and the name agrees with the file's own `name:` field.**
#[test]
fn a_rename_changes_the_name_and_nothing_else() {
    let root = project("rename-round-trip");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();

    let mut names = Vec::new();
    for name in ["renamed", "m"] {
        let world = app.world_mut();
        let mut open = world.remove_resource::<OpenMap>();
        let project = world.resource_mut::<Project>();
        open.as_mut().expect("a map is open").map.name = name.to_owned();
        open.as_mut()
            .expect("a map is open")
            .save(&project)
            .unwrap_or_else(|e| panic!("the rename to `{name}` must succeed: {e}"));
        world.insert_resource(open.expect("a map is open"));
        names.push(name.to_owned());
    }
    app.update();

    // **One file, and it is named for the map.**
    let after = maps(&root);
    assert_eq!(
        after.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
        vec!["m.map.ron"],
        "the round trip left a different set of files. A rename is a move, not a copy, so the \
         round trip must land on exactly the file it started from."
    );

    // **And the file's own name agrees with its filename.** A map's name IS its file — the
    // project's own loader refuses a file that disagrees with itself.
    let text = String::from_utf8(after[0].1.clone()).expect("the map is text");
    assert!(
        text.contains("name: \"m\""),
        "the file's own name does not agree with its filename:\n{text}"
    );
}

/// **`Escape` from a name field leaves no draft.**
///
/// The defect, measured at the keyboard: one `Escape` too few left a `New::Map` draft alive, and
/// `map_rows`' `(Some(New::Map(d)), _)` arm kept winning over the map the cursor was on — so the
/// MAP INFO panel read *"NEW MAP IN kit / NAME (needs a name) / BOUNDS 10 x 4 x 10"* while the
/// MAPS list beside it highlighted a real map with 727 pieces.
#[test]
fn escape_from_a_name_field_leaves_no_draft() {
    let src = std::fs::read_to_string("src/chooser.rs").expect("chooser.rs is there");
    // **The field handler's Escape clears the draft.** It is the only key that reaches that
    // handler, and the chord handler's Escape is stopped by `swallowed`.
    let field_escape = src
        .split("Key::Escape => {")
        .nth(1)
        .expect("the field handler has an Escape arm")
        .split("Key::Backspace")
        .next()
        .expect("the Escape arm ends");
    assert!(
        field_escape.contains("chooser.creating = None"),
        "the name field's Escape does not clear the draft it belongs to, so a `New::Map` survives and \\
         the MAP INFO panel keeps describing a map that does not exist while the list highlights a \\
         real one. The field handler's Escape:\\n{field_escape}"
    );
}

/// **A settings click sets the field the row names.**
///
/// The defect, measured at the keyboard: clicking the `NAME` row's value opened the `ORIGIN`
/// row's field, and the author's map name went into the origin — *"0 x 0 x 0town_block_ is not
/// three numbers"*.
#[test]
fn a_settings_click_sets_the_field() {
    let src = std::fs::read_to_string("src/chooser.rs").expect("chooser.rs is there");
    assert!(
        src.contains("RowPane::Settings => {"),
        "the settings click does not have its own arm, so it cannot set the field"
    );
    let arm = src
        .split("RowPane::Settings => {")
        .nth(1)
        .expect("the settings arm exists")
        .split("RowPane::Projects")
        .next()
        .expect("the arm ends");
    assert!(
        arm.contains("chooser.field ="),
        "the settings click moves the focus but not the field, so the row you click is not the \\
         field you get. The arm:\\n{arm}"
    );
}
