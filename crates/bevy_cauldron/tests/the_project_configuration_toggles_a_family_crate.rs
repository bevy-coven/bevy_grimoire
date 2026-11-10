//! **The project configuration screen lists the family crates and a switch rewrites `family.ron`** (UI-74, #252).
//!
//! A switch lists a crate in the project's `family.ron`, or takes it out, through an atomic write that
//! keeps every other crate where the file has it. The file is then read back through the reload contract's
//! own check and the screen says what became of it: `applied`, or the refusal with the file and the field.
//! A `family.ron` that does not read is shown and never written over.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_cauldron::live::EditorReloads;
use bevy_cauldron::project::Project;
use bevy_cauldron::project_config::OpenProjectConfig;
use bevy_grimoire_core::family::Family;
use bevy_gooey::testing::{click, find_by_label};

use crate::fixtures;

fn rig(tag: &str) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }
    let root = app.world().resource::<Project>().root.clone();
    app.world_mut().write_message(OpenProjectConfig);
    for _ in 0..4 {
        app.update();
    }
    (app, root)
}

fn settle(app: &mut App) {
    for _ in 0..4 {
        app.update();
    }
}

fn verdict(app: &App) -> Option<(String, String)> {
    app.world().resource::<EditorReloads>().kinds().iter().find(|k| k.kind == "family_list").map(|k| (k.outcome.word().to_owned(), k.file.clone()))
}

#[test]
fn toggling_a_crate_writes_family_ron_atomically_and_the_verdict_row_reads_applied() {
    let (mut app, root) = rig("project_config_toggle");
    assert!(!root.join("family.yaml").exists(), "the fixture project lists no family crate");
    assert!(find_by_label(&mut app, "bevy_gooey").is_ok(), "every crate the editor can host has a switch");

    let gooey = find_by_label(&mut app, "bevy_gooey").unwrap();
    click(&mut app, gooey);
    settle(&mut app);
    let family = Family::load(&root).expect("what the screen wrote reads");
    assert_eq!(family.crates, ["bevy_gooey"], "the switch listed the crate");
    let leftovers: Vec<_> = std::fs::read_dir(&root).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.contains("family")).collect();
    assert_eq!(leftovers, ["family.yaml"], "an atomic write leaves no temporary file beside it");
    assert_eq!(verdict(&app), Some(("applied".to_owned(), "family.yaml".to_owned())), "the reload contract's verdict on the file it wrote is recorded");
    assert!(find_by_label(&mut app, "family.ron: applied").is_ok(), "and the screen says so");

    let again = find_by_label(&mut app, "bevy_gooey").unwrap();
    click(&mut app, again);
    settle(&mut app);
    assert!(Family::load(&root).unwrap().crates.is_empty(), "the same switch takes it out again");
}

#[test]
fn a_crate_this_build_lacks_can_be_listed_and_the_line_says_how_to_host_it_and_the_others_keep_their_order() {
    let (mut app, root) = rig("project_config_unbuilt");
    // Written by hand before the next switch: the order a person chose is the order that stays.
    std::fs::write(root.join("family.yaml"), "(version: 1, crates: [\"bevy_gooey\"])").unwrap();
    settle(&mut app);
    let missing = bevy_cauldron::family_host::KNOWN.iter().find(|k| !bevy_cauldron::family_host::built().contains(k)).expect("a default build lacks some family crate");
    let toggle = find_by_label(&mut app, &format!("{missing} (not built here)")).unwrap();
    click(&mut app, toggle);
    settle(&mut app);
    assert_eq!(Family::load(&root).unwrap().crates, ["bevy_gooey".to_owned(), (*missing).to_owned()], "new crates go on the end");
    let mut texts = app.world_mut().query::<&Text>();
    let said: Vec<String> = texts.iter(app.world()).map(|t| t.0.clone()).collect();
    assert!(said.iter().any(|t| t.contains(missing) && t.contains("--features")), "the line tells how to host it: {said:?}");
}

#[test]
fn a_family_ron_that_does_not_read_is_said_and_left_exactly_as_it_is() {
    let (mut app, root) = rig("project_config_unreadable");
    let broken = "(version: 1, crates: [\"bevy_gooey\"";
    std::fs::write(root.join("family.yaml"), broken).unwrap();
    settle(&mut app);
    let mut texts = app.world_mut().query::<&Text>();
    let said: Vec<String> = texts.iter(app.world()).map(|t| t.0.clone()).collect();
    assert!(said.iter().any(|t| t.contains("family.yaml") && t.contains("nothing is written until it reads")), "the screen names the file: {said:?}");
    assert!(find_by_label(&mut app, "bevy_gooey").is_err(), "no switch is offered over a file that does not read");
    assert_eq!(std::fs::read_to_string(root.join("family.yaml")).unwrap(), broken);
    // And the writer itself refuses, for an agent or a test that reaches it without the screen.
    let err = bevy_cauldron::project_config::write_family(&root, "bevy_gooey", false).unwrap_err();
    assert!(err.contains("not changed"), "{err}");
    assert_eq!(std::fs::read_to_string(root.join("family.yaml")).unwrap(), broken);
    let v = bevy_cauldron::project_config::verdict_of(&root);
    assert_eq!(v.outcome.word(), "refused", "the contract's check refuses it too: {v:?}");
}
