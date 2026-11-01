//! **The pool budget is `bevy_lights`' own** (#103): with a pool of one light, the fixture nearest
//! the viewer is the real one and the other stays virtual; moving the viewer swaps them.
#![cfg(feature = "bevy_lights")]

use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

fn editor(name: &str) -> Ctx {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/torch", "alpha")
        .place("site/torch", (0.0, 0.0))
        .place("site/torch", (30.0, 0.0))
        .build("m");
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_lights\"])").unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    let mut ctx = Ctx { project: Some(project), open: Some(open), session: name.to_owned(), ..Ctx::default() };
    let ids: Vec<String> = ctx.open.as_ref().unwrap().map.placements.iter().map(|p| p.id.clone()).collect();
    for id in ids {
        let done = ops::dispatch(&mut ctx, "placement_set_family", &json!({"id": id, "crate": "bevy_lights", "value": "(base: 1000.0)"})).unwrap();
        assert!(matches!(done, Applied::Done { .. }), "{done:?}");
    }
    ctx
}

fn pool(ctx: &mut Ctx, params: serde_json::Value) -> serde_json::Value {
    match ops::dispatch(ctx, "light_pool", &params).unwrap() {
        Applied::Done { outcome, .. } => outcome.after.unwrap(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_nearest_fixture_holds_the_only_real_light() {
    let mut ctx = editor("light_pool_one");
    let ids: Vec<String> = ctx.open.as_ref().unwrap().map.placements.iter().map(|p| p.id.clone()).collect();
    let near_first = pool(&mut ctx, json!({"focus": [1.0, 0.0], "size": 1}));
    assert_eq!(near_first["real"], json!([ids[0]]));
    assert_eq!(near_first["virtual"], json!([ids[1]]));
    assert_eq!((near_first["wanting"].as_u64(), near_first["starved"].as_u64()), (Some(2), Some(1)));
    let near_second = pool(&mut ctx, json!({"focus": [29.0, 0.0], "size": 1}));
    assert_eq!(near_second["real"], json!([ids[1]]));
}

#[test]
fn a_fixture_past_reach_wants_no_light() {
    let mut ctx = editor("light_pool_reach");
    let far = pool(&mut ctx, json!({"focus": [0.0, 0.0], "reach": 10.0}));
    assert_eq!((far["wanting"].as_u64(), far["granted"].as_u64(), far["starved"].as_u64()), (Some(1), Some(1), Some(0)));
}
