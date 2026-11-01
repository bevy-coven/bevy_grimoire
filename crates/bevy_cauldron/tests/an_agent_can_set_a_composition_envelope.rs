//! **An agent changes a composition's envelope, and takes it back** (E-072 slice 3,
//! `composition_set_envelope`). Through the registered BRP methods: define a group (anchored), make
//! it bounded, see the file and project say so, make it anchored again, undo each step.

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
fn an_envelope_flips_both_ways_and_undo_restores_each() {
    let (root, mut app) = editor("set_envelope");
    let a = place(&mut app, "crate", [1.5, 1.5]);
    let b = place(&mut app, "barrel", [2.5, 1.5]);
    id_of(&apply(&mut app, "composition_define", serde_json::json!({"id": "corner", "from": [a, b]})));
    let env = |app: &App| {
        app.world().resource::<Project>().compositions.compositions.iter().find(|c| c.id == "corner").unwrap().envelope
    };
    // A group kept from placements starts out bounded (fitted); make it anchored to start the round trip.
    let start = id_of(&apply(&mut app, "composition_set_envelope", serde_json::json!({"id": "corner", "envelope": "anchored"})));
    assert_eq!(env(&app), bevy_grimoire_core::composition::Envelope::Anchored);
    let defined = comps(&root);

    let up = id_of(&apply(&mut app, "composition_set_envelope", serde_json::json!({"id": "corner", "envelope": "bounded", "size": [4.0, 2.5, 4.0]})));
    assert_eq!(env(&app), bevy_grimoire_core::composition::Envelope::Bounded { size: (4.0, 2.5, 4.0) });
    assert!(comps(&root).contains("Bounded"), "{}", comps(&root));

    let down = id_of(&apply(&mut app, "composition_set_envelope", serde_json::json!({"id": "corner", "envelope": "anchored"})));
    assert_eq!(env(&app), bevy_grimoire_core::composition::Envelope::Anchored);

    assert_eq!(undo(&mut app, &down)["outcome"], "applied");
    assert_eq!(env(&app), bevy_grimoire_core::composition::Envelope::Bounded { size: (4.0, 2.5, 4.0) });
    assert_eq!(undo(&mut app, &up)["outcome"], "applied");
    assert_eq!(comps(&root), defined, "undo puts the file back byte for byte");
    assert_eq!(undo(&mut app, &start)["outcome"], "applied");
    assert!(matches!(env(&app), bevy_grimoire_core::composition::Envelope::Bounded { .. }));
}

#[test]
fn a_bounded_envelope_without_a_size_is_fitted_and_bad_input_is_refused_untouched() {
    let (root, mut app) = editor("set_envelope_refuse");
    let a = place(&mut app, "crate", [1.5, 1.5]);
    id_of(&apply(&mut app, "composition_define", serde_json::json!({"id": "corner", "from": [a]})));
    let before = comps(&root);

    let fitted = apply(&mut app, "composition_set_envelope", serde_json::json!({"id": "corner", "envelope": "bounded"}));
    assert_eq!(fitted["outcome"], "applied", "{fitted}");
    let project = app.world().resource::<Project>();
    let c = project.compositions.compositions.iter().find(|c| c.id == "corner").unwrap();
    assert!(matches!(c.envelope, bevy_grimoire_core::composition::Envelope::Bounded { .. }));
    let kept = comps(&root);

    for bad in [
        serde_json::json!({"id": "ghost", "envelope": "bounded"}),
        serde_json::json!({"id": "corner", "envelope": "round"}),
        serde_json::json!({"id": "corner", "envelope": "bounded", "size": [0.0, 2.0, 2.0]}),
        serde_json::json!({"id": "corner", "envelope": "anchored", "size": [1.0, 1.0, 1.0]}),
    ] {
        let r = apply(&mut app, "composition_set_envelope", bad.clone());
        assert_eq!(r["outcome"], "refused", "{bad}: {r}");
    }
    assert_eq!(comps(&root), kept, "refusals write nothing");
    assert_ne!(before, kept);
}
