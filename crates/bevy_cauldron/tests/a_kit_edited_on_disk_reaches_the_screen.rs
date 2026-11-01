//! **A kit edited on disk reaches the screen, and not only the editor's memory.**
//!
//! # What was broken
//!
//! `watch::adopt` re-reads a kit's `library.ron` and replaces the whole `Project` with a fresh
//! `Project::open`. **That is the memory.** The placed pieces on screen were spawned from the
//! library the project held *then*, and nothing told the Map to redraw them: `Due::map_replaced`
//! is the map file's flag, and the redraw of *pieces* is driven by `Project::touched`, which
//! `tiles::commit_measured` fills by comparing libraries — a door the watcher never goes through.
//!
//! So a person who edits a piece's `stretch_y` (or its `scale`, its `extent`, its mesh) in a text
//! editor gets the inspector's numbers and the palette updated, and **the world unchanged**.
//!
//! That is E-007's shape exactly, one file over: the editor *looks* like it took the edit, the
//! number agrees with the file, and the picture is a lie. A developer who trusts the number is
//! wrong about what they are authoring.
//!
//! # What this test does
//!
//! It runs the real editor headless over a fixture, edits the kit's `library.ron` on disk so the
//! piece is twice as tall, lets the watcher poll, and asks **the world** for the piece's scale.
//! The assertion is on the entity's `Transform`, not on the project's library: a test that
//! asserted on `Project` would have passed while the screen showed the old shape, which is the
//! bug.

use bevy::prelude::*;
use bevy_cauldron::editor::Placement;
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;

use crate::fixtures;

/// **Where a piece named `id` is drawn, as a scale.** The world, not the map.
fn scale_of(app: &mut App, id: &str) -> Option<Vec3> {
    let mut found = None;
    let mut q = app.world_mut().query::<(&Placement, &Transform)>();
    for (placement, transform) in q.iter(app.world()) {
        if placement.0 == id {
            found = Some(transform.scale);
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

/// **The kit's library with the piece twice as tall.**
///
/// The edit is made in the file, because that is the whole case: a person in a text editor. The
/// kit is the fixture's default one, whose directory the fixture wrote.
fn stretch_the_piece_on_disk(root: &std::path::Path, factor: f32) {
    let path = root
        .join("assets")
        .join("grimoire")
        .join(fixtures::DEFAULT_KIT)
        .join("library.ron");
    let text = std::fs::read_to_string(&path).expect("the kit's library is there");
    let edited = text.replace("stretch_y: None", &format!("stretch_y: Some({factor})"));
    assert_ne!(
        edited, text,
        "the library has no `stretch_y: None` to edit: {text}"
    );
    std::fs::write(&path, edited).expect("the library is written");
}

/// **A piece whose kit was edited on disk is drawn at the new size.**
///
/// This fails on the unfixed code: the watcher re-opens the project, the library says twice as
/// tall, and the piece on screen is the one spawned when the editor opened.
#[test]
fn a_piece_stretched_on_disk_is_drawn_stretched() {
    let root = project("hot-reload-kit");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }

    // **The piece is on the screen at its authored height.**
    let Some(before) = scale_of(&mut app, "site/wall@0") else {
        panic!("the map's piece was never spawned, so there is nothing to measure")
    };
    assert!(
        (before.y - 1.0).abs() < 0.001,
        "the piece did not start at the library's own scale: {before:?}"
    );

    // **A person edits the kit.**
    stretch_the_piece_on_disk(&root, 2.0);

    // **And the watcher is allowed its poll** — 250 ms, plus the frames in between.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
    let mut after = before;
    let mut adopted = false;
    while std::time::Instant::now() < deadline {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(20));
        // **The library is the easy half, and it is what says the editor took the edit.** Without
        // it, the test could pass for the wrong reason.
        let library = app.world().resource::<Project>().library.clone();
        adopted = library
            .get("site/wall")
            .is_some_and(|d| d.align.stretch_y == Some(2.0));
        after = scale_of(&mut app, "site/wall@0").unwrap_or(before);
        if adopted && (after.y - 2.0).abs() < 0.001 {
            break;
        }
    }

    assert!(
        adopted,
        "the editor never took the library edit from the file, so this test would pass for the \
         wrong reason"
    );
    assert!(
        (after.y - 2.0).abs() < 0.001,
        "the kit's library says the piece is twice as tall and the editor's project agrees, but the \
         piece on SCREEN is drawn with scale {after:?} — the editor took the edit into its memory \
         and left the world showing the piece as it was when the editor opened. That is E-007's \
         shape on the library path: the number changes and the picture does not, so a developer \
         who trusts the number is wrong about what they are looking at."
    );
}

/// **A kit edit does not make the editor forget which map is open.**
///
/// The second half of the same door. `watch::adopt` re-opens the whole project, and
/// `Project::open` starts with `open_map_name: None` — correct for a first open, wrong for a
/// re-read of the project that is already open. The name is the author's intent about which map
/// they are working on, and it is what a crossing from the Kit door back to the Map door reads
/// when no map is loaded.
///
/// Without the carry, editing a kit's library in a text editor made a later Kit → Map crossing
/// refuse with *"the Map door needs a map, and none has been open"* — a true sentence about an
/// author who was on a map a minute ago.
#[test]
fn a_kit_edit_does_not_forget_the_open_map() {
    let root = project("hot-reload-kit-name");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app.world_mut()
        .resource_mut::<Project>()
        .open_map_name = Some("m".to_owned());

    stretch_the_piece_on_disk(&root, 2.0);

    // **The watcher is allowed its poll**, and the assertion is that the re-read *happened* —
    // otherwise this passes for the wrong reason on an editor that ignored the file.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
    let mut adopted = false;
    while std::time::Instant::now() < deadline {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(20));
        adopted = app
            .world()
            .resource::<Project>()
            .library
            .get("site/wall")
            .is_some_and(|d| d.align.stretch_y == Some(2.0));
        if adopted {
            break;
        }
    }
    assert!(
        adopted,
        "the editor never took the library edit, so this test would pass for the wrong reason"
    );
    assert_eq!(
        app.world().resource::<Project>().open_map_name.as_deref(),
        Some("m"),
        "a kit edit re-opened the project and dropped the name of the map the author had open, so \
         the next crossing back to the Map door refuses with 'none has been open' — about an \
         author who was on a map a minute ago."
    );
}
