//! **Folding a pack and taking the filter box are operations, so a key and an agent's call are one** (E-002, #129 slice 25).
//!
//! Three verbs leave the allowlist of unmigrated actions: `FoldPack` (`Space` on a pack heading or the `EXCLUDED` band, which
//! opens or closes the group) and `FocusFilter` and `FocusTagFilter` (`F`, which puts the keyboard in the box above the list
//! the tab shows, and `/`, which puts it in the tag box). They dispatch `pack_fold` and `filter_focus`; an agent calls the
//! same two through `grimoire/operation_apply`, and the editor's own `ImportState` and `Filters` hold the answer either way.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::filter::{Filters, Pane};
use bevy_cauldron::keys::{Action, MOD_KEYS, binding};
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{ImportState, Mode};
use serde_json::{Value, json};

use crate::fixtures;

fn editor(tag: &str, mode: Mode) -> App {
    let root = fixtures::Fixture::new(&format!("pack_fold_filter_{tag}_{}", std::process::id()))
        .descriptor("wall", "alpha")
        .pack("aaa", &["a1", "a2", "a3"])
        .pack("bbb", &["b1", "b2", "b3"])
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless_at(&root, "m", None, mode).unwrap_or_else(|e| panic!("{e}"));
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

fn state(app: &App) -> &ImportState {
    app.world().resource::<ImportState>()
}

fn focus(app: &App) -> Option<Pane> {
    app.world().resource::<Filters>().focus_pane()
}

/// **`Space` on a pack heading folds the pack through the operation**, in the log as the person's own line, and an agent's
/// calls open it, fold it, and fold it again without undoing it.
#[test]
fn space_folds_a_pack_through_the_operation_and_an_agent_folds_it_too() {
    let mut app = editor("fold", Mode::Meshes);
    {
        let mut s = app.world_mut().resource_mut::<ImportState>();
        s.folded_packs.remove("bbb");
        s.focused_pack = Some("bbb".to_owned());
    }
    app.update();
    tap(&mut app, &[], binding(Action::FoldPack).key);
    assert!(state(&app).folded_packs.contains("bbb"), "Space on the heading folded the pack");
    let lines = logged(&app, "pack_fold");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    assert_eq!(lines[0].1["params"], json!({"pack": "bbb"}), "{:?}", lines[0]);

    let said = apply(&mut app, "pack_fold", json!({"pack": "bbb"}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert!(!state(&app).folded_packs.contains("bbb"), "an agent's call with no `folded` toggles, as the key does");
    apply(&mut app, "pack_fold", json!({"pack": "bbb", "folded": true}));
    assert!(state(&app).folded_packs.contains("bbb"), "`folded: true` folds it");
    apply(&mut app, "pack_fold", json!({"pack": "bbb", "folded": true}));
    assert!(state(&app).folded_packs.contains("bbb"), "and asking again leaves it folded rather than opening it");
    assert!(!state(&app).folded_packs.contains("aaa"), "the other pack was never touched");
    let neither = apply(&mut app, "pack_fold", json!({}));
    assert_eq!(neither["outcome"], "refused", "a call that names no group is refused: {neither}");
    let both = apply(&mut app, "pack_fold", json!({"pack": "aaa", "excluded": true}));
    assert_eq!(both["outcome"], "refused", "a call that names two groups is refused: {both}");
}

/// **`Space` on the `EXCLUDED` band opens or closes it through the same operation.**
#[test]
fn space_opens_the_excluded_band_through_the_operation_and_an_agent_closes_it() {
    let mut app = editor("band", Mode::Meshes);
    // **The band exists only once a pack is left out**: `candidate_list_rows` draws `EXCLUDED` above the packs the
    // project's policy excludes, and a fixture project excludes none. `pack_set_excluded` is the shelf's own switch.
    let left_out = apply(&mut app, "pack_set_excluded", json!({"pack": "bbb", "excluded": true}));
    assert_eq!(left_out["outcome"], "applied", "the fixture's policy can leave a pack out: {left_out}");
    for _ in 0..4 {
        app.update();
    }
    {
        let mut s = app.world_mut().resource_mut::<ImportState>();
        s.excluded_open = false;
        s.focused_pack = None;
        s.excluded_focused = true;
    }
    app.update();
    tap(&mut app, &[], binding(Action::FoldPack).key);
    assert!(state(&app).excluded_open, "Space on the band opened it");
    let lines = logged(&app, "pack_fold");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].1["params"], json!({"excluded": true}), "{:?}", lines[0]);
    let said = apply(&mut app, "pack_fold", json!({"excluded": true, "folded": true}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert!(!state(&app).excluded_open, "an agent closes it: `folded: true` on the band is closed");
}

/// **`F` and `/` take the filter boxes through the operation**; an agent takes any box by name, and the box starts fresh.
#[test]
fn the_filter_keys_and_an_agent_take_the_keyboard_into_a_box() {
    let mut app = editor("filter", Mode::Meshes);
    assert_eq!(focus(&app), None, "no box has the keyboard to begin with");
    tap(&mut app, &[MOD_KEYS[0]], binding(Action::FocusFilter).key);
    assert_eq!(focus(&app), Some(Pane::Candidates), "F takes the box above the Meshes tab's list");
    let lines = logged(&app, "filter_focus");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    assert_eq!(lines[0].1["params"], json!({"pane": "candidates"}), "{:?}", lines[0]);
    app.world_mut().resource_mut::<Filters>().blur();
    app.update();
    tap(&mut app, &[], binding(Action::FocusTagFilter).key);
    assert_eq!(focus(&app), Some(Pane::Tags), "/ takes the tag box");
    let lines = logged(&app, "filter_focus");
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[1].1["params"], json!({"pane": "tags"}), "{:?}", lines[1]);

    app.world_mut().resource_mut::<Filters>().blur();
    let said = apply(&mut app, "filter_focus", json!({"pane": "palette"}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert_eq!(focus(&app), Some(Pane::Palette), "an agent takes the palette's box");
    let refused = apply(&mut app, "filter_focus", json!({"pane": "nowhere"}));
    assert_eq!(refused["outcome"], "refused", "a box that is not one is refused: {refused}");
    assert_eq!(focus(&app), Some(Pane::Palette), "and nothing moved");
}

/// **The three actions name the operations they dispatch**, which is what takes them off the allowlist.
#[test]
fn the_three_actions_name_the_operations_they_dispatch() {
    assert_eq!(Action::FoldPack.operation(), Some("pack_fold"));
    assert_eq!(Action::FocusFilter.operation(), Some("filter_focus"));
    assert_eq!(Action::FocusTagFilter.operation(), Some("filter_focus"));
}
