//! **The AGENT block lists the agent's plan, each step ticked as the agent marks it done** (UI-60b, #340).
//!
//! The agent declares its steps with `plan_set` (`maps/<map>.plan.ron`), the one source the inspector's PLAN section and
//! `grimoire_state.plan` already read. The block's PLAN row reads the same file, so the person looking at what the agent did
//! also sees what it means to do, and which steps are done.

use bevy::prelude::*;
use bevy_cauldron::familiar::{FamiliarItem, FamiliarPlanLine};
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;
use bevy_gooey::ValueChange;
use bevy_grimoire_core::plan::{self, Plan, PlanItem, Status};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("table", "alpha").place_as("table@0", "table", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    show_the_block(&mut app);
    app
}

fn show_the_block(app: &mut App) {
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<FamiliarItem>>().single(world).expect("the View menu carries one Show agent row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: true, is_final: true });
    settle(app);
}

/// Frames at least `PLAN_READ_SECS` of wall time apart, so the block has looked at the file again.
fn settle(app: &mut App) {
    for _ in 0..6 {
        std::thread::sleep(std::time::Duration::from_millis(150));
        app.update();
    }
}

fn item(text: &str, status: Status) -> PlanItem {
    PlanItem { text: text.to_owned(), status }
}

fn plan_row(app: &mut App) -> String {
    let world = app.world_mut();
    world.query_filtered::<&Text, With<FamiliarPlanLine>>().single(world).expect("the block carries one PLAN row").0.clone()
}

fn write_the_plan(app: &App, items: Vec<PlanItem>) {
    let path = plan::path_for_map(&app.world().resource::<OpenMap>().map_path);
    Plan::new(items).unwrap().save(&path).unwrap();
}

#[test]
fn the_block_lists_each_step_and_ticks_it_when_it_is_done() {
    let mut app = editor(&format!("agent_block_plan_{}", std::process::id()));
    write_the_plan(&app, vec![item("lay the walls", Status::Done), item("roof", Status::Pending)]);
    settle(&mut app);
    let text = plan_row(&mut app);
    assert!(text.contains("1 [done] lay the walls"), "the first step, done: {text:?}");
    assert!(text.contains("2 [pending] roof"), "the second step, waiting: {text:?}");
    assert!(text.contains("plan: 1 of 2 done"), "the count: {text:?}");

    write_the_plan(&app, vec![item("lay the walls", Status::Done), item("roof", Status::Done)]);
    settle(&mut app);
    let text = plan_row(&mut app);
    assert!(text.contains("2 [done] roof"), "the second step is ticked once the agent marks it: {text:?}");
    assert!(text.contains("plan: 2 of 2 done"), "the count follows: {text:?}");
}

#[test]
fn with_no_plan_file_the_row_says_so() {
    let mut app = editor(&format!("agent_block_no_plan_{}", std::process::id()));
    assert_eq!(plan_row(&mut app), "no plan");
}
