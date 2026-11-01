//! **`grimoire/state` names the library's pieces, a page at a time (C6 gap 2).**
//!
//! A refusal said *"the palette lists the library's pieces"*, and the state answer held only the
//! count, so an agent that had been refused had no call that listed the ids it could arm. The state
//! now carries a capped page of ids, and `descriptor_ids_next` says where the next page starts, the
//! same way the pin list does. `descriptors` stays the true count.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

/// The most ids one page of `grimoire/state` names. Matches the cap written into the state answer.
const DESCRIPTOR_CAP: usize = 50;
const PIECES: usize = 60;

fn editor(name: &str) -> App {
    let mut fixture = fixtures::Fixture::new(name);
    for i in 0..PIECES {
        fixture = fixture.descriptor(&format!("site/p{i:02}"), "alpha");
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

#[test]
fn grimoire_state_names_the_first_page_of_library_ids_and_says_where_the_next_starts() {
    let mut app = editor("state_names_library");
    let state = call(&mut app, "grimoire/state", serde_json::json!({}));
    let project = &state["project"];

    assert_eq!(
        project["descriptors"],
        serde_json::json!(PIECES),
        "the true count is still reported: {state}"
    );
    let ids = project["descriptor_ids"].as_array().cloned().unwrap_or_default();
    assert_eq!(
        ids.len(),
        DESCRIPTOR_CAP,
        "one page names the cap, not the whole library: {state}"
    );
    assert_eq!(ids[0], "site/p00", "the page starts at the start of the library: {state}");
    assert_eq!(
        project["descriptor_ids_next"],
        serde_json::json!(DESCRIPTOR_CAP),
        "the next page starts where this one stopped: {state}"
    );
}

#[test]
fn grimoire_state_pages_the_library_ids_past_the_cap() {
    let mut app = editor("state_pages_library");
    let state = call(
        &mut app,
        "grimoire/state",
        serde_json::json!({"descriptors_from": DESCRIPTOR_CAP}),
    );
    let project = &state["project"];
    let ids = project["descriptor_ids"].as_array().cloned().unwrap_or_default();
    assert_eq!(
        ids.len(),
        PIECES - DESCRIPTOR_CAP,
        "the second page holds the rest: {state}"
    );
    assert_eq!(
        ids.last().and_then(|v| v.as_str()),
        Some("site/p59"),
        "the last piece is on the last page: {state}"
    );
    assert_eq!(
        project["descriptor_ids_next"],
        serde_json::Value::Null,
        "nothing follows the last page: {state}"
    );
}
