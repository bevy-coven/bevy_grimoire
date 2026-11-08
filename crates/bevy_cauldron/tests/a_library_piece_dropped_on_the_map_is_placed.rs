//! **A library piece carried onto the map is placed where it is let go** (UI-52, #238).
//!
//! The PLACE palette's rows are drag sources carrying the descriptor's id. gooey reports a drag that ends over nothing the
//! interface draws as `DroppedOnWorld`; the editor takes the pointer to the ground as a click does, snaps it by the same rung,
//! and places through the `placement_add` operation: one log line, one undo step.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::editor::{PaletteRow, PieceDrag};
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::view::MainCamera;
use bevy_gooey::{DragPayload, DroppedOnWorld};

use crate::fixtures;

/// Where on the screen a person would point to hit the ground at a map-space spot.
fn pixel_over(app: &mut App, at: (f32, f32)) -> Vec2 {
    let origin = app.world().resource::<OpenMap>().map.origin;
    let world = Vec3::new(origin.0 + at.0, 0.0, origin.2 + at.1);
    let mut cameras = app.world_mut().query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras.single(app.world()).expect("the editor has one main camera");
    camera.world_to_viewport(transform, world).unwrap_or_else(|e| panic!("the spot is not on screen: {e:?}"))
}

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // A window, so the camera has a surface to project onto; nothing moves its cursor.
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..8 {
        app.update();
    }
    app
}

fn drop_on(app: &mut App, id: &str, pointer: Vec2) {
    app.world_mut().trigger(DroppedOnWorld { source: Entity::PLACEHOLDER, payload: PieceDrag(id.to_owned()), pointer });
    for _ in 0..4 {
        app.update();
    }
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

#[test]
fn a_dropped_piece_is_placed_at_the_pointer_by_the_place_operation_and_one_undo_takes_it_back() {
    let mut app = editor("piece_drop");
    assert!(app.world().resource::<OpenMap>().map.placements.is_empty(), "the map starts empty");
    let over = pixel_over(&mut app, (2.0, 2.0));
    drop_on(&mut app, "site/wall", over);

    let map = app.world().resource::<OpenMap>().map.clone();
    assert_eq!(map.placements.len(), 1, "one piece was placed: {:?}", map.placements.iter().map(|p| &p.id).collect::<Vec<_>>());
    let placed = &map.placements[0];
    assert_eq!(placed.descriptor, "site/wall");
    assert!((placed.at.0 - 2.0).abs() <= 0.5 && (placed.at.1 - 2.0).abs() <= 0.5, "at the cell under the pointer, snapped: {:?}", placed.at);
    let entries = log(&app);
    assert_eq!(entries.iter().filter(|e| e.op == "placement_add").count(), 1, "the drop is one placement_add line, as a click is");

    bevy_cauldron::editor::undo_for_test(app.world_mut());
    for _ in 0..3 {
        app.update();
    }
    assert!(app.world().resource::<OpenMap>().map.placements.is_empty(), "one undo takes the dropped piece back");
}

#[test]
fn a_piece_the_library_does_not_have_places_nothing() {
    let mut app = editor("piece_drop_unknown");
    let over = pixel_over(&mut app, (2.0, 2.0));
    drop_on(&mut app, "site/ghost", over);
    assert!(app.world().resource::<OpenMap>().map.placements.is_empty(), "nothing was placed");
    assert!(log(&app).is_empty(), "and nothing was logged");
}

#[test]
fn every_palette_row_carries_its_descriptor_to_the_map() {
    let mut app = editor("piece_drop_rows");
    let world = app.world_mut();
    let mut q = world.query::<(&PaletteRow, &DragPayload<PieceDrag>)>();
    let carried: Vec<String> = q.iter(world).map(|(_, p)| p.0.0.clone()).collect();
    assert!(carried.contains(&"site/wall".to_owned()), "the library's one piece has a row that carries it: {carried:?}");
}
