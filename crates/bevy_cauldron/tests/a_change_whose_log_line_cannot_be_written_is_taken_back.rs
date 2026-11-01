//! **A change whose log line cannot be written is taken back (E-028).**
//!
//! `dispatch_as` commits a change first and appends its log line after. When the append failed, the
//! refusal said the operation was applied, and nothing put the change back: the file and the map
//! had the new state while the log had no line to undo it from, and the screen kept the old picture.
//! The log is the source of truth, so a change that cannot be recorded is taken back, and the
//! refusal says so.
//!
//! Here the log cannot be written, because `oplog.jsonl` in the project directory is a directory. A
//! rename is the change, so the test checks the map, the file on disk and the rules, all of which the
//! rollback must restore.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};

use crate::fixtures;

const RULES: &str = "// the block rules: learned from `m`\n\
(version: 1, blocks: [(example: \"m\", regions: [], cell: 2.0, \
size: (4, 4), border: None, no_empty: true, weights: [], \
footprints: [], attempts: 8, seed: 7)])\n";

/// The app, and the project directory (`assets/grimoire` under the fixture root).
fn editor(name: &str) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .rules(RULES)
        .build("m");
    let project = root.join("assets/grimoire");
    // **The log cannot be written**: a directory is where its file goes.
    std::fs::create_dir(project.join("oplog.jsonl")).expect("a directory where the log goes");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    (app, project)
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

fn example_on_disk(project: &std::path::Path) -> String {
    let text = std::fs::read_to_string(project.join("rules.ron")).expect("rules.ron is on disk");
    bevy_grimoire_core::rules::Rules::parse(&text)
        .expect("rules.ron still parses")
        .blocks[0]
        .example
        .clone()
}

#[test]
fn a_rename_whose_log_line_cannot_be_written_is_taken_back() {
    let (mut app, project) = editor("unlogged_rename");
    assert_eq!(app.world().resource::<OpenMap>().map.name, "m");

    let reply = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "map_rename", "parameters": {"name": "n"}, "dry_run": false}),
    );
    assert!(
        reply.to_string().contains("nothing was changed"),
        "the refusal does not say the change was taken back: {reply}"
    );

    assert_eq!(
        app.world().resource::<OpenMap>().map.name,
        "m",
        "the open map moved although the log line was never written"
    );
    assert!(
        project.join("maps/m.map.ron").exists() && !project.join("maps/n.map.ron").exists(),
        "the map file moved although the log line was never written"
    );
    assert_eq!(
        example_on_disk(&project),
        "m",
        "rules.ron names `n` although the rename was taken back"
    );
    assert_eq!(
        app.world().resource::<Project>().rules.blocks[0].example,
        "m",
        "the project holds `n` although the rename was taken back"
    );
}
