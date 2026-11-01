//! **An agent can send a piece back out of the library, and `Delete` on the Meshes shelf is the same
//! operation (E-002, #129 slice 8).**
//!
//! `Delete` took a piece out of `library.ron` with a write of its own, named by no operation, so an agent
//! could import a piece and never un-import one, no log line named the change, and the registry's
//! two-front-end rule listed the key as unmigrated. It is now the registered operation `piece_remove`.
//! The first three tests go through the registered BRP methods, the way an agent's request goes; the
//! fourth presses the real `Delete` and Cmd+Z on the Meshes tab, the way a person does. All assert on the
//! library the editor holds, the file on disk and the log.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{ImportState, Mode};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .pack("aaa", &["spare_one", "spare_two"])
        .descriptor("site/a", "alpha")
        .descriptor("site/b", "alpha")
        .descriptor("site/c", "alpha")
        .descriptor("site/d", "alpha")
        .composition("site/room", &[("m", "site/d", (0.0, 0.0))])
        .build("m");
    let mut app =
        harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
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
        serde_json::json!({"operation": "piece_remove", "parameters": parameters, "dry_run": false}),
    )
}

fn remove(app: &mut App, id: &str) -> serde_json::Value {
    apply(app, serde_json::json!({"id": id}))
}

/// **The library's ids in the order the file holds them.**
fn ids(app: &App) -> Vec<String> {
    app.world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .map(|d| d.id.clone())
        .collect()
}

/// **The library file as it is on disk**, so a change the editor holds but never wrote shows.
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

fn mesh_of(app: &App, id: &str) -> String {
    app.world()
        .resource::<Project>()
        .library
        .get(id)
        .and_then(|d| d.mesh.clone())
        .unwrap_or_else(|| panic!("`{id}` is in the fixture library with a mesh"))
}

/// **One call takes the piece out, the file and the editor's library follow, one log line names it, and
/// one undo puts the whole descriptor back where it stood.** A piece in the middle of the file is the
/// case that shows the place: a put-back at the end would reorder `library.ron`.
#[test]
fn an_agent_removes_a_piece_and_one_undo_puts_it_back_where_it_stood() {
    let mut app = editor("piece-remove-agent");
    let before = app
        .world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == "site/b")
        .cloned()
        .expect("the fixture has site/b");
    let order = ids(&app);
    let at = order.iter().position(|i| i == "site/b").expect("site/b is listed");
    assert!(at > 0 && at + 1 < order.len(), "the fixture must put `site/b` between two pieces: {order:?}");
    let lines_before = log(&app).len();
    let mesh = mesh_of(&app, "site/b");
    app.world_mut().resource_mut::<ImportState>().selected_library_id = Some("site/b".to_owned());

    let a = remove(&mut app, "site/b");
    assert_eq!(a["outcome"], "applied", "the removal was not applied: {a}");
    assert!(
        app.world().resource::<Project>().library.get("site/b").is_none(),
        "the editor's library still holds the piece"
    );
    assert!(!ids(&app).contains(&"site/b".to_owned()), "the measured layer still holds the piece");
    assert!(!file(&app).contains("\"site/b\""), "the library file still holds the piece: {}", file(&app));
    assert!(file(&app).contains("\"site/a\""), "the removal took more than the one piece");
    let lines = log(&app);
    assert_eq!(lines.len(), lines_before + 1, "a removal is one log line");
    assert_eq!(lines.last().map(|l| l.op.as_str()), Some("piece_remove"));
    assert!(
        a["summary"].as_str().is_some_and(|s| s.contains("site/b") && s.contains(&mesh)),
        "the answer must name the piece and the mesh that comes back: {a}"
    );
    // **The shelf follows an agent's removal as it follows a key's**: the mesh is a candidate again and
    // the cursor is not left on a row that is gone.
    assert!(candidate_at(&app, &mesh).is_some(), "the removed piece's mesh is not offered as a candidate");
    assert_eq!(
        app.world().resource::<ImportState>().selected_library_id,
        None,
        "the cursor was left on the removed piece"
    );

    // A change to ANOTHER piece must not block this undo.
    let other = remove(&mut app, "site/c");
    assert_eq!(other["outcome"], "applied", "{other}");

    let id = a["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let back = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(back["outcome"], "applied", "another piece's removal blocked this undo: {back}");
    let now = ids(&app);
    assert_eq!(
        now.iter().position(|i| i == "site/b"),
        Some(at.min(now.len())),
        "the piece must be back at the place it held, not the end of the file: before {order:?}, now {now:?}"
    );
    assert!(!now.contains(&"site/c".to_owned()), "the undo put back more than the one piece");
    assert_eq!(
        app.world()
            .resource::<Project>()
            .measured
            .descriptors
            .iter()
            .find(|d| d.id == "site/b"),
        Some(&before),
        "the descriptor must come back exactly as it was, judgements and all"
    );
    assert!(file(&app).contains("\"site/b\""), "undo must put the file back: {}", file(&app));
    assert_eq!(
        candidate_at(&app, &mesh),
        None,
        "an agent's undo left the restored piece's mesh listed as a candidate"
    );
}

/// **A piece a map places, a piece a composition holds, a name the library lacks, a put-back from an
/// agent and a malformed request are refused, and nothing is written.**
#[test]
fn a_placed_piece_a_held_piece_an_unknown_one_and_a_forged_put_back_are_refused() {
    let root = fixtures::Fixture::new("piece-remove-refused")
        .descriptor("site/a", "alpha")
        .descriptor("site/d", "alpha")
        .composition("site/room", &[("m", "site/d", (0.0, 0.0))])
        .place("site/a", (0.0, 0.0))
        .build("m");
    let mut app =
        harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    let file_before = file(&app);
    let ids_before = ids(&app);
    let lines_before = log(&app).len();

    let placed = remove(&mut app, "site/a");
    assert_eq!(placed["outcome"], "refused", "a placed piece was removed: {placed}");
    assert!(
        placed["refusal"].as_str().is_some_and(|r| r.contains("placed by")),
        "the refusal must say what places it: {placed}"
    );
    let held = remove(&mut app, "site/d");
    assert_eq!(held["outcome"], "refused", "a piece a composition holds was removed: {held}");
    assert!(
        held["refusal"].as_str().is_some_and(|r| r.contains("site/room")),
        "the refusal must name the composition: {held}"
    );
    let unknown = remove(&mut app, "site/nope");
    assert_eq!(unknown["outcome"], "refused", "{unknown}");
    assert!(
        unknown["refusal"].as_str().is_some_and(|r| r.contains("site/nope")),
        "the refusal must name the piece it could not find: {unknown}"
    );

    let d = app
        .world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == "site/a")
        .cloned()
        .expect("site/a");
    for (what, parameters) in [
        ("an agent's put-back", serde_json::json!({"restore": d, "at": 0})),
        ("no id", serde_json::json!({})),
        ("an id with a put-back", serde_json::json!({"id": "site/a", "restore": d})),
        ("an `at` with an id", serde_json::json!({"id": "site/a", "at": 0})),
        ("a typo'd field", serde_json::json!({"ids": "site/a"})),
    ] {
        let r = apply(&mut app, parameters);
        assert_eq!(r["outcome"], "refused", "{what} was accepted: {r}");
    }
    assert_eq!(file(&app), file_before, "a refusal wrote the library file");
    assert_eq!(ids(&app), ids_before, "a refusal changed the library the editor holds");
    assert_eq!(log(&app).len(), lines_before, "a refusal logged a line");
}

/// **A preview says what the removal would do and refuses what it would refuse, and writes nothing.**
#[test]
fn a_preview_names_the_mesh_that_comes_back_and_writes_nothing() {
    let mut app = editor("piece-remove-preview");
    let file_before = file(&app);
    let lines_before = log(&app).len();
    let mesh = mesh_of(&app, "site/b");

    let p = call(
        &mut app,
        "grimoire/operation_preview",
        serde_json::json!({"operation": "piece_remove", "parameters": {"id": "site/b"}}),
    );
    assert!(
        p.to_string().contains(&mesh),
        "the preview must name the mesh that becomes a candidate again (`{mesh}`): {p}"
    );
    let refused = call(
        &mut app,
        "grimoire/operation_preview",
        serde_json::json!({"operation": "piece_remove", "parameters": {"id": "site/d"}}),
    );
    assert!(
        refused.to_string().contains("site/room"),
        "a preview must refuse what the commit would refuse: {refused}"
    );
    assert_eq!(file(&app), file_before, "a preview wrote the library file");
    assert_eq!(log(&app).len(), lines_before, "a preview logged a line");
    assert!(ids(&app).contains(&"site/b".to_owned()), "a preview removed the piece");
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

fn candidate_at(app: &App, mesh: &str) -> Option<usize> {
    app.world().resource::<ImportState>().candidates.iter().position(|c| c.mesh == mesh)
}

/// **A person's `Delete` on a library piece is one `piece_remove` line stamped as a person's, it sends
/// the mesh back to NOT IMPORTED with the cursor on it, Cmd+Z puts the piece back and lands on it, and
/// Cmd+Shift+Z sends it out again.** Before slice 8 the key wrote `library.ron` itself and logged
/// nothing.
#[test]
fn a_persons_delete_removes_through_the_operation_and_cmd_z_and_redo_follow_it() {
    let mut app = editor("piece-remove-person");
    let mesh = mesh_of(&app, "site/b");
    app.world_mut().resource_mut::<ImportState>().selected_library_id = Some("site/b".to_owned());
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(candidate_at(&app, &mesh), None, "while it is in the library it is not a candidate");
    let lines_before = log(&app).len();

    tap(&mut app, Key::Delete, bevy_cauldron::keys::REMOVE_KEY);
    for _ in 0..3 {
        app.update();
    }
    assert!(app.world().resource::<Project>().library.get("site/b").is_none(), "Delete did not remove the piece");
    assert!(!file(&app).contains("\"site/b\""), "Delete did not reach the library file: {}", file(&app));
    let lines = log(&app);
    assert_eq!(lines.len(), lines_before + 1, "one press of Delete is one log line");
    assert_eq!(lines.last().map(|l| l.op.as_str()), Some("piece_remove"));
    assert_eq!(
        lines.last().map(|l| format!("{:?}", l.source)),
        Some("Human".to_owned()),
        "a person's removal must be stamped as a person's"
    );
    let back = candidate_at(&app, &mesh);
    assert!(back.is_some(), "the mesh must be back among the candidates");
    let state = app.world().resource::<ImportState>();
    assert_eq!(state.selected_library_id, None, "the cursor leaves the library shelf");
    assert_eq!(Some(state.selected), back, "and lands on the piece that was just sent back");

    undo_chord(&mut app, false);
    assert!(
        app.world().resource::<Project>().library.get("site/b").is_some(),
        "one Cmd+Z must put the piece back"
    );
    assert!(file(&app).contains("\"site/b\""), "Cmd+Z must put the file back: {}", file(&app));
    assert_eq!(candidate_at(&app, &mesh), None, "a restored piece is not also a candidate");
    assert_eq!(
        app.world().resource::<ImportState>().selected_library_id.as_deref(),
        Some("site/b"),
        "the cursor must land on the restored piece"
    );

    undo_chord(&mut app, true);
    assert!(
        app.world().resource::<Project>().library.get("site/b").is_none(),
        "redo must send the piece out again"
    );
    let again = candidate_at(&app, &mesh);
    assert!(again.is_some(), "and its mesh is a candidate again");
    assert_eq!(
        Some(app.world().resource::<ImportState>().selected),
        again,
        "redo lands on the reborn candidate as the key did"
    );
}
