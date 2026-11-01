//! **An agent can mark, open, type and forget the cells of a library piece's lattice, and `Z`, `X` and `V`
//! on the Meshes tab are the same operation (E-002, #129 slice 10).**
//!
//! The three keys, the grid's chips and its headers wrote a piece's cells with a write of their own, named
//! by no operation, so an agent could not mark a cell solid or open, no log line named the edit, and the
//! registry's two-front-end rule listed the keys as unmigrated. They are now the registered operation
//! `piece_set_cells`. The first three tests go through the registered BRP methods, the way an agent's
//! request goes; the last presses the real key and Cmd+Z on the Meshes tab, the way a person does. All
//! assert on the library the editor holds, the file on disk and the log.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_grimoire_mapper::harness;
use bevy_grimoire_mapper::ops::{self, Entry};
use bevy_grimoire_mapper::project::Project;
use bevy_grimoire_mapper::tiles::{CellEdit, ImportState, Mode};

use crate::fixtures;

fn editor(name: &str) -> App {
    // Four cells wide, so the piece has four divisions across and a lattice that fills its run face is
    // legal. `wall` is the edge token the fixture's vocabulary already declares.
    let root = fixtures::Fixture::new(name)
        .authored_lattice("site/wall", "alpha", "wall", 4)
        .descriptor("site/other", "alpha")
        .build("m");
    let mut app =
        harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_grimoire_mapper::ops_brp::OpsBridgePlugin);
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

fn apply(app: &mut App, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "piece_set_cells", "parameters": parameters, "dry_run": false}),
    )
}

fn cell(app: &App, at: (u32, u32, u32)) -> Option<(bool, Option<String>)> {
    app.world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == "site/wall")
        .and_then(|d| d.subgrid.as_ref())
        .and_then(|g| g.cells.iter().find(|c| c.at == at))
        .map(|c| (c.solid, c.edge.clone()))
}

fn lattice(app: &App) -> Option<bevy_grimoire_core::descriptor::Subgrid> {
    app.world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == "site/wall")
        .and_then(|d| d.subgrid.clone())
}

fn file(app: &App) -> String {
    std::fs::read_to_string(
        app.world()
            .resource::<Project>()
            .grimoire_dir
            .join(bevy_grimoire_mapper::project::LIBRARY_FILE),
    )
    .expect("the library file")
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

/// **One call per verb edits the cells, the file follows, one log line names each edit, and an undo
/// puts the whole lattice back whatever was done to the piece's other cells since.**
#[test]
fn an_agent_edits_cells_and_an_undo_puts_the_lattice_back() {
    let mut app = editor("piece-cells-agent");
    assert_eq!(cell(&app, (1, 0, 0)), Some((true, Some("wall".to_owned()))), "the fixture's cells");
    let first = lattice(&app);
    let lines_before = log(&app).len();

    // Open two cells at once: one act, the token on them stays.
    let opened = apply(&mut app, serde_json::json!({"id": "site/wall", "cells": [[0, 0, 0], [1, 0, 0]], "solid": false}));
    assert_eq!(opened["outcome"], "applied", "{opened}");
    assert_eq!(cell(&app, (0, 0, 0)), Some((false, Some("wall".to_owned()))));
    assert_eq!(cell(&app, (1, 0, 0)), Some((false, Some("wall".to_owned()))));
    assert_eq!(cell(&app, (2, 0, 0)), Some((true, Some("wall".to_owned()))), "a cell not named changed");
    assert!(opened["summary"].as_str().is_some_and(|s| s.contains("2 cells open")), "{opened}");

    // Take a token off, then forget a cell.
    let off = apply(&mut app, serde_json::json!({"id": "site/wall", "cells": [[2, 0, 0]], "edge": ""}));
    assert_eq!(off["outcome"], "applied", "{off}");
    assert_eq!(cell(&app, (2, 0, 0)), Some((true, None)), "the token comes off and the cell stays solid");
    let gone = apply(&mut app, serde_json::json!({"id": "site/wall", "cells": [[3, 0, 0]], "clear": true}));
    assert_eq!(gone["outcome"], "applied", "{gone}");
    assert_eq!(cell(&app, (3, 0, 0)), None, "a cleared cell is forgotten");
    assert_eq!(log(&app).len(), lines_before + 3, "each edit is one log line");
    assert!(log(&app).iter().rev().take(3).all(|l| l.op == "piece_set_cells"));
    assert_ne!(lattice(&app), first);
    assert!(file(&app).contains("wall"), "the library file: {}", file(&app));

    // **The newest edit's undo takes back that edit and no other.**
    let id = gone["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let back = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(cell(&app, (3, 0, 0)), Some((true, Some("wall".to_owned()))), "the forgotten cell came back");
    assert_eq!(cell(&app, (2, 0, 0)), Some((true, None)), "the undo took back more than its own edit");

    // **An edit to ANOTHER piece must not block this piece's undo.**
    let other = apply(&mut app, serde_json::json!({"id": "site/other", "cells": [[0, 0, 0]], "solid": true}));
    assert_eq!(other["outcome"], "applied", "{other}");
    let id = off["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let back = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(back["outcome"], "applied", "another piece's edit blocked this undo: {back}");
    assert_eq!(cell(&app, (2, 0, 0)), Some((true, Some("wall".to_owned()))), "the token came back");
    assert_eq!(cell(&app, (0, 0, 0)), Some((false, Some("wall".to_owned()))), "the undo took back the older, still-standing edit");
}

/// **Cells outside the lattice, an unknown piece, no verb, two verbs, no cells, a forged put-back, a
/// token the vocabulary lacks, a typo'd field and an edit that changes nothing are refused, and nothing
/// is written.**
#[test]
fn a_bad_request_is_refused_and_writes_nothing() {
    let mut app = editor("piece-cells-refused");
    let file_before = file(&app);
    let lattice_before = lattice(&app);
    let lines_before = log(&app).len();

    for (what, parameters) in [
        ("a cell outside the lattice", serde_json::json!({"id": "site/wall", "cells": [[0, 0, 0], [9, 0, 0]], "solid": false})),
        ("an unknown piece", serde_json::json!({"id": "site/nope", "cells": [[0, 0, 0]], "solid": true})),
        ("no verb", serde_json::json!({"id": "site/wall", "cells": [[0, 0, 0]]})),
        ("two verbs", serde_json::json!({"id": "site/wall", "cells": [[0, 0, 0]], "solid": true, "clear": true})),
        ("no cells", serde_json::json!({"id": "site/wall", "cells": [], "solid": true})),
        ("an agent's put-back", serde_json::json!({"id": "site/wall", "restore": {"subgrid": null}})),
        ("a token the vocabulary lacks", serde_json::json!({"id": "site/wall", "cells": [[0, 0, 0]], "edge": "no_such_token"})),
        ("a typo'd field", serde_json::json!({"id": "site/wall", "cell": [[0, 0, 0]], "solid": true})),
        ("a change that changes nothing", serde_json::json!({"id": "site/wall", "cells": [[0, 0, 0]], "solid": true})),
    ] {
        let r = apply(&mut app, parameters);
        assert_eq!(r["outcome"], "refused", "{what} was accepted: {r}");
    }
    let outside = apply(&mut app, serde_json::json!({"id": "site/wall", "cells": [[9, 0, 0]], "solid": true}));
    assert!(
        outside["refusal"].as_str().is_some_and(|r| r.contains("9,0,0") && r.contains("outside")),
        "the refusal must name the cell: {outside}"
    );
    assert_eq!(file(&app), file_before, "a refusal wrote the library file");
    assert_eq!(lattice(&app), lattice_before, "a refusal changed the lattice the editor holds");
    assert_eq!(log(&app).len(), lines_before, "a refusal logged a line");
}

/// **A preview says what the edit would do and writes nothing.**
#[test]
fn a_preview_writes_nothing() {
    let mut app = editor("piece-cells-preview");
    let file_before = file(&app);
    let lattice_before = lattice(&app);
    let lines_before = log(&app).len();
    let p = call(
        &mut app,
        "grimoire/operation_preview",
        serde_json::json!({"operation": "piece_set_cells", "parameters": {"id": "site/wall", "cells": [[0, 0, 0]], "clear": true}}),
    );
    assert!(p.to_string().contains("cleared"), "the preview must say what it would do: {p}");
    let refused = call(
        &mut app,
        "grimoire/operation_preview",
        serde_json::json!({"operation": "piece_set_cells", "parameters": {"id": "site/wall", "cells": [[7, 0, 0]], "clear": true}}),
    );
    assert!(refused.to_string().contains("outside"), "a preview must refuse what the commit would: {refused}");
    assert_eq!(file(&app), file_before, "a preview wrote the library file");
    assert_eq!(lattice(&app), lattice_before, "a preview edited the lattice");
    assert_eq!(log(&app).len(), lines_before, "a preview logged a line");
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

fn undo_chord(app: &mut App, redo: bool) {
    let held = bevy_grimoire_mapper::keys::MOD_KEYS[0];
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(held);
    if redo {
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ShiftLeft);
    }
    tap(app, Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified), KeyCode::KeyZ);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(held);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::ShiftLeft);
    for _ in 0..3 {
        app.update();
    }
}

/// **A person's cell key on a library piece is one `piece_set_cells` line stamped as a person's, it edits
/// the selected cell and no other, Cmd+Z takes it back and Cmd+Shift+Z does it again.** Before slice 10
/// the key wrote `library.ron` itself and logged nothing.
#[test]
fn a_persons_cell_key_edits_through_the_operation_and_cmd_z_and_redo_follow_it() {
    let mut app = editor("piece-cells-person");
    app.world_mut().resource_mut::<ImportState>().selected_library_id = Some("site/wall".to_owned());
    app.world_mut().resource_mut::<CellEdit>().at = Some((1, 0, 0));
    for _ in 0..3 {
        app.update();
    }
    let lines_before = log(&app).len();
    let key = bevy_grimoire_mapper::keys::binding(bevy_grimoire_mapper::keys::Action::CellSolid).key;

    // One cell keeps the toggle: a solid cell opens.
    tap(&mut app, Key::Character("z".into()), key);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(cell(&app, (1, 0, 0)), Some((false, Some("wall".to_owned()))), "the key did not open the selected cell");
    assert_eq!(cell(&app, (0, 0, 0)), Some((true, Some("wall".to_owned()))), "the key edited a cell that was not selected");
    let lines = log(&app);
    assert_eq!(lines.len(), lines_before + 1, "one press is one log line");
    assert_eq!(lines.last().map(|l| l.op.as_str()), Some("piece_set_cells"));
    assert_eq!(
        lines.last().map(|l| format!("{:?}", l.source)),
        Some("Human".to_owned()),
        "a person's edit must be stamped as a person's"
    );

    undo_chord(&mut app, false);
    assert_eq!(cell(&app, (1, 0, 0)), Some((true, Some("wall".to_owned()))), "one Cmd+Z must take the edit back");

    undo_chord(&mut app, true);
    assert_eq!(cell(&app, (1, 0, 0)), Some((false, Some("wall".to_owned()))), "redo must make it again");
}
