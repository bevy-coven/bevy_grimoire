//! **An agent can select a library piece on the Meshes shelf, and a row click is the same operation.**
//!
//! E-044: selecting a piece was a click handler that wrote the highlight by hand and no operation
//! named it, so an agent could arm a piece (`brush_arm`) but could not point the kit door at one.
//! `piece_select` is the registered operation; the shelf row click dispatches it. These tests go
//! through the registered BRP method, the way an agent's request goes, and assert on the editor's
//! own state.

use bevy::prelude::*;
use bevy::input::{ButtonInput, InputSystems};
use bevy_cauldron::filter::{Filters, Pane};
use bevy_cauldron::harness;
use bevy_cauldron::ops;
use bevy_cauldron::tiles::{ImportState, Mode};

use crate::fixtures;

fn editor_at(name: &str, mode: Mode) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .descriptor("site/gate", "beta")
        .build("m");
    let mut app =
        harness::build_headless_at(&root, "m", None, mode).unwrap_or_else(|e| panic!("{e}"));
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

fn select(app: &mut App, id: &str) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "piece_select",
            "parameters": {"id": id},
            "dry_run": false,
        }),
    )
}

fn selected(app: &App) -> Option<String> {
    app.world().resource::<ImportState>().selected_library_id.clone()
}

#[test]
fn the_registry_has_piece_select() {
    let op = ops::find("piece_select").expect("piece_select is a registered operation");
    assert_eq!(op.undo(), ops::Undo::None, "a selection changes no document");
}

#[test]
fn an_unknown_piece_is_refused_with_the_nearest_ids() {
    let mut app = editor_at("piece-select-unknown", Mode::Meshes);
    let answer = select(&mut app, "site/wal");
    assert_eq!(answer["outcome"], "refused", "{answer}");
    let why = answer["refusal"].as_str().unwrap_or_default();
    assert!(why.contains("site/wall"), "the refusal names the close id: {why}");
    assert_eq!(selected(&app), None, "a refused selection moves nothing");
}

#[test]
fn a_shown_piece_is_selected_and_the_highlight_moves() {
    let mut app = editor_at("piece-select-shown", Mode::Meshes);
    let answer = select(&mut app, "site/gate");
    assert_eq!(answer["outcome"], "applied", "{answer}");
    assert_eq!(selected(&app).as_deref(), Some("site/gate"));
    assert_eq!(app.world().resource::<ImportState>().focused_pack, None);
}

#[test]
fn selecting_needs_the_meshes_shelf_on_screen() {
    let mut app = editor_at("piece-select-map-door", Mode::Map);
    let answer = select(&mut app, "site/gate");
    assert_eq!(answer["outcome"], "refused", "{answer}");
    let why = answer["refusal"].as_str().unwrap_or_default();
    assert!(why.contains("meshes"), "the refusal names the shelf it needs: {why}");
    assert_eq!(selected(&app), None);
}

#[test]
fn a_piece_the_shelf_filter_hides_is_refused() {
    let mut app = editor_at("piece-select-filtered", Mode::Meshes);
    // `site/gate` contains no `w`, so a `w` filter hides it and keeps `site/wall`.
    app.world_mut().resource_mut::<Filters>().push_for_test(Pane::Candidates, 'w');
    let answer = select(&mut app, "site/gate");
    assert_eq!(answer["outcome"], "refused", "{answer}");
    let why = answer["refusal"].as_str().unwrap_or_default();
    assert!(why.contains("filter"), "the refusal says the filter hides it: {why}");
    assert_eq!(selected(&app), None);
}

fn press_arrow_down_once(mut keys: ResMut<ButtonInput<KeyCode>>, mut done: Local<bool>) {
    if !*done {
        keys.press(KeyCode::ArrowDown);
        *done = true;
    }
}

/// **An arrow walk on the Meshes shelf is a selection, so it writes the same log line a row click
/// and an agent's `piece_select` write.** E-044: the walk set `selected_library_id` by hand, so a
/// keyboard selection left no record in the operation log and no agent could see it.
#[test]
fn the_shelf_walk_selects_through_the_operation() {
    let mut app = editor_at("piece-select-walk", Mode::Meshes);
    select(&mut app, "site/gate");
    let log = app.world().resource::<bevy_cauldron::project::Project>().project_dir.join("oplog.jsonl");
    let count = |p: &std::path::Path| {
        std::fs::read_to_string(p)
            .unwrap_or_default()
            .lines()
            .filter(|l| l.contains("\"op\":\"piece_select\""))
            .count()
    };
    let before = count(&log);

    // Pressed in `PreUpdate` after `InputSystems`, which clears the keys each frame: the same
    // injection `a_person_removes_a_piece_and_undoes_it` uses.
    app.add_systems(PreUpdate, press_arrow_down_once.after(InputSystems));
    app.update();


    assert_eq!(selected(&app).as_deref(), Some("site/wall"), "the arrow moved the highlight");
    assert_eq!(
        count(&log),
        before + 1,
        "the walk wrote one piece_select line to the operation log"
    );
    let last = std::fs::read_to_string(&log).unwrap();
    let line: serde_json::Value = serde_json::from_str(last.lines().last().unwrap()).unwrap();
    assert_eq!(line["source"], "human", "a walk is a person's selection: {line}");
    assert_eq!(line["params"]["id"], "site/wall");
}
