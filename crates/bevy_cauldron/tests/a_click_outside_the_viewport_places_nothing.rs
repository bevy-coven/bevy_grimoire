//! **A click outside the viewport is not a click on the world** (B-021, #182).
//!
//! The map camera draws into the frame's hole (`chrome::ViewportSlot`); the door strip above it and
//! the status band below it are chrome. A pointer there has no ground under it, so a press and a
//! release there with a piece armed must place nothing. `Camera::viewport_to_world` does not check
//! that its point is inside the viewport: it extrapolates a ray through an imaginary pixel past the
//! edge, and that ray put B-021's barrel at (-1965.65, -98.65), far off the map and off screen,
//! where the person never saw it to undo it.
//!
//! The pointer is injected the way `a_piece_lands_where_the_pointer_is` injects it: the window's
//! cursor is set in `PreUpdate`, after `InputSystems`.

use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::view::MainCamera;

use crate::fixtures;

#[derive(Default)]
struct Frame {
    pointer: Option<Vec2>,
    mouse: Option<bool>,
}

#[derive(Resource, Default)]
struct Script {
    next: Option<Frame>,
}

fn drive(
    mut script: ResMut<Script>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Some(frame) = script.next.take() else {
        return;
    };
    if let Ok(mut window) = windows.single_mut() {
        window.set_cursor_position(frame.pointer);
    }
    match frame.mouse {
        Some(true) => mouse.press(MouseButton::Left),
        Some(false) => mouse.release(MouseButton::Left),
        None => {}
    }
}

fn step(app: &mut App, frame: Frame) {
    app.world_mut().resource_mut::<Script>().next = Some(frame);
    app.update();
}

/// A press and a release at `at` (logical pixels, scale 1), and the placements the map holds after.
fn click(app: &mut App, at: Vec2) -> Vec<(String, (f32, f32))> {
    for mouse in [None, Some(true), Some(false)] {
        step(app, Frame { pointer: Some(at), mouse });
    }
    app.world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .map(|p| (p.id.clone(), p.at))
        .collect()
}

#[test]
fn a_click_above_or_below_the_viewport_places_nothing_and_one_inside_still_places() {
    let root = fixtures::Fixture::new(&format!("click_outside_viewport_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..12 {
        app.update();
    }
    let index = app
        .world()
        .resource::<Project>()
        .library
        .descriptors
        .iter()
        .position(|d| d.id == "site/wall")
        .expect("the fixture defines site/wall");
    app.world_mut().resource_mut::<EditorState>().brush = Some(index);
    app.update();

    let rect = {
        let world = app.world_mut();
        let camera = world
            .query_filtered::<&Camera, With<MainCamera>>()
            .single(world)
            .expect("the editor has one main camera");
        camera.logical_viewport_rect().expect("the main camera has a viewport")
    };
    let height = {
        let world = app.world_mut();
        world
            .query_filtered::<&Window, With<PrimaryWindow>>()
            .single(world)
            .expect("one primary window")
            .resolution
            .height()
    };
    // **The viewport is a region, or this test proves nothing**: a point past its edge must still
    // be inside the window, on the chrome around it.
    assert!(
        rect.min.y > 8.0 && rect.max.y < height - 8.0,
        "the viewport {rect:?} must leave chrome above and below it in a {height} px window"
    );

    let x = rect.center().x;
    for (what, at) in [
        ("above (the door strip)", Vec2::new(x, rect.min.y - 6.0)),
        ("below (the status band)", Vec2::new(x, rect.max.y + 6.0)),
    ] {
        let placed = click(&mut app, at);
        assert!(
            placed.is_empty(),
            "a click {what} the viewport {rect:?}, at {at:?}, placed {placed:?}"
        );
    }

    let placed = click(&mut app, rect.center());
    assert_eq!(
        placed.len(),
        1,
        "a click inside the viewport still places the armed piece: {placed:?}"
    );
}
