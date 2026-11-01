//! **A snip over a stamp lists the stamped rows it covers, under the stamp they belong to.**
//!
//! B-031: `snip_commit` gathered its placements from `Query<(&Placement, ..)>`. A stamp's drawn rows
//! carry no `Placement`, so a snip over a stamp recorded nothing the person could act on. The fix
//! gathers the stamped rows too, with the stamp's id, which is what `stamp_remove` takes.
//!
//! The record itself is written from an observer that only a render world can complete, so this test
//! asks the same gathering function the snip calls, against the editor's real entities.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_cauldron::editor::{Placement, StampedPiece};
use bevy_cauldron::harness;
use bevy_cauldron::snip;
use bevy_cauldron::view::MainCamera;

use crate::fixtures;
use fixtures::Fixture;

/// A map holding one stamp, `room@1`, of the composition `site/room`, and one free-standing piece,
/// `site/wall@0`. The fixture's map carries no stamps, so the stamp is written into its text.
fn editor(name: &str) -> App {
    let root = Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .composition("site/room", &[("site/wall", "site/wall", (0.0, 0.0))])
        .place("site/wall", (3.0, 0.0))
        .build("m");
    let map = root.join("assets/grimoire/maps/m.map.ron");
    let text = std::fs::read_to_string(&map).expect("the map is there");
    let stamped = text.replace(
        "stamps: [],",
        r#"stamps: [
        (
            id: "room@1",
            of: "site/room",
            at: (0.0, 0.0),
            yaw: 0.0,
            overrides: [],
            of_fingerprint: None,
            owned: false,
            owned_because: None,
            note: None,
        ),
    ],"#,
    );
    assert_ne!(
        stamped, text,
        "the fixture's map has no `stamps: [],` to fill; this test would stamp nothing"
    );
    std::fs::write(&map, stamped).expect("the map is writable");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

#[test]
fn a_snip_over_a_stamp_lists_its_rows_under_the_stamp() {
    let mut app = editor("a_snip_lists_the_stamped_rows_it_covers");

    // **A rectangle about the first drawn stamped row**, in the viewport, and the gathering the snip
    // runs over it — against the real camera, placements and stamped rows.
    let listed = app
        .world_mut()
        .run_system_once(
            |camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
             drawn: Query<&GlobalTransform, With<StampedPiece>>,
             stamped: Query<(&StampedPiece, &GlobalTransform)>,
             placed: Query<(&Placement, &GlobalTransform)>| {
                let (cam, cam_tf) = *camera;
                let tf = drawn
                    .iter()
                    .next()
                    .expect("the stamp drew at least one row");
                let at = cam
                    .world_to_viewport(cam_tf, tf.translation())
                    .expect("the stamped row is on screen");
                let rect = [at.x - 20.0, at.y - 20.0, at.x + 20.0, at.y + 20.0];
                snip::placements_under(rect, cam, cam_tf, &placed, &stamped)
            },
        )
        .expect("the gathering system runs");

    let member = listed
        .iter()
        .find(|p| p.stamp.as_deref() == Some("room@1"))
        .unwrap_or_else(|| {
            panic!(
                "a snip over a stamped row lists nothing under its stamp `room@1`; it recorded \
                 {listed:?}"
            )
        });
    assert!(
        !member.id.is_empty(),
        "the stamped row is listed with no row id: {member:?}"
    );
}
