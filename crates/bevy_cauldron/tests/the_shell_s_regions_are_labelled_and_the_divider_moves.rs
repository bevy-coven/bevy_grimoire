//! **Every region of the workbench shell is findable by name, and its divider resizes the docks**
//! (UI-30, #224).
//!
//! The frame's regions were reachable only by the `Frame` resource: an entity id in Rust and nothing
//! a reader, a test or an agent could search for. A region nobody can name is a region nobody can
//! aim at — `record.py` guesses pixels, a screen reader says "group", and an acceptance item that
//! says "find each region by its accessible label" has nothing to find.
//!
//! The second half is the divider, and it is the half that already worked: `Split` carries
//! `#[require(SplitFraction, SplitMin)]`, so the split spawned in the frame has the minimums
//! `SplitGeometry::read` needs and a fraction change reaches the panes. This test pins that, because
//! the frame's spawn spells its own `Node` beside `bevy_gooey::split` and a future edit that drops
//! the `Split` (or replaces the bundle with a bare `Node`) would take every drag, click, arrow key
//! and double click with it, silently — the divider would still take the pointer's cursor.

use bevy::prelude::*;
use bevy::ui::ComputedNode;

use bevy_cauldron::harness;

use crate::fixtures;

/// The four names the shell's own regions answer to.
const NAMES: [&str; 4] = ["Viewport", "Left dock", "Right dock", "Status bar"];

#[test]
fn the_shells_regions_are_labelled() {
    let root = fixtures::Fixture::new("shell_regions_labelled")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.update();
    app.update();

    let mut labels = app
        .world_mut()
        .query::<(&AccessibleLabel, &ComputedNode)>();
    let found: Vec<String> = labels
        .iter(app.world())
        .filter(|(_, node)| node.size().x > 1.0 && node.size().y > 1.0)
        .map(|(label, _)| label.0.clone())
        .collect();

    for name in NAMES {
        assert!(
            found.iter().any(|l| l == name),
            "no laid-out node answers to {name:?}; a region nobody can name cannot be aimed at by \
             a reader, a test or an agent. Laid out labels found: {found:?}"
        );
    }
}

#[test]
fn the_shells_divider_resizes_the_docks() {
    let root = fixtures::Fixture::new("shell_divider_moves")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.update();
    app.update();

    let split = app
        .world_mut()
        .query_filtered::<Entity, With<bevy_gooey::Split>>()
        .iter(app.world())
        .next()
        .expect("the shell has one split");

    // **The geometry `SplitGeometry::read` needs is on the split** — its fraction and the panes'
    // minimums. `Split` requires both, so this holds until someone replaces the bundle; when it
    // does not, `read` answers `None` for every divider and every drag, click and arrow key in it
    // moves nothing while the divider still takes the pointer's cursor.
    for (what, present) in [
        ("SplitFraction", app.world().get::<bevy_gooey::SplitFraction>(split).is_some()),
        ("SplitMin", app.world().get::<bevy_gooey::SplitMin>(split).is_some()),
    ] {
        assert!(
            present,
            "the shell's split has no {what}, so no divider in it can be read or moved"
        );
    }

    // And a fraction change reaches the layout: the first pane grows by the amount asked for.
    let panes: Vec<Entity> = {
        let children = app.world().get::<Children>(split).expect("the split has children");
        children.iter().collect()
    };
    let first_pane = panes[0];
    let before = app
        .world()
        .get::<ComputedNode>(first_pane)
        .map(|n| n.size().x)
        .unwrap_or(0.0);

    if let Some(mut fraction) = app.world_mut().get_mut::<bevy_gooey::SplitFraction>(split) {
        fraction.0 = 0.40;
    }
    for _ in 0..4 {
        app.update();
    }

    let after = app
        .world()
        .get::<ComputedNode>(first_pane)
        .map(|n| n.size().x)
        .unwrap_or(0.0);
    assert!(
        after > before + 1.0,
        "the split's fraction went 0.22 -> 0.40 and its first pane measured {before} -> {after} px: \
         the fraction is not reaching the layout"
    );
    let _ = App::new();
}
