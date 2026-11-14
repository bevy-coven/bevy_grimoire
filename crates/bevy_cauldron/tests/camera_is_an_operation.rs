//! **The Map camera is driven by operations, so a key and an agent's call are one** (E-002, #129 slice 31).
//!
//! Six verbs leave the allowlist of unmigrated actions: `PanForward`, `PanBack`, `PanLeft` and `PanRight` (`W`, `S`, `A`, `D`,
//! held) and `TurnViewLeft` and `TurnViewRight` (`Q` and `E`). They dispatch `camera_pan` and `camera_turn`; an agent calls the
//! same two through `grimoire/operation_apply`, and the editor's own `Rig` holds the answer either way. A held pan is one act:
//! one log line for the press, and the repeats move the focus without writing more.

use std::f32::consts::{FRAC_PI_2, PI};
use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::keys::Action;
use bevy_cauldron::project::Project;
use bevy_cauldron::view::{Rig, pan_direction};
use serde_json::{Value, json};

use crate::fixtures;

/// One frame of the stepped clock, in seconds: 60 Hz.
const FRAME_SECS: f32 = 1.0 / 60.0;

/// The pan rate in metres per second per metre of what the view shows: 16 m/s at the 18 m map view it was tuned at.
const PAN_RATE_PER_METRE: f32 = 16.0 / 18.0;

/// An editor on the Map, a stepped clock, and a window for the keys to reach.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("camera_ops_{tag}_{}", std::process::id()))
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME_SECS)));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **Through the registered method**, as an agent's request goes.
fn apply(app: &mut App, operation: &str, parameters: Value) -> Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/operation_apply").cloned() else {
        panic!("grimoire/operation_apply is not registered");
    };
    app.world_mut()
        .run_system_with(id, Some(json!({"operation": operation, "parameters": parameters, "dry_run": false})))
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|e| panic!("{e:?}"))
}

fn send(app: &mut App, key: KeyCode, state: ButtonState) {
    app.world_mut().write_message(KeyboardInput {
        key_code: key,
        logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
        state,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
}

/// **A key as a person presses it**: pressed, then released.
fn tap(app: &mut App, key: KeyCode) {
    send(app, key, ButtonState::Pressed);
    app.update();
    send(app, key, ButtonState::Released);
    app.update();
    app.update();
}

fn logged(app: &App, operation: &str) -> Vec<(String, Value)> {
    let root = app.world().resource::<Project>().root.clone();
    let text = std::fs::read_to_string(bevy_cauldron::ops::oplog_path_at(&root)).unwrap_or_default();
    text.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["op"] == operation)
        .map(|v| (v["source"].as_str().unwrap_or_default().to_owned(), v))
        .collect()
}

fn goal_yaw(app: &App) -> f32 {
    app.world().resource::<Rig>().goal_yaw
}

fn focus(app: &App) -> Vec3 {
    app.world().resource::<Rig>().focus
}

/// **`Q` and `E` turn the view a quarter through the operation, and an agent names how many.**
#[test]
fn q_e_and_an_agent_turn_one_view_in_quarters() {
    let mut app = editor("turn");
    let start = goal_yaw(&app);
    tap(&mut app, KeyCode::KeyQ);
    assert!((goal_yaw(&app) - (start + FRAC_PI_2)).abs() < 1.0e-4, "Q turns the view left a quarter: {} from {start}", goal_yaw(&app));
    let lines = logged(&app, "camera_turn");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    assert_eq!(lines[0].1["params"]["by"], "left", "{:?}", lines[0]);

    tap(&mut app, KeyCode::KeyE);
    assert!((goal_yaw(&app) - start).abs() < 1.0e-4, "E turns it back: {}", goal_yaw(&app));

    let said = apply(&mut app, "camera_turn", json!({"by": "right", "quarters": 2}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert!((goal_yaw(&app) - (start - PI)).abs() < 1.0e-4, "two quarters to the right is half a turn: {}", goal_yaw(&app));
    let said = apply(&mut app, "camera_turn", json!({"by": "left"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert!((goal_yaw(&app) - (start - FRAC_PI_2)).abs() < 1.0e-4, "no `quarters` is one: {}", goal_yaw(&app));
    let logged_by_agent = logged(&app, "camera_turn").iter().filter(|(source, _)| source != "human").count();
    assert_eq!(logged_by_agent, 2, "an agent's two calls are logged as the agent's");

    for bad in [json!({"by": "up"}), json!({"by": "left", "quarters": 0}), json!({"by": "left", "quarters": 5}), json!({})] {
        let refused = apply(&mut app, "camera_turn", bad.clone());
        assert_eq!(refused["outcome"], "refused", "{bad} is not a turn: {refused}");
    }
    assert!((goal_yaw(&app) - (start - FRAC_PI_2)).abs() < 1.0e-4, "and nothing moved: {}", goal_yaw(&app));
}

/// **A pan moves the focus along the screen's own axes at the key's own rate**, `seconds` of a held key, and two
/// directions at once are the diagonal a held pair of keys is.
#[test]
fn an_agents_pan_moves_the_focus_by_the_keys_own_rate() {
    let mut app = editor("pan");
    let start = focus(&app);
    let (yaw, height) = {
        let rig = app.world().resource::<Rig>();
        (rig.yaw, rig.height)
    };
    let said = apply(&mut app, "camera_pan", json!({"directions": ["forward"], "seconds": 1.0}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied: {said}");
    let want = start + pan_direction(Vec2::new(0.0, -1.0), yaw) * PAN_RATE_PER_METRE * height;
    assert!((focus(&app) - want).length() < 1.0e-3, "one second forward is the key's rate over what is on screen: {} not {want}", focus(&app));

    let from = focus(&app);
    apply(&mut app, "camera_pan", json!({"directions": ["right", "back"], "seconds": 0.5}));
    let want = from + pan_direction(Vec2::new(1.0, 1.0), yaw) * PAN_RATE_PER_METRE * height * 0.5;
    assert!((focus(&app) - want).length() < 1.0e-3, "right and back together are the diagonal: {} not {want}", focus(&app));

    let from = focus(&app);
    let said = apply(&mut app, "camera_pan", json!({"directions": ["left"]}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert!((focus(&app) - from).length() > 0.0, "no `seconds` is a short tap, not nothing");

    let from = focus(&app);
    for bad in [
        json!({"directions": []}),
        json!({"directions": ["forward", "back"]}),
        json!({"directions": ["up"]}),
        json!({"directions": ["forward"], "seconds": 0.0}),
        json!({"directions": ["forward"], "seconds": 1000.0}),
        json!({"seconds": 1.0}),
    ] {
        let refused = apply(&mut app, "camera_pan", bad.clone());
        assert_eq!(refused["outcome"], "refused", "{bad} is not a pan: {refused}");
    }
    assert_eq!(focus(&app), from, "and nothing moved");
}

/// **Holding `W` pans for as long as it is held, and writes one log line**: the press is the change and the repeats are it
/// continuing.
#[test]
fn a_held_pan_key_pans_every_frame_and_logs_one_line() {
    let mut app = editor("held");
    let start = focus(&app);
    let yaw = app.world().resource::<Rig>().yaw;
    send(&mut app, KeyCode::KeyW, ButtonState::Pressed);
    for _ in 0..6 {
        app.update();
    }
    send(&mut app, KeyCode::KeyW, ButtonState::Released);
    app.update();
    app.update();
    let moved = focus(&app) - start;
    let forward = pan_direction(Vec2::new(0.0, -1.0), yaw);
    assert!(moved.dot(forward) > 0.0, "a held W pans away from the viewer: {moved} against {forward}");
    assert!(moved.length() > 0.5 * PAN_RATE_PER_METRE * 18.0 * FRAME_SECS * 4.0, "it moved for the frames it was held: {moved}");
    let lines = logged(&app, "camera_pan");
    assert_eq!(lines.len(), 1, "one hold, one logged line however many frames it lasted: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    assert_eq!(lines[0].1["params"]["directions"], json!(["forward"]), "{:?}", lines[0]);

    let from = focus(&app);
    send(&mut app, KeyCode::KeyW, ButtonState::Pressed);
    send(&mut app, KeyCode::KeyS, ButtonState::Pressed);
    for _ in 0..4 {
        app.update();
    }
    send(&mut app, KeyCode::KeyW, ButtonState::Released);
    send(&mut app, KeyCode::KeyS, ButtonState::Released);
    app.update();
    assert_eq!(focus(&app), from, "forward and back held together cancel, as they always did");
}

/// **The six actions name the operations they dispatch**, which is what takes them off the allowlist.
#[test]
fn the_six_actions_name_the_operations_they_dispatch() {
    for action in [Action::PanForward, Action::PanBack, Action::PanLeft, Action::PanRight] {
        assert_eq!(action.operation(), Some("camera_pan"), "{action:?}");
    }
    assert_eq!(Action::TurnViewLeft.operation(), Some("camera_turn"));
    assert_eq!(Action::TurnViewRight.operation(), Some("camera_turn"));
}

/// **A press that lands on a frame of no time is still the change, and still logs**: the move may be nothing, but the
/// press is the act, and the frames after it are no longer `just_pressed`.
#[test]
fn a_press_on_a_frame_of_no_time_still_logs_the_pan() {
    let mut app = editor("zero_frame");
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    app.update();
    send(&mut app, KeyCode::KeyD, ButtonState::Pressed);
    app.update();
    send(&mut app, KeyCode::KeyD, ButtonState::Released);
    app.update();
    app.update();
    let lines = logged(&app, "camera_pan");
    assert_eq!(lines.len(), 1, "the press logs one line even when its frame lasted no time: {lines:?}");
}
