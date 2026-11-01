//! **`bevy_carnage` is a hosted family crate, and its own `CarnageSettings` is the saved settings** (#108, #96).
#![cfg(feature = "bevy_carnage")]

use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_carnage::CarnageSettings;
use serde_json::json;

use crate::fixtures;

#[test]
fn a_hit_stop_is_set_through_family_set_and_reads_back_as_the_crates_own_type() {
    let root = fixtures::Fixture::new("carnage_settings").descriptor("site/wall", "alpha").build("m");
    std::fs::create_dir_all(root.join("family")).unwrap();
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_carnage\"])").unwrap();
    let file = root.join("family/bevy_carnage.ron");
    std::fs::write(&file, "(trauma_per_wound: 0.4, hitstop_seconds: 0.055)").unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    assert!(
        bevy_cauldron::family_host::problems_with(&root, &bevy_cauldron::family_host::built()).is_empty(),
        "bevy_carnage is a known, built family crate"
    );
    let mut ctx = Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() };
    let done = ops::dispatch(&mut ctx, "family_set", &json!({"crate": "bevy_carnage", "field": "hitstop_seconds", "value": "0.1"})).unwrap();
    assert!(matches!(done, Applied::Done { .. }));
    let flicker: CarnageSettings = ron::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!((flicker.trauma_per_wound, flicker.hitstop_seconds), (0.4, 0.1));
}
