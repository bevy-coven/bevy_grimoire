//! **The Scripts panel draws one row per trigger of the open map** (#105), and a trigger saved or removed
//! reaches it without a restart. Proved on the real layout: the rows are `Text` entities with a
//! `ComputedNode`, in saved order, one below the other.

#[cfg(feature = "bevy_society")]
mod society {
    use bevy::prelude::*;
    use bevy::ui::{ComputedNode, UiGlobalTransform};
    use bevy_cauldron::ops::{self, Ctx};
    use bevy_cauldron::ops_scripts::trigger_line;
    use bevy_cauldron::project::{OpenMap, Project};
    use serde_json::json;

    use crate::fixtures;

    /// (text, top y, height) of every drawn text.
    fn drawn(app: &mut App) -> Vec<(String, f32, f32)> {
        for _ in 0..3 {
            std::thread::sleep(std::time::Duration::from_millis(300));
            app.update();
        }
        let world = app.world_mut();
        let mut q = world.query::<(&Text, &ComputedNode, &UiGlobalTransform)>();
        q.iter(world)
            .filter(|(_, n, t)| n.size().x > 0.0 && n.size().y > 0.0 && t.translation.y.is_finite())
            .map(|(t, n, tf)| (t.0.clone(), tf.translation.y - n.size().y / 2.0, n.size().y))
            .collect()
    }

    fn row<'a>(texts: &'a [(String, f32, f32)], id: &str) -> Option<&'a (String, f32, f32)> {
        texts.iter().find(|(t, _, _)| t.starts_with(&format!("{id} · ")))
    }

    #[test]
    fn each_trigger_is_a_drawn_row_and_a_removed_one_goes() {
        let root = fixtures::Fixture::new("scripts_panel").descriptor("site/wall", "alpha").build("m");
        let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
        for _ in 0..4 {
            app.update();
        }
        let project = app.world().resource::<Project>().clone();
        let open = app.world().resource::<OpenMap>().clone();
        let mut ctx = Ctx { project: Some(project.clone()), open: Some(open), session: "scripts_panel".into(), ..Ctx::default() };

        let texts = drawn(&mut app);
        assert!(texts.iter().any(|(t, _, _)| t == "SCRIPTS"), "the panel has its heading; drawn: {texts:?}");
        assert!(texts.iter().any(|(t, _, _)| t == "no triggers on this map"));

        for (id, on) in [("dawn", "Activity(\"work\")"), ("dusk", "Activity(\"sleep\")")] {
            ops::dispatch(&mut ctx, "trigger_set", &json!({"map": "m", "id": id, "on": on, "then": "fx.bark(actor, \"hi\");", "bind": {"area": [0, 0, 4, 4]}}))
                .unwrap();
        }
        let texts = drawn(&mut app);
        let (dawn, dusk) = (row(&texts, "dawn").expect("dawn row"), row(&texts, "dusk").expect("dusk row"));
        assert!(dawn.0.contains("Activity(\"work\")") && dawn.0.ends_with("· area"), "{}", dawn.0);
        assert!(dusk.1 >= dawn.1 + dawn.2 - 0.5, "rows stack: dawn {dawn:?}, dusk {dusk:?}");
        assert!(!texts.iter().any(|(t, _, _)| t == "no triggers on this map"));

        ops::dispatch(&mut ctx, "trigger_set", &json!({"map": "m", "id": "dawn", "remove": true})).unwrap();
        let texts = drawn(&mut app);
        assert!(row(&texts, "dawn").is_none() && row(&texts, "dusk").is_some(), "{texts:?}");
    }

    #[test]
    fn a_rows_line_is_what_the_terrain_line_parses_back() {
        use bevy_cauldron::ops_scripts::{Bind, Trigger};
        let t = Trigger {
            id: "t".into(),
            on: "Activity(\"work\")".into(),
            condition: Some("true".into()),
            then: "fx.bark(actor, \"hi\");".into(),
            once: false,
            cooldown_hours: 0.0,
            bind: Bind { people: vec!["Mara".into(), "Tobin".into()], area: Some([0.0, 0.0, 4.0, 4.0]), ..Bind::default() },
        };
        assert_eq!(
            trigger_line(&t),
            "trigger t | Activity(\"work\") | true | fx.bark(actor, \"hi\"); | people=Mara,Tobin | area=0,0,4,4"
        );
    }
}
