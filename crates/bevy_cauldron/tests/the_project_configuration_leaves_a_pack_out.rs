//! **The project configuration screen leaves a pack out of the kit, and brings it back, through the operation** (UI-74, #252).
//!
//! A switch under *Packs the kit leaves out* is `pack_set_excluded`, the operation `Shift+R` on the Meshes
//! tab and an agent's `apply` dispatch: the kit's `project.yaml` is spliced, the editor's policy holds the
//! change, one log line from a person names it, and a refusal is shown under the switches instead of
//! being lost.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::Project;
use bevy_cauldron::project_config::OpenProjectConfig;
use bevy_gooey::testing::{click, find_by_label};

use crate::fixtures;

fn rig(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag)
        .descriptor("site/wall", "alpha")
        .pack("aaa", &["a1", "a2"])
        .pack("bbb", &["b1"])
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..3 {
        app.update();
    }
    app.world_mut().write_message(OpenProjectConfig);
    settle(&mut app);
    app
}

fn settle(app: &mut App) {
    for _ in 0..4 {
        app.update();
    }
}

fn excluded(app: &App) -> Vec<String> {
    app.world().resource::<Project>().policy.exclude.clone()
}

fn policy_file(app: &App) -> String {
    let project = app.world().resource::<Project>();
    std::fs::read_to_string(project.grimoire_dir.join(bevy_grimoire_core::policy::POLICY_FILE)).expect("the policy file")
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Entry>(l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .collect()
}

fn texts(app: &mut App) -> Vec<String> {
    let mut q = app.world_mut().query::<&Text>();
    q.iter(app.world()).map(|t| t.0.clone()).collect()
}

#[test]
fn a_switch_leaves_a_pack_out_through_the_operation_and_the_same_switch_brings_it_back() {
    let mut app = rig("project_config_pack");
    assert!(excluded(&app).is_empty(), "the fixture project leaves nothing out");
    let before = log(&app).len();

    let aaa = find_by_label(&mut app, "aaa").expect("every folder under assets/ has a switch");
    assert!(find_by_label(&mut app, "bbb").is_ok());
    click(&mut app, aaa);
    settle(&mut app);
    assert_eq!(excluded(&app), ["aaa"], "the editor's policy holds the exclusion");
    assert!(policy_file(&app).contains("aaa"), "and project.yaml does: {}", policy_file(&app));
    assert!(app.world().resource::<Project>().policy.excludes("aaa/a1.glb"), "the pack's meshes are no longer offered");
    let lines = log(&app);
    assert_eq!(lines.len(), before + 1, "one switch is one log line");
    assert_eq!(lines.last().map(|l| l.op.as_str()), Some("pack_set_excluded"));

    let again = find_by_label(&mut app, "aaa").unwrap();
    click(&mut app, again);
    settle(&mut app);
    assert!(excluded(&app).is_empty(), "the same switch brings it back");
    assert!(policy_file(&app).contains("exclude: []"), "and project.yaml says so: {}", policy_file(&app));
    assert_eq!(log(&app).len(), before + 2);
}

#[test]
fn a_refusal_is_said_under_the_switches_and_writes_nothing() {
    let mut app = rig("project_config_pack_refused");
    // A folder deleted behind the screen's back: the operation refuses to leave out what is not there.
    let root: PathBuf = app.world().resource::<Project>().root.clone();
    std::fs::remove_dir_all(root.join("assets").join("bbb")).unwrap();
    let before = (policy_file(&app), log(&app).len());
    let bbb = find_by_label(&mut app, "bbb").unwrap();
    click(&mut app, bbb);
    settle(&mut app);
    assert!(excluded(&app).is_empty(), "nothing was left out");
    assert_eq!((policy_file(&app), log(&app).len()), before, "a refusal writes no file and no log line");
    let said = texts(&mut app);
    assert!(said.iter().any(|t| t.contains("no folder `bbb`")), "the screen says why: {said:?}");
}
