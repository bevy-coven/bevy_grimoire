//! **An agent can place a piece (S2.1, `placement_add`).**
//!
//! No operation added a row: an agent could turn a piece, pin it, or remove it, and could not put one
//! down. The acceptance script's `place` step failed on exactly this, and it is the first thing any
//! request needs.
//!
//! # What is being proved
//!
//! 1. **It is in the registry**, so `grimoire_list_operations` names it.
//! 2. **A placed piece is on disk**, so "placed" means saved and not remembered.
//! 3. **A piece on a surface takes its host**, and the row records which — the mount class's whole
//!    point, and `Placed::on` is where it is written.
//! 4. **Undo takes the piece away.**
//! 5. **Placing the same piece on itself is refused as a note, not a problem**, because clicking a
//!    cell that already holds that exact piece is an answer.
//! 6. **An unknown piece is refused by name**, with the route to the list.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

/// **A project with two pieces and a saved map.**
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("crate", "alpha")
        .descriptor("barrel", "alpha")
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

/// **1. The operation is listed.**
#[test]
fn placement_add_is_listed() {
    let (_root, mut app) = ready("placelist");
    let listed = call(&mut app, "grimoire/operation_list", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("{e}"));
    let names: Vec<&str> = listed["operations"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|o| o.get("operation").and_then(|n| n.as_str()))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        names.contains(&"placement_add"),
        "an agent cannot put a piece down, so every request that is not a fill stops. The registry \
         holds: {names:?}"
    );
}

/// **2. An agent places a piece and it is saved.**
#[test]
fn an_agent_places_a_piece_and_it_is_saved() {
    let (_root, mut app) = ready("placeagent");
    assert_eq!(
        app.world().resource::<OpenMap>().map.placements.len(),
        0,
        "the fixture must start empty"
    );

    let applied = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "crate", "at": [1.5, 1.5]}),
    );
    assert_eq!(applied["outcome"], "applied", "the place did not apply: {applied}");
    assert_eq!(applied["saved"], true, "the place was not saved: {applied}");
    let id = applied["log_entry"]["after"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("the place did not name the row it made: {applied}"))
        .to_owned();
    assert!(
        id.starts_with("crate@"),
        "the id is not the piece's short name and a counter: {id}"
    );

    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
    let on_disk: bevy_grimoire_core::map::Map =
        bevy_grimoire_core::map::Map::parse(&text).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        on_disk.placements.len(),
        1,
        "the map holds the piece and the file does not"
    );
    assert_eq!(on_disk.placements[0].id, id, "the file holds a different row");
}

/// **3. Removing then placing the same piece again is one act each, and undo is exact.**
#[test]
fn undo_takes_the_piece_away() {
    let (_root, mut app) = ready("placeundo");
    apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "crate", "at": [2.5, 2.5]}),
    );
    assert_eq!(app.world().resource::<OpenMap>().map.placements.len(), 1);

    let undone = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"dry_run": false}),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(undone["outcome"], "applied", "the undo did not apply: {undone}");
    assert_eq!(
        app.world().resource::<OpenMap>().map.placements.len(),
        0,
        "one undo did not take the placement away"
    );
}

/// **4. Placing the same piece on itself is refused as a note, not a problem.**
///
/// The click path's own distinction: clicking a cell that already holds this exact piece is an
/// answer — nothing went wrong — while a missing host is a refusal the author must act on.
#[test]
fn placing_the_same_piece_on_itself_is_refused() {
    let (_root, mut app) = ready("placesame");
    apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "crate", "at": [3.5, 3.5]}),
    );
    let again = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "crate", "at": [3.5, 3.5]}),
    );
    assert_eq!(
        again["outcome"], "refused",
        "placing this exact piece in the cell it already fills was applied again: {again}"
    );
    let why = again["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains("already there"),
        "the refusal does not say the piece is already there: {why:?}"
    );
    assert_eq!(
        app.world().resource::<OpenMap>().map.placements.len(),
        1,
        "the second place added a row"
    );
}

/// **5. An unknown piece is refused by name**, with the route to the list.
#[test]
fn an_unknown_piece_is_refused_by_name() {
    let (_root, mut app) = ready("placebad");
    let refused = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "statue", "at": [1.5, 1.5]}),
    );
    assert_eq!(refused["outcome"], "refused", "{refused}");
    let why = refused["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains("statue") && why.contains("grimoire_list_pieces"),
        "the refusal neither names the piece nor says where the real ones are: {why:?}"
    );
    assert_eq!(
        app.world().resource::<OpenMap>().map.placements.len(),
        0,
        "a refused place added a row"
    );
}
