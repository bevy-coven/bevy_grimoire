//! **An agent can remove a placement, and take the removal back, through the same log the author's undo uses.**
//!
//! The regression tests for the operation behind the removal click: a single placement, through
//! `placement_remove`. X's stamp and box removal dispatch through the registry too (E-029).
//! These go through the registered BRP methods, the way the MCP server does, and assert on the map
//! the editor holds, the file on disk, and the log.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

fn editor(name: &str, build: impl FnOnce(fixtures::Fixture) -> fixtures::Fixture) -> App {
    let root = build(
        fixtures::Fixture::new(name)
            .descriptor("site/wall", "alpha"),
    )
    .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

/// Two free-standing pieces, `site/wall@0` and `site/wall@1`.
fn two_pieces(name: &str) -> App {
    editor(name, |f| f.place("site/wall", (0.0, 0.0)).place("site/wall", (3.0, 0.0)))
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

fn remove(app: &mut App, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "placement_remove", "parameters": parameters, "dry_run": false}),
    )
}

fn undo(app: &mut App, id: Option<&str>) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": id, "dry_run": false}),
    )
}

fn held(app: &App) -> bevy_grimoire_core::map::Map {
    app.world().resource::<OpenMap>().map.clone()
}

fn on_disk(app: &App) -> bevy_grimoire_core::map::Map {
    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    bevy_grimoire_core::map::Map::parse(&text).unwrap_or_else(|e| panic!("{e}"))
}

fn ids(map: &bevy_grimoire_core::map::Map) -> Vec<String> {
    map.placements.iter().map(|p| p.id.clone()).collect()
}

#[test]
fn an_agent_removes_a_placement_and_the_undo_puts_it_back() {
    let mut app = two_pieces("remove_and_back");
    assert_eq!(ids(&held(&app)), ["site/wall@0", "site/wall@1"], "the fixture starts with two");

    let answer = remove(&mut app, serde_json::json!({"id": "site/wall@0"}));
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let operation_id = answer["operation_id"]
        .as_str()
        .expect("an apply returns its id")
        .to_owned();

    assert_eq!(ids(&held(&app)), ["site/wall@1"], "the editor no longer holds the piece");
    assert_eq!(ids(&on_disk(&app)), ["site/wall@1"], "the file no longer holds the piece");

    let back = undo(&mut app, Some(&operation_id));
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@0", "site/wall@1"],
        "undo puts the piece back in the editor, at the row it held"
    );
    assert_eq!(
        ids(&on_disk(&app)),
        ["site/wall@0", "site/wall@1"],
        "undo puts the piece back in the file, at the row it held"
    );
}

#[test]
fn removing_a_host_takes_the_piece_standing_on_it_and_the_undo_restores_both() {
    let mut app = editor("remove_stack", |f| {
        f.place("site/wall", (0.0, 0.0))
            .place_on("site/wall", "site/wall@0", (0.0, 0.0))
            .place("site/wall", (3.0, 0.0))
    });
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@0", "site/wall@1", "site/wall@2"],
        "the fixture starts with a host, a rider on it, and a piece apart"
    );

    let answer = remove(&mut app, serde_json::json!({"id": "site/wall@0"}));
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let operation_id = answer["operation_id"]
        .as_str()
        .expect("an apply returns its id")
        .to_owned();
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@2"],
        "a stack goes whole: the rider goes with its host, and the piece apart stays"
    );
    assert_eq!(ids(&on_disk(&app)), ["site/wall@2"], "the file agrees");

    let back = undo(&mut app, Some(&operation_id));
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@0", "site/wall@1", "site/wall@2"],
        "undo restores the host and its rider at the rows they held"
    );
    assert_eq!(
        held(&app).placements[1].on.as_deref(),
        Some("site/wall@0"),
        "the rider is still on its host after the undo"
    );
}

#[test]
fn an_unknown_placement_is_refused_and_nothing_changes() {
    let mut app = two_pieces("remove_unknown");
    let answer = remove(&mut app, serde_json::json!({"id": "no-such-piece"}));
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert_eq!(ids(&held(&app)), ["site/wall@0", "site/wall@1"], "a refused removal changes nothing");
    assert_eq!(ids(&on_disk(&app)), ["site/wall@0", "site/wall@1"], "a refused removal writes nothing");
}

#[test]
fn an_unknown_field_is_refused() {
    let mut app = two_pieces("remove_field");
    let answer = remove(
        &mut app,
        serde_json::json!({"id": "site/wall@0", "force": true}),
    );
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert_eq!(ids(&held(&app)), ["site/wall@0", "site/wall@1"], "a refused removal changes nothing");
}

#[test]
fn a_put_back_with_no_logged_removal_behind_it_is_refused() {
    // **An undo restores only what a removal in the log took.** A put-back the log did not record
    // would be a second way to add a piece, so the registry refuses it rather than trusting the
    // caller's word that something was removed.
    let mut app = two_pieces("remove_put_back");
    let placed = serde_json::to_value(&held(&app).placements[0]).expect("a placement serialises");
    let answer = remove(
        &mut app,
        serde_json::json!({"put_back": {"head": "site/wall@0", "rows": [{"index": 0, "placed": placed}]}}),
    );
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert_eq!(ids(&held(&app)), ["site/wall@0", "site/wall@1"], "nothing was put back");
}

#[test]
fn an_agent_cannot_put_back_a_piece_the_person_has_removed_again() {
    // **A put-back is the undo of the removal that took its pieces, and nothing else.** The log still
    // holds the first removal's inverse after the piece is undone and removed again, so a put-back
    // applied on its own used to match that line and bring the piece back, skipping every undo check.
    let mut app = two_pieces("remove_put_back_again");
    let placed = serde_json::to_value(&held(&app).placements[0]).expect("a placement serialises");

    let first = remove(&mut app, serde_json::json!({"id": "site/wall@0"}));
    assert_eq!(first["outcome"], "applied", "{first}");
    let first_id = first["operation_id"]
        .as_str()
        .expect("an apply returns its id")
        .to_owned();
    let back = undo(&mut app, Some(&first_id));
    assert_eq!(back["outcome"], "applied", "{back}");
    let again = remove(&mut app, serde_json::json!({"id": "site/wall@0"}));
    assert_eq!(again["outcome"], "applied", "{again}");
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@1"],
        "the person's second removal stands"
    );

    let put_back = serde_json::json!({
        "put_back": {"head": "site/wall@0", "rows": [{"index": 0, "placed": placed}]}
    });
    let preview = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "placement_remove", "parameters": put_back.clone(), "dry_run": true}),
    );
    assert_eq!(
        preview["outcome"], "refused",
        "a preview refuses what the apply refuses: {preview}"
    );
    let answer = remove(&mut app, put_back);
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@1"],
        "a put-back that is not an undo puts nothing back"
    );
    assert_eq!(ids(&on_disk(&app)), ["site/wall@1"], "and the file agrees");
}
