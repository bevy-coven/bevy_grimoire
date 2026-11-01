//! **An agent can write a token for each of several cells, and Enter on a mesh scan's derived edges is the
//! same operation (E-002, #129 slice 12).**
//!
//! Keeping the edges a scan derived wrote a library piece's lattice with a write of its own (cell by cell,
//! straight to `library.ron`), so an agent could never put one token on one cell and another on the next,
//! no log line named the keep, and the registry's two-front-end rule listed `AcceptEdges` as unmigrated. It
//! is now `piece_set_cells` with its `edges` verb. The first two tests go through the registered BRP
//! method, the way an agent's request goes; the last two press the real key on the Meshes tab, the way a
//! person does. All assert on the library the editor holds, the file on disk and the log.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{Derived, DerivedEdges, ImportState, Mode};

use crate::fixtures;

const SOLID: &str = bevy_grimoire_core::adjacency::EDGE_SOLID;
const OPEN: &str = bevy_grimoire_core::adjacency::EDGE_OPEN;

fn editor(name: &str) -> App {
    // Four cells wide: `wall` is the token the fixture's lattice carries, and `solid` and `open` are the two
    // a derivation names, declared so the project takes them.
    let root = fixtures::Fixture::new(name)
        .authored_lattice("site/wall", "alpha", "wall", 4)
        .edge_tokens(&[SOLID, OPEN])
        .build("m");
    let mut app =
        harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
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

fn edge(app: &App, at: (u32, u32, u32)) -> Option<String> {
    app.world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == "site/wall")
        .and_then(|d| d.subgrid.as_ref())
        .and_then(|g| g.cells.iter().find(|c| c.at == at))
        .and_then(|c| c.edge.clone())
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
        app.world().resource::<Project>().grimoire_dir.join(bevy_cauldron::project::LIBRARY_FILE),
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

/// **One call writes a different token on each cell, as one act: the file follows, one log line names
/// it, and an undo puts the whole lattice back.**
#[test]
fn an_agent_writes_a_token_for_each_cell_and_an_undo_puts_the_lattice_back() {
    let mut app = editor("keep-edges-agent");
    let first = lattice(&app);
    let lines_before = log(&app).len();

    let kept = apply(
        &mut app,
        serde_json::json!({"id": "site/wall", "edges": [
            {"at": [0, 0, 0], "token": SOLID},
            {"at": [1, 0, 0], "token": OPEN},
            {"at": [3, 0, 0], "token": "Solid"},
        ]}),
    );
    assert_eq!(kept["outcome"], "applied", "{kept}");
    assert_eq!(edge(&app, (0, 0, 0)).as_deref(), Some(SOLID));
    assert_eq!(edge(&app, (1, 0, 0)).as_deref(), Some(OPEN), "a second cell takes its own token, not the first's");
    assert_eq!(edge(&app, (3, 0, 0)).as_deref(), Some(SOLID), "a token is snake-cased as `edge` is");
    assert_eq!(edge(&app, (2, 0, 0)).as_deref(), Some("wall"), "a cell not named changed");
    assert!(file(&app).contains(OPEN), "the library file did not take the token: {}", file(&app));
    let lines = log(&app);
    assert_eq!(lines.len(), lines_before + 1, "three cells are one log line");
    assert_eq!(lines.last().map(|l| l.op.as_str()), Some("piece_set_cells"));

    let id = kept["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let undone = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(undone["outcome"], "applied", "{undone}");
    assert_eq!(lattice(&app), first, "one undo must put the whole lattice back");
}

/// **A bad request is refused whole, naming why, and the file, the lattice and the log are untouched.**
#[test]
fn bad_edges_are_refused_before_anything_is_written() {
    let mut app = editor("keep-edges-refused");
    let first = lattice(&app);
    let text = file(&app);
    let lines_before = log(&app).len();

    let cases = [
        // a token the project's vocabulary does not hold
        (serde_json::json!({"id": "site/wall", "edges": [{"at": [0, 0, 0], "token": "nope"}]}), "nope"),
        // a cell outside the lattice refuses the whole edit, not just itself
        (serde_json::json!({"id": "site/wall", "edges": [{"at": [0, 0, 0], "token": SOLID}, {"at": [99, 0, 0], "token": SOLID}]}), "outside"),
        // one cell, two tokens
        (serde_json::json!({"id": "site/wall", "edges": [{"at": [0, 0, 0], "token": SOLID}, {"at": [0, 0, 0], "token": OPEN}]}), "twice"),
        // nothing to write
        (serde_json::json!({"id": "site/wall", "edges": []}), "no edges"),
        // two verbs
        (serde_json::json!({"id": "site/wall", "edges": [{"at": [0, 0, 0], "token": SOLID}], "clear": true}), "exactly one"),
        // `edges` names its own cells
        (serde_json::json!({"id": "site/wall", "cells": [[0, 0, 0]], "edges": [{"at": [0, 0, 0], "token": SOLID}]}), "without `cells`"),
        // a piece the library does not have
        (serde_json::json!({"id": "site/none", "edges": [{"at": [0, 0, 0], "token": SOLID}]}), "site/none"),
    ];
    for (params, why) in cases {
        let r = apply(&mut app, params.clone());
        assert_ne!(r["outcome"], "applied", "{params} was applied: {r}");
        assert!(r.to_string().contains(why), "the refusal of {params} must say `{why}`: {r}");
        assert_eq!(lattice(&app), first, "{params} edited the lattice");
        assert_eq!(file(&app), text, "{params} wrote the file");
        assert_eq!(log(&app).len(), lines_before, "{params} logged a line");
    }
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

fn undo_chord(app: &mut App) {
    let held = bevy_cauldron::keys::MOD_KEYS[0];
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(held);
    tap(app, Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified), KeyCode::KeyZ);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(held);
    for _ in 0..3 {
        app.update();
    }
}

fn stage_and_keep(app: &mut App, cells: Vec<((u32, u32, u32), &'static str)>) {
    app.world_mut().resource_mut::<ImportState>().selected_library_id = Some("site/wall".to_owned());
    app.world_mut().insert_resource(DerivedEdges(Some(Derived { id: "site/wall".to_owned(), cells })));
    for _ in 0..3 {
        app.update();
    }
    let key = bevy_cauldron::keys::binding(bevy_cauldron::keys::Action::AcceptEdges).key;
    tap(app, Key::Enter, key);
    for _ in 0..3 {
        app.update();
    }
}

/// **A person's Enter on a staged derivation is one `piece_set_cells` line stamped as a person's, it writes
/// the staged tokens and no other cell, the proposal is spent, and Cmd+Z takes the whole keep back.** Before
/// slice 12 the key wrote `library.ron` itself and logged nothing (failed with the routing disabled: no line).
#[test]
fn a_persons_enter_keeps_the_derived_edges_through_the_operation_and_cmd_z_follows_it() {
    let mut app = editor("keep-edges-person");
    let first = lattice(&app);
    let lines_before = log(&app).len();

    stage_and_keep(&mut app, vec![((0, 0, 0), SOLID), ((1, 0, 0), OPEN)]);

    assert_eq!(edge(&app, (0, 0, 0)).as_deref(), Some(SOLID));
    assert_eq!(edge(&app, (1, 0, 0)).as_deref(), Some(OPEN));
    assert_eq!(edge(&app, (2, 0, 0)).as_deref(), Some("wall"), "the keep edited a cell that was not staged");
    assert!(app.world().resource::<DerivedEdges>().0.is_none(), "the proposal is spent");
    let lines = log(&app);
    assert_eq!(lines.len(), lines_before + 1, "one press is one log line");
    assert_eq!(lines.last().map(|l| l.op.as_str()), Some("piece_set_cells"));
    assert_eq!(
        lines.last().map(|l| format!("{:?}", l.source)),
        Some("Human".to_owned()),
        "a person's keep must be stamped as a person's"
    );

    undo_chord(&mut app);
    assert_eq!(lattice(&app), first, "one Cmd+Z must take the whole keep back");
}

/// **A staged token the vocabulary lacks is refused with the vocabulary's own words, and nothing is
/// written or logged.** The door the keep always had, kept when the keep became an operation.
#[test]
fn a_staged_token_the_project_lacks_is_refused_and_writes_nothing() {
    let mut app = editor("keep-edges-undeclared");
    let first = lattice(&app);
    let text = file(&app);
    let lines_before = log(&app).len();

    stage_and_keep(&mut app, vec![((0, 0, 0), "undeclared-token")]);

    let said = app.world().resource::<ImportState>().status.problem_text();
    assert!(said.contains("undeclared-token"), "the refusal must name the token: `{said}`");
    assert!(said.contains("vocab.ron"), "and say where to declare it: `{said}`");
    assert_eq!(lattice(&app), first, "a refused keep edited the lattice");
    assert_eq!(file(&app), text, "a refused keep wrote the file");
    assert_eq!(log(&app).len(), lines_before, "a refused keep logged a line");
}
