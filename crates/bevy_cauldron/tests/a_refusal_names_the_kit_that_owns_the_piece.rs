//! **A refusal says why, and for a piece another kit owns it names that kit.** Cycle 9 gap 5.
//!
//! `ozea/roof` is bound from a kit that is not the one being authored (`furniture`). The shelf
//! and `label_set` both read the authoring kit's measurements, so the piece has none here. The
//! refusal used to say the piece "has no measurement yet", which is true and sends the author to
//! measure a piece that is not theirs to measure. It now names the owning kit.
//!
//! `label_set` with an id no kit holds used to say only "pick one from the Meshes shelf". It now
//! names the closest measured ids, as `piece_select` already does.

use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy_cauldron::harness;
use bevy_cauldron::tiles::Mode;

use crate::fixtures;

fn editor_at(name: &str, mode: Mode) -> bevy::prelude::App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .kit("ozea", "alpha", &["ozea/roof"])
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
fn call(app: &mut bevy::prelude::App, method: &str, params: serde_json::Value) -> serde_json::Value {
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

fn apply(app: &mut bevy::prelude::App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": operation,
            "parameters": parameters,
            "dry_run": false,
        }),
    )
}

#[test]
fn piece_select_names_the_kit_that_owns_another_kits_piece() {
    let mut app = editor_at("refusal-kit-select", Mode::Meshes);
    let answer = apply(&mut app, "piece_select", serde_json::json!({"id": "ozea/roof"}));
    assert_eq!(answer["outcome"], "refused", "{answer}");
    let why = answer["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains("belongs to kit `ozea`"),
        "the refusal names the owning kit: {why}"
    );
    assert!(
        !why.contains("no measurement"),
        "another kit's piece is not owed a measurement here: {why}"
    );
}

#[test]
fn label_set_names_the_kit_that_owns_another_kits_piece() {
    let mut app = editor_at("refusal-kit-label", Mode::Meshes);
    let answer = apply(
        &mut app,
        "label_set",
        serde_json::json!({"id": "ozea/roof", "axis": "look", "tokens": ["plain"]}),
    );
    assert_eq!(answer["outcome"], "refused", "{answer}");
    let why = answer["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains("belongs to kit `ozea`"),
        "the refusal names the owning kit: {why}"
    );
}

#[test]
fn label_set_names_the_closest_measured_ids_for_an_unknown_id() {
    let mut app = editor_at("refusal-close-label", Mode::Meshes);
    let answer = apply(
        &mut app,
        "label_set",
        serde_json::json!({"id": "site/wal", "axis": "look", "tokens": ["plain"]}),
    );
    assert_eq!(answer["outcome"], "refused", "{answer}");
    let why = answer["refusal"].as_str().unwrap_or_default();
    assert!(why.contains("Close ids"), "the refusal lists close ids: {why}");
    assert!(why.contains("site/wall"), "the close id is the measured one: {why}");
}
