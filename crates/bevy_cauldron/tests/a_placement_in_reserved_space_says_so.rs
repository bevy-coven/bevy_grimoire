//! **A change says when it puts something in reserved space (E-079, second half).**
//!
//! `a_placement_in_a_doorway_says_so.rs` proved the doorway finding; this is the other half of the same
//! item — `Descriptor::clearance`, documented as *"space that must stay free around the piece"* since
//! the schema was written and read by nothing that decided space.
//!
//! # What is proved
//!
//! 1. Placing a chair inside a table's `Around` band answers with the finding, naming the keeper and the
//!    intruder.
//! 2. A chair clear of the band answers with none, and the total is still one — the difference between
//!    "I broke this" and "this map has one".
//!
//! The fixture is rewritten for the same reason as the doorway test's: `tests/fixtures/mod.rs` has no
//! descriptor with a clearance, because nothing had ever read the field.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

/// **The editor with the bridge open**, eight updates, as the other checks tests do.
fn editor_with_bridge(root: &std::path::Path, map: &str) -> App {
    let mut app = harness::build_headless(root, map, None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    use bevy::remote::RemoteMethodSystemId;
    use bevy::remote::RemoteMethods;
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err(format!("no method `{method}` is registered")),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .map_err(|e| format!("{method}: {e:?}"))?
        .map_err(|e| format!("{method}: {e:?}"))
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": operation,
            "parameters": parameters,
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"))
}

/// **Rewrite the fixture's `furniture` library** so the keeper reserves 0.91 m on every side. The
/// fixture writes every row with `clearance: []`, because nothing had ever read the field.
///
/// Rewriting by text is what the doorway test does too, and for the same reason: the shared fixture
/// module is the only writer of this file, and adding a setter there for one test's need would be a
/// second place the row's shape lives.
fn a_reserved_band(name: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(name)
        .side(16.0)
        .sized_descriptor("boulder", "alpha", 1.2, 1.2)
        .sized_descriptor("crate", "alpha", 0.5, 0.5)
        .build("m");

    let at = root.join("assets/grimoire/furniture/library.ron");
    let was = std::fs::read_to_string(&at).unwrap_or_else(|e| panic!("cannot read {at:?}: {e}"));
    let wrote = band_on(&was, "boulder", "clearance: [ ( dir: Around, dist: 0.91 ) ],");
    assert!(
        wrote != was,
        "the rewrite missed the boulder row — the fixture's descriptor shape changed:\n{was}"
    );
    std::fs::write(&at, wrote).unwrap_or_else(|e| panic!("cannot write {at:?}: {e}"));
    root
}

/// Replace the `clearance` line of the one row whose `id` is `id`, leaving every other row alone.
fn band_on(library: &str, id: &str, clearance: &str) -> String {
    let row = format!(r#"            id: "{id}","#);
    let Some(start) = library.find(&row) else {
        return library.to_owned();
    };
    let rest = &library[start..];
    let Some(line) = rest.find("            clearance: [],") else {
        return library.to_owned();
    };
    let mut out = String::with_capacity(library.len() + clearance.len());
    out.push_str(&library[..start + line]);
    out.push_str(clearance);
    out.push_str(&library[start + line + "            clearance: [],".len()..]);
    out
}

/// **A chair inside the band is named; one outside it is not.**
#[test]
fn a_placement_in_reserved_space_says_so() {
    let root = a_reserved_band("clearance-checks");
    let mut app = editor_with_bridge(&root, "m");

    let keeper = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "boulder", "at": [8.0, 8.0]}),
    );
    assert_eq!(keeper["outcome"], "applied", "{keeper}");
    let keeper_id = keeper["log_entry"]["after"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("the keeper's answer names the id it minted: {keeper}"))
        .to_owned();
    assert_eq!(
        keeper["checks"]["clearances_in_the_way"]["new_total"], 0,
        "a piece alone in a map is in nobody's reserved space: {keeper}"
    );

    // The keeper's edge is 0.6 m from its centre and its band reaches 0.91 m past that; the crate's own
    // half-depth is 0.25. At 0.9 m out its near edge is 0.65 m from centre — clear of the keeper's
    // footprint, which matters because `placement_add` **replaces** what it lands on rather than
    // reporting it — and well inside the band's 1.51 m.
    let inside = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "crate", "at": [8.0, 8.9]}),
    );
    assert_eq!(inside["outcome"], "applied", "{inside}");
    let crate_id = inside["log_entry"]["after"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("the crate's answer names the id it minted: {inside}"))
        .to_owned();
    let found = &inside["checks"]["clearances_in_the_way"];
    assert_eq!(
        found["new_total"], 1,
        "a crate in a reserved band is the finding this field exists for: {inside}"
    );
    assert_eq!(
        found["new"][0],
        serde_json::json!({"keeper": keeper_id, "intruder": crate_id}),
        "the answer names which id reserved the space and which is in it: {inside}"
    );
    assert_eq!(found["total"], 1, "{inside}");

    // At 2.6 m out the crate's near edge is 2.35 m from centre, clear of the band's 1.51 m.
    let outside = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "crate", "at": [8.0, 10.6]}),
    );
    assert_eq!(outside["outcome"], "applied", "{outside}");
    assert_eq!(
        outside["checks"]["clearances_in_the_way"]["new_total"], 0,
        "a crate clear of the band is not a finding: {outside}"
    );
    assert_eq!(
        outside["checks"]["clearances_in_the_way"]["total"], 1,
        "the crate in the band is still there, so the total stays one: {outside}"
    );
}
