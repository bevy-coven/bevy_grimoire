//! **`piece_import` says which of the project's standards the new piece breaks** (#138 slice 3).
//!
//! Slice 2 put the breaches in `grimoire_state`, but an agent importing a golem `.glb` reads the
//! import's own answer, and a piece off the module is cheapest to catch the moment it is admitted.
//! So the applied answer's `after.standards` names the imported piece's breaches (only its own: the
//! rest of the kit is `grimoire_state`'s), read from `standards.ron` at that moment, and the summary
//! says so. A breach is reported, not refused (the issue's assumed decision): the piece is in the
//! library either way, and a file that does not parse is named, never read as "no rules".

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;
use std::path::PathBuf;

use crate::fixtures;

/// A project whose fixture piece `crate` is also copied into `assets/inbox/box.glb` to import.
fn editor(name: &str) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(name)
        .descriptor("crate", "alpha")
        .build("m");
    let inbox = root.join("assets/inbox");
    std::fs::create_dir_all(&inbox).unwrap_or_else(|e| panic!("{e}"));
    std::fs::copy(root.join("assets/alpha/crate.glb"), inbox.join("box.glb"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    (app, root.join("assets/grimoire/standards.ron"))
}

fn import(app: &mut App) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app
        .world()
        .resource::<RemoteMethods>()
        .get("grimoire/operation_apply")
        .cloned()
    else {
        panic!("no method `grimoire/operation_apply` is registered");
    };
    app.world_mut()
        .run_system_with(
            id,
            Some(serde_json::json!({
                "operation": "piece_import",
                "parameters": {"glb": "assets/inbox/box.glb", "id": "inbox_box"},
                "dry_run": false,
            })),
        )
        .unwrap_or_else(|e| panic!("apply: {e:?}"))
        .unwrap_or_else(|e| panic!("apply: {e:?}"))
}

#[test]
fn the_imported_piece_names_the_rule_it_breaks_and_only_its_own() {
    let (mut app, file) = editor(&format!("import_standards_{}", std::process::id()));
    // Every piece is far under 100 m wide, so both the fixture's `crate` and the import break
    // "huge"; nothing is over 1000 m, so "sane" is met.
    std::fs::write(
        &file,
        r#"(version: 1, rules: [
            (name: "huge", dim: Width, min: Some(100.0)),
            (name: "sane", dim: Width, max: Some(1000.0)),
        ])"#,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let applied = import(&mut app);
    assert_eq!(applied["outcome"], "applied", "a breach is reported, not refused: {applied}");
    assert!(app.world().resource::<Project>().library.get("inbox_box").is_some());

    let st = &applied["log_entry"]["after"]["standards"];
    assert_eq!(st["error"], serde_json::Value::Null, "{applied}");
    let rows = st["breaches"].as_array().unwrap_or_else(|| panic!("no breaches: {applied}"));
    assert_eq!(rows.len(), 1, "the import's one breach, not the kit's: {st}");
    assert_eq!(rows[0]["piece"], "inbox_box", "{st}");
    assert_eq!(rows[0]["rule"], "huge", "{st}");
    assert_eq!(rows[0]["why"], "below_min", "{st}");
    let summary = applied["summary"].as_str().unwrap_or_default();
    assert!(summary.contains("huge"), "the summary names the broken rule: {summary}");
}

#[test]
fn no_file_is_no_breaches_and_a_bad_file_is_named() {
    let (mut app, _) = editor(&format!("import_standards_none_{}", std::process::id()));
    let applied = import(&mut app);
    assert_eq!(
        applied["log_entry"]["after"]["standards"],
        serde_json::json!({"breaches": [], "error": null}),
        "{applied}"
    );

    let (mut app, file) = editor(&format!("import_standards_bad_{}", std::process::id()));
    std::fs::write(&file, r#"(version: 1, rules: [(name: "x", dim: Width)])"#)
        .unwrap_or_else(|e| panic!("{e}"));
    let applied = import(&mut app);
    assert_eq!(applied["outcome"], "applied", "{applied}");
    let error = applied["log_entry"]["after"]["standards"]["error"]
        .as_str()
        .unwrap_or_else(|| panic!("a bad file is named, not read as no rules: {applied}"));
    assert!(error.contains("standards.ron"), "{error}");
}
