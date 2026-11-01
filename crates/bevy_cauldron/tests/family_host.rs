//! **A project listing a family crate the build lacks is told so** (#96), and one listing nothing is not.

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::family_host::{self, report_problems};

use crate::fixtures;

fn problems_after_open(family_ron: Option<&str>, tag: &str) -> Vec<String> {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    if let Some(text) = family_ron {
        std::fs::write(root.join("family.ron"), text).unwrap();
    }
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<EditorState>().add_systems(Update, report_problems);
    for _ in 0..3 {
        app.update();
    }
    app.world().resource::<EditorState>().status.problems().iter().map(|p| p.text.clone()).collect()
}

#[test]
fn a_listed_crate_without_its_feature_is_a_problem_and_no_file_is_silent() {
    assert!(problems_after_open(None, "family_none").is_empty());
    let built = family_host::built();
    let want_problem = !built.contains(&"bevy_elements");
    let got = problems_after_open(Some("(version: 1, crates: [\"bevy_elements\"])"), "family_listed");
    if want_problem {
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("bevy_elements") && got[0].contains("--features bevy_elements"), "{got:?}");
    } else {
        assert!(got.is_empty(), "{got:?}");
    }
    let got = problems_after_open(Some("(version: 1, crates: [\"nope\"])"), "family_unknown");
    assert!(got.iter().any(|p| p.contains("nope")), "{got:?}");
}
