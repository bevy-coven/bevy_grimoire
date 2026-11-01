//! **The Meshes pane names the project standards the selected piece breaks** (#138 slice 4).
//!
//! Slices 2 and 3 told an agent (`grimoire_state.standards`, `piece_import`'s answer); a person
//! walking the Meshes shelf saw nothing. So the detail pane gains a `standards` row for the piece in
//! focus: one line per breach naming the rule, the dimension, the measured value and the bound. A
//! piece that meets every rule has no row. The rules are read from `standards.ron`, and an edit to
//! that file reaches the pane through the watcher, without reselecting the piece.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use serde_json::json;
use std::path::PathBuf;

use crate::fixtures;

const HUGE_AND_SANE: &str = r#"(version: 1, rules: [
    (name: "huge", dim: Width, min: Some(100.0)),
    (name: "sane", dim: Width, max: Some(1000.0)),
])"#;

const SANE: &str = r#"(version: 1, rules: [(name: "sane", dim: Width, max: Some(1000.0))])"#;

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
fn the_pane_names_each_broken_rule_and_not_the_met_ones() {
    let (mut app, _) = editor(
        &format!("pane_standards_{}", std::process::id()),
        HUGE_AND_SANE,
    );
    let texts = drawn(&mut app);
    let line = texts
        .iter()
        .find(|t| t.starts_with("huge:"))
        .unwrap_or_else(|| panic!("no `huge` breach line in the pane: {texts:?}"));
    assert!(
        line.contains("width"),
        "the line names the dimension: {line}"
    );
    assert!(
        line.contains("min 100.00 m"),
        "the line names the bound: {line}"
    );
    assert!(
        texts.iter().any(|t| t == "standards"),
        "the row is labelled: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t.starts_with("sane:")),
        "a met rule is not drawn: {texts:?}"
    );
}

#[test]
fn an_edit_to_the_file_reaches_the_pane_without_reselecting() {
    let (mut app, file) = editor(&format!("pane_standards_live_{}", std::process::id()), SANE);
    let texts = drawn(&mut app);
    assert!(
        !texts
            .iter()
            .any(|t| t == "standards" || t.contains(':') && t.starts_with("sane")),
        "a piece that meets every rule has no standards row: {texts:?}"
    );
    std::fs::write(&file, HUGE_AND_SANE).unwrap_or_else(|e| panic!("{e}"));
    // The watcher polls every 250 ms on the wall clock.
    let mut seen = false;
    for _ in 0..40 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        if drawn(&mut app).iter().any(|t| t.starts_with("huge:")) {
            seen = true;
            break;
        }
    }
    assert!(
        seen,
        "the edited rule never reached the pane: {:?}",
        drawn(&mut app)
    );
}
