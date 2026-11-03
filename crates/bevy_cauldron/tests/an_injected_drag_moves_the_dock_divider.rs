//! **An injected press, eight moves and a release resize the dock** (UI-30, #224's second acceptance
//! item — the clip that has been owed since pass 271).
//!
//! Four passes tried and failed at this. The chain, now measured end to end on the fixture project:
//!
//! 1. `retarget_pointer` puts the pointer on the surface image, which is **right**: the editor's
//!    interface camera renders there too, so `bevy_ui`'s backend (`render_target == pointer target`)
//!    accepts it. Pass 280 changed that to the window and was reverted.
//! 2. The divider then reaches the picker — `HoverMap` holds it, and after a press `PointerState`
//!    reads `pressing = 1`.
//! 3. **But `dragging` stayed 0**: `bevy_picking` makes a drag out of `PointerAction::Move` with a
//!    non-zero delta, and `Move` is emitted only by `mouse_pick_events` from `WindowEvent::CursorMoved`,
//!    which an injected cursor never produces. `inject_clicks` wrote press and release only.
//!
//! With `inject_clicks` emitting the move, the same sequence takes `SplitFraction` from **0.22 to
//! 0.349** — the dock grows by exactly the distance dragged over the room.

use bevy::picking::pointer::{PointerButton, PointerId};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

use bevy_cauldron::harness;

use crate::fixtures;

#[test]
fn an_injected_press_and_drag_moves_the_dock_divider() {
    let root = fixtures::Fixture::new("injected_drag_moves_the_dock")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((
        Window { resolution: WindowResolution::new(1280, 720), ..default() },
        PrimaryWindow,
    ));
    for _ in 0..8 {
        app.update();
    }

    let split = app
        .world_mut()
        .query_filtered::<Entity, With<bevy_gooey::Split>>()
        .iter(app.world())
        .next()
        .expect("the shell has one split");
    let divider = {
        let children = app.world().get::<Children>(split).expect("the split has children");
        children
            .iter()
            .find(|c| {
                matches!(
                    app.world().get::<bevy_gooey::Role>(*c),
                    Some(bevy_gooey::Role::SplitDivider)
                )
            })
            .expect("the split has a divider")
    };
    let at = app
        .world()
        .get::<bevy::ui::UiGlobalTransform>(divider)
        .copied()
        .expect("the divider is laid out")
        .translation;
    let before = app
        .world()
        .get::<bevy_gooey::SplitFraction>(split)
        .map(|f| f.0)
        .expect("the split keeps its fraction");

    // **The drag, as `tools/demo/record.py`'s `drag` step performs it**: cursor, press, eight moves,
    // release — through the injected cursor and the button messages an agent's input arrives as.
    let press = |app: &mut App, state| {
        app.world_mut()
            .write_message(bevy::input::mouse::MouseButtonInput {
                button: bevy::input::mouse::MouseButton::Left,
                state,
                window: Entity::PLACEHOLDER,
            });
    };
    let cursor = |app: &mut App, to: Vec2| {
        app.world_mut().resource_mut::<bevy_debugger_bevy::DebugCursor>().0 = Some(to);
    };

    cursor(&mut app, at);
    for _ in 0..3 {
        app.update();
    }
    press(&mut app, bevy::input::ButtonState::Pressed);
    app.update();
    app.update();
    for i in 1..=8 {
        cursor(&mut app, at + Vec2::new(i as f32 * 15.0, 0.0));
        app.update();
    }

    // **The middle of the chain, asserted where it can be seen**: a press that is *also* a drag. Without
    // the move this read `(1, 0)` and the fraction never changed.
    let state = app.world().resource::<bevy::picking::events::PointerState>();
    let buttons = state
        .get(PointerId::Mouse, PointerButton::Primary)
        .expect("the mouse has a primary button state after a press");
    assert_eq!(
        (buttons.pressing.len(), buttons.dragging.len()),
        (1, 1),
        "the divider is pressed and dragged; with `pressing = 1, dragging = 0` no drag can start, \
         which is what four passes measured before `inject_clicks` wrote `Move`"
    );

    press(&mut app, bevy::input::ButtonState::Released);
    app.update();
    app.update();

    let after = app
        .world()
        .get::<bevy_gooey::SplitFraction>(split)
        .map(|f| f.0)
        .expect("the split keeps its fraction");
    assert!(
        after > before + 0.05,
        "the drag left the split at {before} -> {after}: an injected drag must resize the dock, or \
         `tools/demo/record.py` can never record the clip #224 asks for"
    );
}
