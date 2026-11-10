//! **The first-map tour** (#253): offered once, started from Help or from the home's Learn page, four steps on a real map.
//!
//! It is the agent's guide: Help → Take the tour posts `guides/first_map.json` through `bevy_fairy/guide`'s own handler, and the
//! steps advance as the person picks a piece, places it, turns it and saves. Each test does what the person does and reads the
//! guide the way an agent would (`Guide::report`).
#![cfg(feature = "debugger")]

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_cauldron::app_verbs::AppVerb;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::launcher::TourRequested;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::recent::{Recent, RecentProjects};
use bevy_cauldron::tour::{NOT_NOW_LABEL, OFFER, START_LABEL, Tour, TourRunning};
use bevy_debugger_bevy::Guide;
use bevy_gooey::{Activate, Toast, ToastAction, ToastKind};

use crate::fixtures;

fn root_of(tag: &str, pieces: bool) -> PathBuf {
    let mut fixture = fixtures::Fixture::new(&format!("tour_{tag}_{}", std::process::id()));
    if pieces {
        fixture = fixture.descriptor("barrel", "alpha").place_as("barrel@0", "barrel", (0.0, 0.0));
    }
    fixture.build("m")
}

/// A stepped editor on the project at `root`. `setup` runs before its first frame, which is when the editor is entered.
fn editor_at(root: &Path, setup: impl FnOnce(&mut App)) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    setup(&mut app);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn plain(tag: &str) -> App {
    editor_at(&root_of(tag, true), |_| {})
}

fn recents(tag: &str) -> (PathBuf, impl FnOnce(&mut App)) {
    let file = std::env::temp_dir().join(format!("grimoire-tour-{tag}-{}.yaml", std::process::id()));
    let _ = std::fs::remove_file(&file);
    (file.clone(), move |app: &mut App| {
        app.insert_resource(RecentProjects(file));
    })
}

fn guide(app: &App) -> serde_json::Value {
    app.world().resource::<Guide>().report()
}

fn at(app: &App) -> u64 {
    guide(app)["at"].as_u64().unwrap_or(0)
}

fn step_label(app: &App) -> String {
    guide(app)["step"].as_str().unwrap_or_default().to_owned()
}

fn run(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

fn take_the_tour(app: &mut App) {
    let row = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &AppVerb)>();
        q.iter(world).find(|(_, v)| **v == AppVerb::TakeTour).map(|(e, _)| e).expect("Help has Take the tour")
    };
    app.world_mut().commands().trigger(Activate { entity: row });
    run(app, 3);
}

fn offers(app: &mut App) -> Vec<(Entity, Vec<(String, String)>)> {
    let mut q = app.world_mut().query::<(Entity, &Toast)>();
    q.iter(app.world()).filter(|(_, t)| t.text() == OFFER).map(|(e, t)| (e, t.actions().to_vec())).collect()
}

#[test]
fn help_take_the_tour_posts_four_steps_and_starts_at_the_first() {
    let mut app = plain("help");
    assert!(app.world().resource::<Guide>().current().is_none(), "no tour until one is asked for");
    take_the_tour(&mut app);
    let report = guide(&app);
    assert_eq!(report["of"], 4, "the tour has four steps: {report}");
    assert_eq!(at(&app), 1, "and starts at the first");
    assert_eq!(step_label(&app), "Pick a piece");
    assert!(app.world().contains_resource::<TourRunning>(), "and is watched for as long as it runs");
}

/// **Arming a piece and placing it walk the tour to its third step.** The map arrives with a barrel on it: a piece that was already
/// there is not a piece placed.
#[test]
fn arming_and_placing_advance_it_to_step_three() {
    let mut app = plain("walk");
    take_the_tour(&mut app);
    assert_eq!(app.world().resource::<EditorState>().brush, None, "the tour starts with nothing in hand");

    run(&mut app, 3);
    assert_eq!(at(&app), 1, "nothing is chosen yet, so it waits at step one");

    app.world_mut().resource_mut::<EditorState>().brush = Some(0);
    run(&mut app, 3);
    assert_eq!(at(&app), 2, "a piece in hand is step one done: {}", guide(&app));
    assert_eq!(step_label(&app), "Place it");
    run(&mut app, 3);
    assert_eq!(at(&app), 2, "the barrel that was on the map is not a piece placed: it waits for the person's click");

    {
        let mut open = app.world_mut().resource_mut::<OpenMap>();
        let mut placed = open.map.placements[0].clone();
        placed.id = "barrel@1".to_owned();
        placed.at = (2.0, 2.0);
        open.map.placements.push(placed);
    }
    run(&mut app, 3);
    assert_eq!(at(&app), 3, "a piece placed is step two done: {}", guide(&app));
    assert_eq!(step_label(&app), "Turn it");
}

#[test]
fn turning_and_saving_finish_the_tour() {
    let mut app = plain("finish");
    take_the_tour(&mut app);
    app.world_mut().resource_mut::<EditorState>().brush = Some(0);
    run(&mut app, 3);
    {
        let mut open = app.world_mut().resource_mut::<OpenMap>();
        let mut placed = open.map.placements[0].clone();
        placed.id = "barrel@1".to_owned();
        placed.at = (2.0, 2.0);
        open.map.placements.push(placed);
    }
    run(&mut app, 3);
    assert_eq!(at(&app), 3);

    app.world_mut().resource_mut::<EditorState>().brush_yaw += 15.0;
    run(&mut app, 3);
    assert_eq!(at(&app), 4, "turning the piece in hand is step three done: {}", guide(&app));
    assert_eq!(step_label(&app), "Save the map");

    app.world_mut().resource_scope(|world, mut open: Mut<OpenMap>| {
        let project = world.resource::<Project>();
        open.save(project).unwrap_or_else(|e| panic!("{e}"));
    });
    run(&mut app, 3);
    assert_eq!(at(&app), 5, "saving is the last step: {}", guide(&app));
    assert!(!app.world().contains_resource::<TourRunning>(), "and the tour stops being watched");
    assert!(!app.world().contains_resource::<Tour>(), "and its baseline is dropped");
}

#[test]
fn the_tour_is_offered_once_in_a_toast() {
    let (file, setup) = recents("offer");
    let mut app = editor_at(&root_of("offer", true), setup);
    let toasts = offers(&mut app);
    assert_eq!(toasts.len(), 1, "a new person is offered the tour: one toast says {OFFER:?}");
    let labels: Vec<&str> = toasts[0].1.iter().map(|(label, _)| label.as_str()).collect();
    assert_eq!(labels, [START_LABEL, NOT_NOW_LABEL], "with a Start and a No thanks");
    assert!(Recent::load(&file).expect("the list").tour_offered, "and the offer is remembered");
    assert!(app.world().resource::<Guide>().current().is_none(), "offering is not starting");

    // Start is the same tour Help starts.
    let (toast, actions) = toasts.into_iter().next().expect("the offer");
    let start = actions.iter().find(|(label, _)| label == START_LABEL).map(|(_, id)| id.clone()).expect("a Start button");
    app.world_mut().commands().trigger(ToastAction { toast, id: start });
    run(&mut app, 3);
    assert_eq!(guide(&app)["of"], 4, "Start posts the tour");

    // The next time the editor opens, it is not offered again.
    let again = editor_at(&root_of("offer_again", true), |app| {
        app.insert_resource(RecentProjects(file.clone()));
    });
    let mut again = again;
    assert!(offers(&mut again).is_empty(), "the tour is offered once");
    let _ = std::fs::remove_file(&file);
}

#[test]
fn an_offer_that_is_ignored_is_still_the_only_one() {
    let (file, setup) = recents("ignored");
    let mut app = editor_at(&root_of("ignored", true), setup);
    assert_eq!(offers(&mut app).len(), 1);
    run(&mut app, 120);
    assert!(app.world().resource::<Guide>().current().is_none(), "ignoring it starts nothing");
    let mut again = editor_at(&root_of("ignored_again", true), |app| {
        app.insert_resource(RecentProjects(file.clone()));
    });
    assert!(offers(&mut again).is_empty(), "and it is not offered again");
    let _ = std::fs::remove_file(&file);
}

/// **Learn → Take the tour already said yes**: the editor starts the tour on entering, and offers nothing.
#[test]
fn the_learn_pages_tour_starts_when_the_editor_opens() {
    let (file, setup) = recents("learn");
    let mut app = editor_at(&root_of("learn", true), |app| {
        setup(app);
        app.insert_resource(TourRequested);
    });
    assert_eq!(guide(&app)["of"], 4, "the tour is running");
    assert_eq!(at(&app), 1);
    assert!(offers(&mut app).is_empty(), "and it is not also offered");
    assert!(!app.world().contains_resource::<TourRequested>(), "the request is taken");
    assert!(Recent::load(&file).expect("the list").tour_offered, "a person who took the tour has been offered it");
    let _ = std::fs::remove_file(&file);
}

/// **A project with no pieces cannot be toured**: the first step is to pick one. It is not offered, and Help says why.
#[test]
fn a_project_with_no_pieces_is_not_offered_the_tour_and_help_says_why() {
    let (file, setup) = recents("empty");
    let mut app = editor_at(&root_of("empty", false), setup);
    assert!(offers(&mut app).is_empty(), "nothing to pick, nothing to offer");
    assert!(!Recent::load(&file).expect("the list").tour_offered, "and it is offered when there is something to pick");
    take_the_tour(&mut app);
    assert!(app.world().resource::<Guide>().current().is_none(), "no tour starts");
    let said: Vec<String> = {
        let mut q = app.world_mut().query::<&Toast>();
        q.iter(app.world()).filter(|t| t.kind() == ToastKind::Alert).map(|t| t.text().to_owned()).collect()
    };
    assert!(said.iter().any(|t| t.contains("no pieces")), "Help says why: {said:?}");
    let _ = std::fs::remove_file(&file);
}
