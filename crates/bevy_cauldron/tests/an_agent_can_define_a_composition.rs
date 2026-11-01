//! **An agent defines a composition from placements, and takes it back** (E-072, `composition_define`).
//!
//! Through the registered BRP methods, as an agent's request goes: place two pieces, define a group
//! from them, see `compositions.ron` hold it; redefining needs `replace`; undo restores the file byte
//! for byte, both for a new group and for a redefinition.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;

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

fn undo(app: &mut App, id: &str) -> serde_json::Value {
    call(app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}))
}

fn place(app: &mut App, piece: &str, at: [f32; 2]) -> String {
    let a = apply(app, "placement_add", serde_json::json!({"piece": piece, "at": at}));
    assert_eq!(a["outcome"], "applied", "{a}");
    a["log_entry"]["after"]["id"].as_str().expect("a placement names its row").to_owned()
}

fn comps(root: &std::path::Path) -> String {
    std::fs::read_to_string(root.join("assets/grimoire/compositions.ron")).unwrap_or_default()
}

fn id_of(a: &serde_json::Value) -> String {
    assert_eq!(a["outcome"], "applied", "{a}");
    a["operation_id"].as_str().expect("an apply returns its id").to_owned()
}

#[test]
fn placements_become_a_group_and_undo_takes_it_back() {
    let (root, mut app) = editor("define_group_new");
    let a = place(&mut app, "crate", [1.5, 1.5]);
    let b = place(&mut app, "barrel", [2.5, 1.5]);
    let before = comps(&root);
    assert!(!before.contains("\"corner\""));

    let made = id_of(&apply(&mut app, "composition_define", serde_json::json!({"id": "Corner", "from": [a, b]})));
    let after = comps(&root);
    assert!(after.contains("id: \"corner\""), "the id is forced to snake_case and written: {after}");
    let project = app.world().resource::<Project>();
    let c = project.compositions.compositions.iter().find(|c| c.id == "corner").expect("the project holds it");
    assert_eq!(c.members.len(), 2);

    let back = undo(&mut app, &made);
    assert_eq!(back["outcome"], "applied", "{back}");
    // Absence and an empty file are one state to the loader (see `chooser.rs`), so a project that had
    // no file may be left with an empty one.
    let text = comps(&root);
    assert!(text.is_empty() || bevy_grimoire_core::composition::Compositions::parse(&text).unwrap().compositions.is_empty(), "undo leaves no group in compositions.ron: {text}");
    assert!(app.world().resource::<Project>().compositions.compositions.iter().all(|c| c.id != "corner"));
}

#[test]
fn redefining_needs_replace_and_undo_restores_the_old_definition() {
    let (root, mut app) = editor("define_group_replace");
    let a = place(&mut app, "crate", [1.5, 1.5]);
    let b = place(&mut app, "barrel", [2.5, 1.5]);
    id_of(&apply(&mut app, "composition_define", serde_json::json!({"id": "corner", "from": [a.clone(), b]})));
    let one = comps(&root);

    let refused = apply(&mut app, "composition_define", serde_json::json!({"id": "corner", "from": [a.clone()]}));
    assert_eq!(refused["outcome"], "refused", "{refused}");
    assert!(refused.to_string().contains("replace"), "the refusal says how to redefine: {refused}");
    assert_eq!(comps(&root), one, "a refusal writes nothing");

    let redefined = id_of(&apply(
        &mut app,
        "composition_define",
        serde_json::json!({"id": "corner", "from": [a], "replace": true}),
    ));
    assert_ne!(comps(&root), one);
    let project = app.world().resource::<Project>();
    assert_eq!(project.compositions.compositions.iter().find(|c| c.id == "corner").unwrap().members.len(), 1);

    let back = undo(&mut app, &redefined);
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(comps(&root), one, "the old definition is back, byte for byte");
}

#[test]
fn a_placement_the_map_does_not_hold_is_refused_by_name() {
    let (root, mut app) = editor("define_group_missing");
    let before = comps(&root);
    let a = apply(&mut app, "composition_define", serde_json::json!({"id": "g", "from": ["ghost@9"]}));
    assert_eq!(a["outcome"], "refused", "{a}");
    assert!(a.to_string().contains("ghost@9"), "{a}");
    assert_eq!(comps(&root), before);
}
