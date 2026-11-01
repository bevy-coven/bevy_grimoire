//! **The steps are drawn, not just held (E-068).**
//!
//! `an_agents_steps_are_on_screen.rs` proves the data: one row per applied operation, the log's own
//! sentence, a capture hung on the step that made it, the cap. **This proves the surface** — that the
//! journal panel actually renders those rows, because a resource nobody draws is the same defect the
//! item describes: the steps existed in the log and were not on a surface the person watches.
//!
//! # Why a headless `App` is the right instrument
//!
//! The assertion is on `Text` entities under the journal's list, which is the same thing a still would
//! show and is not subject to the toast's seven-second life or to a window size. The brief's own note
//! says it: *"a toast is on for seven seconds and is a centred card in the viewport, so a screenshot
//! cannot settle 'is it visible' — measure the layout instead, which is stronger evidence than a
//! capture."*

use bevy::prelude::*;
use bevy_cauldron::chrome::{Journal, JournalList, JournalPanel, ProblemLogLine};
use bevy_cauldron::harness;

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App) {
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

/// **Every `Text` under the journal's list**, in spawn order.
fn drawn_lines(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let lists: Vec<Entity> = world
        .query_filtered::<Entity, With<JournalList>>()
        .iter(world)
        .collect();
    let mut out = Vec::new();
    for list in lists {
        let mut stack = vec![list];
        while let Some(e) = stack.pop() {
            if let Some(text) = world.get::<Text>(e) {
                out.push(text.0.clone());
            }
            if let Some(children) = world.get::<Children>(e) {
                stack.extend(children.iter());
            }
        }
    }
    out.reverse();
    out
}

/// **The panel draws the agent's steps, in the order they happened, with the log's own words.**
///
/// The panel is opened the way `Cmd+E` opens it — its `Display` set off `None` — and the paint runs on
/// the next frame.
#[test]
fn the_journal_draws_the_agents_steps() {
    let (_root, mut app) = editor("steps_drawn");
    // Three operations an agent would dispatch, through the registry it uses.
    for at in [[2.0, 2.0], [4.0, 2.0], [6.0, 2.0]] {
        let answer = call(
            &mut app,
            "grimoire/operation_apply",
            serde_json::json!({"operation": "placement_add",
                "parameters": {"piece": "crate", "at": at}, "dry_run": false}),
        );
        assert_eq!(answer["outcome"], "applied", "{answer}");
    }
    app.update();

    // **The steps reached the journal**, which is the half the data tests already prove and this one
    // needs as a precondition.
    let steps = app.world().resource::<Journal>().steps.clone();
    assert_eq!(steps.len(), 3, "three applied operations, three steps: {steps:#?}");

    // **And they are drawn.** Open the panel the way `Cmd+E` does and let the paint run.
    let panel = app
        .world_mut()
        .query_filtered::<Entity, With<JournalPanel>>()
        .iter(app.world())
        .next()
        .expect("the journal panel is spawned on the Editor screen");
    app.world_mut()
        .get_mut::<Node>(panel)
        .expect("the panel is a node")
        .display = Display::Flex;
    app.update();
    app.update();

    let lines = drawn_lines(&mut app);
    assert!(
        !lines.is_empty(),
        "the journal drew nothing with three steps in it"
    );
    // A header naming the section, because the steps read downward and the refusals read upward.
    assert!(
        lines.iter().any(|l| l.starts_with("STEPS (")),
        "the panel does not label the steps section: {lines:#?}"
    );
    // **Each step's own sentence, from the log.** The assertion is on the operation's text, which is
    // what `steps::Step::text` holds verbatim.
    for step in &steps {
        assert!(
            lines.iter().any(|l| l.contains(&step.text)),
            "the step `{}` is not drawn: {lines:#?}",
            step.text
        );
    }
    // **In the order they happened**: the first placement's sentence appears before the last one's.
    let first = lines
        .iter()
        .position(|l| l.contains(&steps[0].text))
        .expect("the first step is drawn");
    let last = lines
        .iter()
        .position(|l| l.contains(&steps[2].text))
        .expect("the last step is drawn");
    assert!(
        first < last,
        "the steps are not in the order they happened: {lines:#?}"
    );
    // **And no refusal line is invented**: nothing went wrong, so the list is steps only.
    assert!(
        !lines.iter().any(|l| l.contains("nothing has gone wrong")),
        "the empty-journal message is drawn under three steps: {lines:#?}"
    );
    let _ = app.world().get::<ProblemLogLine>(Entity::PLACEHOLDER);
}
