//! **An agent can add a token to the project's vocabulary, and Enter in the vocabulary prompt is the same
//! operation (E-002, #129 slice 11).**
//!
//! `Shift+T` and the `+` chip opened a prompt whose Enter spliced `vocab.ron` with a write of its own, named
//! by no operation, so an agent could use the vocabulary and never grow it, no log line named the addition,
//! and the registry's two-front-end rule listed the key as unmigrated. It is now the registered operation
//! `vocab_add_token`. The first three tests go through the registered BRP methods, the way an agent's
//! request goes; the last types into the real prompt and presses Enter, the way a person does. All assert
//! on the vocabulary the editor holds, the file on disk and the log.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::Mode;
use bevy_cauldron::token_prompt::{Axis, Draft, Field, TokenPrompt};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("site/a", "alpha").build("m");
    // **The shipped file's shape**: one token a line and comments between, which is what the splice
    // edits. The fixture writes each axis on a single line, which no real project's file is.
    std::fs::write(
        root.join("assets/grimoire/vocab.ron"),
        r#"(
    // what a thing is
    kind: (
        tokens: [
            ( name: "prop", note: "a thing" ),
        ],
    ),
    effects: (
        tokens: [
            ( name: "inert", note: "does nothing" ),
        ],
    ),
    // how it looks
    look: (
        tokens: [
            ( name: "plain", note: "unremarkable" ),
        ],
    ),
    surfaces: (
        tokens: [
            ( name: "worktop", note: "a top" ),
        ],
    ),
    capabilities: (tokens: []),
    edge: (
        tokens: [
            ( name: "wall", note: "a solid run-face" ),
        ],
    ),
    slot: (tokens: []),
)"#,
    )
    .unwrap_or_else(|e| panic!("{e}"));
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
        serde_json::json!({"operation": "vocab_add_token", "parameters": parameters, "dry_run": false}),
    )
}

fn add(app: &mut App, axis: &str, name: &str, note: &str) -> serde_json::Value {
    apply(app, serde_json::json!({"axis": axis, "name": name, "note": note}))
}

fn vocab_file(app: &App) -> String {
    std::fs::read_to_string(app.world().resource::<Project>().root.join("assets/grimoire/vocab.ron"))
        .expect("vocab.ron")
}

fn kinds(app: &App) -> Vec<String> {
    app.world().resource::<Project>().vocab.kind.names().map(str::to_owned).collect()
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

/// **One call appends the token, the file and the editor's vocabulary follow, one log line names it, and
/// a piece can wear it at once.** The first token of an axis the fixture declares one for.
#[test]
fn an_agent_adds_a_token_and_the_editor_holds_it_at_once() {
    let mut app = editor("vocab-token-agent");
    assert_eq!(kinds(&app), vec!["prop".to_owned()], "the fixture's vocabulary");
    let lines_before = log(&app).len();
    let comments_before = vocab_file(&app).matches("//").count();

    let a = add(&mut app, "kind", "crate-stack", "a stack of crates");
    assert_eq!(a["outcome"], "applied", "the addition was not applied: {a}");
    assert_eq!(kinds(&app), vec!["prop".to_owned(), "crate-stack".to_owned()], "the editor's vocabulary is stale");
    assert!(vocab_file(&app).contains("crate-stack"), "the file never took it: {}", vocab_file(&app));
    assert_eq!(vocab_file(&app).matches("//").count(), comments_before, "the splice dropped a comment");
    let lines = log(&app);
    assert_eq!(lines.len(), lines_before + 1, "an addition is one log line");
    assert_eq!(lines.last().map(|l| l.op.as_str()), Some("vocab_add_token"));
    assert!(a["summary"].as_str().is_some_and(|s| s.contains("crate-stack")), "{a}");

    // **A piece may wear it now**: `label_set` refuses a token the vocabulary lacks.
    let worn = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "label_set", "parameters": {"id": "site/a", "axis": "kind", "tokens": ["crate-stack"]}, "dry_run": false}),
    );
    assert_eq!(worn["outcome"], "applied", "a piece could not wear the token just added: {worn}");
}

/// **A name the vocabulary does not allow, an axis the editor does not draw, a token the axis already
/// holds, a note with a quote, no note, a typo'd field and an undo of an addition are refused or
/// unavailable, and nothing is written.**
#[test]
fn a_bad_request_is_refused_and_writes_nothing() {
    let mut app = editor("vocab-token-refused");
    let file_before = vocab_file(&app);
    let lines_before = log(&app).len();

    for (what, parameters) in [
        ("an uppercase name", serde_json::json!({"axis": "kind", "name": "Crate", "note": "a crate"})),
        ("a name with a space", serde_json::json!({"axis": "kind", "name": "big crate", "note": "a crate"})),
        ("an axis the editor does not draw", serde_json::json!({"axis": "edge", "name": "crate", "note": "a crate"})),
        ("a token the axis holds", serde_json::json!({"axis": "kind", "name": "prop", "note": "again"})),
        ("a note with a quote", serde_json::json!({"axis": "kind", "name": "crate", "note": "a \"crate\""})),
        ("no note", serde_json::json!({"axis": "kind", "name": "crate", "note": ""})),
        ("a typo'd field", serde_json::json!({"axes": "kind", "name": "crate", "note": "a crate"})),
        ("a missing field", serde_json::json!({"axis": "kind", "name": "crate"})),
    ] {
        let r = apply(&mut app, parameters);
        assert_eq!(r["outcome"], "refused", "{what} was accepted: {r}");
    }
    let held = add(&mut app, "kind", "prop", "again");
    assert!(
        held["refusal"].as_str().is_some_and(|r| r.contains("already")),
        "the refusal must say the axis holds it: {held}"
    );
    assert_eq!(vocab_file(&app), file_before, "a refusal wrote vocab.ron");
    assert_eq!(kinds(&app), vec!["prop".to_owned()], "a refusal changed the vocabulary the editor holds");
    assert_eq!(log(&app).len(), lines_before, "a refusal logged a line");
}

/// **A preview says what would be added and writes nothing, and a second token goes after the first.**
#[test]
fn a_preview_writes_nothing_and_tokens_keep_their_order() {
    let mut app = editor("vocab-token-preview");
    let file_before = vocab_file(&app);
    let lines_before = log(&app).len();
    let p = call(
        &mut app,
        "grimoire/operation_preview",
        serde_json::json!({"operation": "vocab_add_token", "parameters": {"axis": "look", "name": "weathered", "note": "worn by rain"}}),
    );
    assert!(p.to_string().contains("weathered"), "the preview must name the token: {p}");
    let refused = call(
        &mut app,
        "grimoire/operation_preview",
        serde_json::json!({"operation": "vocab_add_token", "parameters": {"axis": "look", "name": "plain", "note": "again"}}),
    );
    assert!(refused.to_string().contains("already"), "a preview must refuse what the commit would: {refused}");
    assert_eq!(vocab_file(&app), file_before, "a preview wrote vocab.ron");
    assert_eq!(log(&app).len(), lines_before, "a preview logged a line");

    // A token's bit is its position: the second goes after the first, never before.
    assert_eq!(add(&mut app, "kind", "first-one", "one")["outcome"], "applied");
    assert_eq!(add(&mut app, "kind", "second-one", "two")["outcome"], "applied");
    assert_eq!(
        kinds(&app),
        vec!["prop".to_owned(), "first-one".to_owned(), "second-one".to_owned()],
        "tokens must only ever be appended"
    );
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

/// **A person's Enter in the prompt is one `vocab_add_token` line stamped as a person's, the chip appears,
/// and a refused name leaves the prompt open with its text.** Before slice 11 the prompt wrote `vocab.ron`
/// itself and logged nothing.
#[test]
fn a_persons_enter_in_the_prompt_adds_through_the_operation() {
    let mut app = editor("vocab-token-person");
    let lines_before = log(&app).len();

    // A name the vocabulary refuses: the prompt stays open and says why.
    app.world_mut().resource_mut::<TokenPrompt>().open = Some(Draft {
        axis: Axis::Kind,
        name: "Bad Name".to_owned(),
        note: "a thing".to_owned(),
        field: Field::Note,
        problem: None,
    });
    for _ in 0..2 {
        app.update();
    }
    tap(&mut app, Key::Enter, KeyCode::Enter);
    let draft = app.world().resource::<TokenPrompt>().open.clone().expect("a refusal keeps the prompt open");
    assert!(draft.problem.is_some(), "the refusal must be shown");
    assert_eq!(draft.name, "Bad Name", "the text stays as typed");
    assert_eq!(log(&app).len(), lines_before, "a refusal logged a line");

    app.world_mut().resource_mut::<TokenPrompt>().open = Some(Draft {
        axis: Axis::Kind,
        name: "lantern".to_owned(),
        note: "a thing that glows".to_owned(),
        field: Field::Note,
        problem: None,
    });
    for _ in 0..2 {
        app.update();
    }
    tap(&mut app, Key::Enter, KeyCode::Enter);
    for _ in 0..2 {
        app.update();
    }
    assert!(app.world().resource::<TokenPrompt>().open.is_none(), "an accepted token closes the prompt");
    assert!(kinds(&app).contains(&"lantern".to_owned()), "the editor's vocabulary never took it");
    assert!(vocab_file(&app).contains("lantern"), "Enter did not reach vocab.ron");
    let lines = log(&app);
    assert_eq!(lines.len(), lines_before + 1, "one Enter is one log line");
    assert_eq!(lines.last().map(|l| l.op.as_str()), Some("vocab_add_token"));
    assert_eq!(
        lines.last().map(|l| format!("{:?}", l.source)),
        Some("Human".to_owned()),
        "a person's addition must be stamped as a person's"
    );
}
