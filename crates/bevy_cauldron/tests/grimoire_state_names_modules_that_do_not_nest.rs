//! **`grimoire/state` names the sub-kit modules that do not nest** (#168 slice 2).
//!
//! Slice 1 put `standards::unnested` in core: two rules whose modules are not whole multiples of
//! each other, so their pieces drift off each other's grid lines although every piece meets its own
//! rule. An agent laying out a kit reads `grimoire_state`, so the `standards` section carries them,
//! read from `standards.ron` on each call like the breaches beside them.

use bevy::prelude::*;
use bevy_cauldron::harness;
use std::path::{Path, PathBuf};

use crate::fixtures;

fn editor(name: &str) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(name)
        .sized_descriptor("site/wall", "alpha", 2.0, 0.2)
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

#[test]
fn a_fence_module_off_the_wall_module_is_named_with_where_they_meet() {
    let (mut app, file) = editor(&format!("unnested_named_{}", std::process::id()));
    write(
        &file,
        r#"
        (name: "wall module", dim: Width, multiple_of: Some(2.0)),
        (name: "fence module", kind: Some("fence"), dim: Depth, multiple_of: Some(1.5)),
        (name: "storey", kind: Some("storey"), dim: Height, multiple_of: Some(2.4)),
        "#,
    );
    let st = &state(&mut app)["standards"];
    assert_eq!(st["error"], serde_json::Value::Null, "{st}");
    let pairs = st["unnested"].as_array().unwrap_or_else(|| panic!("{st}"));
    assert_eq!(pairs.len(), 1, "a storey never pairs with a footprint: {st}");
    let p = &pairs[0];
    assert_eq!(p["rules"], serde_json::json!(["wall module", "fence module"]), "{p}");
    assert_eq!(p["dims"], serde_json::json!(["width", "depth"]), "{p}");
    assert_eq!(p["modules"], serde_json::json!([2.0, 1.5]), "{p}");
    let meet = p["meet"].as_f64().unwrap_or_else(|| panic!("{p}"));
    assert!((meet - 6.0).abs() < 1e-4, "{meet}");
    let message = p["message"].as_str().unwrap_or_else(|| panic!("{p}"));
    assert!(
        message.contains("wall module") && message.contains("fence module") && message.contains("6 m"),
        "the sentence names both rules and where they meet: {message}"
    );
    // The wall (kind `prop`) meets the one rule that covers it; the pairs are read off the rules
    // alone, so fence and storey rules that cover no piece here still pair.
    assert_eq!(st["breaches"], serde_json::json!([]), "{st}");
    assert_eq!(st["idle"], serde_json::json!(["fence module", "storey"]), "{st}");
}

#[test]
fn an_edit_that_makes_them_nest_is_seen_by_the_next_call() {
    let (mut app, file) = editor(&format!("unnested_reload_{}", std::process::id()));
    write(
        &file,
        r#"
        (name: "wall module", dim: Width, multiple_of: Some(2.0)),
        (name: "odd module", dim: Width, multiple_of: Some(1.215)),
        "#,
    );
    let pairs = state(&mut app)["standards"]["unnested"].clone();
    assert_eq!(pairs.as_array().map(Vec::len), Some(1), "{pairs}");
    assert_eq!(pairs[0]["meet"], serde_json::Value::Null, "2 and 1.215 never meet close by: {pairs}");
    write(
        &file,
        r#"
        (name: "wall module", dim: Width, multiple_of: Some(2.0)),
        (name: "trim module", dim: Width, multiple_of: Some(0.5)),
        "#,
    );
    assert_eq!(state(&mut app)["standards"]["unnested"], serde_json::json!([]));
}
