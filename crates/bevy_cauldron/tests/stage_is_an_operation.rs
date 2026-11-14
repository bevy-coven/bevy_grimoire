//! **The rig bench's stage is driven by operations, so a key and an agent's call are one** (E-002, #129 slice 26).
//!
//! Four verbs leave the allowlist of unmigrated actions: `PlayPause` (`Space`), `ScrubBack` and `ScrubFwd` (left and
//! right, held), and `CycleCamPreset` (`V`). They dispatch `stage_play`, `stage_scrub` and `stage_camera`; an agent
//! calls the same three through `grimoire/operation_apply`, and the editor's own `BenchScrub` and `BenchCamera` hold
//! the answer either way. A held scrub is one act: one log line for the press, and the repeats move the phase without
//! writing more.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::anim_stage::{BenchCamera, BenchScrub, CamPreset};
use bevy_cauldron::keys::Action;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::Mode;
use serde_json::{Value, json};

use crate::fixtures;

fn workspace_asset(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets").join(rel)
}

/// An editor on the Anim tab, over the shipped rig manifest.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("stage_ops_{tag}_{}", std::process::id()))
        .descriptor("floor", "alpha")
        .build("m");
    std::fs::copy(workspace_asset("grimoire/rigs.yaml"), root.join("assets/grimoire/rigs.yaml")).expect("rigs.yaml");
    let mut app = bevy_cauldron::harness::build_headless_at(&root, "m", None, Mode::Anim).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.world_mut().spawn((Window::default(), PrimaryWindow));
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

/// **A key as a person presses it**, with `held` down around it.
fn tap(app: &mut App, held: &[KeyCode], key: KeyCode) {
    for h in held {
        send(app, *h, ButtonState::Pressed);
    }
    send(app, key, ButtonState::Pressed);
    app.update();
    send(app, key, ButtonState::Released);
    for h in held {
        send(app, *h, ButtonState::Released);
    }
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

fn playing(app: &App) -> bool {
    app.world().resource::<BenchScrub>().playing
}

fn phase(app: &App) -> f32 {
    app.world().resource::<BenchScrub>().phase
}

fn preset(app: &App) -> CamPreset {
    app.world().resource::<BenchCamera>().0
}

/// **Space plays and pauses through the operation**, and an agent names the state it wants.
#[test]
fn space_and_an_agent_play_and_pause_one_stage() {
    let mut app = editor("play");
    assert!(playing(&app), "the bench opens alive: the figure walks in place");
    tap(&mut app, &[], KeyCode::Space);
    assert!(!playing(&app), "Space pauses into a scrub");
    let lines = logged(&app, "stage_play");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);

    let said = apply(&mut app, "stage_play", json!({"playing": true}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert!(playing(&app), "an agent plays the editor's own stage");
    apply(&mut app, "stage_play", json!({"playing": true}));
    assert!(playing(&app), "naming the state it is already in leaves it there");
    apply(&mut app, "stage_play", json!({}));
    assert!(!playing(&app), "no `playing` toggles, as Space does");
    let refused = apply(&mut app, "stage_play", json!({"playing": "sometimes"}));
    assert_eq!(refused["outcome"], "refused", "a state that is not a boolean is refused: {refused}");
}

/// **A scrub moves the phase by the rate the key has always moved it, Shift is the fine stride, and a playing stage
/// refuses**: the figure is the clock while it plays.
#[test]
fn an_agents_scrub_moves_the_phase_by_the_keys_own_rate_and_a_playing_stage_refuses() {
    let mut app = editor("scrub");
    let refused = apply(&mut app, "stage_scrub", json!({"by": "fwd", "seconds": 1.0}));
    assert_eq!(refused["outcome"], "refused", "a playing figure has its own phase, so there is nothing to scrub: {refused}");
    apply(&mut app, "stage_play", json!({"playing": false}));
    let start = phase(&app);
    let said = apply(&mut app, "stage_scrub", json!({"by": "fwd", "seconds": 1.0}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert!((phase(&app) - (start + 0.25)).abs() < 1.0e-4, "one second forward is a quarter of the cycle: {}", phase(&app));
    apply(&mut app, "stage_scrub", json!({"by": "back", "seconds": 1.0, "fine": true}));
    assert!((phase(&app) - (start + 0.25 - 0.05)).abs() < 1.0e-4, "fine is a fifth of the sweep: {}", phase(&app));
    apply(&mut app, "stage_scrub", json!({"by": "back", "seconds": 4.0}));
    assert!((0.0..1.0).contains(&phase(&app)), "the phase wraps round the cycle: {}", phase(&app));
    let refused = apply(&mut app, "stage_scrub", json!({"by": "sideways"}));
    assert_eq!(refused["outcome"], "refused", "a direction that is not one is refused: {refused}");
    let refused = apply(&mut app, "stage_scrub", json!({"by": "fwd", "seconds": 0.0}));
    assert_eq!(refused["outcome"], "refused", "a scrub of no time is refused: {refused}");
}

/// **Holding the arrow scrubs for as long as it is held, and writes one log line**: the press is the change and the
/// repeats are it continuing.
#[test]
fn a_held_arrow_scrubs_every_frame_and_logs_one_line() {
    let mut app = editor("held");
    tap(&mut app, &[], KeyCode::Space);
    assert!(!playing(&app));
    let start = phase(&app);
    send(&mut app, KeyCode::ArrowRight, ButtonState::Pressed);
    for _ in 0..6 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        app.update();
    }
    send(&mut app, KeyCode::ArrowRight, ButtonState::Released);
    app.update();
    app.update();
    assert!(phase(&app) > start, "a held right arrow scrubs forward: {} from {start}", phase(&app));
    let lines = logged(&app, "stage_scrub");
    assert_eq!(lines.len(), 1, "one hold, one logged line however many frames it lasted: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    assert_eq!(lines[0].1["params"]["by"], "fwd", "{:?}", lines[0]);

    let before = phase(&app);
    send(&mut app, KeyCode::ArrowLeft, ButtonState::Pressed);
    for _ in 0..4 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        app.update();
    }
    send(&mut app, KeyCode::ArrowLeft, ButtonState::Released);
    app.update();
    assert!(phase(&app) < before, "a held left arrow scrubs back: {} from {before}", phase(&app));
}

/// **`V` cycles the camera preset through the operation, and an agent names one.**
#[test]
fn v_and_an_agent_choose_one_camera_preset() {
    let mut app = editor("camera");
    assert_eq!(preset(&app), CamPreset::Figure);
    tap(&mut app, &[], KeyCode::KeyV);
    assert_eq!(preset(&app), CamPreset::Feet, "V steps to the next preset");
    let lines = logged(&app, "stage_camera");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);

    let said = apply(&mut app, "stage_camera", json!({}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(preset(&app), CamPreset::Side, "no preset named steps to the next, as V does");
    apply(&mut app, "stage_camera", json!({"preset": "ground"}));
    assert_eq!(preset(&app), CamPreset::Ground, "an agent names the preset it wants");
    apply(&mut app, "stage_camera", json!({"preset": "figure"}));
    assert_eq!(preset(&app), CamPreset::Figure);
    let refused = apply(&mut app, "stage_camera", json!({"preset": "overhead"}));
    assert_eq!(refused["outcome"], "refused", "a preset that is not one is refused: {refused}");
    assert_eq!(preset(&app), CamPreset::Figure, "and nothing moved");
}

/// **The four actions name the operations they dispatch**, which is what takes them off the allowlist.
#[test]
fn the_four_actions_name_the_operations_they_dispatch() {
    assert_eq!(Action::PlayPause.operation(), Some("stage_play"));
    assert_eq!(Action::ScrubBack.operation(), Some("stage_scrub"));
    assert_eq!(Action::ScrubFwd.operation(), Some("stage_scrub"));
    assert_eq!(Action::CycleCamPreset.operation(), Some("stage_camera"));
}

/// **A press that lands on a frame of no time is still the change, and still logs**: the move may be nothing, but the
/// press is the act, and the frames after it are no longer `just_pressed`.
#[test]
fn a_press_on_a_frame_of_no_time_still_logs_the_scrub() {
    let mut app = editor("zero_frame");
    tap(&mut app, &[], KeyCode::Space);
    assert!(!playing(&app));
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::ZERO));
    app.update();
    send(&mut app, KeyCode::ArrowRight, ButtonState::Pressed);
    app.update();
    send(&mut app, KeyCode::ArrowRight, ButtonState::Released);
    app.update();
    app.update();
    let lines = logged(&app, "stage_scrub");
    assert_eq!(lines.len(), 1, "the press logs one line even when its frame lasted no time: {lines:?}");
}
