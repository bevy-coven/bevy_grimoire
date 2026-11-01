//! **A trigger is a Rhai script bound to people, and a script that does not compile is refused** (#105).

#[cfg(feature = "bevy_society")]
mod society {
    use bevy_cauldron::ops::{self, Applied, Ctx};
    use bevy_cauldron::ops_scripts::scripts_of;
    use bevy_cauldron::project::Project;
    use serde_json::json;

    use crate::fixtures;

    fn village(tag: &str) -> (Ctx, Project) {
        let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
        let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
        for _ in 0..4 {
            app.update();
        }
        let project = app.world().resource::<Project>().clone();
        let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
        let mut ctx = Ctx { project: Some(project.clone()), open: Some(open), session: tag.to_owned(), ..Ctx::default() };
        let mut ids = Vec::new();
        for at in [[0.5, 0.5], [9.5, 0.5]] {
            let Applied::Done { .. } = ops::dispatch(&mut ctx, "placement_add", &json!({"piece": "site/wall", "at": at})).unwrap() else {
                panic!("placement refused")
            };
            ids.push(ctx.open.as_ref().unwrap().map.placements.last().unwrap().id.clone());
        }
        for (name, x) in [("Mara", 0.0), ("Tobin", 1.0), ("Iva", 30.0)] {
            ops::dispatch(&mut ctx, "person_set", &json!({"map": "m", "name": name, "at": [x, 2.0], "home": ids[0], "work": ids[1]})).unwrap();
        }
        (ctx, project)
    }

    fn fired(ctx: &mut Ctx) -> Vec<(String, String)> {
        let Applied::Done { outcome, .. } = ops::dispatch(ctx, "people_run", &json!({"map": "m", "seed": 3, "days": 1.0})).unwrap() else {
            panic!("run refused")
        };
        let after = outcome.after.unwrap();
        assert_eq!(after["script_errors"], json!([]), "no script failed: {after}");
        after["fired"].as_array().unwrap().iter().map(|f| (f["trigger"].as_str().unwrap().to_owned(), f["person"].as_str().unwrap().to_owned())).collect()
    }

    #[test]
    fn a_script_with_a_syntax_error_is_refused_with_its_line_and_nothing_is_saved() {
        let (mut ctx, project) = village("trigger_syntax");
        let bad = json!({"map": "m", "id": "oops", "on": "Activity(\"work\")", "then": "fx.bark(actor, \"hi\");\nlet x = ;"});
        let e = ops::dispatch(&mut ctx, "trigger_set", &bad).expect_err("a script that does not compile").0;
        assert!(e.contains("oops") && e.contains("line 2"), "the refusal names the trigger and the line: {e}");
        assert!(scripts_of(&project, "m").triggers.is_empty(), "nothing was saved");
        assert!(!project.grimoire_dir.join("scripts").join("m.ron").exists());
    }

    #[test]
    fn a_trigger_bound_to_an_area_fires_only_for_people_inside_it() {
        let (mut ctx, project) = village("trigger_area");
        let all = json!({"map": "m", "id": "everyone", "on": "Activity(\"work\")", "then": "fx.bark(actor, \"on my way\");"});
        let Applied::Done { .. } = ops::dispatch(&mut ctx, "trigger_set", &all).unwrap() else { panic!("refused") };
        let mut anyone: Vec<String> = fired(&mut ctx).into_iter().map(|(_, p)| p).collect();
        anyone.sort();
        anyone.dedup();
        assert!(anyone.contains(&"Iva".to_owned()) && anyone.len() >= 3, "the unbound trigger reaches everyone, or the area test proves nothing: {anyone:?}");

        let area = json!({"map": "m", "id": "everyone", "bind": {"area": [-1.0, 0.0, 5.0, 4.0]}});
        ops::dispatch(&mut ctx, "trigger_set", &area).unwrap();
        assert_eq!(scripts_of(&project, "m").triggers[0].bind.area, Some([-1.0, 0.0, 5.0, 4.0]));
        let inside = fired(&mut ctx);
        assert!(!inside.is_empty(), "the trigger still fires for the people inside");
        assert!(inside.iter().all(|(t, p)| t == "everyone" && (p == "Mara" || p == "Tobin")), "only Mara and Tobin stand in the area: {inside:?}");
    }

    #[test]
    fn one_undo_takes_a_trigger_back() {
        let (mut ctx, project) = village("trigger_undo");
        let Applied::Done { operation_id, .. } =
            ops::dispatch(&mut ctx, "trigger_set", &json!({"map": "m", "id": "t", "on": "Tick", "then": "let a = 1;"})).unwrap()
        else {
            panic!("refused")
        };
        assert_eq!(scripts_of(&project, "m").triggers.len(), 1);
        ops::undo(&mut ctx, Some(&operation_id)).unwrap();
        assert!(scripts_of(&project, "m").triggers.is_empty());
    }
}
