//! **The labeller's three requests are operations, so `L`, `Shift+L`, `Shift+Y` and an agent's call are one** (E-002, #129 slice 34).
//!
//! `SuggestLabels`, `SuggestAll` and `DiscardAllSuggestions` leave the allowlist of unmigrated actions. `L` photographs the focused
//! piece and asks the model for its labels (`labels_suggest`), `Shift+L` walks what the filter scopes, or holds the walk that is
//! running, or lets a held one go on (`labels_walk`), and `Shift+Y` drops everything the labeller holds (`labels_abandon`). An agent
//! calls the same operations through `grimoire/operation_apply`; either way the editor's own booth, queue and status line are what
//! move, and one line is logged per request.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::confirm::Confirm;
use bevy_cauldron::keys::Action;
use bevy_cauldron::label_booth::ShotRig;
use bevy_cauldron::labels::LabelQueue;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{ImportState, Mode};
use serde_json::{Value, json};

use crate::fixtures;

/// An editor on the Meshes tab over a kit with one library piece that has a mesh and every label. The decision endpoint is a
/// socket that listens and says nothing, so a walk's reachability probe passes and no model is ever asked.
fn editor(tag: &str) -> App {
    editor_with(tag, true)
}

/// The same editor, with the endpoint listening (`true`) or a port nothing listens on (`false`), where a walk's probe fails.
fn editor_with(tag: &str, listening: bool) -> App {
    editor_over(tag, listening, &["floor"])
}

/// The same, over the library pieces named (each with a mesh and every label).
fn editor_over(tag: &str, listening: bool, pieces: &[&str]) -> App {
    let fixture = pieces
        .iter()
        .fold(fixtures::Fixture::new(&format!("labeller_{tag}_{}", std::process::id())), |f, id| f.descriptor(id, "alpha"));
    let root = fixture.build("m");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port to listen on");
    let port = listener.local_addr().expect("its address").port();
    if listening {
        std::thread::spawn(move || for _connection in listener.incoming() {});
    } else {
        // Closed again at once: the port is one nothing listens on, so a connect is refused.
        drop(listener);
    }
    std::fs::write(root.join(".env"), format!("GRIMOIRE_DECIDE_ENDPOINT=http://127.0.0.1:{port}\n")).expect(".env");
    let mut app = bevy_cauldron::harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
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

fn note(app: &App) -> String {
    app.world().resource::<ImportState>().status.note_text().to_owned()
}

fn running(app: &App) -> bool {
    app.world().resource::<LabelQueue>().running()
}

fn paused(app: &App) -> bool {
    app.world().resource::<LabelQueue>().paused()
}

/// **`L` and an agent photograph a piece through one operation**: the editor's booth is asked, the status line says so, and an
/// unknown piece or a call with no piece is refused.
#[test]
fn l_and_an_agent_ask_the_labeller_for_one_piece_through_one_operation() {
    let mut app = editor("suggest");
    let said = apply(&mut app, "piece_select", json!({"id": "floor"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    app.update();

    tap(&mut app, &[], KeyCode::KeyL);
    assert!(!app.world().resource::<ShotRig>().is_idle(), "L puts the piece in the booth's queue");
    assert_eq!(note(&app), "photographing `floor` for labels...", "the status line is the operation's own sentence");
    let lines = logged(&app, "labels_suggest");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);
    assert_eq!(lines[0].1["params"]["piece"], "floor", "{:?}", lines[0]);

    app.world_mut().resource_mut::<ImportState>().status.clear_note();
    let said = apply(&mut app, "labels_suggest", json!({"piece": "floor"}));
    assert_eq!(said["outcome"], "applied", "an agent's call is applied, not refused for want of a view: {said}");
    // An agent's call leaves the person's receipt (`agent: <operation>: <summary>`), which carries the operation's own sentence.
    assert!(note(&app).contains("photographing `floor` for labels..."), "an agent's request is the editor's own: {}", note(&app));
    let lines = logged(&app, "labels_suggest");
    assert_eq!(lines.len(), 2, "the agent's call is logged too");
    assert_eq!(lines[1].0, "agent", "{:?}", lines[1]);

    let refused = apply(&mut app, "labels_suggest", json!({"piece": "no_such_piece"}));
    assert_eq!(refused["outcome"], "refused", "a piece nobody has is refused: {refused}");
    let refused = apply(&mut app, "labels_suggest", json!({}));
    assert_eq!(refused["outcome"], "refused", "a request with no piece is refused: {refused}");
}

/// **`Shift+L` and an agent walk, hold and resume through one operation**, and `Shift+Y` and an agent drop everything the
/// labeller holds through another. A scope that holds pieces labelled already asks the person, unless the agent says which.
#[test]
fn shift_l_and_shift_y_and_an_agent_walk_and_abandon_the_labeller_through_operations() {
    let mut app = editor("walk");
    assert!(!running(&app));

    // The agent says which: `relabel` true takes the labelled piece, so there is no question to answer.
    let said = apply(&mut app, "labels_walk", json!({"relabel": true}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert!(running(&app), "the walk is armed on the editor's own queue");
    assert_eq!(app.world().resource::<LabelQueue>().progress().1, 1, "it took the one piece in scope");
    assert!(note(&app).contains("labeling 1 piece(s)"), "the status line carries the operation's own sentence: {}", note(&app));
    let lines = logged(&app, "labels_walk");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].0, "agent", "{:?}", lines[0]);

    // A second call holds the walk, and a third lets it go on; a walk that is running takes no `relabel`.
    let said = apply(&mut app, "labels_walk", json!({}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert!(paused(&app), "a call while the walk runs holds it");
    assert!(note(&app).contains("batch held at"), "{}", note(&app));
    let said = apply(&mut app, "labels_walk", json!({}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert!(!paused(&app), "and the next lets it go on");
    let refused = apply(&mut app, "labels_walk", json!({"relabel": true}));
    assert_eq!(refused["outcome"], "refused", "a running walk takes no `relabel`: {refused}");

    // `Shift+Y` drops everything, through the operation an agent calls.
    tap(&mut app, &[KeyCode::ShiftLeft], KeyCode::KeyY);
    assert!(!running(&app), "Shift+Y abandons the walk");
    assert!(note(&app).starts_with("cleared "), "the status line is the operation's own sentence: {}", note(&app));
    let lines = logged(&app, "labels_abandon");
    assert_eq!(lines.len(), 1, "one key, one logged operation: {lines:?}");
    assert_eq!(lines[0].0, "human", "{:?}", lines[0]);

    // And an agent's abandon drops a walk the same way.
    apply(&mut app, "labels_walk", json!({"relabel": true}));
    assert!(running(&app));
    let said = apply(&mut app, "labels_abandon", json!({}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert!(!running(&app), "an agent's abandon drops the editor's own queue");
    assert_eq!(logged(&app, "labels_abandon").len(), 2);
    let refused = apply(&mut app, "labels_abandon", json!({"all": true}));
    assert_eq!(refused["outcome"], "refused", "it takes no fields: {refused}");

    // `Shift+L` on a scope with a labelled piece asks the person, and a second request waits for the answer.
    tap(&mut app, &[KeyCode::ShiftLeft], KeyCode::KeyL);
    assert!(app.world().resource::<Confirm>().is_open(), "Shift+L asks whether to re-label what is labelled already");
    assert!(!running(&app), "and walks nothing until it is answered");
    let lines = logged(&app, "labels_walk");
    // The agent's start, hold, resume and second start, then the key's: the question the key raised is not a line of its own.
    assert_eq!(lines.len(), 5, "{lines:?}");
    assert_eq!(lines[4].0, "human", "{:?}", lines[4]);
    assert_eq!(lines[4].1["after"]["answer"], "ask", "the key's answer is the question: {:?}", lines[4]);
    let refused = apply(&mut app, "labels_walk", json!({}));
    assert_eq!(refused["outcome"], "refused", "a question is on screen: {refused}");
}

/// **The actions name the operations they dispatch**, which is what takes them off the allowlist.
#[test]
fn the_label_actions_name_the_operations_they_dispatch() {
    assert_eq!(Action::SuggestLabels.operation(), Some("labels_suggest"));
    assert_eq!(Action::SuggestAll.operation(), Some("labels_walk"));
    assert_eq!(Action::DiscardAllSuggestions.operation(), Some("labels_abandon"));
}

/// **A walk with nobody home is refused, for the key and for an agent** (E-002, #129 slice 34 review): `no batch: {remedy}`, the
/// sentence `labels_walk` carries, is the problem the author reads, nothing is queued and no line is logged as applied. The old key
/// said it in `labels.rs`; this pins it where the operation says it now, and the slot it is filed in.
#[test]
fn a_walk_with_no_endpoint_is_refused_as_no_batch_for_the_key_and_for_an_agent() {
    let mut app = editor_with("no_batch", false);

    tap(&mut app, &[KeyCode::ShiftLeft], KeyCode::KeyL);
    {
        let status = &app.world().resource::<ImportState>().status;
        assert!(status.has_problem(), "a refused walk is a problem where the author looks, not a note: {:?}", status.note_text());
        assert!(status.problem_text().starts_with("no batch:"), "the problem is the operation's own sentence: {}", status.problem_text());
    }
    assert!(!running(&app), "nothing was queued");
    assert!(!app.world().resource::<Confirm>().is_open(), "and no question was raised");
    let applied = |app: &App| logged(app, "labels_walk").into_iter().filter(|(_, l)| l["outcome"] == "applied").count();
    assert_eq!(applied(&app), 0, "a refused walk is not a logged change: {:?}", logged(&app, "labels_walk"));

    let refused = apply(&mut app, "labels_walk", json!({"relabel": true}));
    assert_eq!(refused["outcome"], "refused", "an agent's walk is refused for the same reason: {refused}");
    assert!(refused.to_string().contains("no batch:"), "{refused}");
    assert!(!running(&app));
    assert_eq!(applied(&app), 0, "{:?}", logged(&app, "labels_walk"));
}

/// **A walk does not ask its question over somebody else's** (E-002, #129 slice 34 review): `labels_walk` answering `Ask` raised the
/// overwrite question without looking whether another was up, and the second `Confirm::ask` replaced the first. It is refused now,
/// the question already on screen is still the one, and nothing is logged.
#[test]
fn a_walk_is_refused_while_another_question_is_on_screen() {
    let mut app = editor("other_question");
    app.world_mut().resource_mut::<Confirm>().ask(bevy_cauldron::confirm::Asked::QuitApp, "Quit?", "There is a map with unsaved edits.", "Quit", "Stay");
    assert!(app.world().resource::<Confirm>().is_open());

    // No `relabel`: the scope holds a labelled piece, which is the case that asks.
    let refused = apply(&mut app, "labels_walk", json!({}));
    assert_eq!(refused["outcome"], "refused", "a question is on screen: {refused}");
    assert!(!app.world().resource::<LabelQueue>().running());
    assert!(app.world().resource::<Confirm>().is_open(), "the question that was up is still up");
    assert!(logged(&app, "labels_walk").is_empty(), "{:?}", logged(&app, "labels_walk"));
}

/// **`labels_suggest` says where a namespaced library piece is** (E-002, #129 slice 34 review): it guessed library piece against
/// candidate with `!id.contains('/')`, so `kit/rug` was shown nowhere. A candidate's mesh path is a path; a piece is what the library holds.
#[test]
fn labels_suggest_shows_a_library_piece_whatever_its_id_looks_like() {
    let mut app = editor_over("shows", true, &["floor", "kit/rug"]);
    let said = apply(&mut app, "labels_suggest", json!({"piece": "kit/rug"}));
    assert_eq!(said["outcome"], "applied", "{said}");
    assert_eq!(said["shown"]["kind"], "piece", "a library piece with a slash in its id is still a library piece: {said}");
    assert_eq!(said["shown"]["id"], "kit/rug", "{said}");
}
