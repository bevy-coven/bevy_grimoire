//! **A trigger can be bound to the area of a note** (#105, reusing #89's selection): the rectangle a person
//! drew is the rectangle the trigger is for, and the trigger keeps it as an ordinary `area`.

#[cfg(feature = "bevy_society")]
mod society {
    use bevy_cauldron::ops::{self, Applied, Ctx};
    use bevy_cauldron::ops_scripts::scripts_of;
    use bevy_cauldron::project::{OpenMap, Project};
    use serde_json::json;

    use crate::fixtures;

    fn village(tag: &str) -> (Ctx, Project) {
        let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
        let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
        for _ in 0..4 {
            app.update();
        }
        let project = app.world().resource::<Project>().clone();
        let open = app.world().resource::<OpenMap>().clone();
        let mut ctx = Ctx { project: Some(project.clone()), open: Some(open), session: tag.to_owned(), ..Ctx::default() };
        let mut ids = Vec::new();
        for at in [[0.5, 0.5], [9.5, 0.5]] {
            ops::dispatch(&mut ctx, "placement_add", &json!({"piece": "site/wall", "at": at})).unwrap();
            ids.push(ctx.open.as_ref().unwrap().map.placements.last().unwrap().id.clone());
        }
        for (name, x) in [("Mara", 0.0), ("Tobin", 1.0), ("Iva", 30.0)] {
            ops::dispatch(&mut ctx, "person_set", &json!({"map": "m", "name": name, "at": [x, 2.0], "home": ids[0], "work": ids[1]})).unwrap();
        }
        (ctx, project)
    }

    fn fired_for(ctx: &mut Ctx) -> Vec<String> {
        let Applied::Done { outcome, .. } = ops::dispatch(ctx, "people_run", &json!({"map": "m", "seed": 3, "days": 1.0})).unwrap() else {
            panic!("run refused")
        };
        let mut who: Vec<String> =
            outcome.after.unwrap()["fired"].as_array().unwrap().iter().map(|f| f["person"].as_str().unwrap().to_owned()).collect();
        who.sort();
        who.dedup();
        who
    }

    #[test]
    fn a_trigger_bound_to_a_note_is_for_the_people_standing_in_its_area() {
        let (mut ctx, project) = village("trigger_note_area");
        ops::dispatch(&mut ctx, "note_add", &json!({"cells": [0, 0, 2, 2], "text": "the smith's yard"})).unwrap();
        let rect = bevy_grimoire_core::notes::Notes::load(&bevy_grimoire_core::notes::path_for_map(&ctx.open.as_ref().unwrap().map_path))
            .unwrap()
            .get("n1")
            .unwrap()
            .frame_rect()
            .unwrap();

        let set = json!({"map": "m", "id": "yard", "on": "Activity(\"work\")", "then": "fx.bark(actor, \"hi\");", "bind": {"note": "n1"}});
        ops::dispatch(&mut ctx, "trigger_set", &set).unwrap();
        assert_eq!(scripts_of(&project, "m").triggers[0].bind.area, Some(rect), "the note's rectangle is the trigger's area");
        assert_eq!(fired_for(&mut ctx), ["Mara", "Tobin"], "Iva stands outside the note");

        let e = ops::dispatch(&mut ctx, "trigger_set", &json!({"map": "m", "id": "yard", "bind": {"note": "n9"}})).expect_err("no such note").0;
        assert!(e.contains("n9") && e.contains("Nothing was changed"), "{e}");
        assert_eq!(scripts_of(&project, "m").triggers[0].bind.area, Some(rect), "a refused bind changes nothing");
    }
}
