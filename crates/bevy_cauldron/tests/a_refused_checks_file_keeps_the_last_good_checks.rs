//! **A `checks.ron` that is refused keeps the last good checks, and the editor says which file and
//! field** (#200, H3).
//!
//! `Checks::for_project` read `assets/grimoire/checks.ron` fresh on every run, so one bad edit (a hand
//! edit, a truncated write, an agent's mistake) stopped every after-edit check: the journal said "not
//! run" with the parser's position and no field, and the second reader dropped the error altogether.
//! Now one view holds the last good checks and decides each edit once: `grimoire_state.reloads` says
//! `applied`, or `refused` with the file and the field, and the checks the editor asks stay the last
//! good ones until the file reads again.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_oracle::Checks;
use bevy_cauldron::checks_view::{ChecksView, checks_for};
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::project::Project;
use serde_json::{Value, json};

use crate::fixtures;

const FILE: &str = "checks.ron";

fn good(threshold: f64) -> String {
    format!("(checks: [(name: \"mine\", question: \"Is it floating?\", threshold: {threshold})])")
}

/// The ways to break it, and the field the verdict must name: for a file cut off, where reading stopped;
/// `None` for a file that read and broke a rule, so the reason is all there is.
const BAD: [(&str, &str, Option<&str>); 5] = [
    ("corrupt", "(checks: [(name: \"mine\", question: ", Some("checks[0].question")),
    ("wrong type", "(checks: [(name: \"mine\", question: \"q\", threshold: \"high\")])", Some("checks[0].threshold")),
    ("missing field", "(checks: [(name: \"mine\", threshold: 0.5)])", Some("checks[0].question")),
    ("a threshold outside 0..=1", "(checks: [(name: \"mine\", question: \"q\", threshold: 1.5)])", None),
    ("two checks of one name", "(checks: [(name: \"a\", question: \"q\", threshold: 0.5), (name: \"a\", question: \"q\", threshold: 0.5)])", None),
];

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    app
}

fn root(app: &App) -> std::path::PathBuf {
    app.world().resource::<Project>().root.clone()
}

fn path(app: &App) -> std::path::PathBuf {
    root(app).join("assets/grimoire").join(FILE)
}

fn write(app: &App, text: &str) {
    std::fs::write(path(app), text).unwrap_or_else(|e| panic!("{e}"));
    // Distinct modification times, so the view sees every edit.
    std::thread::sleep(Duration::from_millis(30));
}

fn reloads(app: &mut App) -> Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/state").cloned() else {
        panic!("grimoire/state is not registered");
    };
    let state = app
        .world_mut()
        .run_system_with(id, Some(json!({})))
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|e| panic!("{e:?}"));
    state["reloads"].clone()
}

fn verdict(app: &mut App) -> Value {
    reloads(app)["kinds"].as_array().and_then(|k| k.iter().find(|v| v["kind"] == "checks").cloned()).unwrap_or(Value::Null)
}

fn seq(app: &mut App) -> u64 {
    reloads(app)["seq"].as_u64().unwrap()
}

/// The threshold of the project's own check `mine` in the checks the editor would ask now.
fn mine(app: &App) -> f64 {
    let checks = checks_for(app.world().get_resource::<ChecksView>(), &root(app)).unwrap_or_else(|e| panic!("the editor has checks to ask: {e}"));
    checks.get("mine").unwrap_or_else(|| panic!("`mine` is one of the checks: {:?}", checks.checks.iter().map(|c| &c.name).collect::<Vec<_>>())).threshold
}

fn until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < 6.0 {
        if done(app) {
            return;
        }
        app.update();
        std::thread::sleep(Duration::from_millis(40));
    }
    panic!("never saw: {what}; reloads {}", reloads(app));
}

fn pump(app: &mut App, secs: f64) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < secs {
        app.update();
        std::thread::sleep(Duration::from_millis(40));
    }
}

#[test]
fn a_refused_checks_file_keeps_the_checks_and_says_which_file_and_field_once_per_edit() {
    let mut app = editor("checks_view_refused");
    write(&app, &good(0.5));
    until(&mut app, "the good file applied", |a| verdict(a)["outcome"] == json!("applied"));
    assert_eq!(mine(&app), 0.5);

    for (what, text, field) in BAD {
        let before = seq(&mut app);
        let raised_before = app.world().resource::<EditorState>().status.raised();
        write(&app, text);
        // Past the view's once-a-second re-read.
        pump(&mut app, 1.6);
        let v = verdict(&mut app);
        assert_eq!(v["outcome"], json!("refused"), "{what}: {v}");
        assert_eq!(v["file"], json!(FILE), "{what}: {v}");
        match field {
            Some(field) => assert_eq!(v["field"], json!(field), "{what}: {v}"),
            None => assert_eq!(v["field"], Value::Null, "{what}: no one field is wrong: {v}"),
        }
        assert!(!v["reason"].as_str().unwrap().is_empty(), "{what}: the verdict says why: {v}");
        assert_eq!(reloads(&mut app)["ok"], json!(false), "{what}");
        // The checks the editor asks are the last good ones, though the file on disk reads no longer.
        assert!(Checks::for_project(&root(&app)).is_err(), "{what}: a fresh read of the file would stop the checks");
        assert_eq!(mine(&app), 0.5, "{what}: the checks the editor asks stay the last good ones");
        // What this edit raised, not an earlier edit's problem still in the log.
        let raised = (app.world().resource::<EditorState>().status.raised() - raised_before) as usize;
        let said: Vec<String> = app.world().resource::<EditorState>().status.problems().iter().rev().take(raised).map(|p| p.text.clone()).collect();
        assert_eq!(raised, 1, "{what}: the refusal is said once: {said:?}");
        assert!(said[0].contains(FILE) && said[0].contains("refused"), "{what}: the status line names the file: {said:?}");

        assert_eq!(seq(&mut app), before + 1, "{what}: one edit, one decision");
        let refused_at = seq(&mut app);
        pump(&mut app, 1.6);
        assert_eq!(seq(&mut app), refused_at, "{what}: nothing was edited, so no new decision");

        // Back to a good file, so the next bad one is judged against the same checks.
        write(&app, &good(0.5));
        until(&mut app, "the good file applied again", |a| verdict(a)["outcome"] == json!("applied"));
    }
}

#[test]
fn a_fixed_file_replaces_the_checks_and_a_removed_one_leaves_the_editors_own() {
    let mut app = editor("checks_view_fixed");
    write(&app, &good(0.5));
    until(&mut app, "the good file applied", |a| verdict(a)["outcome"] == json!("applied"));
    write(&app, "(checks: 7)");
    pump(&mut app, 1.6);
    assert_eq!(mine(&app), 0.5, "a refused edit leaves the check as it was");
    assert_eq!(verdict(&mut app)["outcome"], json!("refused"));

    write(&app, &good(0.9));
    until(&mut app, "the fix applied", |a| verdict(a)["outcome"] == json!("applied"));
    assert_eq!(mine(&app), 0.9, "the fix replaces the held check");
    assert_eq!(reloads(&mut app)["ok"], json!(true));

    std::fs::remove_file(path(&app)).unwrap_or_else(|e| panic!("{e}"));
    until(&mut app, "the removed file is no longer decided", |a| verdict(a) == Value::Null);
    let checks = checks_for(app.world().get_resource::<ChecksView>(), &root(&app)).unwrap();
    assert!(checks.get("mine").is_none(), "a removed file leaves the editor's own checks alone");
    assert_eq!(checks, Checks::builtin());
}

/// A file broken from the first look has no earlier good state: the checks do not run, as before, and
/// the verdict says why.
#[test]
fn a_file_broken_when_the_project_opens_is_named_and_there_is_nothing_to_keep() {
    let root = fixtures::Fixture::new("checks_view_broken_at_open").descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    std::fs::write(root.join("assets/grimoire").join(FILE), "(checks: [(name: \"mine\", question: \"q\", threshold: \"high\")])").unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    until(&mut app, "the broken file is named", |a| verdict(a)["outcome"] == json!("refused"));
    let v = verdict(&mut app);
    assert_eq!(v["field"], json!("checks[0].threshold"), "{v}");
    assert!(checks_for(app.world().get_resource::<ChecksView>(), &root).is_err(), "no earlier good state, so the reader's error stands");
}

/// A file that is there and cannot be read is refused, as `Checks::for_project` refused it by naming the
/// path; it is not the same as a file that is gone, which would quietly drop the project's own checks.
#[test]
fn a_file_that_cannot_be_read_is_refused_and_is_not_a_removed_one() {
    let mut app = editor("checks_view_unreadable");
    write(&app, &good(0.5));
    until(&mut app, "the good file applied", |a| verdict(a)["outcome"] == json!("applied"));
    std::fs::write(path(&app), [0xff, 0xfe, 0x00, 0xc3]).unwrap();
    until(&mut app, "the unreadable file refused", |a| verdict(a)["outcome"] == json!("refused"));
    let v = verdict(&mut app);
    assert_eq!(v["file"], json!(FILE), "{v}");
    assert_eq!(mine(&app), 0.5, "the checks stay the last good ones, not the editor's alone");
}

/// The checks belong to the project, so an edit is decided in every door, not only while the map is open:
/// the after-edit run asks them whichever door an agent's operation was made in.
#[test]
fn an_edit_is_decided_in_every_door() {
    let mut app = editor("checks_view_doors");
    write(&app, &good(0.5));
    until(&mut app, "the good file applied", |a| verdict(a)["outcome"] == json!("applied"));
    for door in [bevy_cauldron::tiles::Door::Kit, bevy_cauldron::tiles::Door::Rigs] {
        app.world_mut().insert_resource(door);
        write(&app, "(checks: 7)");
        until(&mut app, &format!("{door:?}: the bad edit refused"), |a| verdict(a)["outcome"] == json!("refused"));
        assert_eq!(mine(&app), 0.5, "{door:?}: the checks stay the last good ones");
        write(&app, &good(0.7));
        until(&mut app, &format!("{door:?}: the fix applied"), |a| verdict(a)["outcome"] == json!("applied"));
        assert_eq!(mine(&app), 0.7, "{door:?}");
        write(&app, &good(0.5));
        until(&mut app, &format!("{door:?}: back to 0.5"), |a| mine(a) == 0.5);
    }
}

/// `WATCHED_HERE` is the kinds with tests for their three bad files: `checks` here, `oplog` in
/// `a_damaged_oplog_line_is_named_and_the_rest_still_reads.rs`, `game_data` in
/// `a_game_data_file_that_does_not_read_is_named.rs`, `reference_images` in
/// `a_reference_image_that_does_not_read_is_named.rs`, `family` in `family_settings_typed_verdicts.rs`, `editor_settings` in
/// `a_key_set_in_both_settings_layers_takes_the_projects.rs`.
/// A kind joins it with its tests.
#[test]
fn the_kinds_watched_here_are_the_ones_tested_here() {
    assert_eq!(bevy_cauldron::map_files::WATCHED_HERE, ["checks", "oplog", "game_data", "reference_images", "family", "editor_settings"]);
}
