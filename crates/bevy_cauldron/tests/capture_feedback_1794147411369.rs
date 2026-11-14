//! **A wheel turned over the terminal, the menu bar or the status band is not the map's** (`feedback-1794147411369`).
//!
//! > When moused over this area or any area other than the map, the scroll should not cause the map to zoom. E.g., I just
//! > tried to scroll up in the omp session, and it ALSO zoomed on the map.
//!
//! Oracle `other:key-routing`: a relation between where the pointer is and what a wheel notch moves. The map camera draws into
//! the frame's hole (`chrome::ViewportSlot`); a notch with the pointer anywhere else must leave the rig's zoom where it was,
//! and one inside the hole, clear of any panel, must still zoom it. `view::drive` asked only `OverUi`, which knows the nodes
//! that carry `Hovered`: the terminal pane, the menu bar and the status band are not among them, so each of them zoomed the map
//! (measured on the installed 0.5.0: the viewport height went 12.57 m, 7.28 m, 4.21 m, 2.44 m, 1.41 m over a wheel turned on
//! the terminal body, its bar, the status band and the menu bar, and held over the inspector).
//!
//! The pointer is injected the way `a_click_outside_the_viewport_places_nothing` injects it (the window's cursor, set in
//! `PreUpdate` after `InputSystems`); the wheel is a `MouseWheel` message, which `InputPlugin` accumulates into the
//! `AccumulatedMouseScroll` that `view::drive` reads.

use bevy::input::InputSystems;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::touch::TouchPhase;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::harness;
use bevy_cauldron::terminal_pane::TerminalPaneRoot;
use bevy_cauldron::view::{MainCamera, OverUi, Rig};
use bevy::ui::{ComputedNode, UiGlobalTransform};

use crate::fixtures;

/// The rig's zoom before every notch, well inside `MIN_ZOOM..MAX_ZOOM`, so a notch either way has room to move it.
const START_HEIGHT_M: f32 = 18.0;

#[derive(Resource, Default)]
struct Script {
    pointer: Option<Vec2>,
}

fn aim(script: Res<Script>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = windows.single_mut() {
        window.set_cursor_position(script.pointer);
    }
}

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{name}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<bevy_cauldron::terminal::Terminal>().set_collapsed(false); // #424: the pane starts collapsed; this test needs it raised
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, aim.after(InputSystems));
    for _ in 0..12 {
        app.update();
    }
    app
}

fn viewport(app: &mut App) -> Rect {
    let world = app.world_mut();
    world
        .query_filtered::<&Camera, With<MainCamera>>()
        .single(world)
        .expect("the editor has one main camera")
        .logical_viewport_rect()
        .expect("the main camera has a viewport")
}

fn height(app: &App) -> f32 {
    app.world().resource::<Rig>().height
}

/// Put the pointer at `at` (logical pixels, scale 1), turn the wheel one notch up, and answer the rig's zoom before and after,
/// with the zoom reset first so every probe starts from the same place.
fn notch(app: &mut App, at: Vec2) -> (f32, f32) {
    app.world_mut().resource_mut::<Rig>().height = START_HEIGHT_M;
    app.world_mut().resource_mut::<Script>().pointer = Some(at);
    for _ in 0..2 {
        app.update();
    }
    let before = height(app);
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("one primary window");
    app.world_mut().write_message(MouseWheel { unit: MouseScrollUnit::Line, x: 0.0, y: 1.0, window, phase: TouchPhase::Moved });
    for _ in 0..2 {
        app.update();
    }
    (before, height(app))
}

#[test]
fn a_wheel_off_the_map_leaves_its_zoom_and_one_on_it_zooms() {
    let mut app = editor("wheel_off_the_map");
    let map = viewport(&mut app);
    let (width, height_px) = {
        let world = app.world_mut();
        let window = world.query_filtered::<&Window, With<PrimaryWindow>>().single(world).expect("one primary window");
        (window.resolution.width(), window.resolution.height())
    };
    assert!(
        map.min.y > 8.0 && map.max.y < height_px - 8.0 && map.min.x > 8.0 && map.max.x < width - 8.0,
        "the viewport {map:?} must leave chrome on every side of it in a {width} x {height_px} window, or this proves nothing"
    );

    // Every point of a grid over the window that is not in the viewport is chrome, and a wheel over it is not the map's.
    let mut probed = 0;
    for y in [6.0, 20.0, 45.0, 90.0, 200.0, 330.0, 430.0, 460.0, 520.0, 600.0, 660.0, height_px - 12.0] {
        for x in [20.0, 160.0, 400.0, 640.0, 900.0, 1100.0, width - 20.0] {
            let at = Vec2::new(x, y);
            if map.contains(at) {
                continue;
            }
            probed += 1;
            let (before, after) = notch(&mut app, at);
            assert_eq!(after, before, "a wheel notch at {at:?}, outside the viewport {map:?}, zoomed the map from {before} m to {after} m");
        }
    }
    assert!(probed > 40, "the grid reached only {probed} chrome points");

    // The terminal pane is named, not left to the grid: it is what the capture was taken over.
    let panes: Vec<Rect> = {
        let world = app.world_mut();
        world
            .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<TerminalPaneRoot>>()
            .iter(world)
            .map(|(node, at)| Rect::from_center_size(at.translation * node.inverse_scale_factor(), node.size() * node.inverse_scale_factor()))
            .filter(|rect| rect.size().x > 1.0 && rect.size().y > 1.0)
            .collect()
    };
    assert!(!panes.is_empty(), "the editor has a laid-out terminal pane, which is what the capture was taken over");
    for pane in &panes {
        let at = pane.center();
        let (before, after) = notch(&mut app, at);
        assert_eq!(after, before, "a wheel notch over the terminal pane {pane:?} (at {at:?}) zoomed the map from {before} m to {after} m");
    }

    // And the wheel is still the map's inside the viewport, over a point that no panel covers.
    let mut zoomed = None;
    'scan: for iy in 1..8 {
        for ix in 1..8 {
            let at = map.min + (map.max - map.min) * Vec2::new(ix as f32 / 8.0, iy as f32 / 8.0);
            app.world_mut().resource_mut::<Script>().pointer = Some(at);
            for _ in 0..2 {
                app.update();
            }
            if app.world().resource::<OverUi>().0 {
                continue;
            }
            zoomed = Some((at, notch(&mut app, at)));
            break 'scan;
        }
    }
    let (at, (before, after)) = zoomed.expect("some point of the viewport is clear of every panel");
    assert!(after < before, "a wheel notch up at {at:?}, inside the viewport {map:?} and over no panel, left the zoom at {after} m (from {before} m)");
}
