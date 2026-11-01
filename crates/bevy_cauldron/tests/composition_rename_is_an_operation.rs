//! **An agent can rename a composition, and the COMPOSE tab's `I` field is the same operation
//! (E-002, #129 slice 2).**
//!
//! Renaming a composition was `Project::rename_composition`, called by the COMPOSE tab's name field
//! and named by no operation, so an agent could not rename one and nothing could undo it. These
//! tests go through the registered BRP methods, the way an agent's request goes: the composition,
//! the open map's stamp and both files follow the rename, undo puts every one back, and a name that
//! is taken or a composition that does not exist is refused with nothing written.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name)
        .sized_descriptor("crate", "alpha", 1.0, 1.0)
        .sized_descriptor("barrel", "alpha", 1.0, 1.0)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    (root, app)
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

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    )
}

/// **Two groups, `corner` stamped on the open map and `pair` not**, so a rename has a map to follow
/// it and a taken name to refuse.
fn with_stamped_group(name: &str) -> (std::path::PathBuf, App) {
    let (root, mut app) = editor(name);
    let mut ids = vec![];
    for (piece, at) in [("crate", [1.5, 1.5]), ("barrel", [2.5, 1.5])] {
        let a = apply(&mut app, "placement_add", serde_json::json!({"piece": piece, "at": at}));
        ids.push(a["log_entry"]["after"]["id"].as_str().expect("a row").to_owned());
    }
    for id in ["corner", "pair"] {
        let d = apply(&mut app, "composition_define", serde_json::json!({"id": id, "from": ids}));
        assert_eq!(d["outcome"], "applied", "{d}");
    }
    let s = apply(&mut app, "stamp_add", serde_json::json!({"of": "corner", "at": [6.5, 6.5]}));
    assert_eq!(s["outcome"], "applied", "{s}");
    (root, app)
}

fn ids(app: &App) -> Vec<String> {
    let mut ids: Vec<String> = app
        .world()
        .resource::<Project>()
        .compositions
        .compositions
        .iter()
        .map(|c| c.id.clone())
        .collect();
    ids.sort();
    ids
}

fn stamped(app: &App) -> Vec<String> {
    app.world().resource::<OpenMap>().map.stamps.iter().map(|s| s.of.clone()).collect()
}

fn files(root: &std::path::Path, app: &App) -> (String, String) {
    let comps = std::fs::read_to_string(root.join("assets/grimoire/compositions.ron")).unwrap_or_default();
    let map = std::fs::read_to_string(app.world().resource::<OpenMap>().map_path.clone())
        .expect("the map file");
    (comps, map)
}

#[test]
fn an_agent_renames_a_composition_its_stamps_follow_and_undo_puts_it_back() {
    let (root, mut app) = with_stamped_group("comp-rename-agent");
    let before = files(&root, &app);

    let a = apply(
        &mut app,
        "composition_rename",
        serde_json::json!({"composition": "corner", "name": "hall"}),
    );
    assert_eq!(a["outcome"], "applied", "the rename was not applied: {a}");
    assert_eq!(ids(&app), ["hall", "pair"], "the project still holds the old name");
    assert_eq!(stamped(&app), ["hall"], "the open map's stamp still names the old composition");
    let (comps, map) = files(&root, &app);
    assert!(comps.contains("\"hall\"") && !comps.contains("\"corner\""), "compositions.ron: {comps}");
    assert!(map.contains("\"hall\""), "the map file's stamp does not follow: {map}");

    let id = a["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let back = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": id, "dry_run": false}),
    );
    assert_eq!(back["outcome"], "applied", "undo was refused: {back}");
    assert_eq!(ids(&app), ["corner", "pair"]);
    assert_eq!(stamped(&app), ["corner"]);
    assert_eq!(files(&root, &app), before, "undo must put both files back byte for byte");
}

#[test]
fn a_taken_name_and_an_unknown_composition_are_refused_and_nothing_is_written() {
    let (root, mut app) = with_stamped_group("comp-rename-refused");
    let before = files(&root, &app);

    let taken = apply(
        &mut app,
        "composition_rename",
        serde_json::json!({"composition": "corner", "name": "pair"}),
    );
    assert_eq!(taken["outcome"], "refused", "a taken name was applied: {taken}");
    let unknown = apply(
        &mut app,
        "composition_rename",
        serde_json::json!({"composition": "cornr", "name": "hall"}),
    );
    assert_eq!(unknown["outcome"], "refused", "a typo was applied: {unknown}");
    assert!(
        unknown.to_string().contains("corner"),
        "the refusal does not name the compositions an agent can rename: {unknown}"
    );
    let empty = apply(
        &mut app,
        "composition_rename",
        serde_json::json!({"composition": "corner", "name": "  "}),
    );
    assert_eq!(empty["outcome"], "refused", "an empty name was applied: {empty}");

    assert_eq!(ids(&app), ["corner", "pair"]);
    assert_eq!(files(&root, &app), before, "a refusal wrote a file");
}

/// **The COMPOSE tab's field and the operation leave the same state**, because the field's Enter
/// dispatches it: `editor::rename_composition` is the field's door.
#[test]
fn the_compose_field_renames_through_the_operation() {
    let (root, mut app) = with_stamped_group("comp-rename-field");
    let receipt = app.world_mut().resource_scope(|world: &mut World, mut project: Mut<Project>| {
        let mut open = world.resource_mut::<OpenMap>();
        bevy_cauldron::editor::rename_composition(&mut project, Some(&mut open), "corner", "Hall Two")
    });
    let receipt = receipt.unwrap_or_else(|e| panic!("the field's rename was refused: {e}"));
    assert!(receipt.contains("hall_two"), "the receipt does not name the new id: {receipt}");
    assert_eq!(ids(&app), ["hall_two", "pair"]);
    assert_eq!(stamped(&app), ["hall_two"]);
    assert!(files(&root, &app).1.contains("\"hall_two\""));
}
