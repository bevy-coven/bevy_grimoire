//! **A wheel turned over the inspector scrolls it, in the editor as it is built** (#283).
//!
//! `the_wheel_scrolls_a_panel.rs` pins that `ScrollAreaPlugin` is installed, which was the line that
//! was missing. It cannot say that the wheel *reaches* the inspector once the layout around it exists:
//! the shell's split, the docks and the panel's own padding all sit between the pointer and the
//! `ScrollArea`, and #283's acceptance asks for the behaviour — "a test can assert the panel's
//! `ScrollPosition` changes when a wheel event is delivered over it, and that at least one section
//! below `EDGES` is inside the window after scrolling".
//!
//! Driven the way a person's wheel arrives: a pointer move onto the panel's content, then a scroll
//! `PointerInput`, through picking, the hover map and the observer `bevy_ui_widgets` installs.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform};
use bevy_cauldron::editor::MapControlsPane;
use bevy_cauldron::harness;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..12 {
        app.update();
    }
    app
}

fn pointer(app: &mut App, at: Vec2, action: PointerAction) {
    let handle = app
        .world()
        .get_resource::<bevy_cauldron::surface::Surface>()
        .map(|s| s.image.clone())
        .unwrap_or_default();
    let location = Location {
        target: NormalizedRenderTarget::Image(ImageRenderTarget {
            handle,
            scale_factor: 1.0,
        }),
        position: at,
    };
    app.world_mut()
        .write_message(PointerInput::new(PointerId::Mouse, location, action));
    app.update();
}

/// The inspector's scroll area, its scroll offset, and how far it can go.
fn inspector(app: &mut App) -> (Entity, f32, f32, Rect) {
    let world = app.world_mut();
    let mut q = world.query_filtered::<(Entity, &ScrollPosition, &ComputedNode, &UiGlobalTransform), With<MapControlsPane>>();
    let (e, pos, node, at) = q.single(world).expect("the editor has one inspector scroll area");
    let scale = node.inverse_scale_factor();
    let visible = node.size() * scale;
    let content = node.content_size() * scale;
    let rect = Rect::from_center_size(at.translation * scale, visible);
    (e, pos.y, (content.y - visible.y).max(0.0), rect)
}

#[test]
fn a_wheel_over_the_inspectors_content_scrolls_it() {
    let mut app = editor("wheel_scrolls_the_inspector");
    let (_, before, range, rect) = inspector(&mut app);
    assert!(
        range > 100.0,
        "the inspector's content must be taller than its box for the wheel to have anywhere to go \
         (scrollable range {range}); without that this test proves nothing"
    );
    assert_eq!(before, 0.0, "the inspector starts at the top");

    // A point on the panel's content, away from the scrollbar's lane and the panel's padding.
    let over = rect.center();
    pointer(&mut app, over, PointerAction::Move { delta: Vec2::ZERO });
    for _ in 0..3 {
        pointer(
            &mut app,
            over,
            PointerAction::Scroll {
                unit: bevy::input::mouse::MouseScrollUnit::Line,
                x: 0.0,
                y: -1.0,
                phase: bevy::input::touch::TouchPhase::Moved,
            },
        );
    }
    let (_, after, _, _) = inspector(&mut app);
    assert!(
        after > 1.0,
        "three wheel notches over the inspector's content left its scroll at {after}: the wheel did \
         not reach the panel (the plugin is installed; something between the pointer and the \
         ScrollArea took the event)"
    );
}
