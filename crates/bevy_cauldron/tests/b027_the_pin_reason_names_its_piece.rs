//! **The pin reason stays on the piece it was typed for.**
//!
//! B-027. The reason field stored the placement's ROW, so a row removed below it while the person
//! typed shifted what Enter pinned: the piece that moved up into the row took the pin, and the one
//! the person named was left bare. The field now holds the placement's id.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::editor::{EditorState, Tool};
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::view::MainCamera;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .place("site/wall", (8.0, 0.0))
        .place("site/wall", (16.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.init_resource::<Script>();
    // A window with a cursor, so `sense_pointer` reads the injected position.
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// One frame of person: the keys held down and where the pointer is.
#[derive(Default)]
struct Frame {
    keys: Vec<KeyCode>,
    pointer: Option<Vec2>,
}

/// The next frame to play. `held` is what the previous frame pressed, released at the start of the
/// next so no key is left down by accident.
#[derive(Resource, Default)]
struct Script {
    next: Option<Frame>,
    held: Vec<KeyCode>,
}

/// Runs in `PreUpdate` after `InputSystems`, so the keys survive into `Update` and the window's cursor
/// is where the person put it when `sense_pointer` reads it.
fn drive(
    mut script: ResMut<Script>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
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
}

fn step(app: &mut App, frame: Frame) {
    app.world_mut().resource_mut::<Script>().next = Some(frame);
    app.update();
}

/// Where on the screen a person points to hit the ground at a map-space spot.
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

/// **Opens the reason field on the piece under the pointer with O**, as a person does, and checks the
/// note names the piece the pointer is over.
fn open_reason_on(app: &mut App, at: Vec2, id: &str) {
    step(
        app,
        Frame {
            pointer: Some(at),
            ..default()
        },
    );
    step(
        app,
        Frame {
            keys: vec![KeyCode::KeyO],
            pointer: Some(at),
        },
    );
    let note = app
        .world()
        .resource::<EditorState>()
        .status
        .note_text()
        .to_owned();
    assert!(
        note.contains(&format!("why is {id} pinned?")),
        "O over {id} must open its reason field; the note said: {note}"
    );
}

/// **Through the registered method**, as an agent's request goes.
fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn held(app: &App) -> bevy_grimoire_core::map::Map {
    app.world().resource::<OpenMap>().map.clone()
}

/// One key press and release through the real `KeyboardInput` path.
fn press(app: &mut App, key_code: KeyCode, logical_key: Key) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key: logical_key.clone(),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

/// **Enter pins the piece the reason field was opened on, even after a lower row is removed.**
///
/// Rows are `[first, pinned, next]`. The field opens on `pinned` (row 1), the person types `x`, and
/// an agent removes `first` (row 0) through the registry before Enter. Pinned by row, Enter would
/// land on `next`, which has moved up into row 1; pinned by id it lands on `pinned`.
#[test]
fn enter_pins_the_piece_the_reason_was_opened_on_after_a_lower_row_is_removed() {
    let mut app = editor("b027_reason_row");
    let ids: Vec<String> = held(&app).placements.iter().map(|p| p.id.clone()).collect();
    let (first, pinned, next) = (ids[0].clone(), ids[1].clone(), ids[2].clone());

    let over_pinned = pixel_over(&mut app, (8.0, 0.0));
    open_reason_on(&mut app, over_pinned, &pinned);
    press(&mut app, KeyCode::KeyX, Key::Character("x".into()));

    let removed = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "placement_remove", "parameters": {"id": first}, "dry_run": false}),
    );
    assert_eq!(removed["outcome"], "applied", "{removed}");
    app.update();

    press(&mut app, KeyCode::Enter, Key::Enter);

    let map = held(&app);
    let on_pinned = map
        .placements
        .iter()
        .find(|p| p.id == pinned)
        .expect("the piece the field was opened on still stands");
    let on_next = map
        .placements
        .iter()
        .find(|p| p.id == next)
        .expect("the piece that moved up is still on the map");
    assert!(
        on_pinned.owned,
        "Enter must pin the piece the field was opened on, not the one that moved into its row"
    );
    assert_eq!(on_pinned.owned_because.as_deref(), Some("x"));
    assert!(!on_next.owned, "the piece that moved up into the row must not be pinned");
}

/// **Removing the pinned piece before Enter says so, and closes the field.**
///
/// The field holds the placement's id, so Enter names a piece that no longer stands. The registry
/// refuses it, and the refusal is a problem (the banner the author looks at), not a dim note. The
/// field is closed after Enter, so X is the map's key again and arms removal.
#[test]
fn a_pin_whose_piece_was_removed_before_enter_is_refused_and_closes_the_field() {
    let mut app = editor("b027_removed_pin");
    let ids: Vec<String> = held(&app).placements.iter().map(|p| p.id.clone()).collect();
    let pinned = ids[1].clone();

    let at = pixel_over(&mut app, (8.0, 0.0));
    open_reason_on(&mut app, at, &pinned);
    press(&mut app, KeyCode::KeyX, Key::Character("x".into()));

    let removed = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "placement_remove",
            "parameters": {"id": pinned},
            "dry_run": false
        }),
    );
    assert_eq!(removed["outcome"], "applied", "{removed}");
    app.update();

    press(&mut app, KeyCode::Enter, Key::Enter);

    let problem = app
        .world()
        .resource::<EditorState>()
        .status
        .problem_text()
        .to_owned();
    assert!(
        problem.contains("no placement is called") && problem.contains(&pinned),
        "a pin on a removed piece must say so in the problem banner; it said: {problem}"
    );

    step(
        &mut app,
        Frame {
            keys: vec![KeyCode::KeyX],
            ..default()
        },
    );
    assert!(
        matches!(app.world().resource::<EditorState>().tool, Tool::Remove),
        "the reason field must be closed after Enter, so X reaches the map and arms removal"
    );
}
