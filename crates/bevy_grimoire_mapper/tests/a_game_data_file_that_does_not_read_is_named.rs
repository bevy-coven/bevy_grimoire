//! **A game's data file that no longer reads is refused naming the file and the leaf, once per edit, and
//! one that does read says a running game has not got it** (#200, H3).
//!
//! `<grimoire dir>/data.ron` registers the tuning files a game owns and bounds the leaves the editor
//! tunes. Nothing here applies them to a running game (the registry classes them `Restart`), so a hand
//! edit that stopped reading used to cost the Data panel its rows with no word, and one that did read
//! was never said to be waiting for a restart. One look at the registry and its files now decides into
//! `grimoire_state.reloads`: `game_data` per file (`refused` with the file and the leaf, or
//! `needs_restart`) and `data_registry` (`applied`, or `refused` with the field).

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_grimoire_mapper::editor::EditorState;
use bevy_grimoire_mapper::ops::{self, Ctx};
use bevy_grimoire_mapper::project::Project;
use serde_json::{Value, json};

use crate::fixtures;

const REGISTRY: &str = "(version: 1, files: [(id: \"tuning/mob\", path: \"mob.ron\", bounds: {\"mob.speed\": (min: 0.0, max: 9.0), \"mob.hp\": (min: 1.0)})])";
const GOOD: &str = "// tuning\n(mob: (speed: 1.50, name: \"orc\", hp: 7), fast: false)\n";

fn editor(tag: &str) -> (App, Ctx) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_grimoire_mapper::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<Project>().clone();
    let ctx = Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() };
    (app, ctx)
}

fn dir(ctx: &Ctx) -> std::path::PathBuf {
    ctx.project.as_ref().unwrap().grimoire_dir.clone()
}

/// The file as a verdict names it: relative to the project's directory, not the kit's.
fn named(ctx: &Ctx, file: &str) -> String {
    let p = ctx.project.as_ref().unwrap();
    p.grimoire_dir.join(file).strip_prefix(&p.project_dir).unwrap().to_string_lossy().replace('\\', "/")
}

fn write(ctx: &Ctx, file: &str, text: &str) {
    std::fs::write(dir(ctx).join(file), text).unwrap_or_else(|e| panic!("{e}"));
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

fn verdict(app: &mut App, kind: &str) -> Value {
    reloads(app)["kinds"].as_array().and_then(|k| k.iter().find(|v| v["kind"] == kind).cloned()).unwrap_or(Value::Null)
}

fn seq(app: &mut App) -> u64 {
    reloads(app)["seq"].as_u64().unwrap()
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

/// A project whose registry and file read, looked at once so that is the baseline.
fn project(tag: &str) -> (App, Ctx) {
    let (mut app, ctx) = editor(tag);
    write(&ctx, "mob.ron", GOOD);
    write(&ctx, "data.ron", REGISTRY);
    pump(&mut app, 0.4);
    (app, ctx)
}

/// The ways to break the file, and the leaf the verdict must name: a file cut off, where reading stopped;
/// a bounded leaf that became a word; a bounded leaf that went missing.
const BAD: [(&str, &str, &str); 3] = [
    ("corrupt: a save cut off mid-line", "// tuning\n(mob: (speed: 1.50, name: ", "mob.name"),
    ("wrong type: the speed is a word", "(mob: (speed: \"fast\", name: \"orc\", hp: 7), fast: false)\n", "mob.speed"),
    ("missing field: no hit points", "(mob: (speed: 1.50, name: \"orc\"), fast: false)\n", "mob.hp"),
];

#[test]
fn a_file_that_does_not_read_is_named_once_per_edit_and_the_bytes_stay() {
    let (mut app, mut ctx) = project("data_view_bad");
    let file = named(&ctx, "mob.ron");
    assert!(file.ends_with("mob.ron"), "{file}");
    assert_eq!(verdict(&mut app, "game_data"), Value::Null, "a file seen reading for the first time is not news");

    for (what, bad, field) in BAD {
        let before = seq(&mut app);
        let raised_before = app.world().resource::<EditorState>().status.raised();
        write(&ctx, "mob.ron", bad);
        pump(&mut app, 0.4);

        let v = verdict(&mut app, "game_data");
        assert_eq!(v["outcome"], json!("refused"), "{what}: {v}");
        assert_eq!(v["file"], json!(file), "{what}: {v}");
        assert_eq!(v["field"], json!(field), "{what}: {v}");
        assert!(!v["reason"].as_str().unwrap().is_empty(), "{what}: the verdict says why: {v}");
        assert_eq!(reloads(&mut app)["ok"], json!(false), "{what}");
        assert_eq!(seq(&mut app), before + 1, "{what}: one edit, one decision");
        let raised = (app.world().resource::<EditorState>().status.raised() - raised_before) as usize;
        let said: Vec<String> = app.world().resource::<EditorState>().status.problems().iter().rev().take(raised).map(|p| p.text.clone()).collect();
        assert_eq!(raised, 1, "{what}: the refusal is said once: {said:?}");
        assert!(said[0].contains(&file) && said[0].contains(field), "{what}: the status line names the file and the leaf: {said:?}");

        // Nothing was edited since, so nothing is decided again however many frames pass.
        let refused_at = seq(&mut app);
        pump(&mut app, 0.4);
        assert_eq!(seq(&mut app), refused_at, "{what}: no new decision without an edit");

        // Mended to the text it was first seen with: nothing is waiting for a restart.
        write(&ctx, "mob.ron", GOOD);
        until(&mut app, "the mended file reads", |a| verdict(a, "game_data")["outcome"] == json!("applied"));
        assert_eq!(seq(&mut app), refused_at + 1, "{what}: a repair is one decision");
        assert_eq!(reloads(&mut app)["ok"], json!(true), "{what}");
    }

    // Mended to other text: it reads, and a running game has not got it.
    write(&ctx, "mob.ron", BAD[0].1);
    until(&mut app, "the file refused again", |a| verdict(a, "game_data")["outcome"] == json!("refused"));
    write(&ctx, "mob.ron", &GOOD.replace("1.50", "2.50"));
    until(&mut app, "the mended file reads as other text", |a| verdict(a, "game_data")["outcome"] == json!("needs_restart"));

    // A data edit over a file that does not scan names the file and the leaf and leaves the bytes.
    write(&ctx, "mob.ron", BAD[0].1);
    let bytes = std::fs::read(dir(&ctx).join("mob.ron")).unwrap();
    let refusal = match ops::dispatch(&mut ctx, "data_set", &json!({"file": "tuning/mob", "path": "mob.speed", "value": 2.0})) {
        Err(e) | Ok(ops::Applied::Refused(e)) => e.0,
        Ok(done) => panic!("the edit of a file that does not read was applied: {done:?}"),
    };
    assert!(refusal.contains(&file) && refusal.contains("mob.name"), "the refusal names the file and the leaf: {refusal}");
    assert!(refusal.contains("Nothing was changed"), "{refusal}");
    assert_eq!(std::fs::read(dir(&ctx).join("mob.ron")).unwrap(), bytes, "the refused edit wrote to the file");
}

#[test]
fn a_leaf_outside_its_bounds_is_refused_by_its_path_and_the_range() {
    let (mut app, ctx) = project("data_view_bounds");
    write(&ctx, "mob.ron", "(mob: (speed: 99.0, name: \"orc\", hp: 7), fast: false)\n");
    until(&mut app, "the file refused", |a| verdict(a, "game_data")["outcome"] == json!("refused"));
    let v = verdict(&mut app, "game_data");
    assert_eq!(v["field"], json!("mob.speed"), "{v}");
    assert!(v["reason"].as_str().unwrap().contains("[0, 9]"), "{v}");
}

/// An edit of a file that reads is one decision, whoever made it: a person in an editor, or `data_set`.
/// What the file says is judged against the text first seen reading, so a net-zero edit leaves nothing
/// waiting.
#[test]
fn an_edit_of_a_file_that_reads_is_one_needs_restart_and_the_registry_one_applied() {
    let (mut app, mut ctx) = project("data_view_edit");
    let file = named(&ctx, "mob.ron");
    // The first logged operation creates the log, and the log's own verdict is one decision of its own:
    // make it before the count starts.
    match ops::dispatch(&mut ctx, "map_create", &json!({"name": "east", "bounds": [8.0, 4.0, 8.0]})) {
        Ok(ops::Applied::Done { .. }) => {}
        other => panic!("the map did not apply: {other:?}"),
    }
    until(&mut app, "the log applied", |a| verdict(a, "oplog")["outcome"] == json!("applied"));
    pump(&mut app, 0.4);
    let before = seq(&mut app);

    write(&ctx, "mob.ron", "// tuning\n(mob: (speed: 2.50, name: \"orc\", hp: 7), fast: false)\n");
    until(&mut app, "the hand edit decided", |a| verdict(a, "game_data")["outcome"] == json!("needs_restart"));
    let v = verdict(&mut app, "game_data");
    assert_eq!(v["file"], json!(file), "{v}");
    assert!(v["reason"].as_str().unwrap().contains("next start"), "the reason says what a running game does: {v}");
    assert_eq!(reloads(&mut app)["ok"], json!(false), "an edit that did not land in the game is not ok");
    assert_eq!(seq(&mut app), before + 1);
    pump(&mut app, 0.4);
    assert_eq!(seq(&mut app), before + 1, "frames without an edit decide nothing");

    match ops::dispatch(&mut ctx, "data_set", &json!({"file": "tuning/mob", "path": "mob.hp", "value": 9})) {
        Ok(ops::Applied::Done { .. }) => {}
        other => panic!("the data edit did not apply: {other:?}"),
    }
    until(&mut app, "the operation's edit decided", |a| seq(a) > before + 1);
    pump(&mut app, 0.4);
    assert_eq!(seq(&mut app), before + 2, "one operation, one decision");

    // The registry has its own kind: a change that reads is `applied`, with the files kept.
    write(&ctx, "data.ron", &REGISTRY.replace("max: 9.0", "max: 20.0"));
    until(&mut app, "the registry edit decided", |a| verdict(a, "data_registry")["outcome"] == json!("applied"));
    assert_eq!(verdict(&mut app, "data_registry")["file"], json!(named(&ctx, "data.ron")));
    assert_eq!(verdict(&mut app, "game_data")["outcome"], json!("needs_restart"), "the file's own verdict stands");

    // Back to the text it was first seen with: nothing is waiting any more.
    write(&ctx, "mob.ron", GOOD);
    until(&mut app, "the file back to its first text", |a| verdict(a, "game_data")["outcome"] == json!("applied"));
    assert_eq!(reloads(&mut app)["ok"], json!(true), "a net-zero edit leaves nothing for a restart");
}

/// A registry that does not read is refused with its field and the files it listed are no longer said
/// to be fine; a registered file that is not there is refused; a registry that is gone says nothing.
#[test]
fn a_registry_that_does_not_read_and_a_file_that_is_not_there_are_refused() {
    let (mut app, ctx) = project("data_view_registry");
    let registry = named(&ctx, "data.ron");

    write(&ctx, "data.ron", "(version: 1, files: [(id: \"tuning/mob\", path: 7)])");
    until(&mut app, "the registry refused", |a| verdict(a, "data_registry")["outcome"] == json!("refused"));
    let v = verdict(&mut app, "data_registry");
    assert_eq!((v["file"].clone(), v["field"].clone()), (json!(registry), json!("files[0].path")), "{v}");
    assert_eq!(verdict(&mut app, "game_data"), Value::Null, "a file the registry no longer lists has no verdict");

    write(&ctx, "data.ron", REGISTRY);
    until(&mut app, "the registry mended", |a| verdict(a, "data_registry")["outcome"] == json!("applied"));
    assert_eq!(verdict(&mut app, "game_data"), Value::Null, "a file the registry lists again, unchanged, is no news");

    std::fs::remove_file(dir(&ctx).join("mob.ron")).unwrap();
    until(&mut app, "the missing file refused", |a| verdict(a, "game_data")["outcome"] == json!("refused"));
    let v = verdict(&mut app, "game_data");
    assert_eq!(v["file"], json!(named(&ctx, "mob.ron")), "{v}");
    assert!(v["reason"].as_str().unwrap().contains("not there"), "{v}");
    assert_eq!(v["field"], Value::Null, "a file that is not there has no leaf: {v}");

    write(&ctx, "mob.ron", GOOD);
    until(&mut app, "the file back", |a| verdict(a, "game_data")["outcome"] == json!("applied"));

    std::fs::remove_file(dir(&ctx).join("data.ron")).unwrap();
    until(&mut app, "the verdicts of a registry that is gone cleared", |a| verdict(a, "data_registry") == Value::Null && verdict(a, "game_data") == Value::Null);
    assert_eq!(reloads(&mut app)["ok"], json!(true));
}

/// An edit made while the registry does not read is not lost: the file kept its first text as the one a
/// game started with, so when the registry reads again the edit is said.
#[test]
fn an_edit_made_while_the_registry_was_broken_is_said_when_it_reads_again() {
    let (mut app, ctx) = project("data_view_while_broken");
    write(&ctx, "data.ron", "(version: 1, files: [");
    until(&mut app, "the registry refused", |a| verdict(a, "data_registry")["outcome"] == json!("refused"));
    write(&ctx, "mob.ron", "// tuning\n(mob: (speed: 2.50, name: \"orc\", hp: 7), fast: false)\n");
    pump(&mut app, 0.4);
    write(&ctx, "data.ron", REGISTRY);
    until(&mut app, "the edit made meanwhile said", |a| verdict(a, "game_data")["outcome"] == json!("needs_restart"));
}

/// A file first seen refused has no first text, so what mends it is news, even if it is the text it was.
#[test]
fn a_file_first_seen_refused_is_said_when_it_is_mended() {
    let (mut app, ctx) = editor("data_view_first_refused");
    write(&ctx, "mob.ron", BAD[1].1);
    write(&ctx, "data.ron", REGISTRY);
    until(&mut app, "refused at open", |a| verdict(a, "game_data")["outcome"] == json!("refused"));
    write(&ctx, "mob.ron", GOOD);
    until(&mut app, "mended", |a| verdict(a, "game_data")["outcome"] == json!("needs_restart"));
}
