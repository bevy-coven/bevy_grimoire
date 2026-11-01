//! **A line of `oplog.jsonl` carries the version of its shape, and a line of another version is refused by
//! whoever reads the log, and its bytes stand** (#202, H5 step 4).
//!
//! The log is the last format the editor writes that had no version: a line a newer build wrote was read as
//! this build's (an entry is not `deny_unknown_fields`), and a line from before the version had no way to
//! say so. The refusal is in the entry's own deserialiser (`bevy_grimoire_core::format_version::read`), so
//! the readers that name what is wrong (the reload verdict and the undo) refuse it as they refuse any line
//! that does not read, naming `line N.version` and the version this build reads, and the readers that skip
//! a line (the steps feed, `grimoire/oplog_read`, the after-edit queue) skip it like a damaged one, so the
//! other lines of the log still read. An append never rewrites a line: the new entry is on its own line
//! at this build's version, after the line it did not read.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_grimoire_mapper::ops::{self, Applied, Ctx};
use bevy_grimoire_mapper::project::Project;
use serde_json::{Value, json};

use crate::fixtures;

const FILE: &str = "oplog.jsonl";

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

/// One real, undoable change: a map made, so the log holds a line an undo would take back.
fn make(ctx: &mut Ctx, name: &str) -> String {
    match ops::dispatch(ctx, "map_create", &json!({"name": name, "bounds": [8.0, 4.0, 8.0]})) {
        Ok(Applied::Done { operation_id, .. }) => operation_id,
        other => panic!("the create did not apply: {other:?}"),
    }
}

fn log_path(ctx: &Ctx) -> std::path::PathBuf {
    ops::oplog_path(ctx.project.as_ref().unwrap())
}

fn lines(ctx: &Ctx) -> Vec<String> {
    std::fs::read_to_string(log_path(ctx)).unwrap().lines().map(str::to_owned).collect()
}

fn write(ctx: &Ctx, lines: &[String]) {
    std::fs::write(log_path(ctx), lines.join("\n") + "\n").unwrap_or_else(|e| panic!("{e}"));
    // Distinct modification times, so the view sees every edit.
    std::thread::sleep(Duration::from_millis(30));
}

fn with_version(line: &str, version: Option<u64>) -> String {
    let mut v: Value = serde_json::from_str(line).unwrap();
    let object = v.as_object_mut().unwrap();
    match version {
        Some(n) => object.insert("version".into(), json!(n)),
        None => object.remove("version"),
    };
    v.to_string()
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
    reloads(app)["kinds"].as_array().and_then(|k| k.iter().find(|v| v["kind"] == "oplog").cloned()).unwrap_or(Value::Null)
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

/// **A line is written at the version this build reads**, so a later build can tell this build's lines from
/// its own.
#[test]
fn a_line_is_written_with_the_version_this_build_reads() {
    let (_app, mut ctx) = editor("oplog_version_written");
    make(&mut ctx, "east");
    let line: Value = serde_json::from_str(&lines(&ctx)[0]).unwrap();
    assert_eq!(line["version"], json!(1), "the line says which shape it has: {line}");
}

/// **A line from before the version is the first one**, so every log already on disk keeps reading, the
/// verdict says `applied` and an undo takes the change back; nothing is rewritten.
#[test]
fn a_line_from_before_the_version_reads_as_the_first_and_nothing_is_rewritten() {
    let (mut app, mut ctx) = editor("oplog_version_missing");
    let ids: Vec<String> = ["east", "west"].iter().map(|name| make(&mut ctx, name)).collect();
    let old: Vec<String> = lines(&ctx).iter().map(|l| with_version(l, None)).collect();
    write(&ctx, &old);

    let read = ops::read_entries(&log_path(&ctx));
    assert_eq!(read.len(), 2, "a line with no version is read");
    for entry in &read {
        assert_eq!(serde_json::to_value(entry).unwrap()["version"], json!(1), "and it is the first version");
    }
    ops::undo_target(&ctx, Some(&ids[1])).unwrap_or_else(|e| panic!("an undo of a log from before the version refused: {}", e.0));
    until(&mut app, "the log applied", |a| verdict(a)["outcome"] == json!("applied"));
    assert_eq!(std::fs::read_to_string(log_path(&ctx)).unwrap(), old.join("\n") + "\n", "reading rewrote nothing");
}

/// **A line of another version is refused with a verdict, the undo refuses naming it, and the bytes stand**:
/// the same line is refused whether it is newer (2, 7) or from before the first (0).
#[test]
fn a_line_of_another_version_is_refused_and_stands_and_the_next_entry_is_on_its_own_line() {
    for version in [2u64, 0, 7] {
        let (mut app, mut ctx) = editor(&format!("oplog_version_{version}"));
        let ids: Vec<String> = ["east", "west", "north"].iter().map(|name| make(&mut ctx, name)).collect();
        let good = lines(&ctx);
        until(&mut app, "the log applied", |a| verdict(a)["outcome"] == json!("applied"));

        let mut other = good.clone();
        other[1] = with_version(&good[1], Some(version));
        write(&ctx, &other);
        until(&mut app, "the line of another version refused", |a| verdict(a)["outcome"] == json!("refused"));

        let v = verdict(&mut app);
        assert_eq!(v["file"], json!(FILE), "version {version}: {v}");
        assert_eq!(v["field"], json!("line 2.version"), "version {version}: the verdict names the line and the field: {v}");
        let reason = v["reason"].as_str().unwrap();
        assert!(reason.contains(&format!("version {version} is not the 1 this build reads")), "version {version}: the verdict names the version this build reads: {reason}");

        // The readers that skip a line skip this one, and the log's other lines still read.
        let entries = ops::read_entries(&log_path(&ctx));
        assert_eq!(entries.iter().map(|e| e.seq).collect::<Vec<_>>(), [0, 2], "version {version}: the lines around it read");

        // An undo does not guess around a line it cannot read: it names the line and leaves everything.
        let before = std::fs::read(log_path(&ctx)).unwrap();
        let refusal = ops::undo(&mut ctx, Some(&ids[2])).expect_err("an undo past a line of another version must refuse").0;
        assert!(refusal.contains(FILE) && refusal.contains("line 2.version"), "version {version}: the undo names the line: {refusal}");
        assert!(refusal.contains(&format!("version {version} is not the 1 this build reads")), "version {version}: {refusal}");
        assert!(refusal.contains("Nothing was changed"), "version {version}: {refusal}");
        assert_eq!(std::fs::read(log_path(&ctx)).unwrap(), before, "version {version}: the refused undo wrote to the log");

        // A new operation is appended after it, on its own line at this build's version, and the line it
        // did not read is as it was.
        make(&mut ctx, "south");
        let after = lines(&ctx);
        assert_eq!(after.len(), 4, "version {version}: one new line");
        assert_eq!(&after[..3], &other[..], "version {version}: the lines that were there stand, byte for byte");
        assert_eq!(serde_json::from_str::<Value>(&after[3]).unwrap()["version"], json!(1), "version {version}: the new line is this build's");

        // Repaired: the verdict moves, and the undo works again.
        write(&ctx, &good);
        until(&mut app, "the repaired log applied", |a| verdict(a)["outcome"] == json!("applied"));
        ops::undo_target(&ctx, Some(&ids[2])).unwrap_or_else(|e| panic!("version {version}: a log that reads refused an undo: {}", e.0));
    }
}
