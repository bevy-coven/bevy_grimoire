//! **A tree file that is refused keeps the trees drawn from its last good value, and the editor says
//! which file and field** (#200, H3).
//!
//! A `.tree.ron` is a species: every tree of it, placed or pooled, is drawn from it, and the view
//! regrows them when its modification time moves. A bad edit (a hand edit, a truncated write, an
//! agent's mistake) used to make the species unknown, so every tree of it vanished from the viewport
//! until the file was fixed. Now each edit is one verdict in `grimoire_state.reloads`: `applied`, or
//! `refused` with the field and the reason, and a refusal leaves the trees standing as they were,
//! drawn from the last good value held in memory. A tree file that does not exist at all is not a
//! refusal: its trees are not drawn and nothing is reported.

#[cfg(feature = "bevy_flora")]
mod with_flora {
    use std::time::Duration;

    use bevy::prelude::*;
    use bevy_flora::ProceduralTree;
    use bevy_grimoire_mapper::forest_view::ForestView;
    use bevy_grimoire_mapper::live::EditorReloads;
    use bevy_grimoire_mapper::ops::{self, Ctx};
    use bevy_grimoire_mapper::project::{OpenMap, Project};
    use serde_json::{Value, json};

    use crate::fixtures;

    const SPECIES: &str = "trees/old.tree.ron";

    /// A tree file that reads (every field it leaves out takes its default).
    const GOOD: &str = "(undergrowth: true)";
    const FIXED: &str = "(undergrowth: false)";

    /// The three ways to break it, and the field the verdict must name (`None`: the file is cut off
    /// before any field is known).
    const BAD: [(&str, &str, Option<&str>); 3] = [
        ("corrupt", "(undergrowth: true", None),
        ("wrong type", "(undergrowth: \"yes\")", Some("undergrowth")),
        ("unknown field", "(undergrow: true)", Some("undergrow")),
    ];

    fn reloads(app: &App) -> Value {
        app.world().get_resource::<EditorReloads>().expect("the editor keeps its verdicts").state_json()
    }

    fn trees(app: &App) -> Value {
        reloads(app)["kinds"].as_array().and_then(|k| k.iter().find(|v| v["kind"] == "trees").cloned()).unwrap_or(Value::Null)
    }

    fn seq(app: &App) -> u64 {
        reloads(app)["seq"].as_u64().unwrap()
    }

    /// The trees standing, by entity, with what each was grown from.
    fn standing(app: &mut App) -> Vec<(Entity, bool)> {
        let mut q = app.world_mut().query::<(Entity, &ForestView, &ProceduralTree)>();
        q.iter(app.world()).map(|(e, _, tree)| (e, tree.0.undergrowth)).collect()
    }

    fn until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
        for _ in 0..120 {
            app.update();
            if done(app) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("never happened: {what}; reloads {}", reloads(app));
    }

    fn pump(app: &mut App, frames: usize) {
        for _ in 0..frames {
            app.update();
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    #[test]
    fn a_refused_tree_file_keeps_the_trees_drawn_and_says_which_file_and_field_once_per_edit() {
        let root = fixtures::Fixture::new("refused_tree_file").descriptor("site/wall", "alpha").build("m");
        let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
        for _ in 0..4 {
            app.update();
        }
        let project = app.world().resource::<Project>().clone();
        let open = app.world().resource::<OpenMap>().clone();
        let mut ctx = Ctx { project: Some(project.clone()), open: Some(open), session: "refused_tree_file".to_owned(), ..Ctx::default() };
        let path = project.grimoire_dir.join(SPECIES);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, GOOD).unwrap();
        ops::dispatch(&mut ctx, "forest_scatter", &json!({"map": "m", "place": {"species": SPECIES, "at": [1.0, 1.0], "age_years": 3.0, "seed": 4}})).unwrap();

        until(&mut app, "the tree is drawn from its file", |app| standing(app).len() == 1);
        until(&mut app, "the tree file is applied", |app| trees(app)["outcome"] == "applied");
        let mut drawn = standing(&mut app);
        assert_eq!(drawn.iter().map(|(_, grown)| *grown).collect::<Vec<_>>(), [true], "drawn from the file");
        let applied = trees(&app);
        assert!(applied["file"].as_str().is_some_and(|f| f.ends_with(SPECIES)), "the verdict names the file: {applied}");

        for (what, text, field) in BAD {
            std::thread::sleep(Duration::from_millis(30));
            let before = seq(&app);
            std::fs::write(&path, text).unwrap();
            until(&mut app, &format!("the {what} file is refused"), |app| trees(app)["outcome"] == "refused");
            let refused = trees(&app);
            assert!(refused["file"].as_str().is_some_and(|f| f.ends_with(SPECIES)), "{what}: the verdict names the file: {refused}");
            match field {
                Some(field) => assert_eq!(refused["field"], json!(field), "{what}: {refused}"),
                None => assert!(refused["reason"].as_str().is_some_and(|r| !r.is_empty()), "{what}: says why: {refused}"),
            }
            assert_eq!(standing(&mut app), drawn, "{what}: the same tree stands, grown from the last good file");
            pump(&mut app, 20);
            assert_eq!(seq(&app), before + 1, "{what}: one edit is one verdict however many frames pass");
            assert_eq!(standing(&mut app), drawn, "{what}: still standing after more frames");

            // The next edit is a good one: it applies, and the verdict flips back.
            std::thread::sleep(Duration::from_millis(30));
            std::fs::write(&path, GOOD).unwrap();
            until(&mut app, &format!("the file is applied again after the {what} edit"), |app| trees(app)["outcome"] == "applied");
            // A good edit regrows the tree, so what stands now is the baseline for the next bad one.
            drawn = standing(&mut app);
            assert_eq!(drawn.iter().map(|(_, grown)| *grown).collect::<Vec<_>>(), [true], "{what}: drawn from the file");
        }

        std::thread::sleep(Duration::from_millis(30));
        std::fs::write(&path, FIXED).unwrap();
        until(&mut app, "the edited file regrows the tree", |app| standing(app).iter().map(|(_, grown)| *grown).collect::<Vec<_>>() == [false]);
        assert_eq!(trees(&app)["outcome"], "applied");

        std::fs::remove_file(&path).unwrap();
        until(&mut app, "a deleted file draws no trees", |app| standing(app).is_empty());
        assert_eq!(trees(&app), Value::Null, "a file that is gone is not a refusal");
    }
}
