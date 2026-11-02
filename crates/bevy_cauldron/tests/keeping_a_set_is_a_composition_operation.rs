//! **Keeping the set in hand as a group is the operation an agent calls (E-002, #129 slice 14).**
//!
//! `M` then a name then Enter turned the set in hand into a group with a write of its own
//! (`keep_as_group` from `group_name_keys`), named by no operation: no log line, no undo, and the
//! registry's two-front-end rule listed `GroupFromSet` as unmigrated. The set in hand is not always the
//! placements on the map (it outlives the pieces it was taken from, and carries stamps whole), so
//! `composition_define` takes the set itself as `set`, beside the `from` ids an agent already names.
//! The first tests go through the registered BRP methods, the way an agent's request goes; the last two
//! hold a set and press the real keys, the way a person does.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy_cauldron::editor::{CloneDrag, CloneHost, ClonePiece, CloneSet, EditorState};
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::Project;

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name)
        .sized_descriptor("crate", "alpha", 1.0, 1.0)
        .sized_descriptor("barrel", "alpha", 1.0, 1.0)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    (root, app)
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn define(app: &mut App, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "composition_define", "parameters": parameters, "dry_run": false}),
    )
}

fn comps(root: &std::path::Path) -> String {
    std::fs::read_to_string(root.join("assets/grimoire/compositions.ron")).unwrap_or_default()
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

/// A crate with a barrel stood on it, and a second crate beside them: three pieces, offsets from the
/// set's centre, as an agent writes them.
fn literal_set() -> serde_json::Value {
    serde_json::json!({
        "pieces": [
            {"descriptor": "crate", "offset": [-1.0, 0.0]},
            {"descriptor": "crate", "offset": [1.0, 0.0], "yaw": 90.0, "note": "the far one"},
            {"descriptor": "barrel", "offset": [-1.0, 0.0], "on": {"in_set": 0}}
        ]
    })
}

/// **An agent defines a group from a literal set, with no placements on the map, and one undo takes it
/// back.**
#[test]
fn an_agent_defines_a_group_from_a_literal_set_and_one_undo_takes_it_back() {
    let (root, mut app) = editor("keep_set_agent");
    let before = comps(&root);
    let lines = log(&app).len();

    let made = define(&mut app, serde_json::json!({"id": "Corner Pile", "set": literal_set()}));
    assert_eq!(made["outcome"], "applied", "{made}");
    let after = comps(&root);
    assert!(after.contains("id: \"corner_pile\""), "the id is forced to snake_case and written: {after}");
    let project = app.world().resource::<Project>();
    let c = project.compositions.compositions.iter().find(|c| c.id == "corner_pile").expect("the project holds it");
    assert_eq!(c.members.len(), 3);
    assert!(c.members.iter().any(|m| m.yaw == 90.0 && m.note.as_deref() == Some("the far one")), "a piece's yaw and note ride along");
    assert_eq!(log(&app).len(), lines + 1, "one definition is one log line");

    let id = made["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let undone = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(undone["outcome"], "applied", "{undone}");
    assert_eq!(comps(&root), before, "undo puts compositions.ron back byte for byte");
}

/// **A bad set, or both ways of naming one, is refused whole and writes nothing.**
#[test]
fn a_set_that_cannot_make_a_group_is_refused_and_nothing_is_written() {
    let (root, mut app) = editor("keep_set_refused");
    let before = comps(&root);
    let lines = log(&app).len();
    for (what, params) in [
        (
            "a piece the library lacks",
            serde_json::json!({"id": "g", "set": {"pieces": [{"descriptor": "no_such_piece", "offset": [0.0, 0.0]}]}}),
        ),
        (
            "a host that is not in the set",
            serde_json::json!({"id": "g", "set": {"pieces": [{"descriptor": "crate", "offset": [0.0, 0.0], "on": {"in_set": 4}}]}}),
        ),
        (
            "a host left outside",
            serde_json::json!({"id": "g", "set": {"pieces": [{"descriptor": "crate", "offset": [0.0, 0.0], "on": "outside"}]}}),
        ),
        ("an empty set", serde_json::json!({"id": "g", "set": {"pieces": []}})),
        ("both from and set", serde_json::json!({"id": "g", "from": ["a"], "set": literal_set()})),
        ("a typo'd piece field", serde_json::json!({"id": "g", "set": {"pieces": [{"descriptor": "crate", "offset": [0.0, 0.0], "yawn": 1.0}]}})),
    ] {
        let r = define(&mut app, params);
        assert_eq!(r["outcome"], "refused", "{what} was accepted: {r}");
    }
    assert_eq!(comps(&root), before, "a refusal wrote compositions.ron");
    assert_eq!(log(&app).len(), lines, "a refusal logged a line");
}

fn tap(app: &mut App, logical: Key, code: KeyCode) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
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

fn type_name(app: &mut App, name: &str) {
    for ch in name.chars() {
        let code = match ch {
            'a' => KeyCode::KeyA,
            'b' => KeyCode::KeyB,
            'c' => KeyCode::KeyC,
            'd' => KeyCode::KeyD,
            _ => panic!("add {ch} to the test's key table"),
        };
        tap(app, Key::Character(ch.to_string().into()), code);
    }
}

fn hold_two_pieces(app: &mut App) {
    let piece = |d: &str, x: f32| ClonePiece {
        descriptor: d.to_owned(),
        offset: (x, 0.0),
        yaw: 0.0,
        tip: (0, 0),
        lift: 0.0,
        note: None,
        owned: false,
        owned_because: None,
        patch: None,
        on: CloneHost::Layer,
    };
    let set = CloneSet {
        pieces: vec![piece("crate", -0.5), piece("barrel", 0.5)],
        stamps: Vec::new(),
        centre_off: (0.0, 0.0),
        half: (1.0, 0.5),
        yaw: 0.0,
    };
    let mut drag = app.world_mut().resource_mut::<CloneDrag>();
    bevy_cauldron::editor::hold_set_for_test(set, &mut drag);
    app.update();
}

/// **A person's M, a name and Enter is one `composition_define` line stamped as a person's, and the set
/// in hand is put down and armed**, as before; one undo through the log takes the group back.
#[test]
fn a_persons_m_name_enter_keeps_the_set_through_the_operation() {
    let (root, mut app) = editor("keep_set_person");
    let before = comps(&root);
    let lines = log(&app).len();
    hold_two_pieces(&mut app);

    tap(&mut app, Key::Character("m".into()), KeyCode::KeyM);
    assert!(app.world().resource::<EditorState>().grouping.is_some(), "M opens the name field");
    type_name(&mut app, "abc");
    tap(&mut app, Key::Enter, KeyCode::Enter);
    for _ in 0..2 {
        app.update();
    }

    let after = comps(&root);
    assert!(after.contains("id: \"abc\""), "Enter did not reach compositions.ron: {after}");
    let entries = log(&app);
    assert_eq!(entries.len(), lines + 1, "one Enter is one log line");
    let last = entries.last().expect("a line");
    assert_eq!(last.op, "composition_define");
    assert_eq!(format!("{:?}", last.source), "Human", "a person's group must be stamped as a person's");
    assert_eq!(
        app.world().resource::<bevy_cauldron::compose::Armed>().0.as_deref(),
        Some("abc"),
        "the next click stamps what was just made"
    );
    assert!(app.world().resource::<EditorState>().grouping.is_none());

    let id = last.operation_id.clone();
    let undone = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(undone["outcome"], "applied", "{undone}");
    assert_eq!(comps(&root), before, "one undo takes the group back");
}

/// **Capturing over a name that exists asks first, and the second Enter redefines it** through the
/// operation: the first Enter writes nothing and logs nothing.
#[test]
fn keeping_a_set_over_an_existing_name_asks_then_redefines() {
    let (root, mut app) = editor("keep_set_replace");
    let made = define(&mut app, serde_json::json!({"id": "abc", "set": literal_set()}));
    assert_eq!(made["outcome"], "applied", "{made}");
    let first = comps(&root);
    let lines = log(&app).len();
    hold_two_pieces(&mut app);

    tap(&mut app, Key::Character("m".into()), KeyCode::KeyM);
    type_name(&mut app, "abc");
    tap(&mut app, Key::Enter, KeyCode::Enter);
    assert_eq!(comps(&root), first, "the first Enter over an existing name wrote");
    assert_eq!(log(&app).len(), lines, "the first Enter logged");
    assert!(
        app.world().resource::<EditorState>().replacing.is_some(),
        "the editor must be waiting for the second Enter"
    );

    tap(&mut app, Key::Enter, KeyCode::Enter);
    for _ in 0..2 {
        app.update();
    }
    assert_ne!(comps(&root), first, "the second Enter did not redefine the group");
    let entries = log(&app);
    assert_eq!(entries.len(), lines + 1, "the redefinition is one log line");
    assert_eq!(entries.last().expect("a line").op, "composition_define");
    let project = app.world().resource::<Project>();
    let c = project.compositions.compositions.iter().find(|c| c.id == "abc").expect("abc");
    assert_eq!(c.members.len(), 2, "the group is now the two pieces in hand");
}
