//! **The viewport stays usable when the window is narrow** — B-014.
//!
//! At about 900 logical px the two fixed docks (`chrome::CONTROLS_W` 380 + `chrome::LIST_W` 264, plus
//! `2 * MARGIN`) leave the map a sliver: the cycle-5 measurement was "about 70 logical px of viewport
//! at 1800 x 1406 physical", which is a window where every panel is still on screen and the thing
//! being authored is not.
//!
//! **Measured from the real layout**, not from arithmetic: `ComputedNode` and `UiGlobalTransform` in a
//! headless app, the same reading `a_receipt_does_not_move_the_panel` uses. The question is the one
//! `chrome::ViewportSlot` exists to answer — how wide is the hole the world is drawn through — and a
//! still cannot say what the camera was given.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::{PrimaryWindow, Window, WindowResolution};
use bevy_cauldron::chrome::ViewportSlot;
use bevy_cauldron::harness;

use crate::fixtures;

/// The viewport's width in logical px, at the given window size.
fn viewport_width(logical_w: f32, logical_h: f32, tag: &str) -> f32 {
    let root = fixtures::Fixture::new(tag)
        .descriptor("crate", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(logical_w as u32, logical_h as u32),
            ..default()
        },
        PrimaryWindow,
    ));
    for _ in 0..8 {
        app.update();
    }
    let mut q = app
        .world_mut()
        .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<ViewportSlot>>();
    let (node, _) = q
        .single(app.world())
        .unwrap_or_else(|e| panic!("the editor has one viewport slot: {e:?}"));
    node.size().x
}

/// **A narrow window still gives the map a real area to author in.**
///
/// The floor is deliberately modest — 200 px — because this is not a claim about taste. It is the
/// point past which the viewport stops being a place to place a piece: below it the map is narrower
/// than two palette rows and the ghost's footprint is wider than the view.
#[test]
fn a_narrow_window_keeps_the_viewport_wide_enough_to_author_in() {
    let width = viewport_width(900.0, 700.0, "b014-narrow");
    assert!(
        width >= 200.0,
        "at a 900 px window the viewport is {width} px wide, which is a sliver between the docks \
         rather than a place to author in (B-014: 'about 70 logical px at 1800 x 1406 physical')"
    );
}

/// **And an ordinary window is unaffected**, so whatever the narrow case does cannot cost the
/// common one.
///
/// 769 is what this measured **before** the change as well as after, and that is the point of the
/// test: the two docks owe `(380 + 24 + 264 + 24) * 1.2 = 830` at `UiScale(1.2)`, so 1600 px leaves
/// the map 769 and there is nothing to squeeze. An ordinary window loses not one pixel to this fix.
#[test]
fn an_ordinary_window_keeps_its_viewport() {
    let wide = viewport_width(1600.0, 900.0, "b014-wide");
    assert!(
        wide >= 760.0,
        "at a 1600 px window the viewport should be about 769 px — its width before this change — \
         and the fix must not cost an ordinary window anything; got {wide}"
    );
}
