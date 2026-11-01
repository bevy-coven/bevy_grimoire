//! **An agent can arm a composition, and the picker is the same operation (E-002, #129).**
//!
//! Arming a composition was `Armed::toggle`, called by the picker's `Enter`, a picker row's click
//! and the COMPOSE tab's `Enter`, and no operation named it, so an agent could stamp a composition
//! only if a person had armed it. `composition_arm` is the registered operation; the three UI paths
//! dispatch it. These tests go through the registered BRP method, the way an agent's request goes,
//! and assert on the editor's own `Armed`.

use bevy::prelude::*;
use bevy_cauldron::compose::Armed;
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .composition("site/room", &[("site/wall", "site/wall", (0.0, 0.0))])
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
fn apply(app: &mut App, composition: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let method = "grimoire/operation_apply";
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(
            id,
            Some(serde_json::json!({
                "operation": "composition_arm",
                "parameters": {"composition": composition},
                "dry_run": false,
            })),
        )
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn armed(app: &App) -> Option<String> {
    app.world().resource::<Armed>().id().map(str::to_owned)
}

#[test]
fn an_agent_arms_a_composition_and_the_map_holds_it() {
    let mut app = editor("comp-arm-agent");
    let answer = apply(&mut app, serde_json::json!("site/room"));
    assert_eq!(answer["outcome"], "applied", "the arm was not applied: {answer}");
    assert_eq!(
        armed(&app).as_deref(),
        Some("site/room"),
        "the operation answered but the editor does not hold the composition armed, so the next \
         click would stamp nothing"
    );
    apply(&mut app, serde_json::Value::Null);
    assert_eq!(armed(&app), None, "null must put the composition down");
}

#[test]
fn an_unknown_composition_is_refused_by_name_and_the_arm_stays() {
    let mut app = editor("comp-arm-unknown");
    apply(&mut app, serde_json::json!("site/room"));
    let answer = apply(&mut app, serde_json::json!("site/rooom"));
    assert_eq!(answer["outcome"], "refused", "a typo was applied: {answer}");
    assert!(
        answer["refusal"].as_str().unwrap_or("").contains("site/room"),
        "the refusal does not name the compositions an agent can arm: {answer}"
    );
    assert_eq!(armed(&app).as_deref(), Some("site/room"), "a refused arm changed the arm");
}

/// **The picker row and the operation leave the same state**, because the row dispatches it: a
/// second press of the same row puts it down, as the toggle did.
#[test]
fn the_picker_row_arms_and_disarms_through_the_operation() {
    let mut app = editor("comp-arm-row");
    let press = |app: &mut App| {
        app.world_mut()
            .resource_scope(|world: &mut World, project: Mut<Project>| {
                let mut armed = world.resource_mut::<Armed>();
                let mut status = bevy_cauldron::chrome::Status::default();
                bevy_cauldron::compose::picker::arm_row(0, &project, &mut armed, &mut status);
            });
    };
    press(&mut app);
    assert_eq!(armed(&app).as_deref(), Some("site/room"));
    press(&mut app);
    assert_eq!(armed(&app), None, "a second press must disarm");
}
