//! **The shell's divider follows a drag, in the editor as it is actually built** (#38, G-34).
//!
//! # What was wrong
//!
//! The host measured, on a live editor through its BRP, a horizontal split between the dock and the
//! viewport whose divider was the hover target (29 x 582 px), where `bevy_picking` reported
//! `pressing = 1` and `dragging = 1`, and `SplitFraction` stayed at the **0.22** the shell authored
//! through every move and the release.
//!
//! `bevy_gooey`'s own suite passes for a split in a window, for one drawn into an image at
//! `UiScale` 1.2, for one nested inside another split, and for one in a `dock()`. So the composition
//! gooey tests is not the composition the editor has, and no test in this crate dragged a divider at
//! all — the shell is built here, so the drag is proved here.
//!
//! # What this file holds down
//!
//! 1. **The editor spawns a divider**, laid out with a real size, on the image its interface is drawn
//!    into. If this is a sliver or absent, every number below is meaningless.
//! 2. **A press on it and a move along the axis changes `SplitFraction`**, and the change is the drag
//!    distance over the room, held to `SplitMin` — not merely "it changed".
//! 3. **The dock's `DockLayout` takes the new fraction when the drag is let go**, so the layout the
//!    editor would save is the layout a person dragged.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::harness;

use crate::fixtures;

/// The editor on a one-piece fixture, settled.
fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name).descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..8 {
        app.update();
    }
    (root, app)
}

/// Where the pointer is: the image the editor draws its interface into.
fn on_the_image(app: &mut App, at: Vec2) -> Location {
    let handle = app
        .world()
        .get_resource::<bevy_cauldron::surface::Surface>()
        .map(|s| s.image.clone())
        .unwrap_or_default();
    Location {
        target: NormalizedRenderTarget::Image(ImageRenderTarget {
            handle,
            scale_factor: 1.0,
        }),
        position: at,
    }
}

fn point(app: &mut App, at: Vec2, action: PointerAction) {
    let location = on_the_image(app, at);
    app.world_mut()
        .write_message(PointerInput::new(PointerId::Mouse, location, action));
    app.update();
}

/// The shell's dividers, in layout pixels.
fn dividers(app: &mut App) -> Vec<(Entity, Rect)> {
    let world = app.world_mut();
    let mut query = world.query_filtered::<(Entity, &ComputedNode, &UiGlobalTransform),
        With<bevy_gooey::SplitDivider>>();
    query
        .iter(world)
        .map(|(entity, node, at)| {
            let scale = node.inverse_scale_factor();
            (entity, Rect::from_center_size(at.translation * scale, node.size() * scale))
        })
        .collect()
}

/// **The editor has a divider that is really laid out.**
#[test]
fn the_editor_spawns_a_laid_out_split_divider() {
    let (_root, mut app) = editor("shell-divider-present");
    let found = dividers(&mut app);
    assert!(
        !found.is_empty(),
        "the shell is a gooey dock, so it must have a divider; none was found"
    );
    let (_, rect) = found[0];
    // **A channel the shell's full height** (owner, 2026-11-06, #314). It was `height > 500` px, the
    // shell's height when the shell had the window to itself; the terminal is now docked under it on
    // every screen, so the shell is shorter and the claim is the one the number stood for: the divider
    // runs from the top of the shell's body to its bottom, and that body is itself a real one.
    let world = app.world_mut();
    let body = world.resource::<bevy_cauldron::chrome::Frame>().body;
    let (node, at) = world
        .query::<(&ComputedNode, &UiGlobalTransform)>()
        .get(world, body)
        .expect("the frame has a laid-out body");
    let scale = node.inverse_scale_factor();
    let body = Rect::from_center_size(at.translation * scale, node.size() * scale);
    assert!(
        rect.width() > 1.0
            && body.height() > 250.0
            && rect.min.y <= body.min.y + 1.0
            && rect.max.y >= body.max.y - 1.0,
        "the divider must be a channel down the shell, not a sliver: {rect:?} in a body of {body:?}"
    );
}

/// **A drag on the shell's divider moves the fraction, by the distance over the room.**
#[test]
fn a_drag_on_the_shells_divider_moves_the_split_fraction() {
    let (_root, mut app) = editor("shell-divider-drag");
    let found = dividers(&mut app);
    let (divider, rect) = found.first().copied().expect("a divider");
    let parent = app.world().get::<ChildOf>(divider).expect("a parent").parent();
    let before = app.world().get::<bevy_gooey::SplitFraction>(parent).expect("a split").0;
    let node = app.world().get::<ComputedNode>(parent).expect("laid out");
    let room = node.size().x * node.inverse_scale_factor();

    // **`rect` is logical and `PointerInput` wants physical**, so the scale is applied here — the
    // editor draws at `EDITOR_UI_SCALE`, and aiming a logical point at a scaled interface lands on
    // whatever widget happens to be there (measured: a 906 x 726 node instead of the divider).
    let scale = app.world().resource::<UiScale>().0;
    let from = rect.center() * scale;
    point(&mut app, from, PointerAction::Move { delta: Vec2::ZERO });
    point(&mut app, from, PointerAction::Press(PointerButton::Primary));
    let to = from + Vec2::new(100.0 * scale, 0.0);
    point(&mut app, to, PointerAction::Move { delta: Vec2::new(100.0, 0.0) });
    for _ in 0..3 {
        app.update();
    }
    let during = app.world().get::<bevy_gooey::SplitFraction>(parent).expect("a split").0;
    point(&mut app, to, PointerAction::Release(PointerButton::Primary));
    for _ in 0..4 {
        app.update();
    }
    let after = app.world().get::<bevy_gooey::SplitFraction>(parent).expect("a split").0;

    let want = before + 100.0 / room;
    assert!(
        (during - after).abs() < 1e-6,
        "a drag reports as it moves and the release keeps what the drag reached: {during} then {after}"
    );
    assert!(
        (after - want).abs() < 0.01,
        "a 100 px drag of a {room} px split should take the fraction {before} -> {want}; it is {after}"
    );
}
