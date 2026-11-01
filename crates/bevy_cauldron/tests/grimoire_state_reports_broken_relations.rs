//! **`grimoire/state` says which intended relations the open map breaks** (#137 slice 2).
//!
//! Slice 1 put `relations::check` in core, but an agent placing pieces reads `grimoire_state`, not a
//! library function. So the state answer carries a `relations` section read from the
//! `<map>.relations.ron` beside the open map: every relation with its outcome and what was
//! measured, and counts of the broken and dangling ones. The file is read on each call, so an edit
//! to it is seen by the next `grimoire_state` with no restart, and a file that does not parse is
//! named, never read as "no relations".

use bevy::prelude::*;
use bevy_cauldron::harness;
use std::path::{Path, PathBuf};

use crate::fixtures;

/// Every placement faces +z (yaw 0). The chair at (2, 2) has its table behind it at (2, 0); the
/// crate at (8, 8.5) stands in the door's swing at (8, 8).
fn editor(name: &str) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/chair", "alpha")
        .descriptor("site/table", "alpha")
        .descriptor("site/door", "alpha")
        .descriptor("site/crate", "alpha")
        .place_as("chair", "site/chair", (2.0, 2.0))
        .place_as("table", "site/table", (2.0, 0.0))
        .place_as("door", "site/door", (8.0, 8.0))
        .place_as("crate", "site/crate", (8.0, 8.5))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    (app, root.join("assets/grimoire/maps/m.relations.ron"))
}

fn state(app: &mut App) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get("grimoire/state").cloned()
    else {
        panic!("no method `grimoire/state` is registered");
    };
    app.world_mut()
        .run_system_with(id, Some(serde_json::json!({})))
        .unwrap_or_else(|e| panic!("grimoire/state: {e:?}"))
        .unwrap_or_else(|e| panic!("grimoire/state: {e:?}"))
}

fn write(path: &Path, relations: &str) {
    std::fs::write(path, format!("(version: 1, relations: [{relations}])"))
        .unwrap_or_else(|e| panic!("{path:?}: {e}"));
}

const FOUR: &str = r#"
    (of: "chair", kind: Faces("table")),
    (of: "chair", kind: Near(to: "table", within: 3.0)),
    (of: "door", kind: ClearInFront(depth: 2.0, width: 1.0)),
    (of: "lamp", kind: Near(to: "door", within: 2.0)),
"#;

#[test]
fn the_chair_turned_away_and_the_blocked_door_are_named() {
    let (mut app, file) = editor(&format!("relations_named_{}", std::process::id()));
    write(&file, FOUR);
    let s = state(&mut app);
    let rel = &s["relations"];
    assert_eq!(rel["error"], serde_json::Value::Null, "{rel}");
    assert_eq!(rel["unmet"], 2, "the chair's facing and the door's swing: {rel}");
    assert_eq!(rel["missing"], 1, "the lamp is not on the map: {rel}");
    let rows = rel["relations"].as_array().unwrap_or_else(|| panic!("{rel}"));
    assert_eq!(rows.len(), 4, "{rel}");

    assert_eq!(rows[0]["of"], "chair");
    assert_eq!(rows[0]["outcome"], "unmet", "{}", rows[0]);
    let off = rows[0]["measured"].as_f64().unwrap_or_else(|| panic!("{}", rows[0]));
    assert!((off - 180.0).abs() < 0.5, "the table is dead behind: {off}°");

    assert_eq!(rows[1]["outcome"], "met", "{}", rows[1]);
    assert!((rows[1]["measured"].as_f64().unwrap_or(-1.0) - 2.0).abs() < 1e-3, "{}", rows[1]);

    assert_eq!(rows[2]["outcome"], "unmet", "{}", rows[2]);
    assert_eq!(rows[2]["by"], "crate", "the crate is in the doorway: {}", rows[2]);

    assert_eq!(rows[3]["outcome"], "missing", "{}", rows[3]);
    assert_eq!(rows[3]["missing_id"], "lamp", "{}", rows[3]);
}

#[test]
fn an_edit_to_the_file_is_seen_by_the_next_call() {
    let (mut app, file) = editor(&format!("relations_reload_{}", std::process::id()));
    assert_eq!(
        state(&mut app)["relations"],
        serde_json::json!({"relations": [], "unmet": 0, "missing": 0, "error": null}),
        "a map with no relations file has none, and that is no error"
    );
    write(&file, FOUR);
    assert_eq!(state(&mut app)["relations"]["unmet"], 2);
    write(&file, r#"(of: "chair", kind: Near(to: "table", within: 3.0)),"#);
    assert_eq!(state(&mut app)["relations"]["unmet"], 0);
}

#[test]
fn a_file_that_does_not_parse_is_named_not_read_as_none() {
    let (mut app, file) = editor(&format!("relations_bad_{}", std::process::id()));
    write(&file, r#"(of: "door", kind: ClearInFront(depth: -2.0, width: 1.0)),"#);
    let rel = &state(&mut app)["relations"];
    let error = rel["error"].as_str().unwrap_or_else(|| panic!("no error named: {rel}"));
    assert!(error.contains("m.relations.ron"), "the error names the file: {error}");
    assert!(error.contains("-2"), "and what is wrong in it: {error}");
    assert_eq!(rel["relations"], serde_json::json!([]), "{rel}");
}
