//! **`grimoire/state` carries the editor's own frame clock (E-011).**
//!
//! Frame time read by counting ticks from outside swung 20 to 67 ms with machine load, so an agent
//! could not tell a slow editor from a busy machine. The state now says `frame.frame_ms` (wall time
//! between frames) and `frame.busy_ms` (work done inside one frame).

use bevy_cauldron::harness;

use crate::fixtures;

fn call(app: &mut bevy::prelude::App, method: &str) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => panic!("no method `{method}` is registered"),
    };
    app.world_mut()
        .run_system_with(id, Some(serde_json::json!({})))
        .expect("system runs")
        .expect("method answers")
}

#[test]
fn the_state_reports_frame_time_and_busy_time_once_frames_have_run() {
    let root = fixtures::Fixture::new("frame-clock")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..5 {
        std::thread::sleep(std::time::Duration::from_millis(3));
        app.update();
    }
    let state = call(&mut app, "grimoire/state");
    let frame = &state["frame"];
    assert!(frame["frame_ms"].as_f64().is_some_and(|m| m > 0.0), "{frame}");
    assert!(frame["busy_ms"].as_f64().is_some_and(|m| m > 0.0), "{frame}");
    assert!(frame["frame_count"].as_f64().is_some_and(|c| c >= 3.0), "{frame}");
}
