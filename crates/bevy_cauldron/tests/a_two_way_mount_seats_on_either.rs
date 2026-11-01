//! **One click, two seatings** — issue #1 driven through the editor's own placement path.
//!
//! # What is proved here
//!
//! `bevy_grimoire_core`'s `a_piece_may_rest_on_a_surface_or_on_the_floor` settles the schema and the
//! arithmetic. This file settles the half a developer actually touches: **a click**. Both go through
//! `editor::place_for_test`, which is `place_one` — the release path of the place tool, without the
//! mouse — so nothing here asks a different question from the one a person's click asks.
//!
//! 1. Dropping the piece on a desk seats it on the desk: `Placed::on` names the desk, and the map
//!    resolves to the desk's height.
//! 2. Dropping the same piece over bare floor seats it on the floor: `on` is `None`.
//! 3. A fill may lay it, because its mount permits the floor — the sibling of the 4,089-invisible-
//!    lamps rule, which must not now refuse the one brush that is legal on open ground.
//! 4. The strict surface-only piece is still refused over bare floor, so the permissive mount did not
//!    silently relax its neighbour.

use crate::fixtures;

use bevy_grimoire_core::stack::resolve_y;
use bevy_cauldron::editor::{EditorState, fill_for_test, place_for_test};
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};

/// A desk that offers `worktop`, a set of tools that may go on it **or** on the floor, and a lamp
/// that may only go on a worktop. `tools` is the piece under test; `lamp` is the control.
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .surface_descriptor("desk", "alpha", "worktop")
        .two_way_descriptor("tools", "alpha", "worktop")
        .mounted_descriptor("lamp", "alpha", "worktop")
        .build("m")
}

fn open(name: &str) -> bevy::prelude::App {
    let root = project(name);
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.update();
    app
}

/// The world Y each placement resolves to, in map order.
fn heights(app: &mut bevy::prelude::App) -> Vec<f32> {
    let world = app.world_mut();
    let open = world.resource::<OpenMap>();
    let project = world.resource::<Project>();
    resolve_y(&open.map, &project.library).unwrap_or_else(|e| panic!("{e}"))
}

/// **Dropped on the desk, the tools sit on the desk.** The desk is 0.5 m tall in the fixture, and the
/// placement records which piece it took — the fact a reload reads.
#[test]
fn the_tools_dropped_on_the_desk_rest_on_the_desk() {
    let mut app = open("two-way-on-desk");
    place_for_test(app.world_mut(), "desk", (0.5, 0.5));
    place_for_test(app.world_mut(), "tools", (0.5, 0.5));
    app.update();

    let desk_id = map_id(&mut app, 0);
    let world = app.world();
    let map = &world.resource::<OpenMap>().map;
    assert_eq!(map.placements.len(), 2, "both pieces were placed");
    assert_eq!(
        map.placements[1].on.as_deref(),
        Some(desk_id.as_str()),
        "the tools must record the desk as their host"
    );
    let ys = heights(&mut app);
    assert!(
        (ys[1] - ys[0] - 0.5).abs() < 0.06,
        "expected the tools on the desk's top, {} m above the desk; got {ys:?}",
        ys[0] + 0.5
    );
}

/// The id of the placement at `index`, which the fixture mints rather than the test.
fn map_id(app: &mut bevy::prelude::App, index: usize) -> String {
    app.world()
        .resource::<OpenMap>()
        .map
        .placements
        .get(index)
        .map(|p| p.id.clone())
        .unwrap_or_else(|| panic!("no placement at {index}"))
}

/// **Dropped on bare floor, the same piece stands on the floor** — `on` is `None`, and it is not
/// refused. This is the half a single `OnSurface` could never do, and the reason the issue exists.
#[test]
fn the_tools_dropped_on_bare_floor_stand_on_the_floor() {
    let mut app = open("two-way-on-floor");
    // The desk is far away, so the click lands on open ground.
    place_for_test(app.world_mut(), "desk", (6.0, 6.0));
    place_for_test(app.world_mut(), "tools", (0.5, 0.5));
    app.update();

    let world = app.world();
    let map = &world.resource::<OpenMap>().map;
    assert_eq!(map.placements.len(), 2, "the tools were placed, not refused");
    assert_eq!(
        map.placements[1].on, None,
        "a piece that took the floor records no host"
    );
    let ys = heights(&mut app);
    assert!(
        (ys[1] - ys[0]).abs() < 0.06,
        "expected the tools at the map's floor level, got {ys:?}"
    );
    // And nothing was refused: taking the floor is an answer, not a problem.
    let status = &app.world().resource::<EditorState>().status;
    assert!(
        !status.has_problem(),
        "the editor complained about a legal placement: {}",
        status.problem_text()
    );
}

/// **A fill may lay it**, because its mount permits the floor. The guard that stops a hosted brush
/// (`editor::mount_class`) must read the *strict* mount; reading the permissive one would refuse the
/// very gesture this feature exists for.
#[test]
fn a_fill_may_lay_a_piece_that_may_take_the_floor() {
    let mut app = open("two-way-fill");
    fill_for_test(app.world_mut(), "tools", ((0.5, 0.5), (2.5, 2.5)), 1.0);
    app.update();

    let world = app.world();
    let map = &world.resource::<OpenMap>().map;
    assert!(
        !map.placements.is_empty(),
        "a fill with a piece that may stand on the floor must lay something"
    );
    assert!(
        map.placements.iter().all(|p| p.on.is_none()),
        "every filled piece is on open floor, so none has a host"
    );
}

/// **The strict piece is still refused**, so the permissive mount did not relax its neighbour. This is
/// the pair that makes `needs_surface` and `may_take_surface` two functions rather than one with a
/// flag.
#[test]
fn a_surface_only_piece_is_still_refused_on_bare_floor_by_a_click() {
    let mut app = open("two-way-strict-control");
    place_for_test(app.world_mut(), "desk", (6.0, 6.0));
    place_for_test(app.world_mut(), "lamp", (0.5, 0.5));
    app.update();

    let world = app.world();
    let map = &world.resource::<OpenMap>().map;
    assert_eq!(
        map.placements.len(),
        1,
        "the lamp must not be placed on bare floor: {:?}",
        map.placements
    );
    let status = app.world().resource::<EditorState>().status.problem_text().to_owned();
    assert!(
        status.contains("worktop"),
        "the refusal must name the class the lamp needs: {status}"
    );
}
