//! **A forest, people, triggers or tree file that does not read is refused by the operation that would
//! rewrite it, naming the file and the field, and is never overwritten** (#200, H3).
//!
//! `forest_scatter`, `person_set`, `trigger_set` and `tree_edit` read the file, change one row and write
//! the whole file back. A file that did not parse used to count as no file, so one typo in a hand edit
//! made the next call write a file holding only its own row over everything else. For each of the four
//! kinds: a corrupt file, a file with a field of the wrong type and a file missing a required field
//! (a tree file has no required field, so a field the crate does not know) each refuse the call, the
//! refusal names the file (and the field, where the file got that far), the bytes on disk stay exactly
//! as they were, and once the file is fixed the same call applies and the rows that were there are
//! still there.

use bevy_grimoire_core::reload_check;
use bevy_grimoire_mapper::map_files::{READ_HERE, WATCHED_HERE};
use bevy_grimoire_mapper::ops::{self, Ctx};
use bevy_grimoire_mapper::project::{OpenMap, Project};
use serde_json::{Value, json};

use crate::fixtures;

/// One kind: the operation that rewrites its file, a call that works, the file as a person left it
/// and the three ways to break it.
struct Case {
    kind: &'static str,
    dir: &'static str,
    /// The file's name in `dir`: a per-map file is `m.ron`, a species is `m.tree.ron`.
    file: &'static str,
    op: &'static str,
    call: Value,
    /// A file that reads and holds a row of its own; `marker` is a string of that row.
    good: &'static str,
    marker: Option<&'static str>,
    /// `(what is wrong, the file, the field the refusal must name)`.
    bad: [(&'static str, &'static str, Option<&'static str>); 3],
}

fn cases() -> Vec<Case> {
    #[cfg_attr(not(feature = "bevy_flora"), allow(unused_mut))]
    let mut cases = vec![
        Case {
            kind: "people",
            dir: "people",
            file: "m.ron",
            op: "person_set",
            call: json!({"map": "m", "name": "Ola", "at": [2.0, 2.0]}),
            good: "(people: [(name: \"Mara\", at: (1.0, 2.0))])",
            marker: Some("Mara"),
            bad: [
                ("corrupt", "(people: [(name: \"Mara\", at: (1.0, 2.0)", None),
                ("wrong type", "(people: [(name: \"Mara\", at: \"here\")])", Some("people[0].at")),
                ("missing field", "(people: [(at: (1.0, 2.0))])", Some("people[0].name")),
            ],
        },
        Case {
            kind: "scripts",
            dir: "scripts",
            file: "m.ron",
            op: "trigger_set",
            call: json!({"map": "m", "id": "t", "on": "Activity(\"work\")", "then": "let x = 1;"}),
            good: "(triggers: [(id: \"first\", on: \"Activity(\\\"work\\\")\", then: \"let x = 1;\")])",
            marker: Some("first"),
            bad: [
                ("corrupt", "(triggers: [(id: \"first\", on: \"Activity(\\\"work\\\")\"", None),
                ("wrong type", "(triggers: [(id: 5, on: \"x\", then: \"y\")])", Some("triggers[0].id")),
                ("missing field", "(triggers: [(id: \"first\", on: \"x\")])", Some("triggers[0].then")),
            ],
        },
    ];
    #[cfg(feature = "bevy_flora")]
    cases.push(Case {
        kind: "forest",
        dir: "forest",
        file: "m.ron",
        op: "forest_scatter",
        call: json!({"map": "m", "plant": {"points": [[0.0, 0.0], [12.0, 0.0]], "radius_m": 4.0, "spacing_m": 3.0, "seed": 3, "species": [{"name": "oak", "weight": 1.0}]}}),
        good: "(trees: [])",
        marker: None,
        bad: [
            ("corrupt", "(trees: [(id: 1, species: \"oak\"", None),
            ("wrong type", "(trees: \"none\")", Some("trees")),
            ("missing field", "(trees: [(id: 1)])", Some("trees[0].")),
        ],
    });
    // Every field of a tree file has a default, so none can be missing: the third way to break it is
    // a field the crate does not know, which ron names.
    #[cfg(feature = "bevy_flora")]
    cases.push(Case {
        kind: "trees",
        dir: "trees",
        file: "m.tree.ron",
        op: "tree_edit",
        call: json!({"name": "m", "set": {"seed": 9}}),
        good: "(seed: 5)",
        marker: None,
        bad: [
            ("corrupt", "(seed: 5", None),
            ("wrong type", "(seed: \"five\")", Some("seed")),
            ("unknown field", "(sed: 5)", Some("sed")),
        ],
    });
    cases
}

fn open(tag: &str) -> (Ctx, Project) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<Project>().clone();
    let open = app.world().resource::<OpenMap>().clone();
    (Ctx { project: Some(project.clone()), open: Some(open), session: tag.to_owned(), ..Ctx::default() }, project)
}

fn put(project: &Project, case: &Case, text: &str) -> std::path::PathBuf {
    let dir = project.grimoire_dir.join(case.dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
    let path = dir.join(case.file);
    std::fs::write(&path, text).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    path
}

#[test]
fn a_file_that_does_not_read_refuses_the_operation_and_stays_as_it_was() {
    for case in cases() {
        let (mut ctx, project) = open(&format!("unread_{}", case.kind));
        for (what, text, field) in case.bad {
            let path = put(&project, &case, text);
            let refusal = ops::dispatch(&mut ctx, case.op, &case.call).expect_err(&format!("{} file, {what}: the call must be refused", case.kind)).0;
            assert!(refusal.contains(&format!("{}/{}", case.dir, case.file)), "{} {what}: the refusal names the file: {refusal}", case.kind);
            if let Some(field) = field {
                assert!(refusal.contains(field), "{} {what}: the refusal names the field `{field}`: {refusal}", case.kind);
            }
            assert!(refusal.contains("Nothing was changed"), "{} {what}: {refusal}", case.kind);
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "{} {what}: the file is left exactly as it was", case.kind);
        }
    }
}

#[test]
fn once_the_file_is_fixed_the_same_call_applies_and_keeps_the_rows_that_were_there() {
    for case in cases() {
        // Without `bevy_society` a trigger is refused whatever the file says, so only the refusal half applies.
        if case.kind == "scripts" && !cfg!(feature = "bevy_society") {
            continue;
        }
        let (mut ctx, project) = open(&format!("fixed_{}", case.kind));
        let path = put(&project, &case, case.bad[1].1);
        assert!(ops::dispatch(&mut ctx, case.op, &case.call).is_err(), "{}: refused while the file is bad", case.kind);
        put(&project, &case, case.good);
        ops::dispatch(&mut ctx, case.op, &case.call).unwrap_or_else(|e| panic!("{}: the fixed file applies: {}", case.kind, e.0));
        let after = std::fs::read_to_string(&path).unwrap();
        assert_ne!(after, case.good, "{}: the call changed the file", case.kind);
        if let Some(marker) = case.marker {
            assert!(after.contains(marker), "{}: the row that was there is still there: {after}", case.kind);
        }
    }
}

/// Every kind this layer reads has its three cases above, and the table says which kinds those are.
#[test]
fn the_cases_are_exactly_the_kinds_read_here() {
    let mut tested: Vec<&str> = cases().iter().map(|c| c.kind).collect();
    tested.sort_unstable();
    let mut read: Vec<&str> = READ_HERE.iter().copied().filter(|k| cfg!(feature = "bevy_flora") || !matches!(*k, "forest" | "trees")).collect();
    read.sort_unstable();
    assert_eq!(tested, read, "a kind in `map_files::READ_HERE` needs its three cases here, and a case needs the kind there");
}

/// **The kinds nothing reads with a named refusal**, checked in. Core's own list is
/// `tests/reload_verdicts.rs`'s `UNCHECKED`; the four kinds this layer reads are off it here. A kind
/// comes off when its type gains a reader that names the file and the field; none joins unseen.
const STILL_UNCHECKED: &[&str] =
    &["oplog", "reference_images", "family", "destructible_marks", "destructible_bake", "game_data"];

/// **A literal, not `STILL_UNCHECKED.len()`**: a number that is its own measure can never fail.
const STILL_UNCHECKED_MAX: usize = 6;

#[test]
fn the_kinds_nothing_reads_with_a_named_refusal_only_shrink() {
    assert!(STILL_UNCHECKED.len() == STILL_UNCHECKED_MAX, "the list has {} kinds and the ratchet says {STILL_UNCHECKED_MAX}: lower both together", STILL_UNCHECKED.len());
    for k in reload_check::unchecked() {
        assert!(READ_HERE.contains(&k.id) || WATCHED_HERE.contains(&k.id) || STILL_UNCHECKED.contains(&k.id), "`{}` is read by neither core nor `map_files::READ_HERE` or `WATCHED_HERE` and is not on this list", k.id);
    }
    for id in READ_HERE {
        assert!(reload_check::unchecked().any(|k| k.id == *id), "`{id}` is read here and core reads it too: take it off `READ_HERE`");
    }
    for id in STILL_UNCHECKED {
        assert!(!READ_HERE.contains(id), "`{id}` is read here now: take it off the list and lower STILL_UNCHECKED_MAX");
    }
    for id in WATCHED_HERE {
        assert!(reload_check::unchecked().any(|k| k.id == *id), "`{id}` is watched here and core reads it too: take it off `WATCHED_HERE`");
        assert!(!READ_HERE.contains(id), "`{id}` is on both `READ_HERE` and `WATCHED_HERE`: a kind is read one way");
        assert!(!STILL_UNCHECKED.contains(id), "`{id}` is watched here now: take it off the list and lower STILL_UNCHECKED_MAX");
    }
}

/// A run that reads the file refuses it too, with the real reason: it used to say nobody lives on the
/// map, which sent an agent to author someone over the file it could not read.
#[cfg(feature = "bevy_society")]
#[test]
fn a_run_over_a_file_that_does_not_read_is_refused_naming_it() {
    let (mut ctx, project) = open("unread_run");
    let people = cases().remove(0);
    let scripts = cases().remove(1);
    put(&project, &people, people.bad[1].1);
    let refusal = ops::dispatch(&mut ctx, "people_run", &json!({"map": "m"})).expect_err("a people file that does not read").0;
    assert!(refusal.contains("people/m.ron") && refusal.contains("people[0].at"), "{refusal}");
    put(&project, &people, people.good);
    put(&project, &scripts, scripts.bad[1].1);
    let refusal = ops::dispatch(&mut ctx, "people_run", &json!({"map": "m"})).expect_err("a scripts file that does not read").0;
    assert!(refusal.contains("scripts/m.ron") && refusal.contains("triggers[0].id"), "{refusal}");
}

/// The forest view decides its file once per edit: a refusal names the file and the field in
/// `EditorReloads`, the trees drawn stay, no frame adds a second verdict, and a fix is applied.
#[cfg(feature = "bevy_flora")]
#[test]
fn the_forest_view_decides_its_file_once_per_edit_and_keeps_the_trees() {
    use bevy::prelude::App;
    use bevy_grimoire_mapper::forest_view::ForestView;
    use bevy_grimoire_mapper::live::EditorReloads;

    fn drawn(app: &mut App) -> usize {
        app.world_mut().query::<&ForestView>().iter(app.world()).count()
    }
    fn reloads(app: &App) -> Value {
        app.world().get_resource::<EditorReloads>().expect("the editor keeps its verdicts").state_json()
    }
    fn forest(app: &App) -> Value {
        reloads(app)["kinds"].as_array().and_then(|k| k.iter().find(|v| v["kind"] == "forest").cloned()).unwrap_or(Value::Null)
    }
    fn until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
        for _ in 0..80 {
            app.update();
            if done(app) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("never happened: {what}; reloads {}", reloads(app));
    }

    let root = fixtures::Fixture::new("unread_forest_view").descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<Project>().clone();
    let open = app.world().resource::<OpenMap>().clone();
    let mut ctx = Ctx { project: Some(project.clone()), open: Some(open), session: "unread_forest_view".to_owned(), ..Ctx::default() };
    let case = cases().into_iter().find(|c| c.kind == "forest").expect("the forest case");
    ops::dispatch(&mut ctx, case.op, &case.call).unwrap();
    let path = project.grimoire_dir.join("forest").join("m.ron");
    let good = std::fs::read_to_string(&path).unwrap();

    until(&mut app, "the planted trees are drawn", |app| drawn(app) > 0);
    let standing = drawn(&mut app);
    until(&mut app, "the forest file is applied", |app| forest(app)["outcome"] == "applied");

    std::thread::sleep(std::time::Duration::from_millis(30));
    std::fs::write(&path, case.bad[1].1).unwrap();
    until(&mut app, "the bad file is refused", |app| forest(app)["outcome"] == "refused");
    let refused = forest(&app);
    assert_eq!(refused["field"], json!("trees"), "{refused}");
    assert!(refused["file"].as_str().is_some_and(|f| f.ends_with("forest/m.ron")), "{refused}");
    assert_eq!(drawn(&mut app), standing, "a refused file leaves the trees drawn");
    let seq = reloads(&app)["seq"].as_u64().unwrap();
    for _ in 0..20 {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(reloads(&app)["seq"].as_u64().unwrap(), seq, "one edit is one verdict however many frames pass");

    std::thread::sleep(std::time::Duration::from_millis(30));
    std::fs::write(&path, &good).unwrap();
    until(&mut app, "the fixed file is applied", |app| forest(app)["outcome"] == "applied");
    assert_eq!(drawn(&mut app), standing, "the fix draws the same trees");
}
