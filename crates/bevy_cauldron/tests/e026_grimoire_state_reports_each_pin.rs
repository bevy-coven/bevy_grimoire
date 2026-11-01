//! **`grimoire/state` reports each pinned placement, so a pin change shows in a state dump.**
//!
//! E-026. The state answer counted placements and never named a pin, so an agent could not see a
//! pin without reading the map file or the operation's own answer. These pin through the registered
//! operation, the way an agent does, and read the pins back from `grimoire/state` alone.
//!
//! The list is capped at [`PINNED_CAP`] entries, because a state dump is a summary and not a dump of
//! the map. `pinned_total` always gives the true count, so a reader can tell a full list from a cut
//! one.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

/// The most pins `grimoire/state` names. Matches the cap written into the state answer.
const PINNED_CAP: usize = 50;

fn editor(name: &str, pieces: usize) -> App {
    let mut fixture = fixtures::Fixture::new(name).descriptor("site/wall", "alpha");
    for i in 0..pieces {
        fixture = fixture.place("site/wall", (i as f32 * 2.0, 0.0));
    }
    let root = fixture.build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **Through the registered method**, as an agent's request goes.
fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn pin(app: &mut App, id: &str, because: &str) {
    let answer = call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "placement_set_pin",
            "parameters": {"id": id, "pinned": true, "because": because},
            "dry_run": false
        }),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
}

fn held(app: &App) -> bevy_grimoire_core::map::Map {
    app.world().resource::<OpenMap>().map.clone()
}

#[test]
fn grimoire_state_names_a_pinned_placement_and_why() {
    let mut app = editor("state_names_pins", 3);
    let id = held(&app).placements[1].id.clone();
    pin(&mut app, &id, "load-bearing wall");

    let state = call(&mut app, "grimoire/state", serde_json::json!({}));
    let map = &state["map"];
    assert_eq!(
        map["pinned"],
        serde_json::json!([{"id": id, "because": "load-bearing wall"}]),
        "the one pin is named with its reason, and nothing else is: {state}"
    );
    assert_eq!(map["pinned_total"], serde_json::json!(1), "{state}");
    assert_eq!(map["placements"], serde_json::json!(3), "{state}");
}

#[test]
fn grimoire_state_names_no_pins_on_an_unpinned_map_and_says_so() {
    let mut app = editor("state_no_pins", 2);
    let state = call(&mut app, "grimoire/state", serde_json::json!({}));
    assert_eq!(state["map"]["pinned"], serde_json::json!([]), "{state}");
    assert_eq!(
        state["map"]["pinned_total"],
        serde_json::json!(0),
        "{state}"
    );
}

#[test]
fn grimoire_state_caps_the_pin_list_and_still_counts_every_pin() {
    let total = PINNED_CAP + 5;
    let mut app = editor("state_pin_cap", total);
    let ids: Vec<String> = held(&app).placements.iter().map(|p| p.id.clone()).collect();
    for id in &ids {
        pin(&mut app, id, "held");
    }

    let state = call(&mut app, "grimoire/state", serde_json::json!({}));
    let map = &state["map"];
    assert_eq!(
        map["pinned"].as_array().map(Vec::len),
        Some(PINNED_CAP),
        "the list is capped at {PINNED_CAP}: {state}"
    );
    assert_eq!(
        map["pinned_total"],
        serde_json::json!(total),
        "the total is every pin, not the capped list: {state}"
    );
    // **The cap cuts the end, not a sample.** The first pins in map order are the ones named.
    assert_eq!(map["pinned"][0]["id"], serde_json::json!(ids[0]), "{state}");
}

#[test]
fn grimoire_state_pages_the_pin_list_past_the_cap() {
    let total = PINNED_CAP + 5;
    let mut app = editor("state_pin_pages", total);
    let ids: Vec<String> = held(&app).placements.iter().map(|p| p.id.clone()).collect();
    for id in &ids {
        pin(&mut app, id, "held");
    }

    // **The first page says where the next one starts**, in the shape `oplog_read` uses.
    let first = call(&mut app, "grimoire/state", serde_json::json!({}));
    assert_eq!(
        first["map"]["pinned_next"],
        serde_json::json!(PINNED_CAP),
        "{first}"
    );
    assert!(
        first["map"]["how_to_narrow"]
            .as_str()
            .is_some_and(|s| s.contains("pinned_from=50")),
        "the cut list says how to read the rest: {first}"
    );

    // **The second page starts where the first stopped**, and holds the remainder.
    let rest = call(
        &mut app,
        "grimoire/state",
        serde_json::json!({"pinned_from": PINNED_CAP}),
    );
    let map = &rest["map"];
    assert_eq!(map["pinned"].as_array().map(Vec::len), Some(5), "{rest}");
    assert_eq!(map["pinned"][0]["id"], serde_json::json!(ids[PINNED_CAP]), "{rest}");
    assert_eq!(map["pinned_total"], serde_json::json!(total), "{rest}");
    assert_eq!(map["pinned_next"], serde_json::Value::Null, "{rest}");
    assert_eq!(map["how_to_narrow"], serde_json::Value::Null, "{rest}");
}
