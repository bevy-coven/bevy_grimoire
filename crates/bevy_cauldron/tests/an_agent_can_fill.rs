//! **An agent can flood fill and save (S1.5, S1.6).**
//!
//! `F` lays every open floor cell reachable from the cursor, and `Cmd+S` writes the map. Both worked
//! for a person and neither had a name in the registry, so an agent could place no floor and could
//! not finish.
//!
//! # What is being proved
//!
//! 1. **A fill lays pieces and they are on disk**, so "filled" means saved rather than remembered.
//! 2. **Undo takes the whole fill away**, as one act — a fill is one thing to the person who pressed
//!    the key, and an undo stack that made them press Cmd+Z once per piece would model the code.
//! 3. **An unknown piece is refused by name**, with the route to the list.
//! 4. **A capped fill says where to carry on**, because a truncated fill looks exactly like a
//!    finished one and that sentence is the only thing that distinguishes them.
//! 5. **A save writes the person's unsaved edit**, so `dirty` is false afterwards and the file agrees.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

/// **A project with one floor piece and a map big enough to fill.**
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("floor", "alpha")
        .build("m")
}

/// **The same project with a map big enough to reach the fill cap.**
///
/// `MAX_CELLS` is 4,096 and the fixture's default map is 16 x 16 m — the cap is unreachable there, and
/// a test that cannot reach it proves nothing. **220 x 220 m, and the size is deliberate rather than
/// round**: at a 1 m pitch that is 48,400 cells and at 0.5 m it is 193,600, so the reachable region is
/// far past the cap at either pitch. A first version used 80 m and appeared to report *not* truncated at exactly 4,096
/// pieces; the cause was this test's own bug, not the fill's — see the note below on `log_entry`.
fn big_project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("floor", "alpha")
        .side(220.0)
        .build("m")
}

fn editor_with_bridge(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app
}

fn call(
    app: &mut App,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    use bevy::remote::RemoteMethodSystemId;
    use bevy::remote::RemoteMethods;
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err(format!("no method `{method}` is registered")),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .map_err(|e| format!("{method}: {e:?}"))?
        .map_err(|e| format!("{method}: {e:?}"))
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": operation,
            "parameters": parameters,
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"))
}

fn ready(name: &str) -> (std::path::PathBuf, App) {
    let root = project(name);
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }
    (root, app)
}

/// **1. An agent fills from a point and it is saved.**
#[test]
fn an_agent_fills_from_a_point_and_it_is_saved() {
    let (_root, mut app) = ready("fillagent");
    assert_eq!(
        app.world().resource::<OpenMap>().map.placements.len(),
        0,
        "the fixture must start empty for a fill to be observable"
    );

    let applied = apply(
        &mut app,
        "fill",
        serde_json::json!({"piece": "floor", "at": [0.5, 0.5]}),
    );
    assert_eq!(applied["outcome"], "applied", "the fill did not apply: {applied}");
    assert_eq!(applied["saved"], true, "the fill was not saved: {applied}");
    let n = app.world().resource::<OpenMap>().map.placements.len();
    assert!(n > 1, "the fill laid {n} piece(s), which is a placement rather than a fill");

    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
    let on_disk: bevy_grimoire_core::map::Map =
        bevy_grimoire_core::map::Map::parse(&text).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        on_disk.placements.len(),
        n,
        "the map holds the fill and the file does not"
    );
    assert!(
        on_disk.placements.iter().all(|p| p.descriptor == "floor"),
        "the fill laid something other than the piece it was asked for"
    );
}

/// **2. Undo takes the whole fill away**, as one act.
#[test]
fn undo_takes_the_fill_away() {
    let (_root, mut app) = ready("fillundo");
    apply(
        &mut app,
        "fill",
        serde_json::json!({"piece": "floor", "at": [0.5, 0.5]}),
    );
    let n = app.world().resource::<OpenMap>().map.placements.len();
    assert!(n > 1, "nothing was filled to undo");

    let undone = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"dry_run": false}),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(undone["outcome"], "applied", "the undo did not apply: {undone}");
    assert_eq!(
        app.world().resource::<OpenMap>().map.placements.len(),
        0,
        "one undo did not take the fill away — a fill is one act to the person who pressed the key"
    );
}

/// **3. An unknown piece is refused by name**, with the route to the list.
#[test]
fn an_unknown_piece_is_refused_by_name() {
    let (_root, mut app) = ready("fillbad");
    let refused = apply(
        &mut app,
        "fill",
        serde_json::json!({"piece": "lava", "at": [0.5, 0.5]}),
    );
    assert_eq!(refused["outcome"], "refused", "{refused}");
    let why = refused["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains("lava") && why.contains("grimoire_list_pieces"),
        "the refusal neither names the piece nor says where the real ones are: {why:?}"
    );
    assert_eq!(
        app.world().resource::<OpenMap>().map.placements.len(),
        0,
        "a refused fill laid something"
    );
}

/// **4. A capped fill says where to carry on.**
#[test]
fn a_capped_fill_says_where_to_carry_on() {
    let root = big_project("fillcap");
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }
    // **A map big enough to hit the cap.** `MAX_CELLS` is 4096 and the default map is 10 x 10 m at a
    // 1 m pitch — 100 cells — so the cap needs a map the operation cannot cover.
    let applied = apply(
        &mut app,
        "fill",
        serde_json::json!({"piece": "floor", "at": [0.5, 0.5]}),
    );
    assert_eq!(applied["outcome"], "applied", "{applied}");
    let summary = applied["summary"].as_str().unwrap_or_default();
    let laid = app.world().resource::<OpenMap>().map.placements.len();
    // **`after` is in the log entry, not in the answer.** `grimoire/operation_apply` returns the
    // summary, `affected`, `saved`, the operation id and `log_entry` — and the first version of this
    // test read `applied["after"]`, which is null, so `unwrap_or(false)` reported "not truncated"
    // no matter what the fill did. A missing field that defaults to the answering you expect is the
    // most expensive kind of test bug: it agreed with the code while measuring nothing.
    let truncated = applied["log_entry"]["after"]["truncated"]
        .as_bool()
        .unwrap_or_else(|| panic!("the fill's log line carries no `truncated`: {applied}"));
    assert!(
        truncated,
        "the fill laid {laid} of a possible 48,400 cells without reporting truncation, so the cap \
         is not being enforced at MAX_CELLS ({})",
        bevy_cauldron::fill::MAX_CELLS
    );
    assert!(
        laid <= bevy_cauldron::fill::MAX_CELLS,
        "a capped fill laid {laid}, which is past the {} cell cap",
        bevy_cauldron::fill::MAX_CELLS
    );
    assert!(
        summary.contains(bevy_cauldron::fill::CAP_ROUTE),
        "a capped fill did not say how to carry on, so it looks exactly like a finished one. \
         summary: {summary:?}"
    );
}

/// **5. An agent saves the person's unsaved edit.**
#[test]
fn an_agent_saves_the_persons_unsaved_edit() {
    let (_root, mut app) = ready("fillsave");
    apply(
        &mut app,
        "fill",
        serde_json::json!({"piece": "floor", "at": [0.5, 0.5]}),
    );
    // **An edit the person has not saved**: the same in-memory write `drop_piece` makes.
    {
        let mut open = app.world_mut().resource_mut::<OpenMap>();
        let row = open.map.placements.first_mut().expect("the fill laid something");
        row.at.0 += 1.0;
    }
    assert!(
        app.world().resource::<OpenMap>().dirty(),
        "the map does not report the edit as unsaved, so this test is not testing a save"
    );

    let saved = apply(&mut app, "map_save", serde_json::json!({}));
    assert_eq!(saved["outcome"], "applied", "the save did not apply: {saved}");
    assert_eq!(saved["saved"], true, "{saved}");
    assert!(
        !app.world().resource::<OpenMap>().dirty(),
        "the save reported success and the map is still unsaved"
    );

    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
    let on_disk: bevy_grimoire_core::map::Map =
        bevy_grimoire_core::map::Map::parse(&text).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        on_disk.placements.first().map(|p| p.at.0),
        app.world()
            .resource::<OpenMap>()
            .map
            .placements
            .first()
            .map(|p| p.at.0),
        "the file does not hold the edit the person made"
    );
}
