//! **A damaged line of `oplog.jsonl` is named with its line and field, once per edit, and the rest of the
//! log still reads** (#200, H3).
//!
//! Every reader of the log (the undo walk, the steps feed, `grimoire/oplog_read`, the after-edit queue)
//! skips a line that is not an entry, so a hand edit or a write cut short cost the author what that line
//! recorded and said nothing about it. Now one look at the file decides it into `grimoire_state.reloads`
//! as kind `oplog`: `applied` while every line reads, `refused` with the file, the line and the field of
//! the first line that does not, and an undo refuses naming the same line and field.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_grimoire_mapper::editor::EditorState;
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

fn with_field(line: &str, edit: impl Fn(&mut serde_json::Map<String, Value>)) -> String {
    let mut v: Value = serde_json::from_str(line).unwrap();
    edit(v.as_object_mut().unwrap());
    v.to_string()
}

/// The ways to damage the second of three lines, and the field the verdict must name: for a line cut off,
/// where reading stopped; for text that stopped before it named a field, the line alone.
type Damage = fn(&str) -> String;
const BAD: [(&str, Damage, &str); 4] = [
    ("corrupt: a line cut off mid-write", |l| l[..l.find("\"at\"").unwrap() + 5].to_owned(), "line 2.at"),
    ("wrong type: a sequence that is a word", |l| with_field(l, |o| { o.insert("seq".into(), json!("one")); }), "line 2.seq"),
    ("missing field: no operation named", |l| with_field(l, |o| { o.remove("op"); }), "line 2.op"),
    ("trailing text: two entries glued by a cut write", |l| format!("{l}{l}"), "line 2"),
];

#[test]
fn a_damaged_line_is_named_once_per_edit_and_the_rest_of_the_log_still_reads() {
    let (mut app, mut ctx) = editor("oplog_view_damaged");
    let ids: Vec<String> = ["east", "west", "north"].iter().map(|name| make(&mut ctx, name)).collect();
    let good = lines(&ctx);
    assert_eq!(good.len(), 3);
    until(&mut app, "the log applied", |a| verdict(a)["outcome"] == json!("applied"));
    assert_eq!(verdict(&mut app)["file"], json!(FILE));

    for (what, damage, field) in BAD {
        let before = seq(&mut app);
        let raised_before = app.world().resource::<EditorState>().status.raised();
        let mut broken = good.clone();
        broken[1] = damage(&good[1]);
        write(&ctx, &broken);
        pump(&mut app, 0.4);

        let v = verdict(&mut app);
        assert_eq!(v["outcome"], json!("refused"), "{what}: {v}");
        assert_eq!(v["file"], json!(FILE), "{what}: {v}");
        assert_eq!(v["field"], json!(field), "{what}: {v}");
        assert!(!v["reason"].as_str().unwrap().is_empty(), "{what}: the verdict says why: {v}");
        assert_eq!(reloads(&mut app)["ok"], json!(false), "{what}");
        assert_eq!(seq(&mut app), before + 1, "{what}: one edit, one decision");
        let raised = (app.world().resource::<EditorState>().status.raised() - raised_before) as usize;
        let said: Vec<String> = app.world().resource::<EditorState>().status.problems().iter().rev().take(raised).map(|p| p.text.clone()).collect();
        assert_eq!(raised, 1, "{what}: the refusal is said once: {said:?}");
        assert!(said[0].contains(FILE) && said[0].contains(field), "{what}: the status line names the file and the line: {said:?}");

        // The running state is as it was: the lines before and after the damaged one still read, and an
        // undo does not guess around it, leaves every file and the log as they were.
        let entries = ops::read_entries(&log_path(&ctx));
        assert_eq!(entries.len(), 2, "{what}: the readers skip the damaged line and read the other two");
        assert_eq!(entries.iter().map(|e| e.seq).collect::<Vec<_>>(), [0, 2], "{what}");
        let log_before = std::fs::read(log_path(&ctx)).unwrap();
        let refusal = ops::undo(&mut ctx, Some(&ids[2])).expect_err("an undo past a damaged line must refuse").0;
        assert!(refusal.contains(FILE) && refusal.contains(field), "{what}: the undo names the file and the line: {refusal}");
        assert!(refusal.contains("Nothing was changed"), "{what}: {refusal}");
        assert_eq!(std::fs::read(log_path(&ctx)).unwrap(), log_before, "{what}: the refused undo wrote to the log");
        for name in ["east", "west", "north"] {
            assert!(ctx.project.as_ref().unwrap().maps_dir.join(format!("{name}.map.ron")).exists(), "{what}: the refused undo took `{name}` away");
        }

        // Nothing was edited, so no new decision, however many frames pass.
        let refused_at = seq(&mut app);
        pump(&mut app, 0.4);
        assert_eq!(seq(&mut app), refused_at, "{what}: nothing was edited, so no new decision");

        // Repaired: the verdict says so, and the undo works again (resolving it changes nothing).
        write(&ctx, &good);
        until(&mut app, "the repaired log applied", |a| verdict(a)["outcome"] == json!("applied"));
        assert_eq!(reloads(&mut app)["ok"], json!(true), "{what}");
        ops::undo_target(&ctx, Some(&ids[2])).unwrap_or_else(|e| panic!("{what}: the undo of a log that reads refused: {}", e.0));
    }
}

/// An append is an edit of the file and not news: the editor writes a line for every operation, and a log
/// that still reads decides nothing.
#[test]
fn an_operation_appended_to_a_log_that_reads_decides_nothing() {
    let (mut app, mut ctx) = editor("oplog_view_append");
    make(&mut ctx, "east");
    until(&mut app, "the log applied", |a| verdict(a)["outcome"] == json!("applied"));
    let decided = seq(&mut app);
    make(&mut ctx, "west");
    make(&mut ctx, "north");
    pump(&mut app, 0.4);
    assert_eq!(lines(&ctx).len(), 3);
    assert_eq!(seq(&mut app), decided, "two more operations on a log that reads are not a new verdict");
}

/// Two damaged lines are one refusal that names the first and counts the other; repairing the first moves
/// the verdict to the second, so the author is never told a log is whole while a line still is not.
#[test]
fn two_damaged_lines_are_one_refusal_that_counts_them_and_a_repair_moves_it() {
    let (mut app, mut ctx) = editor("oplog_view_two");
    for name in ["east", "west", "north"] {
        make(&mut ctx, name);
    }
    let good = lines(&ctx);
    until(&mut app, "the log applied", |a| verdict(a)["outcome"] == json!("applied"));

    let mut broken = good.clone();
    broken[0] = with_field(&good[0], |o| { o.remove("at"); });
    broken[2] = with_field(&good[2], |o| { o.insert("seq".into(), json!(-1)); });
    write(&ctx, &broken);
    until(&mut app, "the first damaged line named", |a| verdict(a)["field"] == json!("line 1.at"));
    let reason = verdict(&mut app)["reason"].as_str().unwrap().to_owned();
    assert!(reason.contains("1 more line does not read"), "the other damaged line is counted: {reason}");

    broken[0] = good[0].clone();
    write(&ctx, &broken);
    until(&mut app, "the second damaged line named", |a| verdict(a)["field"] == json!("line 3.seq"));
    let reason = verdict(&mut app)["reason"].as_str().unwrap().to_owned();
    assert!(!reason.contains("more line"), "one damaged line is left, so none is counted after it: {reason}");
}

/// A log that is gone ends the refusal, as a removed file ends any other; a log that is there and cannot
/// be read at all is refused and is not the same as one that is gone.
#[test]
fn a_log_that_is_gone_ends_the_refusal_and_one_that_cannot_be_read_is_refused() {
    let (mut app, mut ctx) = editor("oplog_view_gone");
    make(&mut ctx, "east");
    let good = lines(&ctx);
    until(&mut app, "the log applied", |a| verdict(a)["outcome"] == json!("applied"));

    std::fs::write(log_path(&ctx), [0xff, 0xfe, b'\n']).unwrap();
    until(&mut app, "the unreadable log refused", |a| verdict(a)["outcome"] == json!("refused"));
    let v = verdict(&mut app);
    assert_eq!(v["file"], json!(FILE), "{v}");
    assert_eq!(v["field"], Value::Null, "a file that does not read at all has no line: {v}");

    std::fs::remove_file(log_path(&ctx)).unwrap();
    until(&mut app, "the verdict of a log that is gone cleared", |a| verdict(a) == Value::Null);
    assert_eq!(reloads(&mut app)["ok"], json!(true));

    write(&ctx, &good);
    until(&mut app, "the log that came back applied", |a| verdict(a)["outcome"] == json!("applied"));
}
