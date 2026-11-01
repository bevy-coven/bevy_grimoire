//! **Relations read a map's yaw in degrees** (#165).
//!
//! A map stores `yaw` in degrees (the `place` schema, the spawner's `to_radians`), and the relation
//! layer measures headings. So a lamp in front of a door turned to face -z (`yaw: 180.0`) is in
//! front of it in `grimoire_state.relations`, and `relations_solve` turning a chair to face a table
//! writes degrees into the row, leaving every row it did not move bit for bit.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;
use std::path::PathBuf;

use crate::fixtures;

/// The door at the origin, the lamp at (0, -2): in front of the door once the door faces -z. The
/// chair at (10, 0) and the table at (14, 0), on the chair's +x.
fn editor(name: &str) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/door", "alpha")
        .descriptor("site/lamp", "alpha")
        .descriptor("site/chair", "alpha")
        .descriptor("site/table", "alpha")
        .place_as("door", "site/door", (0.0, 0.0))
        .place_as("lamp", "site/lamp", (0.0, -2.0))
        .place_as("chair", "site/chair", (10.0, 0.0))
        .place_as("table", "site/table", (14.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    turn(&mut app, "door", 180.0);
    (app, root.join("assets/grimoire/maps/m.relations.ron"))
}

/// Set a row's yaw, degrees, as the map stores it.
fn turn(app: &mut App, id: &str, yaw: f32) {
    let mut open = app.world_mut().resource_mut::<OpenMap>();
    let row = open.map.placements.iter_mut().find(|p| p.id == id).unwrap_or_else(|| panic!("{id}"));
    row.yaw = yaw;
}

fn yaw(app: &App, id: &str) -> f32 {
    let open = app.world().resource::<OpenMap>();
    open.map.placements.iter().find(|p| p.id == id).unwrap_or_else(|| panic!("{id}")).yaw
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

fn relations(app: &mut App) -> serde_json::Value {
    call(app, "grimoire/state", serde_json::json!({}))["relations"].clone()
}

fn write(path: &std::path::Path, relations: &str) {
    std::fs::write(path, format!("(version: 1, relations: [{relations}])"))
        .unwrap_or_else(|e| panic!("{path:?}: {e}"));
}

#[test]
fn a_lamp_in_front_of_a_door_turned_180_degrees_is_in_front_of_it() {
    let (mut app, file) = editor(&format!("relations_degrees_front_{}", std::process::id()));
    write(&file, r#"(of: "lamp", kind: InFrontOf(of: "door", within: 3.0))"#);
    let r = relations(&mut app);
    assert_eq!(r["unmet"], 0, "the door faces -z, so the lamp at -z is in front: {r}");
    // Behind the same door is +z.
    write(&file, r#"(of: "lamp", kind: Behind(of: "door", within: 3.0))"#);
    let r = relations(&mut app);
    assert_eq!(r["unmet"], 1, "the lamp is not behind a door that faces it: {r}");
}

#[test]
fn solving_faces_writes_degrees_and_leaves_other_turned_rows_alone() {
    let (mut app, file) = editor(&format!("relations_degrees_solve_{}", std::process::id()));
    turn(&mut app, "lamp", 33.3);
    write(&file, r#"(of: "chair", kind: Faces("table"))"#);
    assert_eq!(relations(&mut app)["unmet"], 1);
    let solved = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "relations_solve", "parameters": {}, "dry_run": false}),
    );
    assert_eq!(solved["outcome"], "applied", "{solved}");
    assert!((yaw(&app, "chair") - 90.0).abs() < 1e-3, "the table is on +x, 90 degrees: {}", yaw(&app, "chair"));
    assert_eq!(relations(&mut app)["unmet"], 0, "{solved}");
    assert_eq!(yaw(&app, "door"), 180.0, "a turned row the solve did not move keeps its yaw exactly");
    assert_eq!(yaw(&app, "lamp"), 33.3, "a turned row the solve did not move keeps its yaw exactly");
}
