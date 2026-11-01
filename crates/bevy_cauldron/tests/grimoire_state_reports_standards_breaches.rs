//! **`grimoire/state` says which kit pieces break the project's standards** (#138 slice 2).
//!
//! Slice 1 put `standards::check` in core, but an agent importing or placing pieces reads
//! `grimoire_state`, not a library function. So the state answer carries a `standards` section read
//! from the `standards.ron` beside `kits.ron`: every breach naming the piece, the rule, what was
//! measured and the bound, and every rule that covers no piece at all (a misspelt `kind` token,
//! which would otherwise read as a kit that meets everything). The file is read on each call, so an
//! edit is seen with no restart, and a file that does not parse is named, never read as "no rules".

use bevy::prelude::*;
use bevy_cauldron::harness;
use std::path::{Path, PathBuf};

use crate::fixtures;

/// Every fixture piece has kind `prop`. The wall is 3.1 m wide, off a 2 m module; the floor is on it.
fn editor(name: &str) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(name)
        .sized_descriptor("site/wall", "alpha", 3.1, 0.2)
        .sized_descriptor("site/floor", "alpha", 2.0, 2.0)
        .place_as("wall", "site/wall", (2.0, 2.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    (app, root.join("assets/grimoire/standards.ron"))
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

fn write(path: &Path, rules: &str) {
    std::fs::write(path, format!("(version: 1, rules: [{rules}])"))
        .unwrap_or_else(|e| panic!("{path:?}: {e}"));
}

const MODULE_AND_TYPO: &str = r#"
    (name: "module", kind: Some("prop"), dim: Width, multiple_of: Some(2.0)),
    (name: "door passage", kind: Some("dorr"), dim: Width, min: Some(1.0)),
"#;

#[test]
fn the_off_module_wall_and_the_rule_that_covers_nothing_are_named() {
    let (mut app, file) = editor(&format!("standards_named_{}", std::process::id()));
    write(&file, MODULE_AND_TYPO);
    let st = &state(&mut app)["standards"];
    assert_eq!(st["error"], serde_json::Value::Null, "{st}");
    let rows = st["breaches"].as_array().unwrap_or_else(|| panic!("{st}"));
    assert_eq!(rows.len(), 1, "only the wall is off the module: {st}");
    assert_eq!(rows[0]["piece"], "site/wall", "{}", rows[0]);
    assert_eq!(rows[0]["rule"], "module", "{}", rows[0]);
    assert_eq!(rows[0]["dim"], "width", "{}", rows[0]);
    assert_eq!(rows[0]["why"], "not_multiple_of", "{}", rows[0]);
    let found = rows[0]["found"].as_f64().unwrap_or_else(|| panic!("{}", rows[0]));
    assert!((found - 3.1).abs() < 1e-3, "{found}");
    assert!((rows[0]["bound"].as_f64().unwrap_or(-1.0) - 2.0).abs() < 1e-6, "{}", rows[0]);
    assert_eq!(
        st["idle"],
        serde_json::json!(["door passage"]),
        "a rule whose kind no piece carries checks nothing, and says so: {st}"
    );
}

#[test]
fn an_edit_to_the_file_is_seen_by_the_next_call() {
    let (mut app, file) = editor(&format!("standards_reload_{}", std::process::id()));
    assert_eq!(
        state(&mut app)["standards"],
        serde_json::json!({"breaches": [], "idle": [], "unnested": [], "error": null}),
        "a project with no standards file has none, and that is no error"
    );
    write(&file, MODULE_AND_TYPO);
    assert_eq!(state(&mut app)["standards"]["breaches"].as_array().map(Vec::len), Some(1));
    write(&file, r#"(name: "module", dim: Width, multiple_of: Some(0.1)),"#);
    assert_eq!(state(&mut app)["standards"]["breaches"], serde_json::json!([]));
}

#[test]
fn a_file_that_does_not_parse_is_named_not_read_as_none() {
    let (mut app, file) = editor(&format!("standards_bad_{}", std::process::id()));
    write(&file, r#"(name: "module", dim: Width, min: Some(3.0), max: Some(2.0)),"#);
    let st = &state(&mut app)["standards"];
    let error = st["error"].as_str().unwrap_or_else(|| panic!("no error named: {st}"));
    assert!(error.contains("standards.ron"), "the error names the file: {error}");
    assert!(error.contains("min 3"), "and what is wrong in it: {error}");
    assert_eq!(st["breaches"], serde_json::json!([]), "{st}");
}
