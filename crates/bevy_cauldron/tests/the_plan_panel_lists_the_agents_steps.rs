//! **The Plan panel draws the agent's plan beside the map, one numbered row per step with its
//! status** (E-069, #134), follows the plan file as it changes, and says so when there is none.

use bevy::prelude::*;
use bevy_grimoire_core::plan::{self, Plan, PlanItem, Status};
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

fn settle(app: &mut App) {
    for _ in 0..6 {
        std::thread::sleep(std::time::Duration::from_millis(150));
        app.update();
    }
}

/// The plan panel's lines: the step rows (`<n> [<status>] <text>`) and its empty line.
fn shown(app: &mut App) -> Vec<String> {
    let mut lines: Vec<String> = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|t| t.0.clone())
        .filter(|t| t == "no plan" || t.starts_with("plan:") || t.split_once(" [").is_some_and(|(n, _)| n.parse::<usize>().is_ok()))
        .collect();
    lines.sort();
    lines
}

fn item(text: &str, status: Status) -> PlanItem {
    PlanItem { text: text.to_owned(), status }
}

#[test]
fn the_panel_lists_the_plan_in_order_and_follows_it() {
    let root = fixtures::Fixture::new(&format!("plan_panel_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    assert_eq!(shown(&mut app), ["no plan"], "with no plan file the panel says so");

    let path = plan::path_for_map(&app.world().resource::<OpenMap>().map_path);
    Plan::new(vec![item("lay the walls", Status::Done), item("add a gate", Status::Doing), item("plant trees", Status::Pending)])
        .unwrap()
        .save(&path)
        .unwrap();
    settle(&mut app);
    assert_eq!(
        shown(&mut app),
        ["1 [done] lay the walls", "2 [doing] add a gate", "3 [pending] plant trees", "plan: 1 of 3 done"],
        "every step is drawn, numbered in the agent's order, with its status and a count"
    );

    Plan::new(vec![item("lay the walls", Status::Done), item("add a gate", Status::Done), item("plant trees", Status::Skipped)])
        .unwrap()
        .save(&path)
        .unwrap();
    settle(&mut app);
    assert_eq!(
        shown(&mut app),
        ["1 [done] lay the walls", "2 [done] add a gate", "3 [skipped] plant trees", "plan: 2 of 3 done"],
        "a changed plan is redrawn"
    );

    Plan::new(Vec::new()).unwrap().save(&path).unwrap();
    settle(&mut app);
    assert_eq!(shown(&mut app), ["no plan"], "a cleared plan leaves the panel empty again");
}
