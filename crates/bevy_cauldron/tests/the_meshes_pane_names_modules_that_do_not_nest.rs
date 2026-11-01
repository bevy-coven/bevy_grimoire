//! **The Meshes pane names the sub-kit modules that do not nest** (#168 slice 3).
//!
//! Slices 1 and 2 found the pairs of `standards.ron` rules whose modules do not nest and told an
//! agent (`grimoire_state.standards.unnested`); a person on the Meshes shelf saw nothing, because
//! every piece can meet its own rule while two sub-kits still drift off each other's grid. So the
//! piece in focus shows, in its `standards` row, each such pair one of whose rules covers it, in
//! the words `grimoire_state` uses. A pair that covers it not at all, or modules that nest, add no line.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use serde_json::json;
use std::path::PathBuf;

use crate::fixtures;

/// The fixture `crate` is a `prop`, 1 m by 1 m: it meets `fine module` and is not covered by
/// `odd module`, whose 0.4 m does not nest with 0.5 m (they meet every 2 m).
const ONE_COVERS_IT: &str = r#"(version: 1, rules: [
    (name: "fine module", dim: Width, multiple_of: Some(0.5)),
    (name: "odd module", kind: Some("fence"), dim: Depth, multiple_of: Some(0.4)),
])"#;

/// `wall module` and `fence module` do not nest, but neither covers a `prop`; `fine module` and
/// `base module` cover it, and their 0.5 m nests with every module in the file. It meets both.
const NONE_FOR_IT: &str = r#"(version: 1, rules: [
    (name: "wall module", kind: Some("wall"), dim: Width, multiple_of: Some(2.0)),
    (name: "fence module", kind: Some("fence"), dim: Width, multiple_of: Some(1.5)),
    (name: "fine module", dim: Width, multiple_of: Some(0.5)),
    (name: "base module", dim: Depth, multiple_of: Some(0.5)),
])"#;

/// The editor on the Meshes tab with `crate` selected, `standards.ron` written before it opens.
fn editor(name: &str, standards: &str) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(name)
        .descriptor("crate", "alpha")
        .build("m");
    let file = root
        .join("assets/grimoire")
        .join(bevy_grimoire_core::standards::FILE);
    std::fs::write(&file, standards).unwrap_or_else(|e| panic!("{e}"));
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    open_kit_door(&mut app);
    app.world_mut()
        .resource_mut::<bevy_cauldron::tiles::ImportState>()
        .selected_library_id = Some("crate".to_owned());
    (app, file)
}

fn open_kit_door(app: &mut App) {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app
        .world()
        .resource::<RemoteMethods>()
        .get("grimoire/operation_apply")
        .cloned()
    else {
        panic!("no method `grimoire/operation_apply` is registered");
    };
    app.world_mut()
        .run_system_with(
            id,
            Some(
                json!({"operation": "door_open", "parameters": {"door": "kit"}, "dry_run": false}),
            ),
        )
        .unwrap_or_else(|e| panic!("door_open: {e:?}"))
        .unwrap_or_else(|e| panic!("door_open: {e:?}"));
    for _ in 0..4 {
        app.update();
    }
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

#[test]
fn a_piece_under_one_rule_of_the_pair_names_the_pair() {
    let (mut app, _) = editor(
        &format!("pane_unnested_{}", std::process::id()),
        ONE_COVERS_IT,
    );
    let texts = drawn(&mut app);
    let line = texts
        .iter()
        .find(|t| t.contains("do not nest"))
        .unwrap_or_else(|| panic!("no line naming the unnested pair in the pane: {texts:?}"));
    assert_eq!(
        line,
        "fine module (width, 0.5 m) and odd module (depth, 0.4 m) do not nest: \
         they share a grid line only every 2 m",
        "the line is grimoire_state's message for the pair"
    );
    assert!(
        texts.iter().any(|t| t == "standards"),
        "the pair is in the standards row, though the piece breaks no rule: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t.starts_with("fine module:")),
        "the piece meets its own rule, so no breach is drawn: {texts:?}"
    );
}

#[test]
fn pairs_that_do_not_cover_the_piece_or_that_nest_add_nothing() {
    let (mut app, _) = editor(
        &format!("pane_unnested_none_{}", std::process::id()),
        NONE_FOR_IT,
    );
    let texts = drawn(&mut app);
    assert!(
        !texts.iter().any(|t| t.contains("do not nest")),
        "a pair covering another kind, or modules that nest, are not this piece's: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t == "standards"),
        "a piece with no breach and no unnested pair of its own has no standards row: {texts:?}"
    );
}
