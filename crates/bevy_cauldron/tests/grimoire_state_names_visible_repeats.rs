//! **`grimoire/state` names the repeats a player will notice** (#136 slice 2).
//!
//! Slice 1 put `repetition::repeats` in core, but an agent dressing a map reads `grimoire_state`, not a
//! library function. So the state answer carries a `repetition` section: the visible repeats of the
//! open map, the one a player notices first, first, with a total so a capped list can be told from a
//! whole one.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

fn editor(name: &str, at: &[(f32, f32)]) -> App {
    let mut fixture = fixtures::Fixture::new(name)
        .descriptor("site/barrel", "alpha")
        .descriptor("site/crate", "alpha");
    for &xz in at {
        fixture = fixture.place("site/barrel", xz);
    }
    fixture = fixture.place("site/crate", (14.0, 14.0));
    let root = fixture.build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
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

#[test]
fn three_barrels_side_by_side_are_named_as_a_clutter_repeat() {
    let mut app = editor(
        &format!("repeat_named_{}", std::process::id()),
        &[(2.0, 2.0), (3.0, 2.0), (4.0, 2.0)],
    );
    let s = state(&mut app);
    let rep = &s["repetition"];
    assert_eq!(rep["total"], 1, "one look repeats: {rep}");
    let first = &rep["repeats"][0];
    assert_eq!(first["descriptor"], "site/barrel", "{rep}");
    assert_eq!(first["role"], "clutter", "{rep}");
    assert_eq!(first["pieces"], 3, "{rep}");
    assert_eq!(first["pairs"], 3, "{rep}");
    assert_eq!(first["patched"], false, "{rep}");
}

#[test]
fn barrels_far_apart_are_no_repeat() {
    let mut app = editor(
        &format!("repeat_none_{}", std::process::id()),
        &[(1.0, 1.0), (14.0, 1.0)],
    );
    let s = state(&mut app);
    assert_eq!(s["repetition"]["total"], 0, "{}", s["repetition"]);
    assert_eq!(s["repetition"]["repeats"], serde_json::json!([]));
}
