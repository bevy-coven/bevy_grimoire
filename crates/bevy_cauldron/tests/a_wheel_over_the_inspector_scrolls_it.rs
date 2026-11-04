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

/// **A wheel turned over the inspector's scrollbar scrolls the inspector too** (#283). The track is a
/// sibling of the scroll area, not inside it, so the wheel over its five pixels bubbled through the panel
/// and reached nothing: measured live, a wheel at the panel's right edge did nothing while one 12 px to
/// the left scrolled it.
///
/// The event is delivered to the track directly, as picking does once the pointer is over it: where the
/// track is drawn in a windowless test is not where it is in the editor (the shell sizes its split from
/// the window, which this app has not), so hit-testing it here would prove nothing about the real panel.
#[test]
fn a_wheel_over_the_inspectors_scrollbar_scrolls_it() {
    use bevy::picking::backend::HitData;
    use bevy::picking::events::{Pointer, PointerScroll};
    use bevy::ui_widgets::Scrollbar;
    use bevy_cauldron::chrome::ScrollTrack;

    let mut app = editor("wheel_over_the_scrollbar");
    let (area, before, range, _) = inspector(&mut app);
    assert!(range > 100.0, "the inspector must have somewhere to scroll (range {range})");
    assert_eq!(before, 0.0);

    let track = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(Entity, &Scrollbar, &Node), With<ScrollTrack>>();
        q.iter(world)
            .find(|(_, bar, node)| bar.target == area && node.display != Display::None)
            .map(|(e, ..)| e)
            .expect("the inspector's scrollbar is drawn, since its content overflows")
    };
    let handle = app
        .world()
        .get_resource::<bevy_cauldron::surface::Surface>()
        .map(|s| s.image.clone())
        .unwrap_or_default();
    let location = Location {
        target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }),
        position: Vec2::ZERO,
    };
    for _ in 0..3 {
        app.world_mut().trigger(PointerScroll {
            entity: track,
            pointer: Pointer::new(PointerId::Mouse, location.clone()),
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: -1.0,
            hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        app.update();
    }
    let (_, after, _, _) = inspector(&mut app);
    assert!(
        after > 1.0,
        "three wheel notches delivered to the scrollbar left the inspector at {after}: the track took \
         the event and nothing forwarded it to the area"
    );
}
