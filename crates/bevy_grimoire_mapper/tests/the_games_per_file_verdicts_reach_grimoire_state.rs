//! **The game's verdict on each file reaches `grimoire_state.game.kinds`, and the journal names the file
//! and the field** (#200, H3). `kinds` has one entry for every file the game read: its kind, path,
//! `applied` or `refused`, and for a refusal the field and the reason, so an agent that wrote a bad
//! file learns which one and where, not only that the project was refused.

use std::time::Duration;

use bevy::prelude::*;
use bevy_grimoire_core::live::{KindVerdict, Verdict};
use bevy_grimoire_mapper::editor::EditorState;
use bevy_grimoire_mapper::harness;
use bevy_grimoire_mapper::live::LiveWrite;
use bevy_grimoire_mapper::project::{OpenMap, Project};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place_as("crate_0", "site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_grimoire_mapper::ops_brp::OpsBridgePlugin);
    app.insert_resource(LiveWrite);
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
fn each_files_verdict_is_in_the_state_and_a_refusal_names_its_file_and_field() {
    let mut app = editor("kind_verdicts");
    let dir = app.world().resource::<Project>().grimoire_dir.clone();
    let map = app.world().resource::<OpenMap>().map_path.clone();

    std::fs::write(&map, std::fs::read_to_string(&map).unwrap()).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    let kinds = vec![
        KindVerdict::applied("kits", "kits.ron"),
        KindVerdict::refused("map", "maps/m.map.ron", Some("placements[0].at[1]".to_owned()), "1:47: Expected float"),
    ];
    Verdict::with_kinds(7, "maps/m.map.ron: refused", kinds).write(&dir).unwrap();
    settle(&mut app);

    let g = game(&mut app);
    assert_eq!(g["ok"], serde_json::json!(false), "{g}");
    assert_eq!(g["seq"], serde_json::json!(7));
    assert_eq!(
        g["kinds"],
        serde_json::json!([
            {"kind": "kits", "file": "kits.ron", "outcome": "applied", "field": null, "reason": ""},
            {"kind": "map", "file": "maps/m.map.ron", "outcome": "refused", "field": "placements[0].at[1]", "reason": "1:47: Expected float"},
        ])
    );
    let problem = app.world().resource::<EditorState>().status.problem_text().to_owned();
    assert!(
        problem.contains("maps/m.map.ron (placements[0].at[1])") && problem.contains("Expected float"),
        "the journal says which file and field: {problem}"
    );
}
