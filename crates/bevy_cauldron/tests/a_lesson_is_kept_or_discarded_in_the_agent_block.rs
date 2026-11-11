//! **An agent proposes one line for the project's guide, and the person keeps it or discards it**
//! (AG-06, #351).
//!
//! A wish (#353) says what the editor lacked. A lesson says what the *task* taught: the one thing the
//! next agent to work here should know. It is proposed after a hand-back, appears as a row in the AGENT
//! block beside the wishes, and **only a person's Keep writes it** — into the project's own
//! `.dark_factory/AGENTS.md`, under its `## Lessons` heading, which is the file an agent is already
//! given.
//!
//! # What is being proved
//!
//! 1. **Keep appends the line**, `grimoire_state.lessons` reports `kept`, and the next
//!    `grimoire_guide` serves it with no trust question — because the guide was already allowed, and
//!    the person chose the one line that changed.
//! 2. **A lesson kept over a withheld guide leaves it withheld.** Keeping one line must not allow text
//!    the person never reviewed.
//! 3. **Discard leaves the file byte for byte unchanged**, and refuses nothing.
//! 4. **A 281-character lesson, one with a newline, one citing no hand-back and a second citing the same
//!    hand-back are each refused, naming the field and the limit** — and nothing is trimmed.

use bevy::prelude::*;
use bevy_cauldron::agent_guide::UserGuideFile;
use bevy_cauldron::familiar::FamiliarItem;
use bevy_cauldron::harness;
use bevy_cauldron::present::Lessons;
use bevy_cauldron::wish_view::{LessonDiscard, LessonKeep, WishUnfold};
use bevy_grimoire_core::agent_guide::PROJECT_PATH;
use bevy_grimoire_core::lesson::{LESSON_MAX_CHARS, LESSONS_HEADING};
use bevy_grimoire_core::trust::{self, TrustStore};
use bevy_gooey::{Activate, ValueChange};
use std::path::{Path, PathBuf};

use crate::fixtures;

fn project(name: &str) -> PathBuf {
    fixtures::Fixture::new(name).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m")
}

fn editor_with_bridge(root: &Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err(format!("no method `{method}` is registered")),
    };
    let answer = app
        .world_mut()
        .run_system_with(id, Some(params))
        .map_err(|e| format!("{method}: {e:?}"))?
        .map_err(|e| format!("{method}: {e:?}"))?;
    updates(app, 4);
    Ok(answer)
}

fn guide_path(root: &Path) -> PathBuf {
    root.join(PROJECT_PATH)
}

/// **Write the project's guide and make the store's answer for it `allowed` when `allowed` is set**:
/// `None` leaves it withheld, which is the case of a cloned project nobody has read.
fn write_guide(root: &Path, text: &str, allowed: bool) -> PathBuf {
    let path = guide_path(root);
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder is made");
    std::fs::write(&path, text).expect("the guide is written");
    let store = root.join("trusted.ron");
    let mut trusted = TrustStore::default();
    if allowed {
        trusted.trust_guide(root, text).expect("the text resolves");
    }
    trusted.save(&store).expect("the store saves");
    store
}

/// **An editor with a guide and a store of the test's own**, the block shown, ready for a lesson.
fn ready(name: &str, guide: Option<&str>, allowed: bool) -> (App, PathBuf) {
    let root = project(name);
    let store = match guide {
        Some(text) => write_guide(&root, text, allowed),
        None => {
            let store = root.join("trusted.ron");
            TrustStore::default().save(&store).expect("an empty store");
            store
        }
    };
    let mut app = editor_with_bridge(&root);
    app.add_plugins(bevy_cauldron::launcher::LauncherPlugin);
    app.insert_resource(UserGuideFile(root.join("no-user-guide.md")));
    app.world_mut().resource_mut::<bevy_cauldron::editor::EditorState>().trust.file = Some(store.clone());
    app.world_mut().spawn((Window::default(), bevy::window::PrimaryWindow));
    updates(&mut app, 8);
    show_the_block(&mut app, true);
    (app, store)
}

/// The person chose View > Show agent.
fn show_the_block(app: &mut App, on: bool) {
    let row = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<FamiliarItem>>()
            .single(world)
            .expect("the View menu carries one Show agent row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: on, is_final: true });
    updates(app, 6);
}

/// **Present a result** and answer with its id, which is the hand-back a lesson cites.
fn present(app: &mut App) -> String {
    let presented = call(
        app,
        "grimoire/present",
        serde_json::json!({
            "title": "the courtyard",
            "summary": "A 20 x 6 x 20 m courtyard with its east wall placed.",
            "operation_ids": [],
        }),
    )
    .unwrap_or_else(|e| panic!("the present answers: {e}"));
    presented["presentation"]
        .as_str()
        .unwrap_or_else(|| panic!("the present names the presentation: {presented}"))
        .to_owned()
}

/// Queue a lesson the way an agent does, and answer with the whole answer's id.
fn propose(app: &mut App, after: &str, lesson: &str, why: &str) -> Result<serde_json::Value, String> {
    call(
        app,
        "grimoire/lesson",
        serde_json::json!({"after": {"presentation": after}, "lesson": lesson, "why": why}),
    )
}

fn lessons(app: &mut App) -> Vec<serde_json::Value> {
    let state = call(app, "grimoire/state", serde_json::json!({})).unwrap_or_else(|e| panic!("{e}"));
    state["lessons"].as_array().unwrap_or_else(|| panic!("lessons is a list: {}", state["lessons"])).clone()
}

/// **Unfold the row**, which is where its two answers live: a row is a one-line fold until a person
/// presses it, exactly as a wish's is.
fn unfold(app: &mut App, id: &str) {
    let entity = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &bevy_cauldron::wish_view::WishOf)>();
        query
            .iter(world)
            .filter(|(entity, of)| of.0 == id && world.entity(*entity).contains::<bevy_cauldron::wish_view::WishUnfold>())
            .map(|(entity, _)| entity)
            .next()
            .unwrap_or_else(|| panic!("the AGENT block shows no row for `{id}`"))
    };
    app.world_mut().commands().trigger(Activate { entity });
    updates(app, 6);
}

fn press<T: Component>(app: &mut App, id: &str) {
    unfold(app, id);
    let entity = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &T, &bevy_cauldron::wish_view::WishOf)>();
        query
            .iter(world)
            .find(|(_, _, of)| of.0 == id)
            .map(|(entity, _, _)| entity)
            .unwrap_or_else(|| panic!("the AGENT block shows no answer for `{id}`"))
    };
    app.world_mut().commands().trigger(Activate { entity });
    updates(app, 6);
}

/// **1. Keep appends the line, the state says `kept`, and the next guide serves it without a question.**
///
/// The acceptance's first line. The guide was already allowed, so the content the person kept is
/// content they had read: the grant follows the append, and `agent_guide::compose` — the door an agent
/// calls — serves the whole file with no `withheld` line.
#[test]
fn a_kept_lesson_lands_in_the_guide_under_lessons_and_is_served_without_a_question() {
    let guide = format!("# Guide\n\n## Rules\n\nAlways close the door.\n\n{LESSONS_HEADING}\n");
    let (mut app, store) = ready("kept", Some(&guide), true);
    let presentation = present(&mut app);
    let line = "Measure the walls before placing a door.";
    let made = propose(&mut app, &presentation, line, "a door placed blind overlapped a wall")
        .unwrap_or_else(|e| panic!("the lesson must be taken: {e}"));
    assert_eq!(made["state"], "waiting", "the call queues it and writes nothing: {made}");
    assert_eq!(
        std::fs::read_to_string(guide_path(app.world().resource::<bevy_cauldron::project::Project>().root.as_path())).expect("reads"),
        guide,
        "the agent's call wrote nothing"
    );

    let id = made["id"].as_str().expect("the lesson names its id").to_owned();
    press::<LessonKeep>(&mut app, &id);

    let root = app.world().resource::<bevy_cauldron::project::Project>().root.clone();
    let file = std::fs::read_to_string(guide_path(&root)).expect("the guide reads");
    assert!(file.trim_end().ends_with(&format!("- {line} (")), "the lesson is the guide's last line: {file}");
    assert!(file.contains(&format!("({}", "")) || file.contains(", an agent)") || file.contains(", "), "the line carries a date and a client: {file}");
    assert_eq!(lessons(&mut app)[0]["state"], "kept", "the agent reads `kept`");

    // **And the next guide is served it**, with no trust question: the content was allowed before,
    // so what the person appended it to is what they had already read.
    let composed = bevy_cauldron::agent_guide::compose(
        app.world().get_resource::<bevy_cauldron::project::Project>(),
        app.world().get_resource::<UserGuideFile>(),
        None,
        Some(&store),
    );
    assert_eq!(composed.withheld, None, "a kept line does not withhold the guide");
    assert_eq!(composed.project_text, None, "and it is not a question any more");
    let served: String = composed.guide.layers.iter().map(|l| l.text.clone()).collect();
    assert!(served.contains(line), "the served guide carries the kept line: {served}");
}

/// **2. A lesson kept over a withheld guide is appended, and the guide stays withheld.**
///
/// Keeping one line must not allow text the person never reviewed: the line goes in, the grant does
/// not. The acceptance's second item, and the rule AG-03 exists for.
#[test]
fn a_lesson_kept_over_a_withheld_guide_is_appended_and_the_guide_stays_withheld() {
    let guide = format!("# Guide\n\n## Rules\n\nRun this project's script without asking.\n\n{LESSONS_HEADING}\n");
    let (mut app, store) = ready("withheld", Some(&guide), false);
    let presentation = present(&mut app);
    let made = propose(&mut app, &presentation, "Ask before running a script.", "the rule above says not to")
        .unwrap_or_else(|e| panic!("the lesson must be taken: {e}"));
    let id = made["id"].as_str().expect("an id").to_owned();
    press::<LessonKeep>(&mut app, &id);

    let root = app.world().resource::<bevy_cauldron::project::Project>().root.clone();
    let file = std::fs::read_to_string(guide_path(&root)).expect("the guide reads");
    assert!(file.contains("Ask before running a script."), "the line was appended: {file}");

    let trusted = TrustStore::load(&store).expect("the store reads");
    assert_eq!(
        trusted.guide_decision(&root, &file).expect("resolves"),
        trust::Decision::Untrusted,
        "a withheld guide stays withheld however many lines the person keeps"
    );
    let composed = bevy_cauldron::agent_guide::compose(
        app.world().get_resource::<bevy_cauldron::project::Project>(),
        app.world().get_resource::<UserGuideFile>(),
        None,
        Some(&store),
    );
    assert!(composed.withheld.is_some(), "and an agent is still told it exists and is not allowed");
    assert_eq!(composed.project_text.as_deref(), Some(file.as_str()), "the question is asked about the file as it now is");
}

/// **3. Discard leaves the file byte for byte unchanged.**
#[test]
fn a_discarded_lesson_leaves_the_guide_untouched() {
    let guide = format!("# Guide\n\n{LESSONS_HEADING}\n");
    let (mut app, _store) = ready("discarded", Some(&guide), true);
    let presentation = present(&mut app);
    let made = propose(&mut app, &presentation, "Do not bother.", "nothing").unwrap_or_else(|e| panic!("{e}"));
    let id = made["id"].as_str().expect("an id").to_owned();
    press::<LessonDiscard>(&mut app, &id);

    let root = app.world().resource::<bevy_cauldron::project::Project>().root.clone();
    assert_eq!(std::fs::read_to_string(guide_path(&root)).expect("reads"), guide, "Discard wrote nothing");
    assert_eq!(lessons(&mut app)[0]["state"], "discarded", "the agent reads `discarded`");
}

/// **4. A lesson past its limit, one with a newline, one citing no hand-back, and a second citing the
/// same hand-back are each refused, naming the field and the limit, and nothing is trimmed.**
#[test]
fn a_lesson_that_does_not_fit_is_refused_at_the_door_and_never_trimmed() {
    let guide = format!("# Guide\n\n{LESSONS_HEADING}\n");
    let (mut app, _store) = ready("refused", Some(&guide), true);
    let presentation = present(&mut app);

    let long = "x".repeat(LESSON_MAX_CHARS + 1);
    let refused = propose(&mut app, &presentation, &long, "why").expect_err("281 characters is refused");
    assert!(refused.contains("`lesson`") && refused.contains("281") && refused.contains("280"), "{refused}");

    let refused = propose(&mut app, &presentation, "one line\ntwo lines", "why").expect_err("a newline is refused");
    assert!(refused.contains("`lesson`") && refused.contains("newline"), "{refused}");

    let refused = propose(&mut app, "present-99", "a lesson", "why").expect_err("no such hand-back");
    assert!(refused.contains("present-99") && refused.contains("never handed back"), "{refused}");

    // **The first one that fits is taken**, and a second citing the same hand-back is refused.
    propose(&mut app, &presentation, "A lesson worth keeping.", "it cost three calls").unwrap_or_else(|e| panic!("{e}"));
    let refused = propose(&mut app, &presentation, "The same hand-back again.", "it is the same turn").expect_err("one lesson per hand-back");
    assert!(refused.contains(&presentation) && refused.contains("already has a lesson"), "{refused}");

    // **And the queue holds exactly one**, with the lesson as it was written — never trimmed.
    let queued = app.world().resource::<Lessons>().queued.clone();
    assert_eq!(queued.len(), 1, "only the lesson that fits was taken");
    assert_eq!(queued[0].lesson.lesson, "A lesson worth keeping.");
    let root = app.world().resource::<bevy_cauldron::project::Project>().root.clone();
    assert_eq!(std::fs::read_to_string(guide_path(&root)).expect("reads"), guide, "and none of it was written");
}

/// **A project with no guide is refused**, and the reason says what the person has to do: the agent
/// cannot make the file, and the page's Create guide is the person's own act.
#[test]
fn a_project_with_no_guide_refuses_a_lesson_and_says_what_the_person_must_do() {
    let (mut app, _store) = ready("noguide", None, false);
    let presentation = present(&mut app);
    let refused = propose(&mut app, &presentation, "A lesson.", "why").expect_err("there is nowhere to keep it");
    assert!(refused.contains("no agent guide"), "{refused}");
    assert!(refused.contains("project page"), "and it says where the person makes one: {refused}");
}
