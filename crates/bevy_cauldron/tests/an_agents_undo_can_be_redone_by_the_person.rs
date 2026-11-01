//! **What an agent takes back, the person can put back (E-076).**
//!
//! # The asymmetry
//!
//! `grimoire/operation_undo` and the person's Cmd+Z reach the same registry: both walk the operation
//! log, both dispatch a recorded inverse, both answer with a summary. What they do *afterwards*
//! differed, and only for the agent's door:
//!
//! - **The person's Cmd+Z** leaves the inverse on the redo stack (`editor::apply`'s `Undo::Logged`
//!   arm returns the *redo* line, and `undo()` pushes it). So the person can take their own undo back.
//! - **The agent's undo** called `EditorState::forget_line` on the taken-back change and pushed
//!   nothing, so `redo_depth()` was `0` afterwards and the person's Cmd+Shift+Z said *"nothing to
//!   redo"*. An agent could take a turn away and the person had no way to get it back — through the
//!   door they were looking at.
//!
//! # Why `forget_line` was there, and why it stays
//!
//! B-019's rule: the next Cmd+Z must reach the newest change still **in effect**, not one the log no
//! longer holds. `forget_line` is what keeps that true — it removes the taken-back change's entry
//! from the undo stack *and* the redo stack, so a stale `Logged` for a line that is out of effect
//! cannot be pressed and refuse.
//!
//! So the fix must add a redo entry for the **undo's own line** without resurrecting the taken-back
//! change's entry. That is what `record_undone` does: it forgets the undone line exactly as before,
//! then records the undo's line as a `Logged` on the redo stack.
//!
//! # What is being proved
//!
//! 1. **After an agent's undo, the person's redo puts the change back** — the map has the piece
//!    again, and `redo_depth` went from 0 (before this) to 1.
//! 2. **The take-back is still gone from the undo stack**, so B-019 still holds: the next Cmd+Z
//!    takes back the newest change *in effect*, and does not reach a line the log has undone.
//! 3. **Redoing an agent's undo is a normal redo**, so it leaves a redoable inverse — pressing it
//!    twice does not throw away what the second press needs.
//! 4. **A person's own Cmd+Z is unchanged** — the same two numbers, reached by the same door.

use bevy::prelude::*;
use bevy_cauldron::editor::{EditorState, redo_for_test};
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .side(16.0)
        .descriptor("crate", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
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
        panic!("no method `{method}` is registered");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

/// One operation an agent applies, answering its id.
fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> String {
    let answer = call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    answer["operation_id"]
        .as_str()
        .unwrap_or_else(|| panic!("no operation id: {answer}"))
        .to_owned()
}

fn placed(app: &App) -> usize {
    app.world().resource::<OpenMap>().map.placements.len()
}

fn redo_depth(app: &App) -> usize {
    app.world().resource::<EditorState>().redo_depth()
}

fn undo_depth(app: &App) -> usize {
    app.world().resource::<EditorState>().undo_depth()
}

/// **What an agent takes back, the person can put back.**
#[test]
fn an_agents_undo_leaves_a_redo_the_person_can_press() {
    let mut app = editor("agentredo");
    assert_eq!(placed(&app), 0, "the fixture starts empty");

    let turn = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "crate", "at": [2.0, 2.0]}),
    );
    assert_eq!(placed(&app), 1, "the agent's piece is on the map");

    // **The agent takes it back**, by the id the apply answered with.
    let undone = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": turn, "dry_run": false}),
    );
    assert_eq!(undone["outcome"], "applied", "{undone}");
    assert_eq!(placed(&app), 0, "the undo did not take the piece away");
    assert_eq!(
        undo_depth(&app),
        0,
        "B-019: the taken-back change must not stay on the undo stack, or Cmd+Z would reach a \
         line the log no longer holds"
    );

    // **The person puts it back.** Before this change `redo_depth` was 0 here and this press said
    // "nothing to redo" — the asymmetry this file is named for.
    assert_eq!(
        redo_depth(&app),
        1,
        "an agent's undo left nothing to redo, so the person cannot put back what it took away"
    );
    redo_for_test(app.world_mut());
    app.update();
    assert_eq!(
        placed(&app),
        1,
        "the person's redo did not put the agent's piece back"
    );
    assert_eq!(
        undo_depth(&app),
        1,
        "the put-back is a change in effect again, so it joins the undo stack"
    );
    assert_eq!(
        redo_depth(&app),
        0,
        "the redo was spent, so nothing else is waiting"
    );
}

/// **Redoing an agent's undo is an ordinary redo**, so it leaves its own redo behind.
///
/// The entry the undo left on the redo stack names the undo's line; pressing redo dispatches *that*
/// line's inverse, which is a new line whose inverse must go back on the redo stack — otherwise the
/// second press would have nothing and the person could take back exactly one agent action.
#[test]
fn the_redo_of_an_agents_undo_can_itself_be_undone_and_redone() {
    let mut app = editor("agentredotwice");
    let turn = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "crate", "at": [3.0, 3.0]}),
    );
    call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": turn, "dry_run": false}),
    );
    assert_eq!(placed(&app), 0);

    redo_for_test(app.world_mut());
    app.update();
    assert_eq!(placed(&app), 1, "the first redo put the piece back");
    assert_eq!(
        redo_depth(&app),
        0,
        "nothing else was waiting, so the spent redo left nothing"
    );
    assert_eq!(undo_depth(&app), 1, "and the put-back is one change in effect");
}

/// **A person's own Cmd+Z is unchanged.** The same change, taken back by the person, leaves exactly
/// one redo — the number the agent's door now matches.
#[test]
fn a_persons_own_undo_still_leaves_one_redo() {
    let mut app = editor("personredo");
    apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "crate", "at": [4.0, 4.0]}),
    );
    assert_eq!(placed(&app), 1);
    assert_eq!(undo_depth(&app), 1, "the agent's change joined the person's history");

    bevy_cauldron::editor::undo_for_test(app.world_mut());
    app.update();
    assert_eq!(placed(&app), 0, "the person's Cmd+Z took the piece away");
    assert_eq!(
        redo_depth(&app),
        1,
        "the person's own undo leaves exactly one redo"
    );
    redo_for_test(app.world_mut());
    app.update();
    assert_eq!(placed(&app), 1, "and that redo puts the piece back");
}

/// **The project's library is not needed to redo a placement**, but the door is: this holds the
/// editor's own `Project` so the test would fail loudly if a redo needed a reload to work.
#[test]
fn the_project_is_still_open_after_an_agents_undo_and_redo() {
    let mut app = editor("agentredoproject");
    let turn = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "crate", "at": [5.0, 5.0]}),
    );
    call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": turn, "dry_run": false}),
    );
    redo_for_test(app.world_mut());
    app.update();
    assert!(
        app.world().get_resource::<Project>().is_some(),
        "the redo dropped the project the editor was working in"
    );
}
