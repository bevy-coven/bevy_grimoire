//! **`bevy_flora` is a hosted family crate** (#102, F0 slice): a project lists it, the build says
//! whether it has it, and its settings are written by the same `family_set` operation.

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::family_host::{self, report_problems};
use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

fn open(tag: &str) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    std::fs::create_dir_all(root.join("family")).unwrap();
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_flora\"])").unwrap();
    std::fs::write(root.join("family/bevy_flora.ron"), "(scatter: (density: 0.2, seed: 7))\n").unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<EditorState>().add_systems(Update, report_problems);
    for _ in 0..4 {
        app.update();
    }
    (app, root)
}

#[test]
fn flora_is_known_and_is_a_problem_only_when_the_build_lacks_it() {
    assert!(family_host::KNOWN.contains(&"bevy_flora"));
    let (app, _) = open("flora_listed");
    let problems: Vec<String> = app.world().resource::<EditorState>().status.problems().iter().map(|p| p.text.clone()).collect();
    if family_host::built().contains(&"bevy_flora") {
        assert!(problems.is_empty(), "{problems:?}");
    } else {
        assert!(problems.len() == 1 && problems[0].contains("--features bevy_flora"), "{problems:?}");
    }
}

#[test]
fn flora_settings_are_written_by_family_set_and_one_undo_restores_them() {
    let (app, root) = open("flora_set");
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    let mut ctx = Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() };
    let file = root.join("family/bevy_flora.ron");
    let before = std::fs::read_to_string(&file).unwrap();
    let r = ops::dispatch(&mut ctx, "family_set", &json!({"crate": "bevy_flora", "field": "scatter.density", "value": "0.6"})).unwrap();
    assert!(matches!(r, Applied::Done { .. }));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "(scatter: (density: 0.6, seed: 7))\n");
    ops::undo(&mut ctx, None).unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
}
