//! **`relations_solve` mends the open map's broken relations as one undoable operation** (#137 slice 4).
//!
//! Slice 3 put `relations::solve` in core; an agent reaches the editor only through the registry. So
//! the operation runs the solver on the open map against its `<map>.relations.ron`, writes the moved
//! rows to the map, and one undo puts every moved row back. A preview writes nothing, and a map with
//! no relations file is refused, not "solved".

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;
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

fn run(app: &mut App, dry_run: bool) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "relations_solve", "parameters": {}, "dry_run": dry_run}),
    )
}

fn undo(app: &mut App, id: &str) -> serde_json::Value {
    call(app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}))
}

fn unmet(app: &mut App) -> u64 {
    let s = call(app, "grimoire/state", serde_json::json!({}));
    s["relations"]["unmet"].as_u64().unwrap_or_else(|| panic!("{}", s["relations"]))
}

/// The open map's rows as (id, x, z, yaw), in file order.
fn rows(app: &App) -> Vec<(String, f32, f32, f32)> {
    app.world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .map(|p| (p.id.clone(), p.at.0, p.at.1, p.yaw))
        .collect()
}

/// The map file as saved, re-read from disk.
fn saved(app: &App) -> String {
    let path = app.world().resource::<OpenMap>().map_path.clone();
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"))
}

fn write(path: &Path, relations: &str) {
    std::fs::write(path, format!("(version: 1, relations: [{relations}])"))
        .unwrap_or_else(|e| panic!("{path:?}: {e}"));
}

const BROKEN: &str = r#"
    (of: "chair", kind: Faces("table")),
    (of: "door", kind: ClearInFront(depth: 2.0, width: 1.0)),
"#;

#[test]
fn solving_mends_both_and_one_undo_puts_every_row_back() {
    let (mut app, file) = editor(&format!("relations_solve_undo_{}", std::process::id()));
    write(&file, BROKEN);
    assert_eq!(unmet(&mut app), 2);
    let before = rows(&app);
    let before_file = saved(&app);

    let solved = run(&mut app, false);
    assert_eq!(solved["outcome"], "applied", "{solved}");
    assert_eq!(unmet(&mut app), 0, "both relations hold after the solve: {solved}");
    let after = rows(&app);
    assert_ne!(after, before, "pieces moved");
    assert_ne!(saved(&app), before_file, "the moved rows are written to the map file");
    // The table and the door are the relations' targets, never moved for them.
    for id in ["table", "door"] {
        let was = before.iter().find(|r| r.0 == id).unwrap();
        let now = after.iter().find(|r| r.0 == id).unwrap();
        assert_eq!(was, now, "{id} is a target and stays put");
    }

    let id = solved["operation_id"].as_str().expect("an id").to_owned();
    let back = undo(&mut app, &id);
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(rows(&app), before, "one undo puts every moved row back, in order");
    assert_eq!(unmet(&mut app), 2);

    let redo_id = back["operation_id"].as_str().expect("the undo has an id").to_owned();
    let again = undo(&mut app, &redo_id);
    assert_eq!(again["outcome"], "applied", "{again}");
    assert_eq!(rows(&app), after, "redo moves them again");
}

#[test]
fn a_preview_says_what_would_move_and_writes_nothing() {
    let (mut app, file) = editor(&format!("relations_solve_preview_{}", std::process::id()));
    write(&file, BROKEN);
    let before = rows(&app);
    let before_file = saved(&app);
    let preview = run(&mut app, true);
    assert_ne!(preview["outcome"], "refused", "{preview}");
    let text = preview.to_string();
    assert!(text.contains("chair") && text.contains("crate"), "the preview names what would move: {text}");
    assert_eq!(rows(&app), before);
    assert_eq!(saved(&app), before_file, "a preview writes nothing");
    assert_eq!(unmet(&mut app), 2);
}

#[test]
fn no_relations_file_and_a_bad_one_are_refused() {
    let (mut app, file) = editor(&format!("relations_solve_refused_{}", std::process::id()));
    let before = rows(&app);
    let none = run(&mut app, false);
    assert_eq!(none["outcome"], "refused", "{none}");
    assert!(none.to_string().contains("m.relations.ron"), "the refusal names the file it looked for: {none}");

    write(&file, r#"(of: "door", kind: ClearInFront(depth: -2.0, width: 1.0)),"#);
    let bad = run(&mut app, false);
    assert_eq!(bad["outcome"], "refused", "{bad}");
    assert!(bad.to_string().contains("-2"), "the refusal says what is wrong: {bad}");
    assert_eq!(rows(&app), before, "a refusal moves nothing");
}

#[test]
fn a_map_that_keeps_its_relations_is_left_alone() {
    let (mut app, file) = editor(&format!("relations_solve_kept_{}", std::process::id()));
    write(&file, r#"(of: "chair", kind: Near(to: "table", within: 3.0)),"#);
    let before_file = saved(&app);
    let solved = run(&mut app, false);
    assert_eq!(solved["outcome"], "applied", "{solved}");
    assert_eq!(saved(&app), before_file, "nothing to mend writes nothing");
}
