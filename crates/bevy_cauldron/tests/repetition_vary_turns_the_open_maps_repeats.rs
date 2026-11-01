//! **`repetition_vary` turns the open map's clutter repeats as one undoable operation** (#143).
//!
//! `grimoire_state.repetition` names the repeats a player notices; an agent reaches the editor only
//! through the registry. So the operation turns each clutter copy out of its repeat under the
//! project's `repetition.ron`, writes the turned rows, and one undo puts every one back. A preview
//! writes nothing, a project whose rules call the pieces structure turns nothing, and a rules file
//! that does not read is refused rather than replaced by the defaults.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;
use std::path::PathBuf;

use crate::fixtures;

/// Three fixture barrels (kind `prop`, clutter by default) a metre apart, all at yaw 0, and one
/// crate far off. Returns the app and the project's `repetition.ron` path.
fn editor(name: &str) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/barrel", "alpha")
        .descriptor("site/crate", "alpha")
        .place("site/barrel", (2.0, 2.0))
        .place("site/barrel", (3.0, 2.0))
        .place("site/barrel", (4.0, 2.0))
        .place("site/crate", (14.0, 14.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    (app, root.join("assets/grimoire").join(bevy_grimoire_core::repetition::FILE))
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
        serde_json::json!({"operation": "repetition_vary", "parameters": {}, "dry_run": dry_run}),
    )
}

fn undo(app: &mut App, id: &str) -> serde_json::Value {
    call(app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}))
}

/// `grimoire_state.repetition.total`: how many looks repeat visibly.
fn repeating(app: &mut App) -> u64 {
    let s = call(app, "grimoire/state", serde_json::json!({}));
    s["repetition"]["total"].as_u64().unwrap_or_else(|| panic!("{}", s["repetition"]))
}

/// The open map's rows as (id, yaw), in file order.
fn rows(app: &App) -> Vec<(String, f32)> {
    app.world().resource::<OpenMap>().map.placements.iter().map(|p| (p.id.clone(), p.yaw)).collect()
}

fn saved(app: &App) -> String {
    let path = app.world().resource::<OpenMap>().map_path.clone();
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"))
}

#[test]
fn varying_ends_the_repeat_and_one_undo_puts_every_row_back() {
    let (mut app, _) = editor(&format!("rep_vary_undo_{}", std::process::id()));
    assert_eq!(repeating(&mut app), 1, "three barrels side by side repeat");
    let before = rows(&app);
    let before_file = saved(&app);

    let varied = run(&mut app, false);
    assert_eq!(varied["outcome"], "applied", "{varied}");
    assert_eq!(repeating(&mut app), 0, "no clutter repeat is left: {varied}");
    let after = rows(&app);
    assert_eq!(after[0], before[0], "the first barrel keeps the author's yaw");
    assert_eq!(after[3], before[3], "the lone crate is not touched");
    assert_ne!(saved(&app), before_file, "the turned rows are written to the map file");

    let id = varied["operation_id"].as_str().expect("an id").to_owned();
    let back = undo(&mut app, &id);
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(rows(&app), before, "one undo puts every turned row back");
    assert_eq!(repeating(&mut app), 1);
}

#[test]
fn a_preview_names_what_would_turn_and_writes_nothing() {
    let (mut app, _) = editor(&format!("rep_vary_preview_{}", std::process::id()));
    let before = rows(&app);
    let before_file = saved(&app);
    let preview = run(&mut app, true);
    assert_ne!(preview["outcome"], "refused", "{preview}");
    assert!(preview.to_string().contains("turned 2"), "the preview says what would turn: {preview}");
    assert_eq!(rows(&app), before);
    assert_eq!(saved(&app), before_file, "a preview writes nothing");
}

#[test]
fn structure_is_never_turned_and_a_bad_rules_file_is_refused() {
    let (mut app, file) = editor(&format!("rep_vary_rules_{}", std::process::id()));
    let before_file = saved(&app);
    // The project says `prop` is structure: the row of barrels is the design.
    std::fs::write(&file, r#"(structure: ["prop"])"#).unwrap_or_else(|e| panic!("{e}"));
    let kept = run(&mut app, false);
    assert_eq!(kept["outcome"], "applied", "{kept}");
    assert_eq!(saved(&app), before_file, "nothing to turn writes nothing");

    std::fs::write(&file, "(radius_m: -1.0)").unwrap_or_else(|e| panic!("{e}"));
    let bad = run(&mut app, false);
    assert_eq!(bad["outcome"], "refused", "{bad}");
    assert!(bad.to_string().contains(bevy_grimoire_core::repetition::FILE), "the refusal names the file: {bad}");
    assert_eq!(saved(&app), before_file, "a refusal turns nothing");
}
