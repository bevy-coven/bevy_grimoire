//! **A society definition the crate refuses to compile is not written** (#104): the saved file is the crate's
//! own `DefsFile`, and `SocietyDefs::from_ron` decides, so a village the editor saves is one the game loads.
#![cfg(feature = "bevy_society")]

use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

const DEFS: &str = r#"(needs: [(id: "hunger", decay_per_day: 1.0)], factions: [(id: "guild")])"#;

fn open(name: &str) -> (Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(name).descriptor("site/wall", "alpha").build("m");
    std::fs::create_dir_all(root.join("family")).unwrap();
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_society\"])").unwrap();
    let file = root.join("family/bevy_society.ron");
    std::fs::write(&file, DEFS).unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    (Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() }, file)
}

#[test]
fn a_need_defined_twice_is_refused_with_the_crates_message_and_the_file_is_untouched() {
    let (mut ctx, file) = open("society_bad");
    let err = ops::dispatch(
        &mut ctx,
        "family_set",
        &json!({"crate": "bevy_society", "field": "needs", "value": r#"[(id: "hunger"), (id: "hunger")]"#}),
    )
    .map(|_| ())
    .unwrap_err();
    assert!(format!("{err:?}").contains("need 'hunger' is defined more than once"), "{err:?}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), DEFS);
}

#[test]
fn a_valid_edit_is_written_and_reads_back_as_the_crates_own_definitions() {
    let (mut ctx, file) = open("society_ok");
    let done = ops::dispatch(
        &mut ctx,
        "family_set",
        &json!({"crate": "bevy_society", "field": "needs", "value": r#"[(id: "hunger"), (id: "rest")]"#}),
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    assert!(matches!(done, Applied::Done { .. }));
    let defs = bevy_society::SocietyDefs::from_ron(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert!(defs.need_id("rest").is_some());
}
