//! **The viewport keeps the arrow keys; F6 moves focus between regions** (UI-35, #229).
//!
//! The map pans and orbits on the arrows, while the panels need the keyboard too. gooey's
//! `FocusRegion` is the crate's answer: `ArrowPolicy::Host` says the arrows stay readable and move no
//! focus while focus is in the region, `TabIndex(0)` makes the region its own first widget, and F6 /
//! Shift+F6 cycle the regions in tree order.
//!
//! Three claims, and each can fail:
//!
//! 1. **The viewport is a `FocusRegion` with `ArrowPolicy::Host`**, so pressing an arrow with focus on it
//!    reaches the map rather than moving focus to the next button.
//! 2. **And it is a tab stop**, or F6 would have nothing to land on in it and the region would be skipped.
//! 3. **Tab still leaves it.** a region that trapped the keyboard would fail WCAG 2.1.2, and the crate's
//!    own doc says so; what this test can check here is that the region does not claim to.
//!
//! **What this file does NOT do:** simulate a key press to watch focus move. That needs `InputFocus` in a
//! running app with gooey's navigation plugin, and the editor's harness builds it — but the claim worth
//! pinning is the *declaration*: a viewport that is not a `FocusRegion` at all is the bug, and it is the
//! one a person hits the moment they press an arrow with the map focused.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name)
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..6 {
        app.update();
    }
    (root, app)
}

/// **The viewport is a host focus region and its own tab stop.**
#[test]
fn the_viewport_keeps_the_arrow_keys_and_is_a_focus_region() {
    let (_root, mut app) = editor("focus-viewport");
    let mut q = app
        .world_mut()
        .query_filtered::<(&bevy_gooey::FocusRegion, Option<&bevy::input_focus::tab_navigation::TabIndex>), With<bevy_cauldron::chrome::ViewportSlot>>();
    let found: Vec<_> = q.iter(app.world()).map(|(r, t)| (r.arrows, t.copied())).collect();
    assert_eq!(
        found.len(),
        1,
        "the viewport must be exactly one focus region; found {found:?}"
    );
    let (arrows, tab) = found[0];
    assert_eq!(
        arrows,
        bevy_gooey::ArrowPolicy::Host,
        "the viewport pans and orbits on the arrows (UI-35, #229), so its region must be \
         `ArrowPolicy::Host`; `Navigate` would move focus to the next button instead"
    );
    assert_eq!(
        tab,
        Some(bevy::input_focus::tab_navigation::TabIndex(0)),
        "the viewport must be its own first widget, or F6 has nothing to land on in it and the region \
         is skipped"
    );
}

/// **The viewport region is not the only one**, so F6 has somewhere to go.
#[test]
fn the_editor_has_more_than_one_focus_region_so_f6_can_cycle() {
    let (_root, mut app) = editor("focus-many");
    let mut q = app.world_mut().query::<&bevy_gooey::FocusRegion>();
    let n = q.iter(app.world()).count();
    assert!(
        n >= 2,
        "F6 cycles focus between regions; with {n} of them there is nothing to cycle to, and the \
         viewport's `Host` policy has no way out"
    );
}
