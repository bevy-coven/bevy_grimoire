//! **A `Shown` the editor has no page for is a problem naming the operation, not a silent no-op** (AF-03, #359).
//!
//! The Files workspace gave `Shown::File` its page, so the variants that still have none are the ones whose name no panel
//! draws: a settings section nobody lists. Doing nothing would leave the person reading the receipt with nowhere to look;
//! the reveal raises a problem naming the operation and what it changed, which is the work item the missing view is built from.

use bevy::prelude::*;

use bevy_cauldron::editor::EditorState;
use bevy_cauldron::ops::Shown;
use bevy_cauldron::reveal::Revealing;
use bevy_cauldron::workspace::Workspace;

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("lamp_tall", "alpha").place("lamp_tall", (2.0, 2.0)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

#[test]
fn a_settings_section_nobody_draws_raises_a_problem_naming_the_operation_and_the_section() {
    let mut app = editor("shown_no_page");
    let before = app.world().resource::<EditorState>().status.problems().len();
    app.world_mut().resource_mut::<Revealing>().request("family_set", &Shown::Settings { section: "no_such_section".to_owned() });
    for _ in 0..30 {
        app.update();
    }
    let status = &app.world().resource::<EditorState>().status;
    let raised = &status.problems()[before..];
    assert_eq!(raised.len(), 1, "one problem, raised once: {raised:?}");
    let said = format!("{:?}", raised[0]);
    assert!(said.contains("family_set"), "it names the operation: {said}");
    assert!(said.contains("no_such_section"), "and what it changed: {said}");
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Map, "and the view stayed where it was");
}

#[test]
fn a_section_the_settings_screen_draws_raises_no_problem() {
    let mut app = editor("shown_has_a_page");
    let before = app.world().resource::<EditorState>().status.problems().len();
    app.world_mut().resource_mut::<Revealing>().request("family_set", &Shown::Settings { section: "packs".to_owned() });
    for _ in 0..30 {
        app.update();
    }
    let status = &app.world().resource::<EditorState>().status;
    assert!(status.problems()[before..].is_empty(), "a section with a page is shown, not reported: {:?}", &status.problems()[before..]);
}
