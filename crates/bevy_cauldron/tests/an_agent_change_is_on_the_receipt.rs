//! **An agent's change is on the person's receipt (B-020).**
//!
//! # What the bug was
//!
//! `grimoire/operation_apply` and `grimoire/operation_undo` recorded the change in the one undo
//! history and the log, but never touched the INSPECTOR's receipt row. So after an agent turned a
//! piece, the person still saw the receipt from their last hand edit: the screen said one thing
//! and the map held another.
//!
//! # What is being proved
//!
//! 1. **An applied agent change replaces the receipt**, and the receipt names the operation and
//!    says what it did, so a person can tell the agent did it.
//! 2. **A refused agent change goes to the journal as a problem**, and does not overwrite the
//!    person's receipt. A refusal is an alarm, not an answer (`Status::problem`).
//! 3. **An agent's undo is on the receipt too**, named `undo`.
//!
//! The receipt is read over `grimoire/state`, the same field the INSPECTOR's receipt row draws, so
//! the test reads what an agent reads and what a person sees.

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;

use crate::fixtures;

/// **A project with one map, one piece, and a map that has been saved.**
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m")
}

/// **The editor, with the agent's door open.** Same plugin graph as `an_agent_can_drive_the_editor`.
fn editor_with_bridge(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app
}

/// **Call one of the editor's BRP commands through the registered method**, as the MCP server does.
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

/// **What the person's receipt row says, and what the problem journal holds, read the way an
/// agent reads them** (`grimoire/state`'s `editor` block).
fn receipt(app: &mut App) -> (String, Vec<String>) {
    let answer = call(app, "grimoire/state", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("the state answers: {e}"));
    let editor = &answer["editor"];
    let note = editor["status"].as_str().unwrap_or_default().to_owned();
    let problems = editor["problems"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|p| p.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    (note, problems)
}

/// **Plant the person's own receipt**, so the test can tell an agent's receipt from a stale one.
fn person_has_a_receipt(app: &mut App) {
    app.world_mut()
        .resource_mut::<EditorState>()
        .status
        .note("a person's receipt");
}

/// **An applied agent change replaces the person's receipt, and names the operation.**
#[test]
fn an_agent_apply_replaces_the_receipt() {
    let root = project("receiptapply");
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }
    person_has_a_receipt(&mut app);

    let applied = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "map_set_settings",
            "parameters": {"bounds": [64.0, 8.0, 64.0]},
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"));
    assert_eq!(applied["outcome"], "applied", "the apply did not apply: {applied}");
    let summary = applied["summary"].as_str().expect("the apply names its summary");

    let (note, _) = receipt(&mut app);
    assert_ne!(
        note, "a person's receipt",
        "the person's receipt is still on screen after an agent changed the map. The screen says \
         nothing happened, and the map says something did."
    );
    assert!(
        note.contains("map_set_settings") && note.contains(summary),
        "the receipt does not name the operation and what it did, so a person cannot tell the \
         agent did it. receipt: {note:?}"
    );

    // **And the change joined the one undo history**, which is what B-019 promised and what the
    // report's `undo_depth 0` said was missing.
    let depth = app.world().resource::<EditorState>().undo_depth();
    assert_eq!(depth, 1, "the agent's applied change is not on the undo history");
}

/// **A refused agent change goes to the journal, and does not overwrite the person's receipt.**
#[test]
fn an_agent_refusal_reaches_the_journal_and_leaves_the_receipt() {
    let root = project("receiptrefuse");
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }
    person_has_a_receipt(&mut app);

    let refused = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "map_set_settings",
            "parameters": {"bounds": [0.0, 8.0, 64.0]},
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the refusal answers: {e}"));
    assert_eq!(refused["outcome"], "refused", "{refused}");

    let (note, problems) = receipt(&mut app);
    assert_eq!(
        note, "a person's receipt",
        "a refusal overwrote the person's receipt. A refusal goes to the journal as a problem."
    );
    assert!(
        problems.iter().any(|p| p.contains("positive")),
        "the refused agent change is not in the problem journal, so the person never learns what \
         the agent tried. problems: {problems:?}"
    );
}

/// **An agent's undo is on the receipt too**, named `undo`.
#[test]
fn an_agent_undo_is_on_the_receipt() {
    let root = project("receiptundo");
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }
    let applied = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "map_set_settings",
            "parameters": {"bounds": [64.0, 8.0, 64.0]},
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"));
    assert_eq!(applied["outcome"], "applied", "{applied}");

    person_has_a_receipt(&mut app);
    let undone = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"dry_run": false}),
    )
    .unwrap_or_else(|e| panic!("the undo answers: {e}"));
    assert_eq!(undone["outcome"], "applied", "the undo did not apply: {undone}");

    let (note, _) = receipt(&mut app);
    assert!(
        note.starts_with("agent: undo"),
        "the agent's undo is not on the receipt, so the person still sees their own last edit. \
         receipt: {note:?}"
    );
}
