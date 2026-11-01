//! **An agent can take its own change back, by the id it was given, and the log says so.**
//!
//! The regression tests for E-009. `grimoire_apply_operation` returned an operation id and told the
//! agent there was no undo, so a wrong change could only be put back by hand. These go through the
//! registered BRP method, `grimoire/operation_undo`, the way the MCP server does, and assert on the
//! map the editor holds, on the file on disk, and on the log.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::ops;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
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

fn set(app: &mut App, parameters: serde_json::Value) -> String {
    let answer = call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "map_set_settings", "parameters": parameters, "dry_run": false}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    answer["operation_id"]
        .as_str()
        .expect("an apply returns its id")
        .to_owned()
}

fn undo(app: &mut App, id: Option<&str>) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": id, "dry_run": false}),
    )
}

fn held(app: &App) -> bevy_grimoire_core::map::Map {
    app.world().resource::<OpenMap>().map.clone()
}

fn on_disk(app: &App) -> bevy_grimoire_core::map::Map {
    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    bevy_grimoire_core::map::Map::parse(&text).unwrap_or_else(|e| panic!("{e}"))
}

fn log(app: &mut App) -> Vec<serde_json::Value> {
    let answer = call(
        app,
        "grimoire/oplog_read",
        serde_json::json!({"since_seq": 0, "limit": 100}),
    );
    answer["entries"].as_array().cloned().unwrap_or_default()
}

/// **The change is taken back in the editor, in the file, and as a new line in the log.**
#[test]
fn an_agents_change_is_taken_back_by_its_id() {
    let mut app = editor("undoid");
    let before = held(&app);
    let id = set(
        &mut app,
        serde_json::json!({"axis": "y", "axis_value": 41.0}),
    );
    assert_eq!(
        held(&app).bounds.1,
        41.0,
        "the change did not land, so there is nothing to undo"
    );

    let answer = undo(&mut app, Some(&id));
    assert_eq!(
        answer["outcome"], "applied",
        "the undo was refused: {answer}"
    );
    assert_eq!(answer["undoes"], id.as_str(), "{answer}");
    assert_eq!(
        held(&app).bounds,
        before.bounds,
        "the editor still holds the change"
    );
    assert_eq!(
        on_disk(&app).bounds,
        before.bounds,
        "the file still holds the change"
    );

    // **A new line, and the old one is still there.**
    let lines = log(&mut app);
    assert_eq!(
        lines.len(),
        2,
        "the undo did not append exactly one line: {lines:?}"
    );
    assert_eq!(
        lines[0]["operation_id"],
        id.as_str(),
        "the change's own line is gone: {lines:?}"
    );
    assert_eq!(
        lines[1]["undoes"],
        id.as_str(),
        "the new line does not name what it undid"
    );
}

/// **A human's newer change blocks undoing the agent's older one, and nothing changes.**
///
/// The size field dispatches the same operation with `Source::Human`, so the human's edit is the
/// newest line. Putting the agent's old values back would overwrite the human's work.
#[test]
fn a_human_edit_after_the_agents_blocks_the_older_undo() {
    let mut app = editor("undohuman");
    let agents = set(
        &mut app,
        serde_json::json!({"axis": "x", "axis_value": 33.0}),
    );

    // **The human's edit, through the same path the size field takes** (`editor.rs`'s size field
    // builds this context and dispatches this operation).
    let mut ctx = ops::Ctx {
        root: None,
        project: Some(
            app.world()
                .resource::<bevy_cauldron::project::Project>()
                .clone(),
        ),
        open: Some(app.world().resource::<OpenMap>().clone()),
        source: ops::Source::Human,
        session: ops::session_id(),
        proposal: None,
    };
    ops::dispatch(
        &mut ctx,
        "map_set_settings",
        &serde_json::json!({"axis": "z", "axis_value": 77.0}),
    )
    .unwrap_or_else(|e| panic!("the human's edit: {e}"));
    let fresh = ctx.open.take().expect("the map comes back");
    *app.world_mut().resource_mut::<OpenMap>() = fresh;
    let after_human = held(&app);
    let lines_before = log(&mut app).len();

    let answer = undo(&mut app, Some(&agents));
    assert_eq!(
        answer["outcome"], "refused",
        "the older undo went through: {answer}"
    );
    let why = answer["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains("Human") && why.contains("first"),
        "the refusal does not say whose newer change blocks it, or what to do: {why}"
    );
    assert_eq!(held(&app), after_human, "a refused undo changed the map");
    assert_eq!(
        log(&mut app).len(),
        lines_before,
        "a refused undo wrote to the log"
    );
}

/// **Undoing twice is refused by name, and undoing the undo puts the change back.**
#[test]
fn an_undo_cannot_be_repeated_and_undoing_it_redoes() {
    let mut app = editor("undoredo");
    let id = set(
        &mut app,
        serde_json::json!({"axis": "y", "axis_value": 12.0}),
    );
    let first = undo(&mut app, Some(&id));
    assert_eq!(first["outcome"], "applied", "{first}");
    let undo_id = first["operation_id"]
        .as_str()
        .expect("the undo has an id")
        .to_owned();

    let again = undo(&mut app, Some(&id));
    assert_eq!(
        again["outcome"], "refused",
        "the same change was taken back twice: {again}"
    );
    assert!(
        again["refusal"]
            .as_str()
            .unwrap_or_default()
            .contains(&undo_id),
        "{again}"
    );

    let redo = undo(&mut app, Some(&undo_id));
    assert_eq!(
        redo["outcome"], "applied",
        "undoing the undo was refused: {redo}"
    );
    assert_eq!(
        held(&app).bounds.1,
        12.0,
        "undoing the undo did not put the change back"
    );
}

/// **A preview resolves the same line and changes nothing.**
#[test]
fn a_preview_names_what_it_would_set_and_changes_nothing() {
    let mut app = editor("undopreview");
    let id = set(
        &mut app,
        serde_json::json!({"axis": "y", "axis_value": 19.0}),
    );
    let lines = log(&mut app).len();
    let answer = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": id, "dry_run": true}),
    );
    assert_eq!(answer["outcome"], "previewed", "{answer}");
    assert!(
        answer["would_set"]["bounds"].is_array(),
        "the preview does not say what it would set: {answer}"
    );
    assert_eq!(held(&app).bounds.1, 19.0, "a preview changed the map");
    assert_eq!(log(&mut app).len(), lines, "a preview wrote to the log");
}

/// **A field that was absent comes back absent** — the inverse clears what the change set.
#[test]
fn undoing_a_new_note_leaves_no_note() {
    let mut app = editor("undonote");
    assert_eq!(held(&app).note, None, "the fixture already has a note");
    let id = set(&mut app, serde_json::json!({"note": "a courtyard"}));
    assert_eq!(undo(&mut app, Some(&id))["outcome"], "applied");
    assert_eq!(
        held(&app).note,
        None,
        "the undo left the note the change added"
    );
    assert_eq!(
        on_disk(&app).note,
        None,
        "the file kept the note the change added"
    );
}

/// **With no id, the newest change is taken back; with nothing left, the answer says so.**
#[test]
fn no_id_means_the_newest_and_an_empty_log_is_a_refusal() {
    let mut app = editor("undonewest");
    let nothing = undo(&mut app, None);
    assert_eq!(
        nothing["outcome"], "refused",
        "an undo with nothing to undo was applied: {nothing}"
    );
    let _older = set(
        &mut app,
        serde_json::json!({"axis": "x", "axis_value": 21.0}),
    );
    let newer = set(
        &mut app,
        serde_json::json!({"axis": "x", "axis_value": 22.0}),
    );
    let answer = undo(&mut app, None);
    assert_eq!(
        answer["undoes"],
        newer.as_str(),
        "no id did not mean the newest change: {answer}"
    );
    assert_eq!(held(&app).bounds.0, 21.0);
}

/// **Two changes, two undos, and the map is back where it started** (review gap 1).
///
/// The first version counted an undo line as a standing change, so after taking back the newer
/// change, the older one was blocked by the undo itself, and an undo with no id toggled undo/redo.
#[test]
fn two_changes_are_taken_back_one_after_the_other() {
    let mut app = editor("undotwo");
    let start = held(&app).bounds;
    let older = set(
        &mut app,
        serde_json::json!({"axis": "x", "axis_value": 31.0}),
    );
    let newer = set(
        &mut app,
        serde_json::json!({"axis": "z", "axis_value": 32.0}),
    );
    let first = undo(&mut app, None);
    assert_eq!(
        first["change"],
        newer.as_str(),
        "the first undo did not take back the newer change: {first}"
    );
    let second = undo(&mut app, None);
    assert_eq!(
        second["outcome"], "applied",
        "the older change could not be taken back: {second}"
    );
    assert_eq!(second["change"], older.as_str(), "{second}");
    assert_eq!(
        held(&app).bounds,
        start,
        "two undos did not put the map back"
    );
    let third = undo(&mut app, None);
    assert_eq!(
        third["outcome"], "refused",
        "a third undo found something to take back: {third}"
    );
}

/// **After undo and redo, the change is undone again by its own id** (review gap 2).
#[test]
fn a_redone_change_is_undone_by_its_own_id() {
    let mut app = editor("undoredoid");
    let start = held(&app).bounds;
    let id = set(
        &mut app,
        serde_json::json!({"axis": "y", "axis_value": 14.0}),
    );
    let taken = undo(&mut app, Some(&id));
    let taken_id = taken["operation_id"]
        .as_str()
        .expect("an undo has an id")
        .to_owned();
    assert_eq!(
        undo(&mut app, Some(&taken_id))["outcome"],
        "applied",
        "the redo was refused"
    );
    assert_eq!(
        held(&app).bounds.1,
        14.0,
        "the redo did not put the change back"
    );

    let again = undo(&mut app, Some(&id));
    assert_eq!(
        again["outcome"], "applied",
        "the change's own id no longer takes it back: {again}"
    );
    assert_eq!(held(&app).bounds, start);
}

/// **A change made outside the log blocks the undo instead of being overwritten** (review gap 3).
///
/// The watcher adopts a map file edited in a text editor without writing a log line. Putting the
/// agent's old settings back over it would silently undo the person's edit.
#[test]
fn an_edit_outside_the_log_blocks_the_undo() {
    let mut app = editor("undooutside");
    let id = set(
        &mut app,
        serde_json::json!({"axis": "y", "axis_value": 15.0}),
    );
    // **What `watch::adopt_map` does with a hand-edited file:** the open map is replaced.
    app.world_mut().resource_mut::<OpenMap>().map.note = Some("edited by hand".to_owned());
    let edited = held(&app);

    let answer = undo(&mut app, Some(&id));
    assert_eq!(
        answer["outcome"], "refused",
        "the undo overwrote an edit it did not know about: {answer}"
    );
    let why = answer["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains("note") && why.contains("outside"),
        "the refusal does not say what changed or where: {why}"
    );
    assert_eq!(held(&app), edited, "a refused undo changed the map");
}

/// **An undo for one map is refused while another map is open** — the log is the project's.
#[test]
fn an_undo_is_refused_on_a_different_map() {
    let mut app = editor("undoothermap");
    let id = set(
        &mut app,
        serde_json::json!({"axis": "y", "axis_value": 16.0}),
    );
    app.world_mut().resource_mut::<OpenMap>().map.name = "another_map".to_owned();
    let answer = undo(&mut app, Some(&id));
    assert_eq!(
        answer["outcome"], "refused",
        "the undo wrote onto a map it did not change: {answer}"
    );
    assert!(
        answer["refusal"]
            .as_str()
            .unwrap_or_default()
            .contains("another_map"),
        "{answer}"
    );
}

/// **A newer change on another map does not block this map's undo** (review round 2, gap 1).
///
/// The log is the project's, and Sanctuary's project has two maps. A person changing `m2` while the
/// agent works on `m` must not stop the agent taking its own change back, and must not be the
/// thing the refusal tells the agent to undo.
#[test]
fn a_change_on_another_map_does_not_block_this_maps_undo() {
    let mut app = editor("undocrossmap");
    let start = held(&app).bounds;
    let agents = set(
        &mut app,
        serde_json::json!({"axis": "y", "axis_value": 17.0}),
    );

    // **A person's change to another map in the same project**, through the same operation.
    let here = app.world().resource::<OpenMap>().clone();
    let mut other = here.map.clone();
    other.name = "m2".to_owned();
    let other_path = here.map_path.with_file_name("m2.map.ron");
    let mut ctx = ops::Ctx {
        root: None,
        project: Some(
            app.world()
                .resource::<bevy_cauldron::project::Project>()
                .clone(),
        ),
        open: Some(OpenMap::new(other, other_path)),
        source: ops::Source::Human,
        session: ops::session_id(),
        proposal: None,
    };
    ops::dispatch(
        &mut ctx,
        "map_set_settings",
        &serde_json::json!({"axis": "x", "axis_value": 90.0}),
    )
    .unwrap_or_else(|e| panic!("the change to m2: {e}"));

    let answer = undo(&mut app, None);
    assert_eq!(
        answer["outcome"], "applied",
        "m2's change blocked m's undo: {answer}"
    );
    assert_eq!(
        answer["change"],
        agents.as_str(),
        "the bare undo picked a change on another map: {answer}"
    );
    assert_eq!(held(&app).bounds, start);
}

/// **A logged change is named as the reason, not "outside the log"** (review round 2, gap 2).
#[test]
fn a_redo_blocked_by_a_logged_change_names_that_change() {
    let mut app = editor("undonames");
    set(
        &mut app,
        serde_json::json!({"axis": "x", "axis_value": 41.0}),
    );
    set(
        &mut app,
        serde_json::json!({"axis": "z", "axis_value": 42.0}),
    );
    let undo_b = undo(&mut app, None)["operation_id"]
        .as_str()
        .expect("an id")
        .to_owned();
    let undo_a = undo(&mut app, None)["operation_id"]
        .as_str()
        .expect("an id")
        .to_owned();
    let before = held(&app);

    let answer = undo(&mut app, Some(&undo_b));
    assert_eq!(
        answer["outcome"], "refused",
        "the redo overwrote a newer logged change: {answer}"
    );
    let why = answer["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains(&undo_a),
        "the refusal does not name the logged change in the way: {why}"
    );
    assert!(
        !why.contains("outside"),
        "the refusal blames an edit outside the log: {why}"
    );
    assert_eq!(held(&app), before);
}
