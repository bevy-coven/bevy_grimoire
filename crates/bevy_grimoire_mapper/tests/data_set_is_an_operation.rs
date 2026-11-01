//! **`data_set` writes one leaf of a registered data file, and is one undo** (#86).

use bevy_grimoire_mapper::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

const TEXT: &str = "// tuning\n(mob: (speed: 1.50, name: \"orc\", hp: 7), fast: false)\n";

fn project(tag: &str, registry: bool) -> (Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_grimoire_mapper::project::Project>().clone();
    let dir = project.grimoire_dir.clone();
    let file = dir.join("mob.ron");
    std::fs::write(&file, TEXT).unwrap();
    if registry {
        std::fs::write(
            dir.join("data.ron"),
            "(version: 1, files: [(id: \"tuning/mob\", path: \"mob.ron\", bounds: {\"mob.speed\": (min: 0.0, max: 9.0)})])",
        )
        .unwrap();
    }
    assert_ne!(dir, root, "the registry lives in the grimoire directory, not the project root (#116)");
    let listed = bevy_grimoire_mapper::ops_data::listing(&dir);
    assert_eq!(listed.is_empty(), !registry);
    (Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() }, file)
}

fn set(ctx: &mut Ctx, path: &str, value: serde_json::Value) -> Result<Applied, ops::Refusal> {
    ops::dispatch(ctx, "data_set", &json!({"file": "tuning/mob", "path": path, "value": value}))
}

fn refused(r: Result<Applied, ops::Refusal>) -> String {
    match r {
        Err(e) | Ok(Applied::Refused(e)) => e.0,
        Ok(_) => panic!("the edit was accepted"),
    }
}

#[test]
fn a_leaf_is_set_read_back_and_one_undo_leaves_the_file_byte_identical() {
    let (mut ctx, file) = project("undo", true);
    assert!(matches!(set(&mut ctx, "mob.speed", json!(2.5)).unwrap(), Applied::Done { .. }));
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "// tuning\n(mob: (speed: 2.5, name: \"orc\", hp: 7), fast: false)\n"
    );
    ops::undo(&mut ctx, None).unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), TEXT, "`1.50` comes back as `1.50`");
}

#[test]
fn a_dry_run_names_the_old_and_new_value_and_writes_nothing() {
    let (ctx, file) = project("plan", true);
    let op = ops::find("data_set").unwrap();
    let plan = op.plan(&ctx, &json!({"file": "tuning/mob", "path": "mob.hp", "value": 9})).unwrap();
    assert_eq!(plan, json!({"file": "tuning/mob", "path": "mob.hp", "from": 7, "to": 9}));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), TEXT);
}

#[test]
fn refusals_change_nothing() {
    let (mut ctx, file) = project("data_set_is_an_operation_refuse", true);
    assert!(refused(set(&mut ctx, "mob.speed", json!(99.0))).contains("outside its bounds"));
    assert!(refused(set(&mut ctx, "mob.name", json!(3))).contains("string"));
    assert!(refused(set(&mut ctx, "mob.nope", json!(3))).contains("not a leaf"));
    let e = refused(ops::dispatch(&mut ctx, "data_set", &json!({"file": "x", "path": "a", "value": 1})));
    assert!(e.contains("tuning/mob"), "{e}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), TEXT);
}

#[test]
fn a_project_with_no_registry_says_so() {
    let (mut ctx, _) = project("noreg", false);
    assert!(refused(set(&mut ctx, "mob.hp", json!(1))).contains("no data.ron"));
}

#[test]
fn an_edit_outside_the_log_blocks_the_undo() {
    let (mut ctx, file) = project("hand", true);
    set(&mut ctx, "mob.hp", json!(8)).unwrap();
    let by_hand = "(mob: (speed: 3.0, name: \"orc\", hp: 8), fast: false)\n";
    std::fs::write(&file, by_hand).unwrap();
    assert!(ops::undo(&mut ctx, None).is_err() || std::fs::read_to_string(&file).unwrap() == by_hand);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), by_hand, "a hand edit is never overwritten");
}

#[test]
fn an_agent_cannot_send_the_undos_own_form() {
    let (mut ctx, file) = project("restore", true);
    let e = refused(ops::dispatch(
        &mut ctx,
        "data_set",
        &json!({"file": "tuning/mob", "path": "mob.hp", "value": 1, "restore": "1"}),
    ));
    assert!(!e.is_empty());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), TEXT);
}

#[test]
fn the_data_panel_lists_each_registered_leaf_with_its_token() {
    let (ctx, _) = project("listing", true);
    let rows = bevy_grimoire_mapper::ops_data::listing(&ctx.project.as_ref().unwrap().grimoire_dir);
    let got: Vec<(&str, &str, &str)> = rows.iter().map(|r| (r.file.as_str(), r.path.as_str(), r.value.as_str())).collect();
    assert!(got.contains(&("tuning/mob", "mob.speed", "1.50")), "{got:?}");
    assert!(got.contains(&("tuning/mob", "fast", "false")), "{got:?}");
    let (ctx, _) = project("listing_none", false);
    assert!(bevy_grimoire_mapper::ops_data::listing(&ctx.project.as_ref().unwrap().grimoire_dir).is_empty());
}
