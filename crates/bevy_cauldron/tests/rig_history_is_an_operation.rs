//! **The rig bench's history is an operation, so `Cmd+Z` and an agent's step are one** (E-002, #129 slice 35).
//!
//! `UndoBench` and `RedoBench` leave the allowlist of unmigrated actions. `Cmd+Z` and `Cmd+Shift+Z` on the Anim tab dispatch
//! `rig_history_step`; an agent calls the same operation through `grimoire/operation_apply`. Either way the manifest the bench held
//! before its newest write is written back to `rigs.yaml` (or the one a step back took away is put again), the bench holds it, and
//! a step with nothing to step is a note on the key and a refusal for an agent.

use std::time::{Duration, Instant};

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::anim_tab::BenchState;
use bevy_cauldron::keys::Action;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::Mode;
use bevy_grimoire_core::rigs::Rigs;
use serde_json::{Value, json};

use crate::fixtures;

fn workspace_asset(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets").join(rel)
}

/// An editor on the Anim tab, over the shipped rig manifest, and the path of its `rigs.yaml`.
fn editor(tag: &str) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("rig_history_{tag}_{}", std::process::id()))
        .descriptor("floor", "alpha")
        .build("m");
    let path = root.join("assets/grimoire/rigs.yaml");
    std::fs::copy(workspace_asset("grimoire/rigs.yaml"), &path).expect("rigs.yaml");
    let mut app = bevy_cauldron::harness::build_headless_at(&root, "m", None, Mode::Anim).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..8 {
        app.update();
    }
    (app, path)
}

/// Frames until `done` holds, or panic with `what` after `secs`.
fn until(app: &mut App, secs: f64, what: &str, done: impl Fn(&App) -> bool) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < secs {
        if done(app) {
            return;
        }
        app.update();
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("never saw: {what}");
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

fn on_disk(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).expect("rigs.yaml")
}

fn held(app: &App) -> String {
    app.world().resource::<BenchState>().text.clone().expect("the bench holds a manifest")
}

/// Let the watcher see a write of the tab's own and decide it is nothing new, so no step is recorded behind the test's back.
fn settle(app: &mut App) {
    for _ in 0..40 {
        app.update();
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// **`Cmd+Z` and an agent step the bench's history through one operation**: the manifest an edit replaced is written back, the bench
/// holds it, and one logged line is left per step.
#[test]
fn cmd_z_and_an_agent_step_the_benchs_history_through_one_operation() {
    let (mut app, path) = editor("steps");
    let original = on_disk(&path);
    assert_eq!(held(&app), original, "the bench read the manifest");

    // A rig is added on disk, by a hand or an agent: the tab takes the edit and records the manifest it replaces.
    let mut rigs = Rigs::parse(&original).expect("the shipped manifest reads");
    let copy = rigs.rigs.values().next().expect("a rig").clone();
    rigs.rigs.insert("aardvark".to_owned(), copy);
    let edited = rigs.to_yaml().expect("serialises");
    std::fs::write(&path, &edited).expect("write");
    until(&mut app, 30.0, "the tab holding the edit", |a| held(a) == edited);
    settle(&mut app);

    let mods = [bevy_cauldron::keys::MOD_KEYS[0]];
    tap(&mut app, &mods, KeyCode::KeyZ);
    assert_eq!(on_disk(&path), original, "Cmd+Z writes the manifest the edit replaced");
    assert_eq!(held(&app), original, "and the bench holds it");
    let lines = logged(&app, "rig_history_step");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    assert_eq!(lines[0].1["params"]["by"], "back", "{:?}", lines[0]);
    assert_eq!(app.world().resource::<BenchState>().status.note_text(), "undid the last bench write");
    settle(&mut app);

    let said = apply(&mut app, "rig_history_step", json!({"by": "forward"}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    assert_eq!(on_disk(&path), edited, "an agent puts the edit back through the same history");
    assert_eq!(held(&app), edited, "and the bench holds it");
    let lines = logged(&app, "rig_history_step");
    assert_eq!(lines.len(), 2, "the agent's call is logged too");
    assert_eq!(lines[1].0, "agent", "{:?}", lines[1]);
    settle(&mut app);

    // The key puts it back the other way, from what the agent's step left on the stacks.
    tap(&mut app, &mods, KeyCode::KeyZ);
    assert_eq!(on_disk(&path), original, "Cmd+Z takes back what an agent put back");
    settle(&mut app);
    tap(&mut app, &[mods[0], KeyCode::ShiftLeft], KeyCode::KeyZ);
    assert_eq!(on_disk(&path), edited, "Cmd+Shift+Z puts it again");
    assert_eq!(logged(&app, "rig_history_step").len(), 4, "{:?}", logged(&app, "rig_history_step"));
    settle(&mut app);

    // Nothing to put back is a refusal for an agent, and a direction that is not one is too.
    let refused = apply(&mut app, "rig_history_step", json!({"by": "forward"}));
    assert_eq!(refused["outcome"], "refused", "there is nothing left to put back: {refused}");
    let refused = apply(&mut app, "rig_history_step", json!({"by": "sideways"}));
    assert_eq!(refused["outcome"], "refused", "a direction that is not one is refused: {refused}");
    assert_eq!(on_disk(&path), edited, "a refused step changes nothing");

    // Back to the manifest the session opened with, and then there is nothing to undo: a note on the key, no problem.
    let said = apply(&mut app, "rig_history_step", json!({"by": "back"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(on_disk(&path), original);
    settle(&mut app);
    tap(&mut app, &mods, KeyCode::KeyZ);
    let bench = app.world().resource::<BenchState>();
    assert!(!bench.status.has_problem(), "nothing to undo is guidance, not a problem: {:?}", bench.status.problem_text());
    assert_eq!(bench.status.note_text(), "nothing to undo on this tab");
    let refused = apply(&mut app, "rig_history_step", json!({"by": "back"}));
    assert_eq!(refused["outcome"], "refused", "nothing to undo: {refused}");
}

/// **The actions name the operation they dispatch**, which is what takes them off the allowlist.
#[test]
fn the_two_bench_history_actions_name_the_operation_they_dispatch() {
    assert_eq!(Action::UndoBench.operation(), Some("rig_history_step"));
    assert_eq!(Action::RedoBench.operation(), Some("rig_history_step"));
}

/// **A step whose log line cannot be written leaves the manifest, the bench and the history as they were** (E-002, #129 slice 35
/// review). The step writes `rigs.yaml` first and the log second; with no inverse the take-back had nothing to run, so the file kept
/// the step while the bench kept the old history, and the watcher then read the file as somebody else's edit: the next Cmd+Z undid
/// the failed step. A directory where the log goes makes the append fail for the key and for an agent's call alike.
#[test]
fn a_step_whose_log_line_cannot_be_written_is_taken_back_with_its_history() {
    let (mut app, path) = editor("badlog");
    let original = on_disk(&path);
    let mut rigs = Rigs::parse(&original).expect("the shipped manifest reads");
    let copy = rigs.rigs.values().next().expect("a rig").clone();
    rigs.rigs.insert("aardvark".to_owned(), copy);
    let edited = rigs.to_yaml().expect("serialises");
    std::fs::write(&path, &edited).expect("write");
    until(&mut app, 30.0, "the tab holding the edit", |a| held(a) == edited);
    settle(&mut app);
    let history = app.world().resource::<BenchState>().history();
    assert_eq!(history.undo.len(), 1, "the edit left one manifest to step back to: {history:?}");
    assert!(history.redo.is_empty(), "{history:?}");

    // **The log cannot be written**: a directory is where its file goes.
    let log = bevy_cauldron::ops::oplog_path_at(&app.world().resource::<Project>().root);
    let _ = std::fs::remove_file(&log);
    std::fs::create_dir_all(&log).expect("a directory where the log goes");

    let mods = [bevy_cauldron::keys::MOD_KEYS[0]];
    tap(&mut app, &mods, KeyCode::KeyZ);
    assert!(app.world().resource::<BenchState>().status.has_problem(), "the key says it could not step");
    let refused = apply(&mut app, "rig_history_step", json!({"by": "back"}));
    assert_eq!(refused["outcome"], "refused", "an agent's call is refused too: {refused}");
    settle(&mut app);
    assert_eq!(on_disk(&path), edited, "a step that could not be logged is taken back: rigs.yaml is as it was");
    assert_eq!(held(&app), edited, "and the bench still holds it");
    assert_eq!(app.world().resource::<BenchState>().history(), history, "and the history is exactly what it was, with no entry for the failed step");

    // With the log writable again the one real step is still there to take.
    std::fs::remove_dir(&log).expect("the directory goes");
    tap(&mut app, &mods, KeyCode::KeyZ);
    assert_eq!(on_disk(&path), original, "Cmd+Z takes back the edit, not a step that never happened");
    assert_eq!(logged(&app, "rig_history_step").len(), 1, "only the step that landed is logged");
}

/// **A step keeps the rig the author is on selected by name** (E-002, #129 slice 35 review), as an edit under the tab does: a rig the
/// step adds or removes ahead of it moves its index, and the selection moves with the rig. One the step removed leaves the selection
/// at the same place in the list.
#[test]
fn a_step_keeps_the_selected_rig_by_name_and_clamps_when_it_is_gone() {
    let (mut app, path) = editor("selection");
    let original = on_disk(&path);
    let names = |app: &App| app.world().resource::<BenchState>().names().iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();
    let on = |app: &App| {
        let bench = app.world().resource::<BenchState>();
        bench.names().get(bench.selected).map(|n| (*n).to_owned())
    };
    let before = names(&app);
    assert!(before.len() >= 3, "the shipped manifest has rigs enough to tell an index from a name: {before:?}");
    app.world_mut().resource_mut::<BenchState>().selected = 1;
    let chosen = before[1].clone();

    // A rig that sorts first is added on disk: the tab takes the edit, the selection follows its rig by name.
    let mut rigs = Rigs::parse(&original).expect("the shipped manifest reads");
    let copy = rigs.rigs.values().next().expect("a rig").clone();
    rigs.rigs.insert("aaa_first".to_owned(), copy);
    let edited = rigs.to_yaml().expect("serialises");
    std::fs::write(&path, &edited).expect("write");
    until(&mut app, 30.0, "the tab holding the edit", |a| held(a) == edited);
    settle(&mut app);
    assert_eq!(on(&app), Some(chosen.clone()), "the edit under the tab keeps the rig selected by name");

    // Cmd+Z removes the added rig, which moves the chosen one back up the list: the selection goes with it.
    let mods = [bevy_cauldron::keys::MOD_KEYS[0]];
    tap(&mut app, &mods, KeyCode::KeyZ);
    assert_eq!(names(&app), before, "the step took the added rig out");
    assert_eq!(on(&app), Some(chosen.clone()), "the step keeps the rig selected by name, not by index");
    settle(&mut app);
    let said = apply(&mut app, "rig_history_step", json!({"by": "forward"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(on(&app), Some(chosen), "and so does an agent's step the other way");

    // The selected rig is the one the step removes: the selection stays at the same place, or on the last rig.
    app.world_mut().resource_mut::<BenchState>().selected = 0;
    assert_eq!(on(&app).as_deref(), Some("aaa_first"));
    settle(&mut app);
    let said = apply(&mut app, "rig_history_step", json!({"by": "back"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(on(&app), Some(before[0].clone()), "the rig is gone, so the selection stays at the top of the list");
}
