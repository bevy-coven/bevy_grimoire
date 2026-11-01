//! **The kit's dead ends reach the agent and the person** (#142 slice 2).
//!
//! Slice 1 put `adjacency::dead_ends` in core: every declared face no piece of the kit meets at any
//! turn. Nobody could see it without calling the library function. So `grimoire_state` carries a
//! `dead_ends` section for the open project's kit, and the Meshes pane gains a `dead ends` row for
//! the piece in focus, one line per face nothing meets.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use serde_json::json;
use std::path::PathBuf;

use crate::fixtures;

/// A project whose `jamb` presents `[jamb_l jamb_r]` on its east face and nothing elsewhere, beside
/// a plain `crate` that declares no face at all. Nothing in the kit presents that pair back.
fn project(name: &str) -> PathBuf {
    let root = fixtures::Fixture::new(name)
        .edge_tokens(&["jamb_l", "jamb_r"])
        .descriptor("jamb", "alpha")
        .descriptor("crate", "alpha")
        .build("m");
    let lib = root.join("assets/grimoire/furniture/library.ron");
    let text = std::fs::read_to_string(&lib).unwrap_or_else(|e| panic!("{lib:?}: {e}"));
    let jamb_at = text.find("id: \"jamb\"").unwrap_or_else(|| panic!("no jamb row: {text}"));
    let (head, tail) = text.split_at(jamb_at);
    let cells = r#"subgrid: Some(( cells: [( at: (1, 0, 0), edge: Some("jamb_l") ), ( at: (1, 0, 1), edge: Some("jamb_r") )] ))"#;
    let tail = tail.replacen("subgrid: None", cells, 1);
    std::fs::write(&lib, format!("{head}{tail}")).unwrap_or_else(|e| panic!("{e}"));
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
fn grimoire_state_names_the_face_nothing_in_the_kit_meets() {
    let mut app = editor(&format!("dead_ends_state_{}", std::process::id()));
    let st = call(&mut app, "grimoire/state", json!({}));
    let de = &st["dead_ends"];
    let faces = de["faces"].as_array().unwrap_or_else(|| panic!("no dead_ends.faces: {de}"));
    let east = faces
        .iter()
        .find(|f| f["piece"] == "jamb" && f["dir"] == "E")
        .unwrap_or_else(|| panic!("the jamb's east face meets nothing: {de}"));
    assert_eq!(east["face"], json!(["jamb_l", "jamb_r"]), "{east}");
    assert!(
        east["message"].as_str().is_some_and(|m| m.contains("jamb_l jamb_r")),
        "{east}"
    );
    assert!(
        !faces.iter().any(|f| f["piece"] == "crate"),
        "a piece that declares no face has no dead end: {de}"
    );
    assert_eq!(de["total"], json!(faces.len()), "{de}");
}

#[test]
fn the_meshes_pane_names_the_selected_piece_s_dead_face() {
    let mut app = editor(&format!("dead_ends_pane_{}", std::process::id()));
    call(
        &mut app,
        "grimoire/operation_apply",
        json!({"operation": "door_open", "parameters": {"door": "kit"}, "dry_run": false}),
    );
    for _ in 0..4 {
        app.update();
    }
    let select = |app: &mut App, id: &str| {
        app.world_mut()
            .resource_mut::<bevy_cauldron::tiles::ImportState>()
            .selected_library_id = Some(id.to_owned());
    };
    select(&mut app, "jamb");
    let texts = drawn(&mut app);
    assert!(
        texts.iter().any(|t| t == "dead ends"),
        "the row is labelled: {texts:?}"
    );
    let line = texts
        .iter()
        .find(|t| t.starts_with("E:"))
        .unwrap_or_else(|| panic!("no line for the east face: {texts:?}"));
    assert!(line.contains("jamb_l jamb_r"), "the line names what the face presents: {line}");

    select(&mut app, "crate");
    let texts = drawn(&mut app);
    assert!(
        !texts.iter().any(|t| t == "dead ends"),
        "a piece with no dead face has no row: {texts:?}"
    );
}

/// Every text laid out on screen with a size, after a few frames.
fn drawn(app: &mut App) -> Vec<String> {
    for _ in 0..3 {
        app.update();
    }
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &ComputedNode)>();
    q.iter(world)
        .filter(|(_, n)| n.size().x > 0.0 && n.size().y > 0.0)
        .map(|(t, _)| t.0.clone())
        .collect()
}
