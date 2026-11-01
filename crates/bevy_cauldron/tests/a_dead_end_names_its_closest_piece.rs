//! **A dead end names the piece nearest to closing it, to the agent and the person** (#145).
//!
//! `grimoire_state.dead_ends` rows carry `closest`: the piece, its turn in degrees, and each seam cell
//! where it presents something other than the face. The Meshes pane's dead-end line names it too.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use serde_json::json;
use std::path::PathBuf;

use crate::fixtures;

/// `jamb` presents `[jamb_l jamb_r]` east; `b_near` presents `[jamb_l jamb_x]` west, one token off.
fn project(name: &str) -> PathBuf {
    let root = fixtures::Fixture::new(name)
        .edge_tokens(&["jamb_l", "jamb_r", "jamb_x"])
        .descriptor("jamb", "alpha")
        .descriptor("b_near", "alpha")
        .build("m");
    let lib = root.join("assets/grimoire/furniture/library.ron");
    let mut text = std::fs::read_to_string(&lib).unwrap_or_else(|e| panic!("{lib:?}: {e}"));
    for (id, cells) in [
        (
            "jamb",
            r#"subgrid: Some(( cells: [( at: (1, 0, 0), edge: Some("jamb_l") ), ( at: (1, 0, 1), edge: Some("jamb_r") )] ))"#,
        ),
        (
            "b_near",
            r#"subgrid: Some(( cells: [( at: (0, 0, 0), edge: Some("jamb_l") ), ( at: (0, 0, 1), edge: Some("jamb_x") )] ))"#,
        ),
    ] {
        let at = text
            .find(&format!("id: \"{id}\""))
            .unwrap_or_else(|| panic!("no {id} row: {text}"));
        let (head, tail) = text.split_at(at);
        text = format!("{head}{}", tail.replacen("subgrid: None", cells, 1));
    }
    std::fs::write(&lib, text).unwrap_or_else(|e| panic!("{e}"));
    root
}

fn editor(name: &str) -> App {
    let root = project(name);
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

#[test]
fn grimoire_state_names_the_closest_piece_and_the_cell_it_gets_wrong() {
    let mut app = editor(&format!("dead_end_closest_state_{}", std::process::id()));
    let st = call(&mut app, "grimoire/state", json!({}));
    let de = &st["dead_ends"];
    let east = de["faces"]
        .as_array()
        .and_then(|f| f.iter().find(|f| f["piece"] == "jamb" && f["dir"] == "E"))
        .unwrap_or_else(|| panic!("the jamb's east face meets nothing: {de}"));
    assert_eq!(
        east["closest"],
        json!({"piece": "b_near", "turn": 0, "misses": [{"at": 1, "want": "jamb_r", "found": "jamb_x"}]}),
        "{east}"
    );
}

#[test]
fn the_meshes_pane_line_names_the_closest_piece() {
    let mut app = editor(&format!("dead_end_closest_pane_{}", std::process::id()));
    call(
        &mut app,
        "grimoire/operation_apply",
        json!({"operation": "door_open", "parameters": {"door": "kit"}, "dry_run": false}),
    );
    for _ in 0..4 {
        app.update();
    }
    app.world_mut()
        .resource_mut::<bevy_cauldron::tiles::ImportState>()
        .selected_library_id = Some("jamb".to_owned());
    for _ in 0..3 {
        app.update();
    }
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &ComputedNode)>();
    let texts: Vec<String> = q
        .iter(world)
        .filter(|(_, n)| n.size().x > 0.0 && n.size().y > 0.0)
        .map(|(t, _)| t.0.clone())
        .collect();
    let line = texts
        .iter()
        .find(|t| t.starts_with("E:"))
        .unwrap_or_else(|| panic!("no line for the east face: {texts:?}"));
    assert!(
        line.contains("closest b_near at 0°, 1 off"),
        "the line names what nearly meets it: {line}"
    );
}
