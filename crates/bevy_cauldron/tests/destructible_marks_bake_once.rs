//! **A destructible piece bakes once, is cached in the project, and a hit breaks it into closed chunks** (#108).
#![cfg(feature = "bevy_carnage")]

use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

fn ctx(tag: &str) -> (Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/cart", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    (Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() }, root)
}

fn done(c: &mut Ctx, op: &str, params: serde_json::Value) -> ops::Outcome {
    match ops::dispatch(c, op, &params).unwrap() {
        Applied::Done { outcome, .. } => outcome,
        other => panic!("{op} refused: {other:?}"),
    }
}

#[test]
fn marking_bakes_once_and_the_cache_survives_an_unmark() {
    let (mut c, root) = ctx("destructible_bake");
    let first = done(&mut c, "destructible_set", json!({"piece": "site/cart", "on": true}));
    let after = first.after.as_ref().unwrap();
    assert_eq!(after["baked"], true, "{first:?}");
    assert!(after["chunks"].as_u64().unwrap() >= 2);
    let cache = root.join("destructible/site__cart.bake.ron");
    let bytes = std::fs::read(&cache).expect("the bake is cached in the project");
    assert!(std::fs::read_to_string(root.join("destructible.ron")).unwrap().contains("site/cart"));

    // Off keeps the bake; on again reads it instead of baking.
    let off = done(&mut c, "destructible_set", json!({"piece": "site/cart", "on": false}));
    assert_eq!(off.inverse, Some(json!({"piece": "site/cart", "on": true})));
    let again = done(&mut c, "destructible_set", json!({"piece": "site/cart", "on": true}));
    assert_eq!(again.after.as_ref().unwrap()["baked"], false, "a cached bake is not baked again");
    assert_eq!(std::fs::read(&cache).unwrap(), bytes);
}

#[test]
fn a_hit_breaks_the_piece_into_watertight_chunks_and_writes_nothing() {
    let (mut c, root) = ctx("destructible_hit");
    let refused = ops::dispatch(&mut c, "destructible_hit", &json!({"piece": "site/cart", "point": [0.0, 0.0, 0.0], "radius": 0.4}));
    assert!(matches!(refused, Err(_) | Ok(Applied::Refused(_))), "an unmarked piece does not break");
    done(&mut c, "destructible_set", json!({"piece": "site/cart", "on": true}));
    let before = std::fs::read(root.join("destructible.ron")).unwrap();
    let hit = done(&mut c, "destructible_hit", json!({"piece": "site/cart", "point": [0.0, 0.25, 0.0], "radius": 0.3}));
    let r = hit.after.unwrap();
    assert!(!hit.saved);
    assert!(r["broken"].as_u64().unwrap() >= 1, "{r}");
    assert!(r["total"].as_u64().unwrap() >= 2);
    assert_eq!(r["all_watertight"], true, "{r}");
    assert_eq!(std::fs::read(root.join("destructible.ron")).unwrap(), before);
}
