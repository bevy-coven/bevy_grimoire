//! **A project file written for the first time while the editor is open is read** (#158).
//!
//! `repetition.ron`, `standards.ron`, `rules.ron` and `compositions.ron` are optional, so the first
//! edit an author makes to one is creating it. The watcher took a file with no earlier stamp for
//! the editor's first sighting of what was already there and skipped it, so the REPEATS line kept
//! the defaults until the file was edited a second time.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

fn texts(app: &mut App) -> Vec<String> {
    for _ in 0..3 {
        app.update();
    }
    let world = app.world_mut();
    let mut q = world.query::<&Text>();
    q.iter(world).map(|t| t.0.clone()).collect()
}

#[test]
fn a_repetition_file_created_after_the_open_reaches_the_repeats_row() {
    let root = fixtures::Fixture::new(&format!("created_while_open_{}", std::process::id()))
        .descriptor("site/barrel", "alpha")
        .place("site/barrel", (2.0, 2.0))
        .place("site/barrel", (3.0, 2.0))
        .place("site/barrel", (4.0, 2.0))
        .build("m");
    let file = root
        .join("assets/grimoire")
        .join(bevy_grimoire_core::repetition::FILE);
    assert!(!file.exists(), "the project starts with no {file:?}");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // Past the watcher's first poll, so the file's absence has been sighted.
    for _ in 0..8 {
        app.update();
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    let before = texts(&mut app);
    assert!(
        before.iter().any(|t| t == "site/barrel \u{d7}3"),
        "three barrels a metre apart repeat under the defaults: {before:?}"
    );

    // A metre apart is out of sight once the radius is half a metre.
    std::fs::write(&file, "(radius_m: 0.5)").unwrap_or_else(|e| panic!("{e}"));
    // The watcher polls every 250 ms on the wall clock.
    let mut seen = false;
    for _ in 0..40 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        if !texts(&mut app).iter().any(|t| t.starts_with("site/barrel \u{d7}")) {
            seen = true;
            break;
        }
    }
    assert!(
        seen,
        "a repetition.ron created while the editor was open never reached the REPEATS row: {:?}",
        texts(&mut app)
    );
}
