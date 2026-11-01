//! **A referenced picture that is no longer the picture its name says is refused naming the file and the
//! header field, once per edit** (#200, H3, kind `reference_images`).
//!
//! `reference_set` lists a project-relative picture an agent is told to look at, and nothing here decodes
//! it, so a file a full disk cut off, a text saved over or another format renamed cost the agent a picture
//! it could not open and the author a word nobody said. One look at the open map's references and the
//! pictures they list now decides into `grimoire_state.reloads`, kind `reference_images`, per file:
//! `refused` with the file and the header field, or `applied` once it is a picture again.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::ops::{self, Ctx};
use bevy_cauldron::project::{OpenMap, Project};
use serde_json::{Value, json};

use crate::fixtures;

const FILE: &str = "references/court.png";

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R'];
    b.extend(w.to_be_bytes());
    b.extend(h.to_be_bytes());
    b.extend([8, 6, 0, 0, 0]);
    b
}

fn gif(w: u16, h: u16) -> Vec<u8> {
    let mut b = b"GIF89a".to_vec();
    b.extend(w.to_le_bytes());
    b.extend(h.to_le_bytes());
    b.extend([0, 0, 0]);
    b
}

fn editor(tag: &str) -> (App, Ctx) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<Project>().clone();
    let open = app.world().resource::<OpenMap>().clone();
    let ctx = Ctx { project: Some(project), open: Some(open), session: tag.to_owned(), ..Ctx::default() };
    (app, ctx)
}

fn path(ctx: &Ctx, file: &str) -> std::path::PathBuf {
    ctx.project.as_ref().unwrap().root.join(file)
}

/// The references file beside the open map.
fn references_file(ctx: &Ctx) -> std::path::PathBuf {
    bevy_grimoire_core::references::path_for_map(&ctx.open.as_ref().unwrap().map_path)
}

fn write(ctx: &Ctx, file: &str, bytes: &[u8]) {
    let p = path(ctx, file);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap_or_else(|e| panic!("{e}"));
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

fn verdict(app: &mut App, file: &str) -> Value {
    reloads(app)["kinds"].as_array().and_then(|k| k.iter().find(|v| v["kind"] == "reference_images" && v["file"] == file).cloned()).unwrap_or(Value::Null)
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

/// A map that references a picture that reads, looked at once so that is the baseline.
fn project(tag: &str) -> (App, Ctx) {
    let (mut app, mut ctx) = editor(tag);
    write(&ctx, FILE, &png(8, 8));
    ops::dispatch(&mut ctx, "reference_set", &json!({"path": FILE, "caption": "the court"})).unwrap_or_else(|e| panic!("{e:?}"));
    pump(&mut app, 0.4);
    (app, ctx)
}

/// The ways to break the picture, and the header field the verdict must name: bytes that are no picture, a
/// picture of another format than its name, a header cut off inside the height.
fn bad() -> [(&'static str, Vec<u8>, &'static str); 3] {
    [
        ("corrupt: a text where the picture was", b"court picture".to_vec(), "signature"),
        ("wrong type: a gif under a png's name", gif(8, 8), "format"),
        ("missing field: a header cut off inside the height", png(8, 8)[..22].to_vec(), "height"),
    ]
}

#[test]
fn a_picture_that_is_not_a_picture_is_named_once_per_edit_and_a_repair_is_applied() {
    let (mut app, ctx) = project("reference_view_bad");
    assert_eq!(verdict(&mut app, FILE), Value::Null, "a picture seen reading for the first time is not news");
    assert_eq!(reloads(&mut app)["ok"], json!(true));
    let references_before = std::fs::read_to_string(references_file(&ctx)).unwrap();

    for (what, bytes, field) in bad() {
        let before = seq(&mut app);
        let raised_before = app.world().resource::<EditorState>().status.raised();
        write(&ctx, FILE, &bytes);
        pump(&mut app, 0.4);

        let v = verdict(&mut app, FILE);
        assert_eq!(v["outcome"], json!("refused"), "{what}: {v}");
        assert_eq!(v["file"], json!(FILE), "{what}: {v}");
        assert_eq!(v["field"], json!(field), "{what}: {v}");
        assert!(!v["reason"].as_str().unwrap().is_empty(), "{what}: the verdict says why: {v}");
        assert_eq!(reloads(&mut app)["ok"], json!(false), "{what}");
        assert_eq!(seq(&mut app), before + 1, "{what}: one edit, one decision");
        let raised = (app.world().resource::<EditorState>().status.raised() - raised_before) as usize;
        let said: Vec<String> = app.world().resource::<EditorState>().status.problems().iter().rev().take(raised).map(|p| p.text.clone()).collect();
        assert_eq!(raised, 1, "{what}: the refusal is said once: {said:?}");
        assert!(said[0].contains(FILE) && said[0].contains(field), "{what}: the status line names the file and the field: {said:?}");

        // The picture is the author's: the editor reads it and writes nothing, and the reference stays.
        assert_eq!(std::fs::read(path(&ctx, FILE)).unwrap(), bytes, "{what}: the bytes stay");

        // Nothing was edited since, so nothing is decided again however many frames pass.
        let refused_at = seq(&mut app);
        pump(&mut app, 0.4);
        assert_eq!(seq(&mut app), refused_at, "{what}: no new decision without an edit");

        write(&ctx, FILE, &png(8, 8));
        until(&mut app, "the mended picture reads", |a| verdict(a, FILE)["outcome"] == json!("applied"));
        assert_eq!(seq(&mut app), refused_at + 1, "{what}: a repair is one decision");
        assert_eq!(reloads(&mut app)["ok"], json!(true), "{what}");
    }
    assert_eq!(std::fs::read_to_string(references_file(&ctx)).unwrap(), references_before, "the reference list is untouched");
}

/// A picture that is still a picture of other dimensions was edited, so it is one decision; the same
/// dimensions saved again are nothing to say.
#[test]
fn a_picture_that_still_reads_as_another_size_is_applied_and_the_same_size_is_not_news() {
    let (mut app, ctx) = project("reference_view_resized");
    let before = seq(&mut app);
    write(&ctx, FILE, &png(16, 8));
    until(&mut app, "the resized picture is looked at", |a| verdict(a, FILE)["outcome"] == json!("applied"));
    assert_eq!(seq(&mut app), before + 1);
    let applied = seq(&mut app);
    let mut same = png(16, 8);
    same.extend([1, 2, 3]);
    write(&ctx, FILE, &same);
    pump(&mut app, 0.4);
    assert_eq!(seq(&mut app), applied, "a picture saved again at the same size says nothing");
}

/// A picture first seen refused is news at once, and what mends it is news too.
#[test]
fn a_picture_that_is_already_refused_when_it_is_listed_is_named_and_mended() {
    let (mut app, mut ctx) = editor("reference_view_first");
    write(&ctx, "art/hall.jpg", b"hall picture");
    ops::dispatch(&mut ctx, "reference_set", &json!({"path": "art/hall.jpg"})).unwrap_or_else(|e| panic!("{e:?}"));
    until(&mut app, "the listed text is refused", |a| verdict(a, "art/hall.jpg")["outcome"] == json!("refused"));
    let v = verdict(&mut app, "art/hall.jpg");
    assert_eq!(v["field"], json!("signature"), "{v}");
    write(&ctx, "art/hall.jpg", &gif(2, 2));
    until(&mut app, "a gif under a jpeg's name is refused again", |a| verdict(a, "art/hall.jpg")["field"] == json!("format"));
}

/// A picture that is gone, and one that was never there, are not refusals (`grimoire_state.references`
/// says `missing`); a reference taken back is no longer looked at.
#[test]
fn a_picture_that_is_gone_or_no_longer_listed_is_forgotten_and_a_missing_one_is_not_decided() {
    let (mut app, mut ctx) = project("reference_view_forgotten");
    ops::dispatch(&mut ctx, "reference_set", &json!({"path": "references/later.png"})).unwrap_or_else(|e| panic!("{e:?}"));
    pump(&mut app, 0.4);
    assert_eq!(verdict(&mut app, "references/later.png"), Value::Null, "a listed picture that is not there yet is not decided");

    write(&ctx, FILE, b"court picture");
    until(&mut app, "the text is refused", |a| verdict(a, FILE)["outcome"] == json!("refused"));
    std::fs::remove_file(path(&ctx, FILE)).unwrap();
    until(&mut app, "the removed picture is forgotten", |a| verdict(a, FILE) == Value::Null);
    assert_eq!(reloads(&mut app)["ok"], json!(true));

    write(&ctx, FILE, b"court picture");
    until(&mut app, "the text is refused again", |a| verdict(a, FILE)["outcome"] == json!("refused"));
    ops::dispatch(&mut ctx, "reference_set", &json!({"remove": "r1"})).unwrap_or_else(|e| panic!("{e:?}"));
    until(&mut app, "the reference taken back is forgotten", |a| verdict(a, FILE) == Value::Null);
    assert_eq!(reloads(&mut app)["ok"], json!(true));
}
