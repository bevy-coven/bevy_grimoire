//! **A piece's description and its width are operations an agent calls, and the Meshes tab's two fields
//! are the same operations (E-002, #129 slices 17 and 18).**
//!
//! The description field and the SIZE field wrote the library with writes of their own, so an agent could
//! label a piece's kind and look but not describe or resize it, and no log line named either edit. They
//! are now `piece_set_note` and `piece_set_width`. The agent tests go through the registered BRP methods;
//! the person tests type into the real fields and press Enter, and assert on the library the editor holds,
//! the file on disk and the log.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{ImportState, Mode, NoteEdit, ScaleEdit};
use bevy_grimoire_core::descriptor::{Axis, By};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .sized_descriptor("site/crate", "alpha", 1.0, 0.5)
        .descriptor("site/gate", "beta")
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

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

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    )
}

fn undo(app: &mut App, id: &str) -> serde_json::Value {
    call(app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}))
}

fn piece(app: &App, id: &str) -> bevy_grimoire_core::descriptor::Descriptor {
    app.world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == id)
        .unwrap_or_else(|| panic!("`{id}` is not in the live measured library"))
        .clone()
}

fn file(app: &App) -> String {
    std::fs::read_to_string(&app.world().resource::<Project>().library_path).expect("the library file")
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

fn select(app: &mut App, id: &str) {
    app.world_mut().resource_scope(|world, mut import: Mut<ImportState>| {
        let project = world.resource::<Project>();
        bevy_cauldron::editor::select_piece(&mut import, project, id);
    });
    for _ in 0..4 {
        app.update();
    }
}

/// **An agent describes a piece, the file follows, it is named as the source, and one undo puts the
/// description and the label record back.**
#[test]
fn an_agent_describes_a_piece_and_one_undo_puts_it_back() {
    let mut app = editor("note-agent");
    let before = piece(&app, "site/crate");
    assert_eq!(before.note.as_deref(), Some("a fixture piece"), "the fixture's note changed");
    let lines = log(&app).len();

    let a = apply(&mut app, "piece_set_note", serde_json::json!({"id": "site/crate", "note": "  the one with the cracked lid  "}));
    assert_eq!(a["outcome"], "applied", "{a}");
    let d = piece(&app, "site/crate");
    assert_eq!(d.note.as_deref(), Some("the one with the cracked lid"), "the description is trimmed and held");
    assert_eq!(d.labels.as_ref().and_then(|l| l.by.get(Axis::Note)), Some(By::Agent), "an agent's note is stamped as an agent's");
    assert!(file(&app).contains("the one with the cracked lid"), "the library file does not hold it");
    let entries = log(&app);
    assert_eq!(entries.len(), lines + 1, "a description is one log line");
    assert_eq!(entries.last().map(|l| l.op.as_str()), Some("piece_set_note"));

    let id = a["operation_id"].as_str().expect("id").to_owned();
    let back = undo(&mut app, &id);
    assert_eq!(back["outcome"], "applied", "{back}");
    let d = piece(&app, "site/crate");
    assert_eq!(d.note, before.note, "one undo puts the description back");
    assert_eq!(d.labels, before.labels, "and the label record");
    assert!(!file(&app).contains("cracked lid"));
    assert!(file(&app).contains("a fixture piece"), "and the file holds the old description");
}

/// **A blank note clears, a repeat, an unknown piece, a typo'd field and an agent's own put-back are
/// refused, and nothing is written.**
#[test]
fn bad_descriptions_are_refused_and_a_blank_one_clears() {
    let mut app = editor("note-refused");
    let a = apply(&mut app, "piece_set_note", serde_json::json!({"id": "site/crate", "note": "a crate"}));
    assert_eq!(a["outcome"], "applied", "{a}");
    let written = file(&app);
    let lines = log(&app).len();
    for (what, params) in [
        ("the same description again", serde_json::json!({"id": "site/crate", "note": "a crate"})),
        ("an unknown piece", serde_json::json!({"id": "site/nope", "note": "x"})),
        ("a typo'd field", serde_json::json!({"id": "site/crate", "notes": "x"})),
        ("no note", serde_json::json!({"id": "site/crate"})),
        ("a put-back from an agent", serde_json::json!({"id": "site/crate", "restore": {"note": "forged", "labels": null}})),
    ] {
        let r = apply(&mut app, "piece_set_note", params);
        assert_eq!(r["outcome"], "refused", "{what} was accepted: {r}");
    }
    assert_eq!(file(&app), written, "a refusal wrote the library");
    assert_eq!(log(&app).len(), lines, "a refusal logged a line");

    let cleared = apply(&mut app, "piece_set_note", serde_json::json!({"id": "site/crate", "note": ""}));
    assert_eq!(cleared["outcome"], "applied", "{cleared}");
    assert_eq!(piece(&app, "site/crate").note, None, "a blank note clears it");
}

/// **An agent resizes a piece, the file follows, and one undo puts the extent and alignment back byte
/// for byte.**
#[test]
fn an_agent_resizes_a_piece_and_one_undo_puts_it_back() {
    let mut app = editor("width-agent");
    let before = piece(&app, "site/crate");
    let written = file(&app);
    assert_eq!(before.extent.footprint, Some((1.0, 0.5)), "the fixture's footprint changed");
    let lines = log(&app).len();

    let a = apply(&mut app, "piece_set_width", serde_json::json!({"id": "site/crate", "width": 2.0}));
    assert_eq!(a["outcome"], "applied", "{a}");
    let d = piece(&app, "site/crate");
    assert_eq!(d.extent.footprint, Some((2.0, 1.0)), "the footprint scales with the width, depth in proportion");
    assert_eq!(d.align.scale, Some(2.0), "the mesh scale composes");
    assert_ne!(file(&app), written, "the library file does not hold the resize");
    assert_eq!(log(&app).len(), lines + 1, "a resize is one log line");
    assert_eq!(log(&app).last().map(|l| l.op.as_str()), Some("piece_set_width"));

    let id = a["operation_id"].as_str().expect("id").to_owned();
    let back = undo(&mut app, &id);
    assert_eq!(back["outcome"], "applied", "{back}");
    let d = piece(&app, "site/crate");
    assert_eq!(d.extent, before.extent, "one undo puts the extent back");
    assert_eq!(d.align, before.align, "and the alignment");
    assert!(file(&app).contains("footprint: Some((1.0, 0.5))"), "and the file holds the old footprint");
}

/// **A width that is not a length, one it already stands at, a piece with no footprint, an unknown piece,
/// a typo'd field and an agent's own put-back are refused, and nothing is written.**
#[test]
fn bad_widths_are_refused_and_nothing_is_written() {
    let mut app = editor("width-refused");
    let written = file(&app);
    let lines = log(&app).len();
    for (what, params) in [
        ("zero", serde_json::json!({"id": "site/crate", "width": 0.0})),
        ("a negative width", serde_json::json!({"id": "site/crate", "width": -1.0})),
        ("the width it already stands at", serde_json::json!({"id": "site/crate", "width": 1.0})),
        ("an unknown piece", serde_json::json!({"id": "site/nope", "width": 1.0})),
        ("a typo'd field", serde_json::json!({"id": "site/crate", "widht": 2.0})),
        ("no width", serde_json::json!({"id": "site/crate"})),
        (
            "a put-back from an agent",
            serde_json::json!({"id": "site/crate", "restore": {"extent": {"footprint": [9.0, 9.0]}, "align": {}}}),
        ),
    ] {
        let r = apply(&mut app, "piece_set_width", params);
        assert_eq!(r["outcome"], "refused", "{what} was accepted: {r}");
    }
    assert_eq!(file(&app), written, "a refusal wrote the library");
    assert_eq!(log(&app).len(), lines, "a refusal logged a line");
}

fn type_text(app: &mut App, text: &str) {
    for ch in text.chars() {
        let code = match ch {
            'a' => KeyCode::KeyA,
            'b' => KeyCode::KeyB,
            'c' => KeyCode::KeyC,
            '1' => KeyCode::Digit1,
            '2' => KeyCode::Digit2,
            '.' => KeyCode::Period,
            ' ' => KeyCode::Space,
            _ => panic!("add {ch} to the test's key table"),
        };
        let key = if ch == ' ' { Key::Space } else { Key::Character(ch.to_string().into()) };
        tap(app, key, code);
    }
}

/// **A person's typed description is one `piece_set_note` line stamped as a person's**, reaches the file,
/// and one Cmd+Z takes it back.
#[test]
fn a_persons_typed_description_is_one_logged_change_and_cmd_z_takes_it_back() {
    let mut app = editor("note-person");
    select(&mut app, "site/crate");
    let lines = log(&app).len();
    app.world_mut().resource_mut::<NoteEdit>().open_for_test("site/crate");
    app.update();
    type_text(&mut app, "abc a");
    tap(&mut app, Key::Enter, KeyCode::Enter);
    app.update();

    let d = piece(&app, "site/crate");
    assert_eq!(d.note.as_deref(), Some("abc a"), "Enter did not set the description");
    assert_eq!(d.labels.as_ref().and_then(|l| l.by.get(Axis::Note)), Some(By::Human));
    let entries = log(&app);
    assert_eq!(entries.len(), lines + 1, "one Enter is one log line");
    let last = entries.last().expect("a line");
    assert_eq!(last.op, "piece_set_note");
    assert_eq!(format!("{:?}", last.source), "Human", "a person's description must be stamped as a person's");
    assert!(file(&app).contains("abc a"), "the file does not hold it");

    let held = bevy_cauldron::keys::MOD_KEYS[0];
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(held);
    tap(&mut app, Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified), KeyCode::KeyZ);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(held);
    app.update();
    assert_eq!(piece(&app, "site/crate").note.as_deref(), Some("a fixture piece"), "one Cmd+Z must take the description back");
}

/// **A person's typed width is one `piece_set_width` line stamped as a person's**, resizes the piece in
/// the library and the file, and one Cmd+Z takes it back.
#[test]
fn a_persons_typed_width_is_one_logged_change_and_cmd_z_takes_it_back() {
    let mut app = editor("width-person");
    select(&mut app, "site/crate");
    let before = piece(&app, "site/crate");
    let lines = log(&app).len();
    app.world_mut().resource_mut::<ScaleEdit>().open_for_test("site/crate");
    app.update();
    type_text(&mut app, "2");
    tap(&mut app, Key::Enter, KeyCode::Enter);
    app.update();

    let d = piece(&app, "site/crate");
    assert_eq!(d.extent.footprint, Some((2.0, 1.0)), "Enter did not resize the piece");
    let entries = log(&app);
    assert_eq!(entries.len(), lines + 1, "one Enter is one log line");
    let last = entries.last().expect("a line");
    assert_eq!(last.op, "piece_set_width");
    assert_eq!(format!("{:?}", last.source), "Human", "a person's resize must be stamped as a person's");

    let held = bevy_cauldron::keys::MOD_KEYS[0];
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(held);
    tap(&mut app, Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified), KeyCode::KeyZ);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(held);
    app.update();
    assert_eq!(piece(&app, "site/crate").extent, before.extent, "one Cmd+Z must take the resize back");
}
