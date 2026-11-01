//! **A piece placed with the mouse lands under the pointer, at the window's real scale.**
//!
//! The window reports its cursor in logical pixels, and the editor renders into a surface image
//! sized in physical ones. `view::sense_pointer` is the one place that converts between them: it
//! multiplies the logical cursor by the window's scale factor, and `cursor_ground` then reads the
//! ground point through the camera, whose viewport is the surface. So a press at the logical
//! position of a ground point must place a piece at that ground point, at scale 1 and at scale 2.
//!
//! The pointer is injected the way `a_person_removes_a_piece_and_undoes_it` injects it: the window's
//! cursor is set in `PreUpdate`, and the test reads `view::Pointer` to see what the editor saw.

use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::view::{MainCamera, Pointer};

use crate::fixtures;

/// One frame's worth of person: the pointer, and a mouse press or release.
#[derive(Default)]
struct Frame {
    pointer: Option<Vec2>,
    mouse: Option<bool>,
}

#[derive(Resource, Default)]
struct Script {
    next: Option<Frame>,
}

/// Runs in `PreUpdate` after `InputSystems`, so `sense_pointer` reads the position in `Update`.
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

/// Where the editor's surface puts a map-space spot: the pixel a person would point at.
fn surface_pixel_over(app: &mut App, at: (f32, f32)) -> Vec2 {
    let origin = app.world().resource::<OpenMap>().map.origin;
    let world = Vec3::new(origin.0 + at.0, 0.0, origin.2 + at.1);
    let mut cameras = app
        .world_mut()
        .query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras
        .single(app.world())
        .expect("the editor has one main camera");
    camera
        .world_to_viewport(transform, world)
        .unwrap_or_else(|e| panic!("the spot is not on screen: {e:?}"))
}

/// Put a piece down with a press and a release at the logical pixel that a window at `scale`
/// reports for the spot `target`, and return where the editor placed it.
fn place_at(scale: f32, target: (f32, f32)) -> ((f32, f32), Vec2, Vec2) {
    let root = fixtures::Fixture::new(&format!("lands_at_scale_{}", (scale * 10.0) as u32))
        .descriptor("site/wall", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    {
        let world = app.world_mut();
        let mut window = world
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(world)
            .expect("the editor has one primary window");
        // A real display at `scale` keeps its logical window and has `scale` times the pixels.
        // Changing only the scale would halve the logical window, which is smaller than the
        // editor's own docks, so the docks would cover the map and no press could land on it.
        let (w, h) = (window.resolution.width(), window.resolution.height());
        window
            .resolution
            .set_physical_resolution((w * scale) as u32, (h * scale) as u32);
        window.resolution.set_scale_factor_override(Some(scale));
    }
    for _ in 0..4 {
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

    // The surface pixel the spot is at, and the logical pixel a window at `scale` reports for it.
    let surface = surface_pixel_over(&mut app, target);
    let logical = surface / scale;

    step(
        &mut app,
        Frame {
            pointer: Some(logical),
            ..default()
        },
    );
    let seen = app
        .world()
        .resource::<Pointer>()
        .0
        .expect("the editor reads a pointer once the window has one");

    step(
        &mut app,
        Frame {
            pointer: Some(logical),
            mouse: Some(true),
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(logical),
            mouse: Some(false),
        },
    );

    let placed = app.world().resource::<OpenMap>().map.placements.clone();
    assert_eq!(
        placed.len(),
        1,
        "one press and release must place exactly one piece"
    );
    (placed[0].at, surface, seen)
}

/// The placed piece must be on the cell the spot is in, to within half a metre on each axis.
/// The editor snaps to its cell lattice, so a piece lands on a cell centre; a wrong pointer space
/// moves it by metres.
fn assert_lands(scale: f32) {
    let target = (3.5_f32, 1.5_f32);
    let (at, surface, seen) = place_at(scale, target);
    assert!(
        (seen - surface).length() < 0.5,
        "at scale {scale}: the editor read pointer {seen:?}, but the spot is at surface pixel \
         {surface:?} — sense_pointer must multiply the logical cursor by the scale factor"
    );
    assert!(
        (at.0 - target.0).abs() <= 0.5 && (at.1 - target.1).abs() <= 0.5,
        "at scale {scale}: a press on ({}, {}) placed the piece at ({}, {})",
        target.0,
        target.1,
        at.0,
        at.1,
    );
}

#[test]
fn a_piece_placed_at_scale_one_lands_under_the_pointer() {
    assert_lands(1.0);
}

#[test]
fn a_piece_placed_at_scale_two_lands_under_the_pointer() {
    assert_lands(2.0);
}
