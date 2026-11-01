//! **The tree editor writes a `.tree.ron` the crate's own loader reads, one undo per edit** (#102).
//! `cargo test -p bevy_cauldron --features bevy_flora --test editor tree_editor::`

#![cfg(feature = "bevy_flora")]

use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::project::Project;
use bevy_flora::{TreeParams, TreePreset};
use serde_json::json;

use crate::fixtures;

fn open(tag: &str) -> (Ctx, Project) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<Project>().clone();
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    (Ctx { project: Some(project.clone()), open: Some(open), session: "test".to_owned(), ..Ctx::default() }, project)
}

fn read(project: &Project, name: &str) -> Option<TreeParams> {
    let text = std::fs::read_to_string(project.grimoire_dir.join("trees").join(format!("{name}.tree.ron"))).ok()?;
    Some(TreeParams::from_ron(&text).unwrap_or_else(|e| panic!("{e}")))
}

#[test]
fn an_edit_writes_the_tree_file_the_loader_reads_and_one_undo_takes_it_back() {
    let (mut ctx, project) = open("tree_editor");
    let Applied::Done { .. } = ops::dispatch(&mut ctx, "tree_edit", &json!({"name": "elm", "preset": "oak"})).unwrap() else { panic!("refused") };
    assert_eq!(read(&project, "elm"), Some(TreePreset::Oak.params()), "a preset starts the file");

    let Applied::Done { operation_id, .. } =
        ops::dispatch(&mut ctx, "tree_edit", &json!({"name": "elm", "set": {"trunk.radius": 0.9, "seed": 9}})).unwrap()
    else {
        panic!("refused")
    };
    let edited = read(&project, "elm").unwrap();
    let oak = TreePreset::Oak.params();
    assert_eq!((edited.trunk.radius, edited.seed), (0.9, 9));
    assert_eq!(edited.trunk.height, oak.trunk.height, "only the named fields change");

    ops::undo(&mut ctx, Some(&operation_id)).unwrap();
    assert_eq!(read(&project, "elm"), Some(oak));
}

#[test]
fn a_misspelt_field_or_preset_is_refused_and_writes_nothing() {
    let (mut ctx, project) = open("tree_editor_typo");
    for bad in [
        json!({"name": "elm", "preset": "oak", "set": {"trunk.radious": 1.0}}),
        json!({"name": "elm", "preset": "oak", "set": {"trunk.radius": "wide"}}),
        json!({"name": "elm", "preset": "sequoia"}),
        json!({"name": "elm"}),
        json!({"name": "../elm", "preset": "oak"}),
    ] {
        assert!(!matches!(ops::dispatch(&mut ctx, "tree_edit", &bad), Ok(Applied::Done { .. })), "{bad}");
    }
    assert!(read(&project, "elm").is_none());
}
