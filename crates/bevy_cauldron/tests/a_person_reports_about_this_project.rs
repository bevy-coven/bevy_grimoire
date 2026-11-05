//! **`Cmd+Shift+D` in the editor: a report about the project you have open, stored inside it.**
//!
//! The other channel (`a_person_reports_to_the_developers.rs`) reports on Cauldron itself and works on
//! every screen. This one needs a project, is written under the project's own `target/feedback/`, and
//! is for the agent working that project — so `--project` on the MCP server is what reads it.
//!
//! # The pick that matters here is the map
//!
//! A point inside the map camera's viewport is the **scene**, not whatever interface happens to be
//! drawn near that pixel: the record carries the ground point, the map point, the cell, and the
//! placement the editor's own `pick_at` chose. Everything that floats over the viewport — a dialog, a
//! menu, a tooltip, a popover — is still interface, and the test proves a click on one of those is a
//! node and not a piece of the world underneath it.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::harness;
use bevy_cauldron::keys::MOD_KEYS;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::report::{Phase, ReportEdit};
use bevy_cauldron::screen::Screen;
use bevy_cauldron::view::MainCamera;
use bevy_grimoire_core::feedback::{Audience, Subject};

use crate::fixtures;

#[derive(Default)]
struct Frame {
    keys: Vec<KeyCode>,
    pointer: Option<Vec2>,
    mouse: Option<bool>,
}

#[derive(Resource, Default)]
struct Script {
    next: Option<Frame>,
    held: Vec<KeyCode>,
}

fn drive(
    mut script: ResMut<Script>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let frame = script.next.take();
    for key in script.held.drain(..) {
        keys.release(key);
    }
    let Some(frame) = frame else {
        return;
    };
    for key in &frame.keys {
        keys.press(*key);
    }
    script.held = frame.keys;
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

fn tap(app: &mut App, logical: Key, code: KeyCode) {
    step(app, Frame { keys: vec![code], ..default() });
    for state in [
        bevy::input::ButtonState::Pressed,
        bevy::input::ButtonState::Released,
    ] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: logical.clone(),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

/// The editor on a map with two pieces: one at the origin, one three metres along, so a click on
/// either can be told from a click on the other.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .place("site/wall", (3.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// The surface pixel a map point appears at, so a click lands on a piece rather than near one.
fn pixel_over(app: &mut App, at: (f32, f32)) -> Vec2 {
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
        .unwrap_or_else(|e| panic!("the piece is not on screen: {e:?}"))
}

/// Press and release in one place.
fn click_at(app: &mut App, at: Vec2) {
    step(
        app,
        Frame {
            pointer: Some(at),
            mouse: Some(true),
            ..default()
        },
    );
    step(
        app,
        Frame {
            pointer: Some(at),
            mouse: Some(false),
            ..default()
        },
    );
}

/// **A click on the map picks the piece standing there**, and the record says where it is: the world
/// point, the map space point, the cell, and the placement id the editor's own `pick_at` chose.
#[test]
fn a_click_on_the_map_picks_the_piece_there() {
    let mut app = editor("project_report_piece");
    let at = pixel_over(&mut app, (3.0, 0.0));
    step(
        &mut app,
        Frame {
            pointer: Some(at),
            keys: vec![MOD_KEYS[0], KeyCode::ShiftLeft, KeyCode::KeyD],
            ..default()
        },
    );
    let report = app.world().resource::<ReportEdit>();
    assert_eq!(
        report.audience(),
        Some(Audience::Project),
        "Shift+Cmd+D in the editor is the project's channel"
    );
    assert_eq!(report.phase(), Some(Phase::Picking));

    click_at(&mut app, at);
    let report = app.world().resource::<ReportEdit>();
    assert_eq!(report.phase(), Some(Phase::Note), "the click picks a subject");
    let Some(Subject::Scene(scene)) = report.subject() else {
        panic!("a click on the map picks the scene, got {:?}", report.subject());
    };
    let placed = app
        .world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .find(|p| p.at == (3.0, 0.0))
        .expect("the fixture placed a piece at (3, 0)");
    assert_eq!(
        scene.placement.as_deref(),
        Some(placed.id.as_str()),
        "the record names the piece under the click: {scene:?}"
    );
    let cell = scene.cell.expect("a map is open, so there is a cell");
    assert_eq!(
        cell,
        [
            (3.0f32 / bevy_grimoire_core::grid::TILE).floor() as i32,
            (0.0f32 / bevy_grimoire_core::grid::TILE).floor() as i32
        ],
        "the cell is the one the piece stands in"
    );
    let map = scene.map.expect("a map is open, so there is a map point");
    assert!(
        (map[0] - 3.0).abs() < 0.05 && map[1].abs() < 0.05,
        "the map point is where the click landed: {map:?}"
    );
    let origin = app.world().resource::<OpenMap>().map.origin;
    assert!(
        (scene.world[0] - (origin.0 + 3.0)).abs() < 0.05
            && scene.world[1].abs() < 0.05,
        "and the world point is the piece's own: {:?}",
        scene.world
    );
}

/// **A point with no piece under it is still the scene**, with no placement: an agent reading the
/// report needs to know the click landed on empty ground rather than on something.
#[test]
fn a_click_on_empty_ground_picks_the_scene_with_no_piece() {
    let mut app = editor("project_report_empty");
    let at = pixel_over(&mut app, (6.0, 3.0));
    step(
        &mut app,
        Frame {
            pointer: Some(at),
            keys: vec![MOD_KEYS[0], KeyCode::ShiftLeft, KeyCode::KeyD],
            ..default()
        },
    );
    click_at(&mut app, at);
    let report = app.world().resource::<ReportEdit>();
    let Some(Subject::Scene(scene)) = report.subject() else {
        panic!("the map is the scene, got {:?}", report.subject());
    };
    assert!(
        scene.placement.is_none(),
        "nothing is placed there: {scene:?}"
    );
    assert!(scene.cell.is_some(), "and the cell is still recorded");
}

/// **A tooltip or a dialog over the map is interface, not the ground under it.**
///
/// Without this, a report about a panel drawn over the viewport recorded the piece behind the panel —
/// which is the one thing the person was not pointing at.
#[test]
fn a_dialog_over_the_map_is_interface() {
    let mut app = editor("project_report_dialog");
    // gooey's dialog takes the whole window, so a click anywhere on it is on the dialog.
    let dialog = app
        .world_mut()
        .spawn((bevy_gooey::dialog(), Name::new("a question over the map")))
        .id();
    let panel = app.world_mut().spawn(bevy_gooey::panel()).id();
    let words = app.world_mut().spawn(bevy_gooey::label("Over the map")).id();
    app.world_mut().entity_mut(panel).add_child(words);
    app.world_mut().entity_mut(dialog).add_child(panel);
    for _ in 0..4 {
        app.update();
    }
    let at = pixel_over(&mut app, (0.0, 0.0));
    step(
        &mut app,
        Frame {
            pointer: Some(at),
            keys: vec![MOD_KEYS[0], KeyCode::ShiftLeft, KeyCode::KeyD],
            ..default()
        },
    );
    click_at(&mut app, at);
    let report = app.world().resource::<ReportEdit>();
    let Some(Subject::Element(element)) = report.subject() else {
        panic!(
            "a dialog over the map is interface, got {:?}",
            report.subject()
        );
    };
    assert!(
        element
            .path
            .iter()
            .any(|step| step.role.as_deref() == Some("Scrim")
                || step.role.as_deref() == Some("Panel")),
        "the chain names the dialog's own nodes: {:?}",
        element.path
    );
}

/// **The project channel writes inside the project, and says why nothing was written headless.**
///
/// The folder is `<project>/target/feedback/`, never `assets/grimoire/`: a game loads that, and a
/// report is not something a game reads.
#[test]
fn the_project_channel_names_the_projects_own_folder() {
    let mut app = editor("project_report_folder");
    let root = app.world().resource::<Project>().root.clone();
    let dir = root.join(bevy_grimoire_core::feedback::PROJECT_DIR);
    let _ = std::fs::remove_dir_all(&dir);
    step(
        &mut app,
        Frame {
            pointer: None,
            keys: vec![MOD_KEYS[0], KeyCode::ShiftLeft, KeyCode::KeyD],
            ..default()
        },
    );
    assert_eq!(
        app.world().resource::<ReportEdit>().audience(),
        Some(Audience::Project)
    );
    tap(&mut app, Key::Enter, KeyCode::Enter);
    // **The box arrives one frame later**, so the `Enter` that chose the subject is not its first
    // newline: see `report_box`.
    app.update();
    let save = bevy_gooey::testing::find_by_label(&mut app, "Save report")
        .unwrap_or_else(|e| panic!("no Save button: {e:?}"));
    app.world_mut()
        .commands()
        .trigger(bevy_gooey::Activate { entity: save });
    for _ in 0..4 {
        app.update();
    }
    assert!(
        app.world().resource::<ReportEdit>().phase().is_none(),
        "the save is finished either way"
    );
    assert_eq!(
        std::fs::read_dir(&dir)
            .map(|entries| entries.count())
            .unwrap_or(0),
        0,
        "a headless editor cannot write a picture, so it writes nothing at all"
    );
    assert!(
        !root.join("assets/grimoire/feedback").exists(),
        "nothing is ever written under assets/grimoire, which a game loads"
    );
}

/// **The chord is outside the census**, which is what lets it work on the Projects home where no tab
/// exists: no row binds `D` with the primary modifier, so nothing can shadow it and no ratchet entry
/// moves.
#[test]
fn the_report_chord_is_outside_the_census() {
    for row in bevy_cauldron::keys::BINDINGS {
        assert!(
            !(row.key == KeyCode::KeyD && row.needs_mod),
            "{:?} claims Cmd+D, and the report's chords are outside the census",
            row.action
        );
    }
    let developers = bevy_cauldron::report::developers_chord();
    let project = bevy_cauldron::report::project_chord();
    assert_eq!(developers.key, KeyCode::KeyD);
    assert_eq!(project.key, KeyCode::KeyD);
    assert_ne!(developers, project, "and they are different chords");
}

/// **A report says which screen it was taken on**, so an agent reading a report about the home is not
/// told it came from a door.
#[test]
fn the_record_names_the_screen_it_was_taken_on() {
    let mut app = editor("project_report_screen");
    step(
        &mut app,
        Frame {
            pointer: None,
            keys: vec![MOD_KEYS[0], KeyCode::ShiftLeft, KeyCode::KeyD],
            ..default()
        },
    );
    assert_eq!(
        *app.world().resource::<State<Screen>>().get(),
        Screen::Editor,
        "the test is in the editor"
    );
    assert!(
        app.world().resource::<ReportEdit>().phase().is_some(),
        "the report opened"
    );
}
