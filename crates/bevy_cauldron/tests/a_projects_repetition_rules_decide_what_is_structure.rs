//! **A project says which pieces are meant to repeat** (#139).
//!
//! `repetition::RepetitionRules` is data an author tunes, but `grimoire_state.repetition` read the
//! built-in defaults, so a project whose walls are kind `wall` saw them ranked as clutter with no
//! file to say otherwise. `repetition.ron` beside `kits.ron` now decides: absent is the defaults, a
//! partial file keeps the defaults for what it leaves out, and a file that cannot be read is named
//! in `error`, never silently replaced by the defaults.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

/// Three fixture pieces (kind `prop`) side by side, with `repetition.ron` written first when given.
fn editor(name: &str, rules: Option<&str>) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/barrel", "alpha")
        .place("site/barrel", (2.0, 2.0))
        .place("site/barrel", (3.0, 2.0))
        .place("site/barrel", (4.0, 2.0))
        .build("m");
    let file = root
        .join("assets/grimoire")
        .join(bevy_grimoire_core::repetition::FILE);
    if let Some(text) = rules {
        std::fs::write(&file, text).unwrap_or_else(|e| panic!("{e}"));
    }
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    (app, file)
}

fn repetition(app: &mut App) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get("grimoire/state").cloned()
    else {
        panic!("no method `grimoire/state` is registered");
    };
    let s = app
        .world_mut()
        .run_system_with(id, Some(serde_json::json!({})))
        .unwrap_or_else(|e| panic!("grimoire/state: {e:?}"))
        .unwrap_or_else(|e| panic!("grimoire/state: {e:?}"));
    s["repetition"].clone()
}

#[test]
fn a_kind_the_project_calls_structure_is_weighed_as_structure() {
    let (mut app, _) = editor(
        &format!("rep_rules_structure_{}", std::process::id()),
        Some(r#"(structure: ["prop"])"#),
    );
    let rep = repetition(&mut app);
    assert_eq!(rep["error"], serde_json::Value::Null, "{rep}");
    let first = &rep["repeats"][0];
    assert_eq!(first["role"], "structure", "the project's file says prop is structure: {rep}");
    // The weight left out of the file is the default's 0.1: three pieces score 0.3.
    let score = first["score"].as_f64().unwrap_or_else(|| panic!("{rep}"));
    assert!((score - 0.3).abs() < 1e-5, "{rep}");
}

#[test]
fn no_file_is_the_default_rules() {
    let (mut app, _) = editor(&format!("rep_rules_none_{}", std::process::id()), None);
    let rep = repetition(&mut app);
    assert_eq!(rep["error"], serde_json::Value::Null, "{rep}");
    assert_eq!(rep["repeats"][0]["role"], "clutter", "{rep}");
}

#[test]
fn an_edit_to_the_file_is_read_by_the_next_answer() {
    let (mut app, file) = editor(&format!("rep_rules_edit_{}", std::process::id()), None);
    assert_eq!(repetition(&mut app)["total"], 1);
    // Two metres apart is now out of sight: nothing repeats.
    std::fs::write(&file, "(radius_m: 0.5)").unwrap_or_else(|e| panic!("{e}"));
    let rep = repetition(&mut app);
    assert_eq!(rep["total"], 0, "{rep}");
}

#[test]
fn a_bad_file_is_named_never_read_as_the_defaults() {
    for (case, text) in [("parse", "(radius_m: \"far\")"), ("negative", "(radius_m: -1.0)")] {
        let (mut app, _) = editor(
            &format!("rep_rules_bad_{case}_{}", std::process::id()),
            Some(text),
        );
        let rep = repetition(&mut app);
        let error = rep["error"].as_str().unwrap_or_else(|| panic!("{case}: no error: {rep}"));
        assert!(error.contains(bevy_grimoire_core::repetition::FILE), "{case}: {error}");
        assert_eq!(rep["total"], 0, "{case}: {rep}");
    }
}
