//! **The Kit workspace's panels are built from gooey's widgets** (UI-59, #245).
//!
//! The Meshes, Tiles and Compose panels laid themselves out with raw `Node` literals. This file holds
//! down what #245 moved and how much is left, so the next slice starts from a measured count rather
//! than a guess:
//!
//! - the **Compose** pane's body — its label/value rows and the indent under them — is gooey's `row`,
//!   `column` and `cell`, and the module is down to the two `Node`s it cannot express (a world-space
//!   slot label and one flex item whose grow-and-zero-minimum measure was chosen by measurement);
//! - the **Tiles** module's direction-only rows and columns are gooey's too, and the count of what
//!   remains is the ratchet.
//!
//! # Why a count and not zero
//!
//! A `bevy_gooey::row` carries no margin, and about twenty of `tiles.rs`'s remaining literals are rows
//! whose whole reason for existing is a block or half-step gap above them (`margin: top(GAP_GROUP)`).
//! Converting those would silently close the spacing between a panel's blocks, which is a visible
//! regression in a commit whose subject is "no raw `Node`". They wait for a gooey that can say a
//! layout's own margin, and this file says how many there are so the number cannot grow.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_gooey::Role;

use crate::fixtures;

/// The number of raw `Node {` literals `tiles.rs` may carry. Every one that goes is a step of #245;
/// nothing may raise it.
// 43 -> 42: #379 removed the strip chips' digit label (a `Node` margin), with the slot keys it printed.
const TILES_NODES_LEFT: usize = 42;
/// The number `compose.rs` may carry: a world-space slot label, the flex item the 2026-08-17
/// measurement sized, and one `Node::default()` that is a bundle's own direction.
const COMPOSE_NODES_LEFT: usize = 3;

/// **Every spelling of a raw `Node` literal in `name`** (review of #245): `Node {` and `Node{` are the
/// same thing to Rust, and `Node::default()` is the same thing to a reader — counting one spelling let
/// a literal move between the two and pass.
fn count(dir: &std::path::Path, name: &str) -> usize {
    let text = std::fs::read_to_string(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    text.lines()
        .filter(|line| line.contains("Node {") || line.contains("Node{") || line.contains("Node::default()"))
        .count()
}

/// **The ratchet**: each module's raw `Node` count, against the number this issue left it at.
#[test]
fn the_kit_modules_are_down_to_the_nodes_this_issue_could_not_express() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for (name, left) in [("tiles.rs", TILES_NODES_LEFT), ("compose.rs", COMPOSE_NODES_LEFT)] {
        let found = count(&dir, name);
        assert!(
            found == left,
            "{name} carries {found} raw `Node` literals, and this issue left it at {left}: a raw `Node` \
             is a second layout path beside gooey's `row`, `column`, `cell`, `image_box` and `overlay`. \
             If a new one is genuinely needed (a margin gooey cannot carry, a flex measure chosen by \
             measurement), lower nothing: give it a `CHROME-OK:` marker and lower the number here by \
             the same amount in one commit."
        );
    }
    // **And the labels module never had one**, so it cannot gain one unnoticed.
    assert_eq!(count(&dir, "labels.rs"), 0, "labels.rs lays itself out with gooey's widgets only");
}

/// The Compose pane, settled.
fn compose(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("kit_gooey_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, bevy_cauldron::tiles::Mode::Compose)
        .unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..10 {
        app.update();
    }
    app
}

/// **The Compose pane draws gooey rows and columns**: the whole pane is the crate's, not a hand-built
/// column of `Node`s.
#[test]
fn the_compose_pane_is_laid_out_with_gooey_rows_and_columns() {
    let mut app = compose("rows");
    let (rows, columns) = {
        let world = app.world_mut();
        let count = |world: &mut World, want: Role| {
            world.query::<&Role>().iter(world).filter(|role| **role == want).count()
        };
        (count(world, Role::Row), count(world, Role::Column))
    };
    assert!(rows > 0, "the Compose pane's label/value lines are gooey `row`s");
    assert!(columns > 0, "and the blocks of lines beside a label are gooey `column`s");
}
