//! **The game's verdict on a reload reaches `grimoire_state.game` (S3.5).**
//!
//! `null` with no game, `"pending"` while the newest verdict predates the map's last write, the
//! verdict itself once one is newer. A refusal also lands in the journal as a problem.

use std::time::Duration;

use bevy::prelude::*;
use bevy_grimoire_core::live::Verdict;
use bevy_cauldron::harness;
use bevy_cauldron::live::LiveWrite;
use bevy_cauldron::project::{OpenMap, Project};

use crate::fixtures;

fn editor(name: &str, live: bool) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place_as("crate_0", "site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    if live {
        app.insert_resource(LiveWrite);
    }
    for _ in 0..8 {
        app.update();
    }
    app
}

fn game(app: &mut App) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get("grimoire/state").cloned()
    else {
        panic!("grimoire/state is not registered");
    };
    let state = app
        .world_mut()
        .run_system_with(id, Some(serde_json::json!({})))
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|e| panic!("{e:?}"));
    state["game"].clone()
}

fn settle(app: &mut App) {
    std::thread::sleep(Duration::from_millis(300));
    app.update();
    app.update();
}

#[test]
fn the_verdict_is_null_pending_or_the_answer() {
    let mut app = editor("verdict_state", true);
    let dir = app.world().resource::<Project>().grimoire_dir.clone();
    let map = app.world().resource::<OpenMap>().map_path.clone();
    assert_eq!(game(&mut app), serde_json::Value::Null, "no game has answered");

    // A verdict older than the map file is the answer to an earlier write.
    let mut old = Verdict::now(true, "town: 0 added");
    old.at_unix_ms -= 60_000;
    old.write(&dir).unwrap();
    settle(&mut app);
    assert_eq!(game(&mut app), serde_json::json!("pending"));

    // One written after the last write is the answer.
    std::fs::write(&map, std::fs::read_to_string(&map).unwrap()).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    Verdict::now(false, "descriptor `x/y` does not resolve").write(&dir).unwrap();
    settle(&mut app);
    let g = game(&mut app);
    assert_eq!(g["ok"], serde_json::json!(false), "{g}");
    assert_eq!(g["message"], serde_json::json!("descriptor `x/y` does not resolve"));
}

#[test]
fn without_live_the_file_is_never_read() {
    let mut app = editor("verdict_not_live", false);
    let dir = app.world().resource::<Project>().grimoire_dir.clone();
    Verdict::now(true, "ok").write(&dir).unwrap();
    settle(&mut app);
    assert_eq!(game(&mut app), serde_json::Value::Null);
}
