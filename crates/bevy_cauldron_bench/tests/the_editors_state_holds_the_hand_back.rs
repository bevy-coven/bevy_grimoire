//! **`grimoire/state`'s `presentation` carries the hand-back's words** (#434, BM-10).
//!
//! The runner asks the editor what it was handed when the transcript cannot say (an agent's `eval` call shows no arguments),
//! so the state's presentation holds the title, summary and the three lists the agent wrote, beside the id and the person's
//! answer it always held. Headless editor and the real bridge, no model.

use bevy::prelude::*;
use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy_cauldron_bench::fixture;
use serde_json::{json, Value};

fn call(app: &mut App, method: &str, params: Value) -> Value {
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get(method).cloned() else {
        panic!("{method} is not registered");
    };
    let answer = app
        .world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method} is callable: {e:?}"))
        .unwrap_or_else(|e| panic!("{method} answered: {e:?}"));
    app.update();
    answer
}

#[test]
fn the_state_holds_what_the_agent_handed_back() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap_or_else(|e| panic!("{e}"));
    let project = std::env::temp_dir().join(format!("grimoire-bench-state-handback-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    fixture::lay_out(&repo, &project).unwrap_or_else(|e| panic!("{e}"));
    let mut app = bevy_cauldron::harness::build_headless(&project, "bench_bedroom", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    let before = call(&mut app, "grimoire/state", json!({}));
    assert!(before["presentation"].is_null(), "nothing is handed back yet: {before}");
    call(
        &mut app,
        "grimoire/present",
        json!({"title": "No velvet armchair", "summary": "The library has no velvet piece, so the map is as it was.",
            "specified": ["a velvet armchair at (-2.0, 1.5)"], "inferred": ["it would be a chair"],
            "check": [{"what": "the map is unchanged", "placements": []}]}),
    );
    let state = call(&mut app, "grimoire/state", json!({}));
    let presentation = &state["presentation"];
    assert_eq!(presentation["title"], "No velvet armchair", "{state}");
    assert_eq!(presentation["summary"], "The library has no velvet piece, so the map is as it was.");
    assert_eq!(presentation["specified"], json!(["a velvet armchair at (-2.0, 1.5)"]));
    assert_eq!(presentation["inferred"], json!(["it would be a chair"]));
    assert_eq!(presentation["check"], json!(["the map is unchanged"]));
    assert!(presentation["id"].is_string() && presentation["answer"].is_null(), "what it always held stays: {presentation}");
    let _ = std::fs::remove_dir_all(&project);
}
