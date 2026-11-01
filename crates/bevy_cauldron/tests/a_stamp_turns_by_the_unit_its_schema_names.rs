//! **An agent that turns a stamp by its schema gets the turn it asked for** (#189).
//!
//! `stamp_add`'s schema is all an agent reads before it calls, so the unit the schema names for
//! `yaw` is the unit the stamp must turn by. The test reads that unit from `grimoire/operation_list`,
//! asks for a quarter turn in it, expands the map's stamps, and checks every member row against the
//! same composition stamped unturned: its offset from the anchor is turned 90° about +Y and its own
//! yaw is 90° more. A schema that says radians while the map stores degrees turns the stamp 1.57°.

use bevy::prelude::*;
use bevy_grimoire_core::composition::{self, Stamped};
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};

use crate::fixtures;

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

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    )
}

/// An editor with `corner` defined from a crate and a barrel a metre apart.
fn with_group(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .sized_descriptor("crate", "alpha", 1.0, 1.0)
        .sized_descriptor("barrel", "alpha", 1.0, 1.0)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    let mut ids = vec![];
    for (piece, at) in [("crate", [1.5, 1.5]), ("barrel", [2.5, 1.5])] {
        let a = apply(&mut app, "placement_add", serde_json::json!({"piece": piece, "at": at}));
        ids.push(a["log_entry"]["after"]["id"].as_str().expect("a row").to_owned());
    }
    let d = apply(&mut app, "composition_define", serde_json::json!({"id": "corner", "from": ids}));
    assert_eq!(d["outcome"], "applied", "{d}");
    app
}

/// A quarter turn in the unit `stamp_add`'s published schema names for `yaw`.
fn quarter_turn_by_the_schema(app: &mut App) -> f32 {
    let listed = call(app, "grimoire/operation_list", serde_json::json!({}));
    let op = listed["operations"]
        .as_array()
        .and_then(|a| a.iter().find(|o| o["operation"] == "stamp_add"))
        .unwrap_or_else(|| panic!("stamp_add is listed: {listed}"));
    let said = op["parameters"]["properties"]["yaw"]["description"]
        .as_str()
        .unwrap_or_else(|| panic!("yaw is described: {op}"))
        .to_lowercase();
    match (said.contains("degree"), said.contains("radian")) {
        (true, false) => 90.0,
        (false, true) => std::f32::consts::FRAC_PI_2,
        _ => panic!("the schema names exactly one unit for yaw, degrees or radians: {said:?}"),
    }
}

fn same_turn(a: f32, b: f32) -> bool {
    let d = (a - b).rem_euclid(360.0);
    d < 1e-3 || 360.0 - d < 1e-3
}

#[test]
fn a_quarter_turn_in_the_schemas_unit_turns_every_member_a_quarter_turn() {
    let mut app = with_group(&format!("stamp_yaw_unit_{}", std::process::id()));
    let quarter = quarter_turn_by_the_schema(&mut app);
    let anchor = (6.5_f32, 6.5_f32);
    let a = apply(
        &mut app,
        "stamp_add",
        serde_json::json!({"of": "corner", "at": [anchor.0, anchor.1], "yaw": quarter, "id": "turned"}),
    );
    assert_eq!(a["outcome"], "applied", "{a}");

    let map = app.world().resource::<OpenMap>().map.clone();
    let project = app.world().resource::<Project>();
    let comps = &project.compositions.compositions;
    let turned = composition::expand_map(&map, comps, &project.library)
        .unwrap_or_else(|e| panic!("the stamped map expands: {e}"));
    // The same composition at the same anchor, unturned: each member's offset and yaw as authored.
    let flat_stamp = Stamped { id: "flat".into(), of: "corner".into(), at: anchor, yaw: 0.0, ..Default::default() };
    let mut trial = map.clone();
    trial.stamps.retain(|s| s.id != "turned");
    let flat = composition::expand(&trial, &[flat_stamp], comps, &project.library)
        .unwrap_or_else(|e| panic!("the unturned stamp expands: {e}"));

    let mut checked = 0;
    for row in &flat.placements {
        let (_, path) = &flat.from[&row.id];
        let twin = turned
            .placements
            .iter()
            .find(|r| turned.from.get(&r.id) == Some(&("turned".to_owned(), path.clone())))
            .unwrap_or_else(|| panic!("member `{path}` of the turned stamp is on the map"));
        let local = (row.at.0 - anchor.0, row.at.1 - anchor.1);
        let off = composition::rotate_xz(local, 90.0);
        let want = (anchor.0 + off.0, anchor.1 + off.1);
        assert!(
            (twin.at.0 - want.0).abs() < 1e-4 && (twin.at.1 - want.1).abs() < 1e-4,
            "member `{path}` sent a quarter turn ({quarter}) lands at {:?}, and a quarter turn about \
             {anchor:?} puts it at {want:?}",
            twin.at
        );
        let want_yaw = composition::add_yaw(row.yaw, 90.0);
        assert!(
            same_turn(twin.yaw, want_yaw),
            "member `{path}` is turned {}°, and a quarter turn makes it {want_yaw}°",
            twin.yaw
        );
        checked += 1;
    }
    assert_eq!(checked, 2, "both members of `corner` are checked");
}
