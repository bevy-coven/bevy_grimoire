//! **A refusal on the person's screen is a sentence, with no hole in it** (`feedback-1794147495932`).
//!
//! > Huh?
//!
//! The pick is on the editor's toast: *agent: map_create: these are not the parameters for making a map: invalid type: null,
//! expected struct Params. The fields are `name`,* then a gap of about twenty blanks, then *`bounds`, `origin` and `note`, and
//! `name` is required.* The toast wraps its text, so the gap fell at a line's end and the next line began a column in: the
//! message looked broken. The source had a string literal whose line continuation had been collapsed into a run of blanks;
//! it was in this refusal and in a dozen other messages written the same way.
//!
//! Oracle `other:message-text`: an invariant on one run. Whatever an agent's refused operation leaves on the person's screen
//! (the editor's status banner, which the toast shows), it is the operation's name and one sentence, and the sentence holds no
//! run of blanks. The operations and parameters below are the refusals whose literals carried the run: not the parameters of a
//! map, a name that leaves nothing usable, a name that is taken, bounds that are not positive, and two answers for one axis.

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;

use crate::fixtures;

fn editor_with_bridge(root: &std::path::Path, map: &str) -> App {
    let mut app = harness::build_headless(root, map, None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

/// An agent's `grimoire/operation_apply`, as the MCP server forwards it. The answer is not read: what is on screen is.
fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get("grimoire/operation_apply") {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => panic!("grimoire/operation_apply is not registered"),
    };
    let params = serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false});
    let _ = app.world_mut().run_system_with(id, Some(params));
    app.update();
}

#[test]
fn a_refusal_an_agent_causes_is_one_unbroken_sentence_on_the_screen() {
    let root = fixtures::Fixture::new(&format!("refusal_text_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let mut app = editor_with_bridge(&root, "m");

    let probes: [(&str, serde_json::Value, &str); 6] = [
        ("map_create", serde_json::Value::Null, "not the parameters for making a map"),
        ("map_create", serde_json::json!({"name": "???"}), "leaves nothing usable as a name"),
        ("map_create", serde_json::json!({"name": "m"}), "already exists in this project"),
        ("map_create", serde_json::json!({"name": "fresh", "bounds": [-1.0, 4.0, 10.0]}), "has to be positive"),
        ("map_set_settings", serde_json::json!({"bounds": [0.0, 4.0, 10.0]}), "has to be positive"),
        (
            "map_set_settings",
            serde_json::json!({"bounds": [10.0, 4.0, 10.0], "axis": "x", "axis_value": 5.0}),
            "one way or the other",
        ),
    ];
    for (operation, parameters, says) in probes {
        apply(&mut app, operation, parameters.clone());
        let on_screen = app.world().resource::<EditorState>().status.problem_text().to_owned();
        let prefix = format!("agent: {operation}: ");
        assert!(
            on_screen.starts_with(&prefix) && on_screen.contains(says),
            "{operation} {parameters} should have been refused with `{says}`, and the screen says {on_screen:?}"
        );
        assert!(
            !on_screen.contains("  ") && !on_screen.contains('\n') && !on_screen.contains('\t'),
            "the refusal of {operation} {parameters} has a run of blanks in it: {on_screen:?}"
        );
    }
}
