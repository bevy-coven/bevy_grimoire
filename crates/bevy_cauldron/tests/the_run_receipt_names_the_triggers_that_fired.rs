//! **The line a person reads after `people run` says which triggers fired and which scripts failed** (#105):
//! the editor shows only the receipt, so a trigger that fired for nobody visible would otherwise be invisible.

#[cfg(feature = "bevy_society")]
mod society {
    use bevy_cauldron::ops::{self, Applied, Ctx};
    use serde_json::json;

    use crate::fixtures;

    fn run_receipt(tag: &str, trigger: Option<serde_json::Value>) -> String {
        let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
        let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
        for _ in 0..4 {
            app.update();
        }
        let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
        let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
        let mut ctx = Ctx { project: Some(project), open: Some(open), session: tag.to_owned(), ..Ctx::default() };
        let mut ids = Vec::new();
        for at in [[0.5, 0.5], [9.5, 0.5]] {
            let Applied::Done { .. } = ops::dispatch(&mut ctx, "placement_add", &json!({"piece": "site/wall", "at": at})).unwrap() else {
                panic!("placement refused")
            };
            ids.push(ctx.open.as_ref().unwrap().map.placements.last().unwrap().id.clone());
        }
        ops::dispatch(&mut ctx, "person_set", &json!({"map": "m", "name": "Mara", "at": [0.0, 2.0], "home": ids[0], "work": ids[1]})).unwrap();
        if let Some(t) = trigger {
            ops::dispatch(&mut ctx, "trigger_set", &t).unwrap();
        }
        let Applied::Done { outcome, .. } = ops::dispatch(&mut ctx, "people_run", &json!({"map": "m", "seed": 3, "days": 1.0})).unwrap() else {
            panic!("run refused")
        };
        outcome.summary
    }

    #[test]
    fn a_fired_trigger_is_named_on_the_receipt_and_a_run_without_one_says_nothing_of_triggers() {
        let with = run_receipt(
            "receipt_fired",
            Some(json!({"map": "m", "id": "go_to_work", "on": "Activity(\"work\")", "then": "fx.bark(actor, \"on my way\");"})),
        );
        assert!(with.contains("go_to_work") && with.contains("Mara"), "the receipt names the trigger and the person: {with}");
        let without = run_receipt("receipt_none", None);
        assert!(!without.contains("fired"), "no trigger, nothing fired to report: {without}");
    }
}
