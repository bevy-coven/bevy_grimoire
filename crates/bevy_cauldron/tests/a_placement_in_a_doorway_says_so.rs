//! **A change says when it puts something in a doorway (E-079).**
//!
//! `a_change_says_what_it_broke.rs` proved the checks field for edge faults and overlaps. This is the
//! third kind: a piece whose plan reaches into a measured opening at a height the hole spans.
//!
//! # Why the fixture writes its own library here
//!
//! `tests/fixtures/mod.rs` has a helper for every mount the suite needed — floor, surface, two-way —
//! and none for `InOpening`, because nothing had ever exercised the mount's `clear` field. This test
//! is the first, so it rewrites the library the fixture wrote, exactly as `Fixture::bind` rewrites
//! `kits.ron` for the tests that are about binding itself. Adding an `InOpening` helper to the shared
//! fixture module would be the better home the day a second test wants one.
//!
//! # What is proved
//!
//! 1. Placing a crate in a measured doorway answers with the finding, naming the door and the crate.
//! 2. A crate beside it answers with none — the hole is `clear`'s rectangle, not the frame.
//! 3. The total is exact and the diff is a diff: the first crate's finding is still *there* on the
//!    second answer while `new_total` reads zero, which is the difference between "I broke this" and
//!    "this map has one".

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

/// **The editor with the bridge open**, eight updates, as `a_change_says_what_it_broke.rs` does: the
/// bridge's systems, the map watcher and the fault panel all settle inside that budget.
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

/// **Rewrite the fixture's `furniture` library** so the door carries a measured hole and the crate a
/// real size. Both are `OnFloor`/`InOpening` rows the shared fixture cannot express.
fn a_measured_doorway(name: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(name)
        .side(8.0)
        .descriptor("door", "alpha")
        .sized_descriptor("crate", "alpha", 0.5, 0.5)
        .build("m");

    let at = root.join("assets/grimoire/furniture/library.ron");
    let was = std::fs::read_to_string(&at).unwrap_or_else(|e| panic!("cannot read {at:?}: {e}"));
    let wrote = was
        .replace(
            // The fixture's door row, given the shipped doorway shape: 0.2 m through the wall, 1.0 m
            // of frame along it, framing a 0.6 m hole 1.9 m tall.
            r#"id: "door",
            mesh: Some("alpha/door.glb"),
            align: ( scale: None, stretch_y: None, y_offset: Some(0.0000), pivot: None, front: None ),
            extent: ( footprint: Some((1.0, 1.0)), height: Some(0.5) ),
            mount: Some(OnFloor),"#,
            r#"id: "door",
            mesh: Some("alpha/door.glb"),
            align: ( scale: None, stretch_y: None, y_offset: None, pivot: None, front: None ),
            extent: ( footprint: Some((0.2, 1.0)), height: Some(2.0) ),
            mount: Some(InOpening( clear: Some((0.6, 1.9)) )),"#,
        )
        .replace(
            r#"id: "crate",
            mesh: Some("alpha/crate.glb"),
            align: ( scale: None, stretch_y: None, y_offset: Some(0.0000), pivot: None, front: None ),
            extent: ( footprint: Some((0.5, 0.5)), height: Some(0.5) ),"#,
            r#"id: "crate",
            mesh: Some("alpha/crate.glb"),
            align: ( scale: None, stretch_y: None, y_offset: None, pivot: None, front: None ),
            extent: ( footprint: Some((0.5, 0.5)), height: Some(0.5) ),"#,
        );
    assert!(
        wrote.contains("InOpening( clear:"),
        "the rewrite missed the door row — the fixture's descriptor shape changed under this test:\n{was}"
    );
    std::fs::write(&at, wrote).unwrap_or_else(|e| panic!("cannot write {at:?}: {e}"));
    root
}

/// **A crate dropped in the doorway is named, and one beside it is not.**
#[test]
fn a_placement_that_fills_a_doorway_says_so() {
    let root = a_measured_doorway("doorway-checks");
    let mut app = editor_with_bridge(&root, "m");

    // The door already stands in the empty map's middle; the crate goes exactly in its hole.
    let door = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "door", "at": [4.0, 4.0]}),
    );
    assert_eq!(door["outcome"], "applied", "{door}");
    // **The ids come from the answer, never from a guess.** The editor mints `<piece>@<n>` and this
    // map's numbering is the editor's business; a test that hardcoded `door@0` would fail the day the
    // minting rule changed while saying nothing about doorways.
    let door_id = door["log_entry"]["after"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("the door's answer names the id it minted: {door}"))
        .to_owned();
    assert_eq!(
        door["checks"]["doorways_in_the_way"]["new_total"], 0,
        "a door in an empty map is in nobody's way: {door}"
    );

    let inside = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "crate", "at": [4.0, 4.0]}),
    );
    assert_eq!(inside["outcome"], "applied", "{inside}");
    let crate_id = inside["log_entry"]["after"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("the crate's answer names the id it minted: {inside}"))
        .to_owned();
    let found = &inside["checks"]["doorways_in_the_way"];
    assert_eq!(
        found["new_total"], 1,
        "a crate in a measured doorway is the finding this field exists for: {inside}"
    );
    assert_eq!(
        found["new"][0],
        serde_json::json!({"opening": door_id, "inside": crate_id}),
        "the answer names which id is the hole and which is the thing in it: {inside}"
    );
    assert_eq!(found["total"], 1, "{inside}");

    // **A crate beside the frame is clear.** The hole is 0.6 m along Z from the door's centre, so at
    // z = 4.7 the crate's near edge is 0.45 m out — inside the 1.0 m frame, outside the 0.6 m hole.
    let beside = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "crate", "at": [4.0, 4.7]}),
    );
    assert_eq!(beside["outcome"], "applied", "{beside}");
    assert_eq!(
        beside["checks"]["doorways_in_the_way"]["new_total"], 0,
        "a piece standing beside a doorway is not in it: {beside}"
    );
    assert_eq!(
        beside["checks"]["doorways_in_the_way"]["total"], 1,
        "the crate in the hole is still there, so the total stays one: {beside}"
    );
}
