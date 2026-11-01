//! **An agent can remove a stamp, whole, and take the removal back, through the same log the author's
//! undo uses.**
//!
//! The regression tests for the operation behind the stamp removal: `stamp_remove`, which takes one
//! stamp off `map.stamps` by its id. X's click and box removal reach it through the registry, so
//! these tests are the registry's contract for it. They go through the registered BRP methods, the
//! way the MCP server does, and assert on the map the editor holds, the file on disk, and the undo.

use bevy::prelude::*;
use bevy_cauldron::editor::plant_unstamp_for_test;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;
use fixtures::Fixture;

/// A map holding one stamp, `room@1`, of the composition `site/room`, and one free-standing piece,
/// `site/wall@0`. The fixture's map carries no stamps, so the stamp is written into its text the way
/// `a_composition_can_be_renamed` does it.
fn editor(name: &str) -> App {
    let root = Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .composition("site/room", &[("site/wall", "site/wall", (0.0, 0.0))])
        .place("site/wall", (3.0, 0.0))
        .build("m");
    let map = root.join("assets/grimoire/maps/m.map.ron");
    let text = std::fs::read_to_string(&map).expect("the map is there");
    let stamped = text.replace(
        "stamps: [],",
        r#"stamps: [
        (
            id: "room@1",
            of: "site/room",
            at: (0.0, 0.0),
            yaw: 0.0,
            overrides: [],
            of_fingerprint: None,
            owned: false,
            owned_because: None,
            note: None,
        ),
    ],"#,
    );
    assert_ne!(
        stamped, text,
        "the fixture's map has no `stamps: [],` to fill; this test would stamp nothing"
    );
    std::fs::write(&map, stamped).expect("the map is writable");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
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

fn remove(app: &mut App, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "stamp_remove", "parameters": parameters, "dry_run": false}),
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

fn stamps(map: &bevy_grimoire_core::map::Map) -> Vec<String> {
    map.stamps.iter().map(|s| s.id.clone()).collect()
}

fn placements(map: &bevy_grimoire_core::map::Map) -> Vec<String> {
    map.placements.iter().map(|p| p.id.clone()).collect()
}

#[test]
fn an_agent_removes_a_whole_stamp_and_the_undo_puts_it_back() {
    let mut app = editor("stamp_remove_and_back");
    assert_eq!(
        stamps(&held(&app)),
        ["room@1"],
        "the fixture starts with one stamp"
    );
    assert_eq!(
        placements(&held(&app)),
        ["site/wall@0"],
        "and one free piece"
    );

    let answer = remove(&mut app, serde_json::json!({"id": "room@1"}));
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let operation_id = answer["operation_id"]
        .as_str()
        .expect("an apply returns its id")
        .to_owned();

    assert!(
        held(&app).stamps.is_empty(),
        "the editor no longer holds the stamp"
    );
    assert!(
        on_disk(&app).stamps.is_empty(),
        "the file no longer holds the stamp"
    );
    assert_eq!(
        placements(&on_disk(&app)),
        ["site/wall@0"],
        "a stamp's removal leaves the free piece alone"
    );

    let back = undo(&mut app, Some(&operation_id));
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(
        stamps(&held(&app)),
        ["room@1"],
        "undo puts the stamp back in the editor"
    );
    assert_eq!(
        stamps(&on_disk(&app)),
        ["room@1"],
        "undo puts the stamp back in the file"
    );
    assert_eq!(
        on_disk(&app).stamps[0].of,
        "site/room",
        "and it still names its composition"
    );
}

#[test]
fn an_unknown_stamp_is_refused_and_nothing_changes() {
    let mut app = editor("stamp_remove_unknown");
    let answer = remove(&mut app, serde_json::json!({"id": "no-such-stamp"}));
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert_eq!(
        stamps(&held(&app)),
        ["room@1"],
        "a refused removal changes nothing"
    );
    assert_eq!(
        stamps(&on_disk(&app)),
        ["room@1"],
        "a refused removal writes nothing"
    );
}

#[test]
fn an_unknown_field_is_refused() {
    let mut app = editor("stamp_remove_field");
    let answer = remove(&mut app, serde_json::json!({"id": "room@1", "force": true}));
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert_eq!(
        stamps(&held(&app)),
        ["room@1"],
        "a refused removal changes nothing"
    );
}

#[test]
fn a_put_back_with_no_logged_removal_behind_it_is_refused() {
    // **An undo restores only what a removal in the log took.** A put-back the log did not record
    // would be a second way to add a stamp, so the registry refuses it.
    let mut app = editor("stamp_remove_put_back");
    let stamp = serde_json::to_value(&held(&app).stamps[0]).expect("a stamp serialises");
    let answer = remove(
        &mut app,
        serde_json::json!({"put_back": {"index": 0, "stamp": stamp}}),
    );
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert_eq!(
        stamps(&held(&app)),
        ["room@1"],
        "nothing was put back, and nothing was doubled"
    );
}

/// **An agent's stamp undo waits while a stamp change recorded by place sits above it.** The undo of a
/// `stamp_remove` puts the stamp back at its index, which moves every stamp after it. A person's
/// `UnstampAt` above the agent's entry names a stamp by that index, so it would take the wrong one.
/// Gap 4 of the cycle-1 review: the refusal is the same rule the placement side already has.
#[test]
fn an_agent_stamp_undo_waits_while_a_stamp_change_by_place_sits_above_it() {
    let mut app = editor("stamp_remove_waits");
    let answer = remove(&mut app, serde_json::json!({"id": "room@1"}));
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let operation_id = answer["operation_id"]
        .as_str()
        .expect("an apply returns its id")
        .to_owned();

    plant_unstamp_for_test(app.world_mut(), 0);
    let back = undo(&mut app, Some(&operation_id));
    assert_eq!(back["outcome"], "refused", "{back}");
    assert!(
        back.to_string().contains("Cmd+Z"),
        "the refusal names the way out: {back}"
    );
    assert!(
        stamps(&held(&app)).is_empty(),
        "a refused undo leaves the stamp off the map"
    );
    assert!(stamps(&on_disk(&app)).is_empty(), "and off the file too");
}

#[test]
fn a_stamp_put_back_is_refused_as_an_apply_and_only_its_undo_brings_the_stamp_back() {
    // **A put-back is the undo of the removal that took its stamp, and nothing else.** The removal is on
    // the log, so its inverse is there to match; an apply that sends the same put-back must still be
    // refused, in preview and in apply, and the undo of that removal must still work.
    let mut app = editor("stamp_remove_put_back_apply");
    let stamp = serde_json::to_value(&held(&app).stamps[0]).expect("a stamp serialises");
    let removed = remove(&mut app, serde_json::json!({"id": "room@1"}));
    assert_eq!(removed["outcome"], "applied", "{removed}");
    let operation_id = removed["operation_id"]
        .as_str()
        .expect("an apply returns its id")
        .to_owned();

    let put_back = serde_json::json!({"put_back": {"index": 0, "stamp": stamp}});
    let preview = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "stamp_remove", "parameters": put_back.clone(), "dry_run": true}),
    );
    assert_eq!(
        preview["outcome"], "refused",
        "a preview refuses what the apply refuses: {preview}"
    );
    let answer = remove(&mut app, put_back);
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert!(
        stamps(&held(&app)).is_empty(),
        "a put-back that is not an undo puts nothing back"
    );
    assert!(stamps(&on_disk(&app)).is_empty(), "and the file agrees");

    let back = undo(&mut app, Some(&operation_id));
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(
        stamps(&held(&app)),
        ["room@1"],
        "the undo of that removal still puts the stamp back"
    );
}
