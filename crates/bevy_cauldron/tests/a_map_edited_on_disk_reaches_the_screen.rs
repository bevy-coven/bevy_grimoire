//! **A map edited on disk reaches the screen, and not only the editor's memory.**
//!
//! The regression test for **E-007**, the 2026-10-21-04 run's bug.
//!
//! # What was broken
//!
//! **The file watcher read the map file and put the new map into `OpenMap`** — and the placed
//! pieces were spawned **once, when the editor opened** (`editor.rs::spawn_existing`, on
//! `OnEnter(Screen::Editor)`). So a person editing `town.map.ron` in a text editor got:
//!
//! - **the inspector's numbers updated** — the bounds, the placement count, the note, all read
//!   from `OpenMap` every frame;
//! - **the world unchanged** — the pieces on screen were the ones spawned at startup, in the
//!   places they were then.
//!
//! **And that is the worst shape a bug can have:** the editor *looks* like it took the edit. The
//! number says the map is 55 m deep and the ground in the viewport is 26 m deep. A developer who
//! trusts the number is wrong about the picture, and the picture is what they are authoring.
//!
//! # What this test does
//!
//! **It runs the real editor, headless, and measures the world.** The map file is edited on
//! disk, the watcher is allowed its poll, and then the test asks the world where the piece is.
//!
//! **The assertion is on the piece's transform, not on the map's state.** The state is the easy
//! half — it already worked. A test that asserted on `OpenMap` would have passed while the
//! screen showed the old pieces, which is the bug.

use bevy::prelude::*;
use bevy_cauldron::editor::Placement;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

/// **Where a piece named `id` stands in the world.**
///
/// **The world, not the map.** The map is a file; the world is what the author sees.
fn where_it_stands(app: &mut App, id: &str) -> Option<Vec3> {
    let mut found = None;
    let mut q = app.world_mut().query::<(&Placement, &Transform)>();
    for (placement, transform) in q.iter(app.world()) {
        if placement.0 == id {
            found = Some(transform.translation);
        }
    }
    found
}

/// **A project with one map and one piece on it.**
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m")
}

/// **The map file's text, with the piece's position changed.**
///
/// **The edit is made in the file, not in the editor** — because that is the whole case: a person
/// in a text editor.
fn move_the_piece_on_disk(root: &std::path::Path, to: (f32, f32)) {
    let path = root
        .join("assets")
        .join("grimoire")
        .join("maps")
        .join("m.map.ron");
    let text = std::fs::read_to_string(&path).expect("the map file is there");
    let (x, z) = to;
    let moved = text.replace("at: (0.0, 0.0)", &format!("at: ({x}, {z})"));
    assert_ne!(
        moved, text,
        "the piece's position is not in the file: {text}"
    );
    std::fs::write(&path, moved).expect("the map file is written");
}

/// **A piece moved in the file stands in the new place on screen.**
///
/// **This is the whole of E-007**, and it fails on the unfixed code: the watcher puts the new
/// map into `OpenMap` and the piece stays where it was spawned.
#[test]
fn a_piece_moved_on_disk_stands_in_its_new_place() {
    let root = project("hot-reload-map");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }

    // **The piece is on the screen, where the file says.**
    let before = where_it_stands(&mut app, "site/wall@0");
    let Some(before) = before else {
        panic!("the map's piece was never spawned, so there is nothing to move")
    };
    assert!(
        before.x.abs() < 0.001 && before.z.abs() < 0.001,
        "the piece did not start at the origin: {before:?}"
    );

    // **A person edits the file.**
    move_the_piece_on_disk(&root, (12.0, -7.0));

    // **And the watcher is allowed its poll** — 250 ms, plus the frames in between.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
    let mut after = before;
    while std::time::Instant::now() < deadline {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(20));
        // **The map's state is the easy half, and it is what says the editor took the edit.**
        let map = app.world().resource::<OpenMap>().map.clone();
        if let Some(p) = map.placements.iter().find(|p| p.descriptor == "site/wall")
            && (p.at.0 - 12.0).abs() < 0.001
        {
            // **The editor has the new map. The question is the screen.**
            after = where_it_stands(&mut app, "site/wall@0").unwrap_or(before);
            break;
        }
    }

    // **The editor took the edit.** Without this, the test would pass for the wrong reason.
    let map = app.world().resource::<OpenMap>().map.clone();
    let placed = map
        .placements
        .iter()
        .find(|p| p.descriptor == "site/wall")
        .expect("the piece is in the map");
    assert!(
        (placed.at.0 - 12.0).abs() < 0.001 && (placed.at.1 + 7.0).abs() < 0.001,
        "the editor never took the edit from the file, so this test would pass for the wrong \
         reason: the map says {placed:?}"
    );

    // **And the piece is where the file says.** The world is the screen, and the screen is what
    // the author is authoring.
    assert!(
        (after.x - 12.0).abs() < 0.5 && (after.z + 7.0).abs() < 0.5,
        "the file says the piece is at (12.0, -7.0) and the editor's map agrees, but the piece on \\
         SCREEN is at {after:?} -- the editor took the edit into its memory and left the world \\
         showing the piece as it was when the editor opened. That is E-007: the inspector's \\
         numbers change and the picture does not, so a developer who trusts the number is wrong \\
         about what they are looking at."
    );
}

/// **A map file saved with a typo is refused where the Map door shows it, and the last good map
/// stays.**
///
/// B-016. The watcher kept the last good map (right) but wrote its refusal into the kit door's
/// status (`tiles::ImportState`), so on the Map door, the only door with a map open, nothing said
/// the edit had been refused: no problem, no toast, no journal line, and `grimoire_state` reported
/// `problems: []`. The author saw the old map and no reason. Measured live on 2026-10-21 with
/// `bounds: ((` written into Sanctuary's town map (scratch copy).
#[test]
fn a_malformed_map_on_disk_is_refused_on_the_map_door() {
    let root = project("hot-reload-bad-map");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    let path = root
        .join("assets")
        .join("grimoire")
        .join("maps")
        .join("m.map.ron");
    let text = std::fs::read_to_string(&path).expect("the map file is there");
    let broken = text.replacen("bounds: (", "bounds: ((", 1);
    assert_ne!(broken, text, "the map file has no bounds to break: {text}");
    std::fs::write(&path, broken).expect("the map file is written");

    // **The watcher is allowed its poll**, as above.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
    let mut said = String::new();
    while std::time::Instant::now() < deadline {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(20));
        said = app
            .world()
            .resource::<bevy_cauldron::editor::EditorState>()
            .status
            .problem_text()
            .to_owned();
        if !said.is_empty() {
            break;
        }
    }
    assert!(
        said.contains("m.map.ron"),
        "the malformed map file was refused nowhere the Map door shows, so the author sees the old \
         map and no reason; the Map's problem slot reads {said:?}"
    );
    // **And the last good map is still the one loaded.**
    let map = app.world().resource::<OpenMap>().map.clone();
    assert!(
        map.placements.iter().any(|p| p.descriptor == "site/wall"),
        "the refusal dropped the last good map"
    );
}
