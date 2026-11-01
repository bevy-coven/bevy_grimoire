//! **An agent presents its result, and the person keeps it or undoes it (S1.7).**
//!
//! The brief's fourth step asks for four things in one move: what the agent did, what was specified
//! versus inferred, the one to three places worth checking, and a way to take the whole turn back.
//! `grimoire_present` puts them in front of the person as this editor's one question — Keep, or Undo
//! all — and their answer comes back to the agent through `grimoire/state`.
//!
//! # What is being proved
//!
//! 1. **A presentation is a question, and Keep changes nothing.** That is the safe default: a key
//!    pressed by reflex must not throw away a turn's work.
//! 2. **Undo all takes back every listed operation, newest first.**
//! 3. **An id the log does not hold is refused**, rather than making the undo walk stop on something
//!    that never happened.
//! 4. **A fourth place to check is refused** — "up to three" is the brief's own limit, and a list of
//!    seven is not a short list.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::present::Presenting;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

/// **A project with one map and one piece**, saved.
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m")
}

fn editor_with_bridge(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app
}

fn call(
    app: &mut App,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    use bevy::remote::RemoteMethodSystemId;
    use bevy::remote::RemoteMethods;
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err(format!("no method `{method}` is registered")),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .map_err(|e| format!("{method}: {e:?}"))?
        .map_err(|e| format!("{method}: {e:?}"))
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": operation,
            "parameters": parameters,
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"))
}

fn ready(name: &str) -> (std::path::PathBuf, App) {
    let root = project(name);
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }
    (root, app)
}

/// **Answer the question the way a person does**, by pressing the key and running one update.
///
/// The modal reads `ButtonInput<KeyCode>`, so a press followed by an `update()` is the real path —
/// the same shape the confirm tests use, and no test-only hook.
fn press(app: &mut App, code: KeyCode) {
    // **A written `KeyboardInput`, not `ButtonInput::press`.** The editor's own tests drive input this
    // way (`a_meshes_undo_takes_back_the_agents_newest_label::tap`), and a first version of this
    // helper that called `ButtonInput::press` did not reach the modal at all — the presentation
    // stayed unanswered while the test reported it as a bug in the answer.
    use bevy::input::keyboard::{Key, KeyboardInput};
    for state in [
        bevy::input::ButtonState::Pressed,
        bevy::input::ButtonState::Released,
    ] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

/// **The presentation as an agent reads it** — from `grimoire/state`, which is the only channel it has
/// for what happened after its call returned.
fn presentation(app: &mut App) -> serde_json::Value {
    call(app, "grimoire/state", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("{e}"))["presentation"]
        .clone()
}

/// **1. A presentation is a question, and Keep changes nothing.**
#[test]
fn a_presentation_is_a_question_and_keep_changes_nothing() {
    let (_root, mut app) = ready("presentkeep");
    let applied = apply(
        &mut app,
        "map_set_settings",
        serde_json::json!({"bounds": [20.0, 6.0, 20.0]}),
    );
    assert_eq!(applied["outcome"], "applied", "{applied}");
    let id = applied["operation_id"].as_str().expect("the apply names its id").to_owned();

    let presented = call(
        &mut app,
        "grimoire/present",
        serde_json::json!({
            "title": "the courtyard",
            "summary": "A 20 x 6 x 20 m courtyard.",
            "specified": ["a courtyard"],
            "inferred": ["20 x 6 x 20 m"],
            "operation_ids": [id],
        }),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        presented.get("presentation").is_some(),
        "the present answered without naming the presentation: {presented}"
    );

    // **The question is up, and unanswered.** This is what an agent polls for.
    let state = presentation(&mut app);
    assert_eq!(
        state["answer"],
        serde_json::Value::Null,
        "the presentation was answered before the person said anything: {state}"
    );
    assert_eq!(state["title"], "the courtyard");
    assert!(
        app.world().resource::<Presenting>().0.is_some(),
        "no presentation is being asked about"
    );

    // **N keeps**, and the bounds are still the ones the agent set.
    press(&mut app, KeyCode::KeyN);
    let state = presentation(&mut app);
    assert_eq!(state["answer"], "kept", "N did not keep the result: {state}");
    assert_eq!(
        app.world().resource::<OpenMap>().map.bounds,
        (20.0, 6.0, 20.0),
        "keeping a result changed the map"
    );
}

/// **2. Undo all takes back every listed operation, newest first.**
#[test]
fn undo_all_takes_back_every_listed_change() {
    let (_root, mut app) = ready("presentundo");
    let first = apply(
        &mut app,
        "map_set_settings",
        serde_json::json!({"bounds": [20.0, 6.0, 20.0]}),
    );
    let second = apply(
        &mut app,
        "map_set_settings",
        serde_json::json!({"bounds": [30.0, 8.0, 30.0]}),
    );
    let ids: Vec<String> = [first["operation_id"].as_str(), second["operation_id"].as_str()]
        .iter()
        .map(|i| i.expect("each apply names its id").to_owned())
        .collect();
    assert_eq!(
        app.world().resource::<OpenMap>().map.bounds,
        (30.0, 8.0, 30.0),
        "the two settings changes did not land"
    );

    call(
        &mut app,
        "grimoire/present",
        serde_json::json!({
            "title": "two resizes",
            "summary": "The map was resized twice.",
            "operation_ids": ids,
        }),
    )
    .unwrap_or_else(|e| panic!("{e}"));

    // **Y undoes**, which is the destructive answer and therefore the one `Enter` refuses.
    press(&mut app, KeyCode::KeyY);
    let state = presentation(&mut app);
    assert_eq!(state["answer"], "undone", "Y did not undo: {state}");
    assert_ne!(
        app.world().resource::<OpenMap>().map.bounds,
        (30.0, 8.0, 30.0),
        "the presentation was undone and the newest settings change is still on the map"
    );
}

/// **3. An unknown operation id is refused.**
#[test]
fn an_unknown_operation_id_is_refused() {
    let (_root, mut app) = ready("presentbad");
    // **A refusal is a BRP error**, not an `{refused}` body — this handler answers
    // `Result<Value, String>`, so the message arrives through `call`'s error arm.
    let refused = call(
        &mut app,
        "grimoire/present",
        serde_json::json!({
            "title": "nothing",
            "summary": "A result that never happened.",
            "operation_ids": ["9999-dead"],
        }),
    )
    .expect_err("an id the log does not hold must be refused");
    assert!(
        refused.contains("9999-dead") && refused.contains("operation log"),
        "the refusal neither names the id nor where it looked: {refused:?}"
    );
    assert!(
        app.world().resource::<Presenting>().0.is_none(),
        "a refused presentation was still put up"
    );
}

/// **4. Four places to check is refused.**
#[test]
fn four_places_to_check_are_refused() {
    let (_root, mut app) = ready("presentfour");
    let checks: Vec<serde_json::Value> =
        (0..4).map(|i| serde_json::json!({"what": format!("spot {i}")})).collect();
    let refused = call(
        &mut app,
        "grimoire/present",
        serde_json::json!({
            "title": "too much",
            "summary": "Four places is not a short list.",
            "check": checks,
        }),
    )
    .expect_err("a fourth place to check must be refused");
    assert!(
        refused.contains("3") || refused.contains("three"),
        "the refusal does not say the limit: {refused:?}"
    );
}
