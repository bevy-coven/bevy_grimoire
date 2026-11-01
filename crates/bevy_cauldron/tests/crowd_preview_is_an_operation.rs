//! **`crowd_preview` measures the open map and changes nothing** (#107).
#![cfg(all(feature = "bevy_orca", feature = "bevy_stigmergy"))]

use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

fn ctx(tag: &str) -> (Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    let map_path = open.map_path.clone();
    (Ctx { project: Some(project), open: Some(open), session: "test".to_owned(), ..Ctx::default() }, map_path)
}

#[test]
fn a_run_reports_who_arrived_writes_nothing_and_repeats_by_seed() {
    let (mut c, map_path) = ctx("crowd_op");
    let before = std::fs::read(&map_path).unwrap();
    let params = json!({"from": [-2.0, -2.0, 0.0, 0.0], "to": [2.0, 2.0, 4.0, 4.0], "walkers": 5, "seed": 3});
    let run = |c: &mut Ctx| match ops::dispatch(c, "crowd_preview", &params).unwrap() {
        Applied::Done { outcome, .. } => outcome,
        other => panic!("refused: {other:?}"),
    };
    let a = run(&mut c);
    assert!(!a.saved && a.inverse.is_none());
    assert_eq!(a.after.as_ref().unwrap()["arrived"], 5, "{a:?}");
    assert_eq!(run(&mut c).after, a.after, "the same seed repeats");
    assert_eq!(std::fs::read(&map_path).unwrap(), before, "a preview never writes the map");
}

#[test]
fn nonsense_is_refused_with_the_reason() {
    let (mut c, _) = ctx("crowd_op_refuse");
    let zero = ops::dispatch(&mut c, "crowd_preview", &json!({"from": [0.0, 0.0, 1.0, 1.0], "to": [2.0, 2.0, 3.0, 3.0], "walkers": 0}));
    let e = match zero {
        Err(e) | Ok(Applied::Refused(e)) => e.0,
        Ok(_) => panic!("zero walkers ran"),
    };
    assert!(e.contains("walker"), "{e}");
    let typo = ops::dispatch(&mut c, "crowd_preview", &json!({"from": [0.0, 0.0, 1.0, 1.0], "walkers": 3}));
    assert!(matches!(typo, Err(_) | Ok(Applied::Refused(_))));
}
