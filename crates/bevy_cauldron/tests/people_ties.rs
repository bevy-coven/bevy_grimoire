//! **People have ties to each other, authored beside the map and carried into a run** (#104).

use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::ops_people::people_of;
use serde_json::json;

use crate::fixtures;

fn open(tag: &str) -> Ctx {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    Ctx { project: Some(project), open: Some(open), session: tag.to_owned(), ..Ctx::default() }
}

fn set(ctx: &mut Ctx, params: serde_json::Value) -> Result<(), String> {
    match ops::dispatch(ctx, "person_set", &params) {
        Ok(Applied::Done { .. }) => Ok(()),
        Ok(Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

#[test]
fn a_tie_is_saved_updated_ended_and_refused_when_it_names_nobody() {
    let mut ctx = open("people_ties");
    let project = ctx.project.clone().unwrap();
    set(&mut ctx, json!({"map": "m", "name": "Mara", "at": [1.0, 1.0]})).unwrap();
    set(&mut ctx, json!({"map": "m", "name": "Odo", "at": [3.0, 1.0]})).unwrap();

    set(&mut ctx, json!({"map": "m", "name": "Mara", "tie": {"with": "Odo", "affinity": 0.3}})).unwrap();
    let t = &people_of(&project, "m").people[0].ties;
    assert_eq!((t.len(), t[0].with.as_str(), t[0].affinity, t[0].trust), (1, "Odo", 0.3, 0.5));

    // Setting only trust keeps the affinity.
    set(&mut ctx, json!({"map": "m", "name": "Mara", "tie": {"with": "Odo", "trust": 0.9}})).unwrap();
    let t = &people_of(&project, "m").people[0].ties;
    assert_eq!((t[0].affinity, t[0].trust), (0.3, 0.9));

    let before = people_of(&project, "m");
    for bad in [
        json!({"with": "Nobody", "affinity": 0.1}),
        json!({"with": "Mara", "affinity": 0.1}),
        json!({"with": "Odo", "affinity": 2.0}),
    ] {
        assert!(set(&mut ctx, json!({"map": "m", "name": "Mara", "tie": bad})).is_err());
        assert_eq!(people_of(&project, "m"), before, "a refused tie writes nothing");
    }

    // Removing the person drops the ties that pointed at them.
    set(&mut ctx, json!({"map": "m", "name": "Odo", "remove": true})).unwrap();
    assert!(people_of(&project, "m").people[0].ties.is_empty());
}

#[cfg(feature = "bevy_society")]
#[test]
fn an_authored_tie_is_in_the_society_the_run_starts_from() {
    let mut ctx = open("people_ties_run");
    set(&mut ctx, json!({"map": "m", "name": "Mara", "at": [1.0, 1.0]})).unwrap();
    set(&mut ctx, json!({"map": "m", "name": "Odo", "at": [3.0, 1.0]})).unwrap();
    set(&mut ctx, json!({"map": "m", "name": "Mara", "tie": {"with": "Odo", "affinity": 0.3, "trust": 0.8}})).unwrap();
    let Applied::Done { outcome, .. } = ops::dispatch(&mut ctx, "people_run", &json!({"map": "m", "seed": 1, "days": 0.01})).unwrap() else {
        panic!("refused")
    };
    let after = outcome.after.unwrap();
    let mara = after["people"].as_array().unwrap().iter().find(|p| p["name"] == "Mara").unwrap();
    let bond = &mara["bonds"].as_array().unwrap().iter().find(|b| b[0] == "Odo").unwrap_or_else(|| panic!("no tie to Odo: {mara}"));
    assert!((bond[1].as_f64().unwrap() - 0.3).abs() < 0.2 && bond[2].as_f64().unwrap() > 0.5, "{bond}");
}
