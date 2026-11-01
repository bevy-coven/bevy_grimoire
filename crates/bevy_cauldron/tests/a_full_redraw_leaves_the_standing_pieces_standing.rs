//! **A full redraw does not despawn what the map still holds** — B-048.
//!
//! `docs/media/agent-drives.gif` frame 22 (about 2.2 s): during the first `Cmd+Z` the red field
//! patches **vanish for one frame and return**.
//!
//! **Measured cause.** `draw_every_placement` swept every standing entity unconditionally and then
//! spawned the map again, so for one frame the screen had no placements at all. On the town map at
//! 15 fps the undo frame (`f009`, t=0.60 s) had **0 green field pixels** where its neighbours have
//! 143,694. The person's `Cmd+Z` on a *key* removal never blanked — it takes the targeted path — so
//! the defect was the sweep, which the logged undo and the agent's undo both reach.
//!
//! **The fix is a narrowing, not a removal.** A file edited on disk can take a piece away, so a
//! sweep still has to despawn what the map no longer holds; it no longer despawns what the map still
//! does. This test pins that, and the pairing that makes it correct: survivors must not be spawned a
//! second time either.
//!
//! Asserted on **entity identity**, which is the fact the picture shows: the same entities are still
//! in the world after a redraw. A still cannot see a frame that is not there, and a colour count
//! cannot tell "same piece" from "an identical new piece".

use bevy::prelude::*;
use bevy_cauldron::editor::{Placement, redraw_all_for_test};
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag)
        .descriptor("crate", "alpha")
        .descriptor("barrel", "beta")
        .place("crate", (0.5, 0.5))
        .place("barrel", (4.5, 0.5))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }
    app
}

/// `(placement id, entity)` for every standing piece.
fn standing(app: &mut App) -> std::collections::BTreeMap<String, Entity> {
    let mut q = app.world_mut().query::<(Entity, &Placement)>();
    q.iter(app.world())
        .map(|(e, p)| (p.0.clone(), e))
        .collect()
}

/// Draw the whole map, the way a logged undo and the agent's undo both ask for.
fn redraw(app: &mut App) {
    redraw_all_for_test(app.world_mut());
    for _ in 0..3 {
        app.update();
    }
}

#[test]
fn a_full_redraw_keeps_the_entities_the_map_still_holds() {
    let mut app = editor("b048-redraw-keeps");
    // The harness does not draw the map at open, so this is also the first draw: it must put the
    // map's pieces up.
    redraw(&mut app);
    let before = standing(&mut app);
    assert_eq!(
        before.len(),
        2,
        "the redraw must draw both of the map's pieces: {:?}",
        before.keys().collect::<Vec<_>>()
    );

    // The map has not changed, so a second redraw must change nothing — not one entity.
    redraw(&mut app);
    let after = standing(&mut app);
    assert_eq!(
        after, before,
        "a redraw that despawns a piece the map still holds blanks the screen for a frame, which is \
         B-048: the undo frame had 0 green field pixels where its neighbours had 143,694"
    );
}

#[test]
fn a_full_redraw_still_takes_away_a_piece_the_map_no_longer_holds() {
    let mut app = editor("b048-redraw-removes");
    redraw(&mut app);
    assert_eq!(standing(&mut app).len(), 2);

    // The map loses a row, as a file edited on disk can.
    app.world_mut()
        .resource_mut::<OpenMap>()
        .map
        .placements
        .retain(|p| p.descriptor != "barrel");
    redraw(&mut app);

    let left = standing(&mut app);
    assert_eq!(
        left.len(),
        1,
        "a redraw must take away what the map no longer holds: {:?}",
        left.keys().collect::<Vec<_>>()
    );
    assert!(
        left.contains_key("crate@0"),
        "and leave the piece it does hold standing: {:?}",
        left.keys().collect::<Vec<_>>()
    );
}
