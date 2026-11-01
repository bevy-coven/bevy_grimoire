//! **An agent can turn a library piece's mesh, and `N`, `O` and `P` on the Meshes tab are the same
//! operation (E-002, #129 slice 9).**
//!
//! The three keys, the three rotate chips and the labeler's righting turned a library piece with a write
//! of their own, named by no operation, so an agent could place a piece and never turn its mesh upright, no
//! log line named the turn, and the registry's two-front-end rule listed the keys as unmigrated. They are
//! now the registered operation `piece_turn`. The first three tests go through the registered BRP methods,
//! the way an agent's request goes; the last presses the real `N` and Cmd+Z on the Meshes tab, the way a
//! person does. All assert on the library the editor holds, the file on disk and the log.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{ImportState, Mode};

use crate::fixtures;

fn open(root: &std::path::Path) -> App {
    let mut app =
        harness::build_headless_at(root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/a", "alpha")
        .descriptor("site/b", "alpha")
        .descriptor("site/c", "alpha")
        .build("m");
    open(&root)
}

/// **Through the registered method**, as an agent's request goes.
fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
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

fn apply(app: &mut App, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "piece_turn", "parameters": parameters, "dry_run": false}),
    )
}

fn turn(app: &mut App, id: &str, axis: &str) -> serde_json::Value {
    apply(app, serde_json::json!({"id": id, "axis": axis}))
}

fn piece(app: &App, id: &str) -> bevy_grimoire_core::descriptor::Descriptor {
    app.world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == id)
        .cloned()
        .unwrap_or_else(|| panic!("`{id}` is in the fixture library"))
}

fn file(app: &App) -> String {
    std::fs::read_to_string(
        app.world()
            .resource::<Project>()
            .grimoire_dir
            .join(bevy_cauldron::project::LIBRARY_FILE),
    )
    .expect("the library file")
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

/// **One call turns the piece and re-measures it, the file and the editor's library follow, one log
/// line names the turn, and one undo puts the whole descriptor back as it was.** The fixture's mesh is
/// 1.0 x 0.5 x 1.0 m, so a turn about X lays it on its face: its height becomes 1.0 m, which is the
/// re-measure the issue's keys always did.
#[test]
fn an_agent_turns_a_piece_and_one_undo_puts_it_back() {
    let mut app = editor("piece-turn-agent");
    let before = piece(&app, "site/a");
    assert_eq!(before.align.rotate, None, "the fixture's pieces start unturned");
    let lines_before = log(&app).len();

    let a = turn(&mut app, "site/a", "x");
    assert_eq!(a["outcome"], "applied", "the turn was not applied: {a}");
    let after = piece(&app, "site/a");
    assert_eq!(after.align.rotate, Some((90, 0, 0)), "one quarter turn about X");
    assert_ne!(after.extent.height, before.extent.height, "the turn must re-measure the piece's height");
    assert_eq!(
        app.world().resource::<Project>().library.get("site/a").map(|d| d.align.rotate),
        Some(Some((90, 0, 0))),
        "the editor's layered library still holds the unturned piece"
    );
    assert!(file(&app).contains("rotate"), "the library file never recorded the turn: {}", file(&app));
    let lines = log(&app);
    assert_eq!(lines.len(), lines_before + 1, "a turn is one log line");
    assert_eq!(lines.last().map(|l| l.op.as_str()), Some("piece_turn"));
    assert!(
        a["summary"].as_str().is_some_and(|s| s.contains("site/a") && s.contains("90,0,0")),
        "the answer must name the piece and its rotation: {a}"
    );

    // **Two quarters in one act is one turn**, and a change to ANOTHER piece must not block this undo.
    let other = apply(&mut app, serde_json::json!({"id": "site/b", "axis": "y", "quarters": 2}));
    assert_eq!(other["outcome"], "applied", "{other}");
    assert_eq!(piece(&app, "site/b").align.rotate, Some((0, 180, 0)), "two quarters about Y");

    let id = a["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let back = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(back["outcome"], "applied", "another piece's turn blocked this undo: {back}");
    assert_eq!(piece(&app, "site/a"), before, "the descriptor must come back exactly as it was");
    assert_eq!(piece(&app, "site/b").align.rotate, Some((0, 180, 0)), "the undo took back more than the one piece");
}

/// **An unknown piece, a name that is not an axis, a count that is not a turn, a put-back from an
/// agent, a typo'd field and a piece with authored cells turned about X are refused, and nothing is
/// written.** The last takes `force` to turn, and clearing the cells is then said.
#[test]
fn a_bad_request_is_refused_and_writes_nothing() {
    let root = fixtures::Fixture::new("piece-turn-refused")
        .descriptor("site/a", "alpha")
        .authored_lattice("site/wall", "alpha", "wall", 2)
        .build("m");
    let mut app = open(&root);
    let file_before = file(&app);
    let lines_before = log(&app).len();
    let a = piece(&app, "site/a");

    for (what, parameters) in [
        ("an unknown piece", serde_json::json!({"id": "site/nope", "axis": "x"})),
        ("a word that is not an axis", serde_json::json!({"id": "site/a", "axis": "w"})),
        ("no axis", serde_json::json!({"id": "site/a"})),
        ("zero quarters", serde_json::json!({"id": "site/a", "axis": "x", "quarters": 0})),
        ("four quarters", serde_json::json!({"id": "site/a", "axis": "x", "quarters": 4})),
        ("an agent's put-back", serde_json::json!({"id": "site/a", "restore": a})),
        ("a put-back with an axis", serde_json::json!({"id": "site/a", "axis": "x", "restore": a})),
        ("a typo'd field", serde_json::json!({"id": "site/a", "axes": "x"})),
        ("authored cells turned about x", serde_json::json!({"id": "site/wall", "axis": "x"})),
    ] {
        let r = apply(&mut app, parameters);
        assert_eq!(r["outcome"], "refused", "{what} was accepted: {r}");
    }
    let cells = apply(&mut app, serde_json::json!({"id": "site/wall", "axis": "x"}));
    assert!(
        cells["refusal"].as_str().is_some_and(|r| r.contains("2 authored cell")),
        "the refusal must name the count: {cells}"
    );
    assert_eq!(file(&app), file_before, "a refusal wrote the library file");
    assert_eq!(log(&app).len(), lines_before, "a refusal logged a line");

    // **Forced, the turn clears them and says so**, and Y carries them without force.
    let forced = apply(&mut app, serde_json::json!({"id": "site/wall", "axis": "x", "force": true}));
    assert_eq!(forced["outcome"], "applied", "{forced}");
    assert!(forced["summary"].as_str().is_some_and(|s| s.contains("2 authored cell")), "{forced}");
    assert!(piece(&app, "site/wall").subgrid.is_none(), "force clears the cells a turn about X cannot carry");
}

/// **A preview says where the piece would turn and writes nothing.**
#[test]
fn a_preview_writes_nothing() {
    let mut app = editor("piece-turn-preview");
    let file_before = file(&app);
    let lines_before = log(&app).len();
    let p = call(
        &mut app,
        "grimoire/operation_preview",
        serde_json::json!({"operation": "piece_turn", "parameters": {"id": "site/a", "axis": "z"}}),
    );
    assert!(p.to_string().contains("rotate"), "the preview must say the rotation it would write: {p}");
    let refused = call(
        &mut app,
        "grimoire/operation_preview",
        serde_json::json!({"operation": "piece_turn", "parameters": {"id": "site/nope", "axis": "z"}}),
    );
    assert!(refused.to_string().contains("site/nope"), "a preview must refuse what the commit would: {refused}");
    assert_eq!(file(&app), file_before, "a preview wrote the library file");
    assert_eq!(log(&app).len(), lines_before, "a preview logged a line");
    assert_eq!(piece(&app, "site/a").align.rotate, None, "a preview turned the piece");
}

fn tap(app: &mut App, logical: Key, code: KeyCode) {
    for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
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

fn undo_chord(app: &mut App, redo: bool) {
    let held = bevy_cauldron::keys::MOD_KEYS[0];
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(held);
    if redo {
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ShiftLeft);
    }
    tap(app, Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified), KeyCode::KeyZ);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(held);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::ShiftLeft);
    for _ in 0..3 {
        app.update();
    }
}

/// **A person's `N` on a library piece is one `piece_turn` line stamped as a person's, it turns that
/// piece and no other, Cmd+Z takes it back and Cmd+Shift+Z turns it again.** Before slice 9 the key
/// wrote `library.ron` itself and logged nothing.
#[test]
fn a_persons_n_turns_through_the_operation_and_cmd_z_and_redo_follow_it() {
    let mut app = editor("piece-turn-person");
    app.world_mut().resource_mut::<ImportState>().selected_library_id = Some("site/b".to_owned());
    for _ in 0..3 {
        app.update();
    }
    let lines_before = log(&app).len();

    tap(&mut app, Key::Character("n".into()), KeyCode::KeyN);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(piece(&app, "site/b").align.rotate, Some((90, 0, 0)), "N did not turn the piece about X");
    assert_eq!(piece(&app, "site/a").align.rotate, None, "N turned a piece that was not selected");
    assert!(file(&app).contains("rotate"), "N did not reach the library file: {}", file(&app));
    let lines = log(&app);
    assert_eq!(lines.len(), lines_before + 1, "one press of N is one log line");
    assert_eq!(lines.last().map(|l| l.op.as_str()), Some("piece_turn"));
    assert_eq!(
        lines.last().map(|l| format!("{:?}", l.source)),
        Some("Human".to_owned()),
        "a person's turn must be stamped as a person's"
    );

    undo_chord(&mut app, false);
    assert_eq!(piece(&app, "site/b").align.rotate, None, "one Cmd+Z must take the turn back");
    assert_eq!(log(&app).len(), lines_before + 2, "the undo is a line of its own");

    undo_chord(&mut app, true);
    assert_eq!(piece(&app, "site/b").align.rotate, Some((90, 0, 0)), "redo must turn it again");
}
