//! **A label on one piece does not stop an undo of a label on another piece.**
//!
//! Cycle 9 gap 3. Undo's guard refused any newer logged change to the same map, and a library
//! change has no map, so two `label_set` calls on two different pieces refused each other: the
//! older one could not be taken back while the newer one stood, and the refusal said "changed the
//! map", which was not true. The guard now asks whether the newer change touched the same piece.
//! A second label on the same piece still blocks, and the refusal names that piece.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::Mode;

use crate::fixtures;

fn editor_at(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .descriptor("site/gate", "beta")
        .look_tokens(&["worn"])
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

/// **Through the registered method**, as an agent's request goes. An error answer is returned, not
/// panicked on, because the refusal text is what these tests read.
fn try_call(
    app: &mut App,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .map_err(|e| e.message)
}

fn agent_labels(app: &mut App, id: &str, tokens: &[&str]) -> serde_json::Value {
    try_call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "label_set",
            "parameters": {"id": id, "axis": "look", "tokens": tokens},
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("label_set {id}: {e}"))
}

fn undo(app: &mut App, operation_id: &str) -> Result<serde_json::Value, String> {
    try_call(
        app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": operation_id, "dry_run": false}),
    )
}

fn look_of(app: &App, id: &str) -> Vec<String> {
    app.world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == id)
        .unwrap_or_else(|| panic!("`{id}` is not in the live measured library"))
        .look
        .clone()
}

#[test]
fn an_older_label_on_one_piece_is_taken_back_while_a_newer_label_on_another_stands() {
    let mut app = editor_at("library-scope-other-piece");
    let wall_start = look_of(&app, "site/wall");

    let wall = agent_labels(&mut app, "site/wall", &["worn"]);
    assert_eq!(wall["outcome"], "applied", "{wall}");
    let gate = agent_labels(&mut app, "site/gate", &["worn"]);
    assert_eq!(gate["outcome"], "applied", "{gate}");

    let wall_op = wall["operation_id"].as_str().expect("the wall label has an id").to_owned();
    let answer = undo(&mut app, &wall_op).unwrap_or_else(|e| panic!("undo: {e}"));
    assert_eq!(
        answer["outcome"], "applied",
        "undoing the wall label was refused by the gate's label, which is another piece: {answer}"
    );
    assert_eq!(look_of(&app, "site/wall"), wall_start, "the wall's label went back to its start");
    assert_eq!(look_of(&app, "site/gate"), vec!["worn".to_string()], "the gate's label stands");
}

#[test]
fn an_older_label_on_a_piece_is_still_blocked_by_a_newer_label_on_that_same_piece() {
    let mut app = editor_at("library-scope-same-piece");

    let first = agent_labels(&mut app, "site/wall", &["worn"]);
    assert_eq!(first["outcome"], "applied", "{first}");
    let second = agent_labels(&mut app, "site/wall", &["plain"]);
    assert_eq!(second["outcome"], "applied", "{second}");

    let first_op = first["operation_id"].as_str().expect("the first label has an id").to_owned();
    let answer = undo(&mut app, &first_op).unwrap_or_else(|e| panic!("undo: {e}"));
    assert_eq!(answer["outcome"], "refused", "the newer label on the same piece blocks: {answer}");
    let refusal = answer["refusal"].as_str().unwrap_or_default();
    assert!(
        refusal.contains("changed `site/wall`"),
        "the refusal names the piece that the newer change touched: {refusal}"
    );
}
