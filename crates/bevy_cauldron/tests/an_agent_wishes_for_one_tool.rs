//! **An agent wishes for the one tool that would have helped, once per hand-back** (#353).
//!
//! The hand-back says what was done. The wish says what it cost: the single tool that would have
//! removed the steps the agent spent. That is the one thing a person reading a finished result
//! cannot see, and it is worth exactly as much as it is cheap to write — so it is written at the end
//! of the turn, in four short fields, and it is filed as an issue.
//!
//! # What is being proved
//!
//! 1. **A wish citing a hand-back is `waiting`**, read the way an agent reads the person's answer:
//!    from `grimoire_state.wishes`.
//! 2. **One wish per hand-back.** A second wish citing the same hand-back is refused, and so is one
//!    citing an id that was never handed back — no presentation, no resolved note.
//! 3. **An empty field, and one over its limit, are each refused with the field's name.** The wish
//!    is refused at the door and never trimmed, so nothing an agent wrote arrives shortened.
//! 4. **The editor writes the context it already has**: the hand-back's title, the operations the
//!    task applied, the editor's version and the client's name and version — and never the project's
//!    path.
//! 5. **Waiting wishes queue in arrival order**, so several arriving at once are not lost.

use bevy::prelude::*;
use bevy_cauldron::harness;

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

/// **Present a result and answer for its id**, which is the hand-back a wish cites.
fn present(app: &mut App, operation_ids: Vec<String>) -> String {
    let presented = call(
        app,
        "grimoire/present",
        serde_json::json!({
            "title": "the courtyard",
            "summary": "A 20 x 6 x 20 m courtyard with its east wall placed.",
            "operation_ids": operation_ids,
        }),
    )
    .unwrap_or_else(|e| panic!("the present answers: {e}"));
    presented["presentation"]
        .as_str()
        .unwrap_or_else(|| panic!("the present names the presentation: {presented}"))
        .to_owned()
}

/// **Answer the question the way a person does**, by pressing the key and running one update — the
/// same shape `an_agent_presents_its_result.rs` uses, and no test-only hook.
fn press(app: &mut App, code: KeyCode) {
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

/// **A wish with every field filled in**, so each test varies the one field it is about.
fn wish_body(after: &str) -> serde_json::Value {
    serde_json::json!({
        "after": {"presentation": after},
        "task": "place a wall along the courtyard's east side",
        "hardship": "three calls to grimoire_state to find the wall's piece id",
        "tool": "grimoire_find_piece: searches the library by label and answers the ids.",
        "example_call": "grimoire_find_piece({\"label\": \"wall\"})",
    })
}

/// **The wishes as an agent reads them**, from `grimoire/state`.
fn wishes(app: &mut App) -> serde_json::Value {
    call(app, "grimoire/state", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("{e}"))["wishes"]
        .clone()
}

/// **1. A wish citing a hand-back is `waiting`.**
#[test]
fn a_wish_citing_a_hand_back_is_waiting() {
    let (_root, mut app) = ready("wishwaiting");
    let applied = apply(
        &mut app,
        "map_set_settings",
        serde_json::json!({"bounds": [20.0, 6.0, 20.0]}),
    );
    let operation_id = applied["operation_id"].as_str().expect("the apply names its id").to_owned();
    let after = present(&mut app, vec![operation_id]);

    let made = call(&mut app, "grimoire/wish", wish_body(&after))
        .unwrap_or_else(|e| panic!("a wish citing a hand-back must be taken: {e}"));
    assert_eq!(
        made["state"], "waiting",
        "the wish was not left waiting in the editor's queue: {made}"
    );

    let queued = wishes(&mut app);
    let list = queued.as_array().unwrap_or_else(|| panic!("wishes is a list: {queued}"));
    assert_eq!(list.len(), 1, "the queue does not hold the wish just made: {queued}");
    assert_eq!(list[0]["id"], made["id"].clone());
    assert_eq!(list[0]["after"], after.as_str());
    assert_eq!(list[0]["state"], "waiting");
}

/// **2. One wish per hand-back**: a second is refused, and so is an id never handed back.
#[test]
fn a_second_wish_for_the_same_hand_back_is_refused() {
    let (_root, mut app) = ready("wishonce");
    let after = present(&mut app, Vec::new());
    call(&mut app, "grimoire/wish", wish_body(&after))
        .unwrap_or_else(|e| panic!("the first wish must be taken: {e}"));

    let refused = call(&mut app, "grimoire/wish", wish_body(&after))
        .expect_err("a second wish citing the same hand-back must be refused");
    assert!(
        refused.contains(&after) && refused.contains("wish"),
        "the refusal neither names the hand-back nor why it is refused: {refused:?}"
    );
    assert_eq!(
        wishes(&mut app).as_array().map(Vec::len),
        Some(1),
        "a refused second wish was queued anyway"
    );
}

/// **2b. An id that was never handed back is refused.**
#[test]
fn a_wish_citing_an_id_never_handed_back_is_refused() {
    let (_root, mut app) = ready("wishunknown");
    let refused = call(
        &mut app,
        "grimoire/wish",
        wish_body("present-99"),
    )
    .expect_err("an id that was never handed back must be refused");
    assert!(
        refused.contains("present-99"),
        "the refusal does not name the id it looked for: {refused:?}"
    );
    assert_eq!(
        wishes(&mut app).as_array().map(Vec::len),
        Some(0),
        "a refused wish was queued anyway"
    );
}

/// **3. An empty field, and one over its limit, are each refused with the field's name.**
#[test]
fn an_empty_field_and_an_over_long_one_are_refused_by_name() {
    let (_root, mut app) = ready("wishfields");
    let after = present(&mut app, Vec::new());

    let mut empty = wish_body(&after);
    empty["hardship"] = serde_json::json!("   ");
    let refused = call(&mut app, "grimoire/wish", empty).expect_err("an empty field must be refused");
    assert!(
        refused.contains("hardship"),
        "the refusal does not name the empty field: {refused:?}"
    );

    let mut long = wish_body(&after);
    long["example_call"] = serde_json::json!("x".repeat(bevy_grimoire_core::wish::WISH_FIELD_MAX_CHARS + 1));
    let refused = call(&mut app, "grimoire/wish", long).expect_err("an over-long field must be refused");
    assert!(
        refused.contains("example_call")
            && refused.contains(&bevy_grimoire_core::wish::WISH_FIELD_MAX_CHARS.to_string()),
        "the refusal does not name the field and its limit: {refused:?}"
    );
    assert_eq!(
        wishes(&mut app).as_array().map(Vec::len),
        Some(0),
        "a refused wish was queued anyway"
    );
}

/// **4. The editor fills the context from what it knows**, so the agent does not retype it.
#[test]
fn the_editor_adds_the_context_it_has_and_never_the_path() {
    let (root, mut app) = ready("wishcontext");
    let applied = apply(
        &mut app,
        "map_set_settings",
        serde_json::json!({"bounds": [20.0, 6.0, 20.0]}),
    );
    let operation_id = applied["operation_id"].as_str().expect("the apply names its id").to_owned();
    // **The client says who it is**, as the MCP server tells the editor after its handshake.
    call(
        &mut app,
        "grimoire/agent_hello",
        serde_json::json!({"token": null, "client": {"name": "omp", "version": "18.16.0"}, "tools": 23}),
    )
    .unwrap_or_else(|e| panic!("the hello answers: {e}"));
    let after = present(&mut app, vec![operation_id.clone()]);

    // **The context comes back on the wish's own answer**, and `grimoire_state.wishes` keeps the
    // queue to `{id, after, state}` so a state dump stays a summary.
    let made = call(&mut app, "grimoire/wish", wish_body(&after))
        .unwrap_or_else(|e| panic!("the wish must be taken: {e}"));
    let context = made["context"]
        .as_str()
        .unwrap_or_else(|| panic!("a wish carries its context: {made}"));
    assert!(
        context.contains("the courtyard"),
        "the context does not carry the hand-back's title: {context:?}"
    );
    assert!(
        context.contains("map_set_settings"),
        "the context does not name the operation the task applied: {context:?}"
    );
    assert!(
        context.contains(env!("CARGO_PKG_VERSION")),
        "the context does not carry the editor's version: {context:?}"
    );
    assert!(
        context.contains("omp") && context.contains("18.16.0"),
        "the context does not carry the client's name and version: {context:?}"
    );
    let root_text = root.to_string_lossy();
    assert!(
        !context.contains(root_text.as_ref()),
        "the context carries the project's path, which is what it must never do: {context:?}"
    );
}

/// **5. Waiting wishes queue in arrival order.**
#[test]
fn wishes_queue_in_arrival_order() {
    let (_root, mut app) = ready("wishorder");
    let first = present(&mut app, Vec::new());
    // **Answered before the next one is asked**, because one question is on screen at a time.
    press(&mut app, KeyCode::KeyN);
    let second = present(&mut app, Vec::new());
    for after in [&first, &second] {
        call(&mut app, "grimoire/wish", wish_body(after))
            .unwrap_or_else(|e| panic!("each wish must be taken: {e}"));
    }
    let queued = wishes(&mut app);
    let list = queued.as_array().unwrap_or_else(|| panic!("wishes is a list: {queued}"));
    assert_eq!(list.len(), 2, "both wishes must be queued: {queued}");
    assert_eq!(list[0]["after"], first.as_str(), "the queue is not in arrival order: {queued}");
    assert_eq!(list[1]["after"], second.as_str(), "the queue is not in arrival order: {queued}");
    assert!(list[0]["id"].as_str() < list[1]["id"].as_str(), "the ids do not count up: {queued}");
}
