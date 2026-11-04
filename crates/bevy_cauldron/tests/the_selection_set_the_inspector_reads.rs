//! **The Map has a selection set, and the lock anchors it** (UI-50, #236).
//!
//! Until this existed "the selection" was one id (`editor.rs:7450`), so the inspector could not show a
//! mixed value and `placement_set_pose`'s `ids` form had no caller. `Selection` is the set; `TargetLock`
//! stays the anchor the marker, the piece-verbs and an agent's frame target read.

use bevy::prelude::*;
use bevy_cauldron::editor::{lock_on_placement, place_for_test};
use bevy_cauldron::selection::Selection;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for x in [1.5, 2.5, 3.5] {
        place_for_test(app.world_mut(), "site/wall", (x, 0.5));
        app.update();
    }
    app
}

fn ids(app: &App) -> Vec<String> {
    app.world().resource::<Selection>().ids().to_vec()
}

/// **The lock's piece is the selection**, so a click still selects exactly what it did.
#[test]
fn locking_a_piece_selects_it() {
    let mut app = editor("selection_lock");
    let first = {
        let open = app.world().resource::<bevy_cauldron::project::OpenMap>();
        open.map.placements[0].id.clone()
    };
    assert!(lock_on_placement(app.world_mut(), &first));
    app.update();
    assert_eq!(ids(&app), vec![first.clone()], "one lock, one selection");

    let second = {
        let open = app.world().resource::<bevy_cauldron::project::OpenMap>();
        open.map.placements[1].id.clone()
    };
    assert!(lock_on_placement(app.world_mut(), &second));
    app.update();
    assert_eq!(
        ids(&app),
        vec![second],
        "a new lock replaces the set, as a plain click does — the primary is never left behind"
    );
}

/// **Shift-click adds beside the primary and takes it out again**, and the primary itself cannot be
/// toggled away.
#[test]
fn shift_click_adds_beside_the_primary() {
    let mut app = editor("selection_toggle");
    let (primary, other) = {
        let open = app.world().resource::<bevy_cauldron::project::OpenMap>();
        (open.map.placements[0].id.clone(), open.map.placements[1].id.clone())
    };
    assert!(lock_on_placement(app.world_mut(), &primary));
    app.update();
    {
        let mut selection = app.world_mut().resource_mut::<Selection>();
        assert!(selection.toggle(&other, Some(&primary)), "the other piece joins");
    }
    app.update();
    assert_eq!(ids(&app), vec![primary.clone(), other.clone()], "primary first, then the one added");

    {
        let mut selection = app.world_mut().resource_mut::<Selection>();
        assert!(!selection.toggle(&primary, Some(&primary)), "the primary is not toggled away");
        assert!(selection.toggle(&other, Some(&primary)), "and the other leaves again");
    }
    app.update();
    assert_eq!(ids(&app), vec![primary], "the set is the primary alone");
}

/// **A piece taken off the map leaves the set**, as an undo or a reload can leave it.
#[test]
fn a_piece_that_leaves_the_map_leaves_the_selection() {
    let mut app = editor("selection_prune");
    let (primary, other) = {
        let open = app.world().resource::<bevy_cauldron::project::OpenMap>();
        (open.map.placements[0].id.clone(), open.map.placements[2].id.clone())
    };
    assert!(lock_on_placement(app.world_mut(), &primary));
    app.update();
    app.world_mut().resource_mut::<Selection>().toggle(&other, Some(&primary));
    app.update();
    assert_eq!(ids(&app).len(), 2);

    // The map loses the other piece, as an undo of its placement would.
    {
        let mut open = app.world_mut().resource_mut::<bevy_cauldron::project::OpenMap>();
        open.map.placements.retain(|p| p.id != other);
    }
    app.update();
    assert_eq!(ids(&app), vec![primary], "a piece no longer on the map is no longer selected");
}
