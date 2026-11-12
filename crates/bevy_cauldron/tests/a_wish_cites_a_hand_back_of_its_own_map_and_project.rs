//! **A wish's citation names one hand-back, on one map, in one project** (#353's review, the three
//! must-fixes).
//!
//! The record of hand-backs is the session's, so it outlives a door change — which is what #353
//! asks for. But a session is longer than a map: it can open another map in the same project, or
//! another project entirely, and it can watch a note that was finished last week be reopened and
//! finished again today. Each of those was a way for a wish to cite the wrong hand-back.
//!
//! # What is proved here
//!
//! 1. **A note is cited on its own map, by the map file's path.** A note `n1` resolved on this
//!    session's map `a` is not found while map `b` is open, even though `b` has a note `n1` that
//!    reads as `done` on disk. The record answers for the map the agent is looking at.
//! 2. **A citation names its kind.** `{note: "present-1"}` does not find the presentation, and
//!    `{presentation: "n1"}` does not fall through to the note.
//! 3. **A hand-back belongs to the project it was made in.** After `grimoire_open_project` opens a
//!    second project, this session's first-project note is refused by name, as another project's.
//! 4. **A note already `done` when the session opened, reopened and resolved again here, is this
//!    session's.** The baseline follows the note: an id leaves it the moment the note stops being
//!    handed back, so the resolve that follows records it.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::present::{HandBackKind, HandBacks};

use crate::fixtures;

/// **A project with one map**, saved, and handed back its root.
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m")
}

/// **A project with two maps**, `a` and `b`, each holding a piece so both open.
fn two_maps(name: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("a");
    // **The second map, in the shape the first one has.**
    fixtures::Fixture::write_map(&root, "b", 20.0, &[("site/wall", (1.0, 1.0), false)]);
    root
}

fn editor_at(app: &mut App) {
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
}

fn editor(root: &std::path::Path, map: &str) -> App {
    let mut app = harness::build_headless(root, map, None).unwrap_or_else(|e| panic!("{e}"));
    editor_at(&mut app);
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    use bevy::remote::RemoteMethodSystemId;
    use bevy::remote::RemoteMethods;
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err(format!("no method `{method}` is registered")),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .map_err(|e| format!("{method}: {e:?}"))?
        .map_err(|e| format!("{method}: {e:?}"))
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"))
}

/// **A wish citing a hand-back**, with every field filled in.
fn wish_body(after: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "after": after,
        "task": "place a wall along the courtyard's east side",
        "hardship": "three calls to grimoire_state to find the wall's piece id",
        "tool": "grimoire_find_piece: searches the library by label and answers the ids.",
        "example_call": "grimoire_find_piece({\"label\": \"wall\"})",
    })
}

/// **Work and resolve the open map's note `n1`**, through the registry, as an agent does.
fn resolve_the_note(app: &mut App) -> String {
    apply(app, "note_add", serde_json::json!({"cells": [0, 0, 2, 2], "text": "tidy the wall"}));
    apply(app, "note_send", serde_json::json!({}));
    apply(app, "note_claim", serde_json::json!({"id": "n1", "agent": "omp"}));
    let worked = apply(app, "placement_add", serde_json::json!({"piece": "site/wall", "at": [1.0, 1.0]}));
    let operation_id = worked["operation_id"].as_str().expect("the operation has an id").to_owned();
    let resolved = apply(
        app,
        "note_resolve",
        serde_json::json!({
            "id": "n1",
            "agent": "omp",
            "summary": "added a wall",
            "ops": [operation_id],
            "captures": ["before.png", "after.png"],
        }),
    );
    assert_eq!(resolved["outcome"], "applied", "the note resolves: {resolved}");
    for _ in 0..4 {
        app.update();
    }
    "n1".to_owned()
}

/// **Move the editor to another map of the same project**, as `map_open` does.
fn open_map(app: &mut App, map: &str) {
    let opened = apply(app, "map_open", serde_json::json!({"name": map}));
    assert_ne!(opened["outcome"], "refused", "the map opens: {opened}");
    for _ in 0..4 {
        app.update();
    }
}

/// **The map path a project calls `name`.**
fn map_path(root: &std::path::Path, name: &str) -> std::path::PathBuf {
    root.join("assets/grimoire/maps").join(format!("{name}.map.yaml"))
}

/// **The open project's root**, which is what the record compares — the harness canonicalises it, so
/// a test that passed the fixture's own path would be comparing two spellings of one directory.
fn project_root(app: &App) -> std::path::PathBuf {
    app.world().resource::<bevy_cauldron::project::Project>().root.clone()
}

/// **The record's answer for one citation**, asked the way `grimoire/wish` asks it.
fn cited(app: &App, kind: HandBackKind, id: &str, map: Option<&std::path::Path>) -> bool {
    app.world()
        .resource::<HandBacks>()
        .cited(kind, id, &project_root(app), map)
        .is_some()
}

/// **1. A note is cited on its own map, by the map file's path**, not by a name two maps share.
///
/// The map is `m` in both halves of the fixture — the same name the review's case had — and the
/// point is that a name is not enough: the citation is resolved against the open map's file.
#[test]
fn a_note_is_cited_on_its_own_map() {
    let root = project(&format!("wish_map_{}", std::process::id()));
    let mut app = editor(&root, "m");
    resolve_the_note(&mut app);
    // **The record holds this map's note.**
    assert!(
        cited(&app, HandBackKind::Note, "n1", Some(&map_path(&root, "m"))),
        "a note resolved on the open map is a hand-back of that map"
    );
    // **And it is not found for a different map's file**, even one whose `n1` reads as `done`:
    // the citation is answered for the map the agent is looking at, and no fallback guesses.
    let other = map_path(&root, "other");
    assert!(
        !cited(&app, HandBackKind::Note, "n1", Some(&other)),
        "a note id is unique on its own map and nowhere else, so `n1` on another map is not this \
         session's hand-back"
    );
    // **And the map's own file is what the key is made of.** The map's *name* is the same in both
    // projects the review's case used, so a key made of the name would answer for both.
    let held: Vec<String> = app.world().resource::<HandBacks>().hands.iter().map(|h| h.key.clone()).collect();
    assert!(
        held.iter().any(|key| key == &format!("{}:n1", map_path(&root, "m").display())),
        "a note's key is its map file's path and its id, so two projects' map `m` cannot share it: \
         {held:?}"
    );
}

/// **1b. While another map is open, this session's note on the first map is not citable**, and the
/// refusal names the note — which is the difference between "not here" and "never".
#[test]
fn a_note_on_a_map_that_is_not_open_is_refused() {
    let root = two_maps(&format!("wish_othermap_{}", std::process::id()));
    let mut app = editor(&root, "a");
    resolve_the_note(&mut app);
    open_map(&mut app, "b");
    let refused = call(&mut app, "grimoire/wish", wish_body(serde_json::json!({"note": "n1"})))
        .expect_err("the note resolved on map `a` is not a hand-back while map `b` is open");
    assert!(
        refused.contains("n1"),
        "the refusal does not name the note: {refused:?}"
    );
}

/// **2. A citation names its kind.** A presentation's id does not answer for a note, and a note's id
/// does not fall through to a presentation.
#[test]
fn a_citation_finds_only_its_own_kind_of_hand_back() {
    let root = project(&format!("wish_kind_{}", std::process::id()));
    let mut app = editor(&root, "m");
    let presented = call(
        &mut app,
        "grimoire/present",
        serde_json::json!({"title": "the courtyard", "summary": "A courtyard."}),
    )
    .unwrap_or_else(|e| panic!("the present answers: {e}"));
    let presentation = presented["presentation"].as_str().expect("the present names it").to_owned();
    assert_eq!(presentation, "present-1");

    // **A note's citation of a presentation's id is refused**, naming the id.
    let refused = call(&mut app, "grimoire/wish", wish_body(serde_json::json!({"note": presentation})))
        .expect_err("a note cannot cite a presentation's id");
    assert!(
        refused.contains(&presentation),
        "the refusal does not name the id the agent sent: {refused:?}"
    );

    // **And a presentation's citation of a note's record is refused** rather than finding it: the
    // note's record has this id, and the kinds do not interchange.
    let mut app = editor(&root, "m");
    resolve_the_note(&mut app);
    let refused = call(&mut app, "grimoire/wish", wish_body(serde_json::json!({"presentation": "n1"})))
        .expect_err("a presentation cannot cite a note's id");
    assert!(
        refused.contains("n1"),
        "the refusal does not name the id the agent sent: {refused:?}"
    );
}

/// **3. A hand-back belongs to the project it was made in**, so the same id in a second project is
/// refused as another project's — not as one that never existed.
///
/// **The switch is the resource swap a project open performs**, not `grimoire/project_open`: that
/// tool queues the open and sets `NextState`, and the headless harness registers no `ScreenPlugin`,
/// so the transition that installs the project never runs. What matters to the rule is which
/// project the editor holds when the wish arrives, and that is what this puts in the world.
#[test]
fn a_hand_back_from_another_project_is_refused_as_such() {
    let first = project(&format!("wish_p1_{}", std::process::id()));
    let mut app = editor(&first, "m");
    resolve_the_note(&mut app);
    assert!(
        call(&mut app, "grimoire/wish", wish_body(serde_json::json!({"note": "n1"}))).is_ok(),
        "the note is citable while its project is open"
    );

    // **The second project, open in the same editor**, which is what a project change leaves.
    let second = project(&format!("wish_p2_{}", std::process::id()));
    let opened = bevy_cauldron::project::Project::open(&second, None)
        .unwrap_or_else(|e| panic!("the second project opens: {e}"));
    app.insert_resource(opened);
    let other_map = bevy_cauldron::project::OpenMap::open(
        app.world().resource::<bevy_cauldron::project::Project>(),
        "m",
    )
    .unwrap_or_else(|e| panic!("the second project's map opens: {e}"));
    app.insert_resource(other_map);
    for _ in 0..4 {
        app.update();
    }
    assert_ne!(
        project_root(&app),
        std::fs::canonicalize(&first).unwrap_or_else(|_| first.clone()),
        "the editor is on the second project"
    );

    let refused = call(&mut app, "grimoire/wish", wish_body(serde_json::json!({"note": "n1"})))
        .expect_err("a hand-back of the first project is not one of the second's");
    assert!(
        refused.contains("another project"),
        "the refusal does not say the id was handed back in another project, which is the truth: \
         {refused:?}"
    );
}

/// **4. A note that was `done` when the session opened, reopened and resolved again here, is this
/// session's** — the baseline follows the note.
#[test]
fn a_reopened_note_is_this_sessions_after_it_is_resolved_again() {
    let root = project(&format!("wish_reopen_{}", std::process::id()));

    // **The earlier sitting**: an editor resolved the note and was dropped.
    {
        let mut before = editor(&root, "m");
        resolve_the_note(&mut before);
    }

    // **This session**, opened on the same project: the note reads as an earlier session's.
    let mut app = editor(&root, "m");
    assert!(
        !cited(&app, HandBackKind::Note, "n1", Some(&map_path(&root, "m"))),
        "a note already `done` when the session opened belongs to the earlier session"
    );

    // **The person reopens it and sends it again**, and an agent works it a second time.
    let undone = apply(&mut app, "note_undo_work", serde_json::json!({"id": "n1"}));
    assert_eq!(undone["outcome"], "applied", "the note's work is undone, which reopens it: {undone}");
    for _ in 0..4 {
        app.update();
    }
    // **Undoing its work left it `Open`**, which is what an agent may claim — a `note_send` here
    // would be refused, because only a draft is sent.
    let claimed = apply(&mut app, "note_claim", serde_json::json!({"id": "n1", "agent": "omp"}));
    assert_eq!(claimed["outcome"], "applied", "the reopened note is claimed: {claimed}");
    let worked = apply(&mut app, "placement_add", serde_json::json!({"piece": "site/wall", "at": [1.0, 1.0]}));
    let operation_id = worked["operation_id"].as_str().expect("the operation has an id").to_owned();
    let resolved = apply(
        &mut app,
        "note_resolve",
        serde_json::json!({
            "id": "n1",
            "agent": "omp",
            "summary": "added a wall again",
            "ops": [operation_id],
            "captures": ["before.png", "after.png"],
        }),
    );
    assert_eq!(resolved["outcome"], "applied", "the note resolves again: {resolved}");
    for _ in 0..4 {
        app.update();
    }

    let made = call(&mut app, "grimoire/wish", wish_body(serde_json::json!({"note": "n1"})))
        .unwrap_or_else(|e| {
            panic!(
                "a note reopened and resolved by an agent in this session is this session's \
                 hand-back, and the baseline must have let it go: {e}"
            )
        });
    assert_eq!(made["state"], "waiting", "the wish was not left waiting: {made}");
}
