//! **An agent's `placement_add` owes the after-edit checks once; a person's own placement owes none (#111).**

use bevy::prelude::*;
use bevy_cauldron::after_edit::AfterEditQueue;
use bevy_cauldron::harness;
use bevy_cauldron::ops;
use bevy_cauldron::project::{OpenMap, Project};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .side(16.0)
        .descriptor("crate", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn agent_places(app: &mut App, at: [f32; 2]) {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app
        .world()
        .resource::<RemoteMethods>()
        .get("grimoire/operation_apply")
        .cloned()
    else {
        panic!("no grimoire/operation_apply");
    };
    let answer = app
        .world_mut()
        .run_system_with(
            id,
            Some(serde_json::json!({"operation": "placement_add",
                "parameters": {"piece": "crate", "at": at}, "dry_run": false})),
        )
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(answer["outcome"], "applied", "{answer}");
}

fn person_places(app: &mut App, at: [f32; 2]) {
    let mut ctx = ops::Ctx {
        root: None,
        project: Some(app.world().resource::<Project>().clone()),
        open: Some(app.world().resource::<OpenMap>().clone()),
        source: ops::Source::Human,
        session: ops::session_id(),
        proposal: None,
    };
    ops::dispatch(
        &mut ctx,
        "placement_add",
        &serde_json::json!({"piece": "crate", "at": at}),
    )
    .unwrap_or_else(|e| panic!("the person's placement: {e}"));
    let fresh = ctx.open.take().expect("the map comes back");
    *app.world_mut().resource_mut::<OpenMap>() = fresh;
}

#[test]
fn an_agents_placement_owes_the_checks_once_and_a_persons_owes_none() {
    let mut app = editor("after_edit");
    assert!(
        app.world().resource::<AfterEditQueue>().pending.is_empty(),
        "nothing is owed before anyone edits"
    );

    person_places(&mut app, [2.0, 2.0]);
    app.update();
    app.update();
    assert!(
        app.world().resource::<AfterEditQueue>().pending.is_empty(),
        "a person's own placement owed checks: {:?}",
        app.world().resource::<AfterEditQueue>().pending
    );

    agent_places(&mut app, [6.0, 2.0]);
    for _ in 0..4 {
        app.update();
    }
    let pending = app.world().resource::<AfterEditQueue>().pending.clone();
    assert_eq!(pending.len(), 1, "one agent operation, one run: {pending:?}");
    assert_eq!(pending[0].op, "placement_add");
}
