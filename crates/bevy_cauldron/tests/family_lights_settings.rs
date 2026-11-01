//! **`bevy_lights` is a hosted family crate, and its own `Flicker` is the saved settings** (#103, #96).
#![cfg(feature = "bevy_lights")]

use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_lights::Flicker;
use serde_json::json;

use crate::fixtures;

#[test]
fn a_brazier_flicker_is_set_through_family_set_and_reads_back_as_the_crates_own_type() {
    let root = fixtures::Fixture::new("lights_flicker").descriptor("site/wall", "alpha").build("m");
    std::fs::create_dir_all(root.join("family")).unwrap();
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_lights\"])").unwrap();
    let file = root.join("family/bevy_lights.ron");
    std::fs::write(&file, "(base: 900.0, phase: 1.3, depth: 0.15)").unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    assert!(
        bevy_cauldron::family_host::problems_with(&root, &bevy_cauldron::family_host::built()).is_empty(),
        "bevy_lights is a known, built family crate"
    );
    let mut ctx = Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() };
    let done = ops::dispatch(&mut ctx, "family_set", &json!({"crate": "bevy_lights", "field": "depth", "value": "0.3"})).unwrap();
    assert!(matches!(done, Applied::Done { .. }));
    let flicker: Flicker = ron::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!((flicker.base, flicker.phase, flicker.depth), (900.0, 1.3, 0.3));
}
