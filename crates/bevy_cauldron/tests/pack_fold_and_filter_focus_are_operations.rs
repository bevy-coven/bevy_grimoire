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
use bevy_cauldron::tiles::{ImportState, Mode, PackHeader};
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

/// **A call to `filter_focus` naming the box that already has the keyboard still starts its search fresh** (E-002, #129
/// review M1). The schema promises `Its search starts fresh`, and the guide prints the same sentence, so the clear has to
/// be the operation's answer rather than a comparison against which box has the keyboard — that comparison is why an
/// agent naming the focused box used to answer `applied` and clear nothing.
#[test]
fn a_call_names_the_box_already_focused_and_the_search_starts_fresh() {
    let mut app = editor("filter-again", Mode::Meshes);
    // **A box with the keyboard and half a search typed into it**, as a person leaves one.
    {
        let mut filters = app.world_mut().resource_mut::<Filters>();
        filters.take_focus(Pane::Candidates);
        filters.push_for_test(Pane::Candidates, 'w');
        filters.push_for_test(Pane::Candidates, 'a');
    }
    app.update();
    assert_eq!(focus(&app), Some(Pane::Candidates), "setup: the candidates box has the keyboard");
    assert_eq!(app.world().resource::<Filters>().text(Pane::Candidates), "wa", "setup: and two characters");

    let said = apply(&mut app, "filter_focus", json!({"pane": "candidates"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(focus(&app), Some(Pane::Candidates), "the box keeps the keyboard");
    assert_eq!(
        app.world().resource::<Filters>().text(Pane::Candidates),
        "",
        "and its search started fresh: a call naming the focused box is not a no-op"
    );
}

/// **A click on a pack heading folds it away through the operation** (E-002, #129 review): the heading has its own
/// `Activate`, and it dispatches `pack_fold` rather than writing `folded_packs` itself, so a click, `Space` and an
/// agent's call are one path with one log line each.
#[test]
fn a_click_on_a_pack_heading_folds_it_through_the_operation() {
    let mut app = editor("heading-click", Mode::Meshes);
    {
        let mut s = app.world_mut().resource_mut::<ImportState>();
        s.folded_packs.remove("bbb");
    }
    app.update();
    assert!(!state(&app).folded_packs.contains("bbb"), "setup: bbb is open");

    let heading = {
        let mut q = app.world_mut().query::<(Entity, &PackHeader)>();
        q.iter(app.world())
            .find(|(_, h)| h.pack() == "bbb")
            .map(|(e, _)| e)
            .unwrap_or_else(|| panic!("a heading is drawn for bbb"))
    };
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity: heading });
    for _ in 0..4 {
        app.update();
    }
    assert!(state(&app).folded_packs.contains("bbb"), "the click folded the pack");

    let lines = logged(&app, "pack_fold");
    assert_eq!(lines.len(), 1, "one click, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "a click is the person's own line: {:?}", lines[0]);
    assert_eq!(lines[0].1["params"], json!({"pack": "bbb"}), "{:?}", lines[0]);
}

/// **The three actions name the operations they dispatch**, which is what takes them off the allowlist.
#[test]
fn the_three_actions_name_the_operations_they_dispatch() {
    assert_eq!(Action::FoldPack.operation(), Some("pack_fold"));
    assert_eq!(Action::FocusFilter.operation(), Some("filter_focus"));
    assert_eq!(Action::FocusTagFilter.operation(), Some("filter_focus"));
}
