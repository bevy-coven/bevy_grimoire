//! **A rename carries the rules' example with it (B-029).**
//!
//! `rules.ron` names the map a block's grammar is learned from (`example: "m"`, so
//! `maps/m.map.ron`). `map_rename` moved the file and left that name behind, so generating the block
//! failed with "the example map cannot be read" and the rename's receipt said nothing about it. The
//! rename now rewrites each rule that names the old map, in memory and in `rules.ron`, and undo
//! takes both back the same way. These go through the registered BRP methods, as an agent's do.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};

use crate::fixtures;

const RULES: &str = "// the block rules: learned from `m`\n\
(version: 1, blocks: [(example: \"m\", regions: [], cell: 2.0, \
size: (4, 4), border: None, no_empty: true, weights: [], \
footprints: [], attempts: 8, seed: 7)])\n";

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .rules(RULES)
        .build("m");
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

fn apply(app: &mut App, name: &str) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "map_rename", "parameters": {"name": name}, "dry_run": false}),
    )
}

fn undo(app: &mut App) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": null, "dry_run": false}),
    )
}

fn rules_path(app: &App) -> std::path::PathBuf {
    app.world().resource::<Project>().project_dir.join("rules.ron")
}

fn example_in_memory(app: &App) -> String {
    app.world().resource::<Project>().rules.blocks[0].example.clone()
}

fn example_on_disk(app: &App) -> String {
    let text = std::fs::read_to_string(rules_path(app)).expect("rules.ron is on disk");
    bevy_grimoire_core::rules::Rules::parse(&text)
        .expect("rules.ron still parses")
        .blocks[0]
        .example
        .clone()
}

#[test]
fn a_rename_rewrites_the_rules_example_on_disk_and_in_memory() {
    let mut app = editor("rule_example_rename");
    assert_eq!(example_on_disk(&app), "m", "the fixture rule names `m`");

    let reply = apply(&mut app, "n");
    assert!(
        !reply.to_string().contains("refused"),
        "the rename was refused: {reply}"
    );
    assert_eq!(app.world().resource::<OpenMap>().map.name, "n");

    assert_eq!(
        example_on_disk(&app),
        "n",
        "rules.ron still names the map the rename left behind"
    );
    assert_eq!(
        example_in_memory(&app),
        "n",
        "the project still holds the old example, so generate would read a missing file"
    );
    let text = std::fs::read_to_string(rules_path(&app)).expect("rules.ron is on disk");
    assert!(
        text.starts_with("// the block rules"),
        "the rewrite kept the author's comment: {text}"
    );
}

#[test]
fn undoing_a_rename_puts_the_rules_example_back() {
    let mut app = editor("rule_example_undo");
    apply(&mut app, "n");
    assert_eq!(example_on_disk(&app), "n");

    undo(&mut app);
    assert_eq!(app.world().resource::<OpenMap>().map.name, "m");
    assert_eq!(example_on_disk(&app), "m", "undo left rules.ron naming `n`");
    assert_eq!(example_in_memory(&app), "m", "undo left the project naming `n`");
}
