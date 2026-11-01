//! **Editing a script file while a project is open takes effect on the next run, with no restart** (#105): the
//! triggers are read from `scripts/<map>.ron` each run, so what a live editor or a person writes there is what fires.

#[cfg(feature = "bevy_society")]
mod society {
    use bevy_cauldron::ops::{self, Applied, Ctx};
    use bevy_cauldron::project::{OpenMap, Project};
    use serde_json::json;

    use crate::fixtures;

    fn fired_triggers(ctx: &mut Ctx) -> Vec<String> {
        let Applied::Done { outcome, .. } = ops::dispatch(ctx, "people_run", &json!({"map": "m", "seed": 3, "days": 1.0})).unwrap() else {
            panic!("run refused")
        };
        let after = outcome.after.unwrap();
        let mut ids: Vec<String> = after["fired"].as_array().unwrap().iter().map(|f| f["trigger"].as_str().unwrap().to_owned()).collect();
        ids.sort();
        ids.dedup();
        ids
    }

    #[test]
    fn a_script_edited_on_disk_takes_effect_on_the_next_run() {
        let root = fixtures::Fixture::new("script_hot").descriptor("site/wall", "alpha").build("m");
        let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
        for _ in 0..4 {
            app.update();
        }
        let project = app.world().resource::<Project>().clone();
        let open = app.world().resource::<OpenMap>().clone();
        let mut ctx = Ctx { project: Some(project.clone()), open: Some(open), session: "script_hot".to_owned(), ..Ctx::default() };
        let mut ids = Vec::new();
        for at in [[0.5, 0.5], [9.5, 0.5]] {
            ops::dispatch(&mut ctx, "placement_add", &json!({"piece": "site/wall", "at": at})).unwrap();
            ids.push(ctx.open.as_ref().unwrap().map.placements.last().unwrap().id.clone());
        }
        ops::dispatch(&mut ctx, "person_set", &json!({"map": "m", "name": "Mara", "at": [0.0, 2.0], "home": ids[0], "work": ids[1]})).unwrap();
        ops::dispatch(&mut ctx, "trigger_set", &json!({"map": "m", "id": "before", "on": "Activity(\"work\")", "then": "fx.bark(actor, \"hi\");"})).unwrap();
        assert_eq!(fired_triggers(&mut ctx), vec!["before".to_owned()]);

        let file = project.grimoire_dir.join("scripts").join("m.ron");
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("before"));
        std::fs::write(&file, text.replace("before", "after")).unwrap();
        assert_eq!(fired_triggers(&mut ctx), vec!["after".to_owned()], "the edited file is what the next run fires");
    }
}
