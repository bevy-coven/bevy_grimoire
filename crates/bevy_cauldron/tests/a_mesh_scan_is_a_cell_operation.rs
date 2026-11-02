//! **An agent can mark the cells a mesh occupies, and `B` on the Meshes tab is the same operation
//! (E-002, #129 slice 19).**
//!
//! `B` (and the SCAN chip) read a library piece's mesh and marked the cells it occupies solid with a
//! write of its own, so an agent could only mark cells it already knew about, no log line named the scan,
//! and the registry's two-front-end rule listed `ScanMesh` as unmigrated. It is now `piece_set_cells`
//! with its `scan` verb. The first two tests go through the registered BRP method, the way an agent's
//! request goes; the last presses the real key, the way a person does.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{DerivedEdges, ImportState, Mode};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("site/wall", "alpha").build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

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

fn apply(app: &mut App, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "piece_set_cells", "parameters": parameters, "dry_run": false}),
    )
}

fn solid_cells(app: &App) -> usize {
    app.world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == "site/wall")
        .and_then(|d| d.subgrid.as_ref())
        .map_or(0, |g| g.cells.iter().filter(|c| c.solid).count())
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

/// **One call reads the mesh and marks the cells it occupies, as one log line and one undo; a second
/// scan, which would change nothing, is refused.**
#[test]
fn an_agent_scans_a_pieces_mesh_and_one_undo_clears_the_cells() {
    let mut app = editor("scan-agent");
    assert_eq!(solid_cells(&app), 0, "the fixture's lattice changed");
    let lines = log(&app).len();

    let a = apply(&mut app, serde_json::json!({"id": "site/wall", "scan": true}));
    assert_eq!(a["outcome"], "applied", "{a}");
    let marked = solid_cells(&app);
    assert!(marked > 0, "the scan marked no cell: {a}");
    assert!(
        a["summary"].as_str().unwrap_or_default().contains("scanned"),
        "the receipt does not say what was scanned: {a}"
    );
    assert_eq!(log(&app).len(), lines + 1, "a scan is one log line");
    assert!(
        std::fs::read_to_string(&app.world().resource::<Project>().library_path).expect("library").contains("solid: true"),
        "the library file does not hold the cells"
    );

    let again = apply(&mut app, serde_json::json!({"id": "site/wall", "scan": true}));
    assert_eq!(again["outcome"], "refused", "a scan that changes nothing was logged: {again}");

    let id = a["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let back = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(solid_cells(&app), 0, "one undo clears the scanned cells");
}

/// **A scan beside cells or another verb, a piece with no mesh and an unknown piece are refused, and
/// nothing is written.**
#[test]
fn bad_scans_are_refused_and_nothing_is_written() {
    let mut app = editor("scan-refused");
    let lines = log(&app).len();
    for (what, params) in [
        ("a scan beside cells", serde_json::json!({"id": "site/wall", "scan": true, "cells": [[0, 0, 0]]})),
        ("a scan beside another verb", serde_json::json!({"id": "site/wall", "scan": true, "solid": true, "cells": [[0, 0, 0]]})),
        ("an unknown piece", serde_json::json!({"id": "site/nope", "scan": true})),
    ] {
        let r = apply(&mut app, params);
        assert_eq!(r["outcome"], "refused", "{what} was accepted: {r}");
    }
    assert_eq!(solid_cells(&app), 0, "a refusal marked cells");
    assert_eq!(log(&app).len(), lines, "a refusal logged a line");
}

fn tap(app: &mut App, logical: Key, code: KeyCode) {
    for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: logical.clone(),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

/// **A person's `B` on a library piece is one `piece_set_cells` line stamped as a person's**, marks the
/// cells, stages the derived edges for Enter as it always did, and one Cmd+Z takes the cells back.
#[test]
fn a_persons_b_scans_through_the_operation_and_still_stages_the_edges() {
    let mut app = editor("scan-person");
    app.world_mut().resource_scope(|world, mut import: Mut<ImportState>| {
        let project = world.resource::<Project>();
        bevy_cauldron::editor::select_piece(&mut import, project, "site/wall");
    });
    for _ in 0..4 {
        app.update();
    }
    let lines = log(&app).len();
    let marked_before = solid_cells(&app);

    tap(&mut app, Key::Character("b".into()), bevy_cauldron::keys::binding(bevy_cauldron::keys::Action::ScanMesh).key);
    app.update();

    assert!(solid_cells(&app) > marked_before, "B marked no cell");
    let entries = log(&app);
    assert_eq!(entries.len(), lines + 1, "one press of B is one log line");
    let last = entries.last().expect("a line");
    assert_eq!(last.op, "piece_set_cells");
    assert_eq!(format!("{:?}", last.source), "Human", "a person's scan must be stamped as a person's");
    assert!(
        app.world().resource::<DerivedEdges>().0.is_some(),
        "the scan stages the edges it derived, which Enter keeps"
    );

    let held = bevy_cauldron::keys::MOD_KEYS[0];
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(held);
    tap(&mut app, Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified), KeyCode::KeyZ);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(held);
    app.update();
    assert_eq!(solid_cells(&app), marked_before, "one Cmd+Z must take the cells back");
}
