//! **A kit's save re-reads that kit, not the whole project (E-003's cost half).**
//!
//! # What was wrong
//!
//! `watch::adopt` answered a save to a kit's `library.ron` or `project.ron` by calling
//! `Project::open` again: `kits.ron`, the vocabulary, every bound kit's library and policy, and a
//! triangle count for every `.glb` in the merge. Measured on the shipped project (90 descriptors,
//! 2 kits) one open took 100–315 ms, and the triangle pass alone 22–255 ms — on the file an author
//! edits most, per save.
//!
//! # What this measures, and why a counter rather than a clock
//!
//! A wall-clock assertion on a shared machine is a flaky test (E-011 is the same lesson). So the
//! cost is measured as **work**: how many kit directories were re-layered, and how many `.glb`
//! files were read for their triangle count. The whole-project open does all of them; the per-kit
//! re-read does the edited kit's, and none of another kit's.
//!
//! Both counts are incremented exactly where the work happens, so a test cannot pass by measuring
//! somewhere the work does not go. The three assertions are:
//!
//! 1. a kit save re-layers exactly **one** kit — and it is the edited one;
//! 2. a kit save opens **none** of another kit's GLBs;
//! 3. the answer is still right: the edited piece reaches the screen at its new height, another
//!    kit's piece keeps its count, and the project-wide files are still validated against the
//!    merge (a kit edit that breaks a composition is refused, and the editor keeps what it had).

use bevy::prelude::*;
use bevy_cauldron::editor::Placement;
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;

use crate::fixtures;

/// **One test in this file at a time, because the counters are the process's.**
///
/// `policy::layered_for_test` and `project::triangles_measured_for_test` count work, not threads:
/// the watcher layers a kit on a Bevy task-pool thread, so a `thread_local` would read empty (and
/// did — the first version of this test passed against the unfixed code). A process-wide count is
/// only meaningful if no other test is doing the same work at the same time, so every test here
/// holds this for its whole body. Locking the *file* is enough: the counter assertions are here.
static COUNTERS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// **Take the file's lock, and ignore a poisoned one** — a panicking test must not make the next
/// test fail for a reason that has nothing to do with what it measures.
fn sole_reader() -> std::sync::MutexGuard<'static, ()> {
    COUNTERS.lock().unwrap_or_else(|e| e.into_inner())
}

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

/// **Run the watcher until `done` holds, or the deadline passes.**
///
/// The poll is 250 ms, so this is the watcher's own cadence and not a sleep tuned to the machine.
fn pump_until(app: &mut App, mut done: impl FnMut(&mut App) -> bool) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
    while std::time::Instant::now() < deadline {
        app.update();
        if done(app) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    false
}

/// **The kit's library with one piece twice as tall.**
fn stretch_on_disk(dir: &std::path::Path, factor: f32) {
    let path = dir.join("library.ron");
    let text = std::fs::read_to_string(&path).expect("the kit's library is there");
    let edited = text.replace("stretch_y: None", &format!("stretch_y: Some({factor})"));
    assert_ne!(
        edited, text,
        "the library has no `stretch_y: None` to edit: {text}"
    );
    std::fs::write(&path, edited).expect("the library is written");
}

/// **A project with two kits, one map, and a piece from each on it.**
///
/// Two kits is the point: a one-kit project cannot tell "re-read that kit" from "re-read
/// everything", because they are the same directory.
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("lamp", "alpha")
        .kit("site", "beta", &["site/wall"])
        .place("lamp", (0.0, 0.0))
        .place("site/wall", (2.0, 0.0))
        .build("m")
}

/// **A kit's save re-reads that kit alone.**
///
/// Fails on the unfixed code: `watch::adopt` called `Project::open`, which layers both kits and
/// reads both kits' GLBs.
#[test]
fn a_kit_save_relayers_only_that_kit() {
    let _sole = sole_reader();
    let root = project("e003-one-kit");
    let site_dir = root.join("assets/grimoire/site");
    let furniture_dir = root.join("assets/grimoire/furniture");

    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }

    // **The counts restart here**, so the editor's own open is not charged to the save.
    bevy_grimoire_core::policy::forget_layered_for_test();
    bevy_cauldron::project::forget_triangles_measured_for_test();

    stretch_on_disk(&site_dir, 2.0);

    let adopted = pump_until(&mut app, |app| {
        app.world()
            .resource::<Project>()
            .library
            .get("site/wall")
            .is_some_and(|d| d.align.stretch_y == Some(2.0))
    });
    assert!(
        adopted,
        "the editor never took the library edit, so this test would pass for the wrong reason"
    );

    // **Only this project's kits.** The editor's tests share one process (`tests/all.rs`), so other
    // files' tests layer their own fixtures beside this one; the count is about this project.
    let layers: Vec<_> = bevy_grimoire_core::policy::layered_for_test()
        .into_iter()
        .filter(|dir| dir.starts_with(&root))
        .collect();
    assert_eq!(
        layers.len(),
        1,
        "a save to one kit's library re-layered {layers:?}. A kit's own file can only change that \
         kit — which directory answers for which namespace is `kits.ron`'s business and a different \
         file — so re-reading the others is work that cannot produce a different answer. The \
         whole-project open re-layers every kit, and that is the cost E-003 is about."
    );
    assert_eq!(
        layers[0], site_dir,
        "the save re-layered the wrong kit: {layers:?}"
    );
    assert!(
        !layers.contains(&furniture_dir),
        "the save re-layered a kit that did not change: {layers:?}"
    );

    // **And no other kit's GLB was opened for its triangle count.** A `.glb` nobody edited has
    // not moved, and the count is a fact about the file.
    //
    // **The paths are derived from the project, not from the kit directories**, because a kit's
    // pieces live in a *pack* directory (`assets/beta/wall.glb` for the descriptor `site/wall`) and
    // the pack is the fixture's own choice. What E-003 promises is about which *kit* the files
    // belong to, so that is the question asked.
    let project = app.world().resource::<Project>();
    let elsewhere: Vec<std::path::PathBuf> = project
        .kits
        .iter()
        .filter(|k| k.dir != site_dir)
        .flat_map(|k| k.library.descriptors.iter())
        .filter_map(|d| d.mesh.as_deref().map(|m| project.root.join("assets").join(m)))
        .collect();
    assert!(
        !elsewhere.is_empty(),
        "the fixture has no piece outside the edited kit, so this test would pass for the wrong \
         reason: {elsewhere:?}"
    );
    let reads = bevy_cauldron::project::triangles_measured_for_test();
    assert!(
        reads.iter().all(|p| !elsewhere.contains(p)),
        "the save measured triangles for {reads:?}, which includes a file belonging to a kit that \
         did not change ({elsewhere:?}). The triangle pass is the largest single item in a project \
         open — 22–255 ms of a 100–315 ms open on the shipped project — and re-reading it for a \
         `.glb` nobody touched is the cost E-003 names."
    );
}

/// **And the answer is still right: the piece is drawn at the new height.**
///
/// The cheap path must not be a different answer from the open. This is the same assertion
/// `a_kit_edited_on_disk_reaches_the_screen` makes, kept here because the cost change rewrote the
/// door that test goes through.
#[test]
fn the_edited_piece_is_drawn_at_its_new_height() {
    let _sole = sole_reader();
    let root = project("e003-drawn");
    let site_dir = root.join("assets/grimoire/site");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    let Some(before) = scale_of(&mut app, "site/wall@1") else {
        panic!("the map's piece was never spawned, so there is nothing to measure")
    };
    assert!(
        (before.y - 1.0).abs() < 0.001,
        "the piece did not start at the library's own scale: {before:?}"
    );

    stretch_on_disk(&site_dir, 2.0);

    let mut after = before;
    let drawn = pump_until(&mut app, |app| {
        after = scale_of(app, "site/wall@1").unwrap_or(before);
        (after.y - 2.0).abs() < 0.001
    });
    assert!(
        drawn,
        "the kit's library says the piece is twice as tall and the piece on SCREEN is drawn with \
         scale {after:?}"
    );
}

/// **A kit edit the rest of the project cannot accept is refused, and changes nothing.**
///
/// `Project::open` validates the project's compositions and block rules against the merged
/// library; a per-kit re-read that skipped them would be a second, weaker answer. The composition
/// names a member from the edited kit, so emptying that kit's library makes the composition name a
/// descriptor nothing defines — which the open refuses.
#[test]
fn a_kit_edit_that_breaks_a_composition_is_refused_and_keeps_the_last_good_project() {
    let _sole = sole_reader();
    let root = fixtures::Fixture::new("e003-refused")
        .kit("site", "beta", &["site/wall"])
        .place("site/wall", (0.0, 0.0))
        .composition("tile", &[("a", "site/wall", (0.0, 0.0))])
        .build("m");
    let site_dir = root.join("assets/grimoire/site");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<Project>().compositions.compositions.len(),
        1,
        "the fixture's composition did not load, so this test would pass for the wrong reason"
    );

    // **And now the kit loses its only piece**, which the composition names.
    std::fs::write(
        site_dir.join("library.ron"),
        "(version: 1, note: None, descriptors: [])",
    )
    .expect("the library is written");

    let refused = pump_until(&mut app, |app| {
        app.world()
            .resource::<bevy_cauldron::editor::EditorState>()
            .status
            .problem_text()
            .contains("site/wall")
    });
    assert!(
        refused,
        "removing the only descriptor a composition names was accepted, or refused silently. The \
         project open refuses it — the composition check is against the merge — and a per-kit \
         re-read that skipped the project-wide checks would be a second, weaker answer."
    );
    // **And the editor kept the last good project**: the piece is still in the library the palette
    // draws from, and the composition is still there.
    let project = app.world().resource::<Project>();
    assert!(
        project.library.get("site/wall").is_some(),
        "the refused re-read emptied the library anyway, so the editor is showing a project it \
         never validated"
    );
    assert_eq!(
        project.compositions.compositions.len(),
        1,
        "the refused re-read dropped the project's compositions"
    );
}
