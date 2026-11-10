//! **The editor starts a program a project names only after a person has allowed it** (the review of
//! 2026-11-02, risk 1). `play.yaml` and `agent.yaml` come with a cloned project; before this, Play and Send
//! ran whatever they named. The model is `bevy_grimoire_core::trust`; these tests are the editor's side:
//! the refusals, the question and its three answers, and that an agent cannot answer it.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_cauldron::confirm::{Asked, Confirm};
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::ops::{self, Applied, Ctx, Source};
use bevy_cauldron::trusted::{self, TrustRequest};
use bevy_grimoire_core::play::PlayConfig;
use bevy_grimoire_core::trust::{Decision, Scope, TrustStore};
use serde_json::json;
use std::path::{Path, PathBuf};

use crate::fixtures;

/// A project whose `play.yaml` runs `touch <root>/ran.marker`: harmless, and it leaves proof it ran.
fn project(tag: &str) -> (PathBuf, PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("town");
    let marker = root.join("ran.marker");
    write_play(&root, &marker);
    (root, marker)
}

fn write_play(root: &Path, marker: &Path) {
    std::fs::write(
        root.join("play.yaml"),
        format!("version: 1\ncommand: [\"/usr/bin/touch\", \"{}\"]\n", marker.display()),
    )
    .unwrap();
}

/// A store of the test's own that trusts nothing.
fn empty_store(root: &Path) -> PathBuf {
    let file = root.join("nothing-trusted.ron");
    TrustStore::default().save(&file).unwrap();
    file
}

fn ctx(root: &Path, trust_file: PathBuf, source: Source) -> Ctx {
    Ctx { root: Some(root.to_path_buf()), session: "test".to_owned(), source, trust_file: Some(trust_file), ..Ctx::default() }
}

/// `play`, as `Ok` when it ran and `Err` with the refusal's sentence when it did not: a refusal comes back
/// from `dispatch` as an `Err`, from the plan, before anything is committed.
fn play(c: &mut Ctx) -> Result<(), String> {
    match ops::dispatch(c, "play", &json!({"map": "town"})) {
        Ok(Applied::Done { .. }) => Ok(()),
        Ok(Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

fn assert_not_started(marker: &Path) {
    std::thread::sleep(std::time::Duration::from_millis(400));
    assert!(!marker.exists(), "the program must not have started");
}

fn wait_for(marker: &Path) {
    for _ in 0..50 {
        if marker.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("{} never appeared", marker.display());
}

#[cfg(unix)]
#[test]
fn play_from_an_untrusted_project_starts_nothing_for_a_person_or_an_agent() {
    let (root, marker) = project(&format!("trust_play_none_{}", std::process::id()));
    for source in [Source::Human, Source::Agent] {
        let mut c = ctx(&root, empty_store(&root), source);
        let why = play(&mut c).expect_err(&format!("{source:?}: play ran an untrusted game"));
        assert!(why.contains("no person has allowed"), "{source:?}: the refusal says why: {why}");
        assert!(why.contains("touch"), "{source:?}: and shows the command: {why}");
    }
    assert_not_started(&marker);
}

#[cfg(unix)]
#[test]
fn an_edit_to_play_ron_after_the_person_allowed_the_command_asks_again() {
    let (root, marker) = project(&format!("trust_play_edit_{}", std::process::id()));
    let store = empty_store(&root);
    let config = PlayConfig::load(&root).unwrap();
    trusted::grant(Some(&store), &TrustRequest { root: root.clone(), spec: config.spec() }, Scope::Command).unwrap();

    // The command the person read is the one that runs.
    let mut c = ctx(&root, store.clone(), Source::Human);
    play(&mut c).expect("an allowed command runs");
    wait_for(&marker);
    let _ = ops::dispatch(&mut c, "play_stop", &json!({}));

    // A planted edit is a different command.
    let planted = root.join("planted.marker");
    write_play(&root, &planted);
    let mut c = ctx(&root, store, Source::Agent);
    play(&mut c).expect_err("an edited play.yaml ran on the old trust");
    assert_not_started(&planted);
}

#[cfg(unix)]
#[test]
fn trusting_the_project_covers_an_edit_because_that_is_what_it_means() {
    let (root, _marker) = project(&format!("trust_play_project_{}", std::process::id()));
    let store = empty_store(&root);
    trusted::grant(Some(&store), &TrustRequest { root: root.clone(), spec: PlayConfig::load(&root).unwrap().spec() }, Scope::Project).unwrap();

    let other = root.join("other.marker");
    write_play(&root, &other);
    let mut c = ctx(&root, store, Source::Human);
    play(&mut c).expect("a trusted project runs an edited command");
    wait_for(&other);
    let _ = ops::dispatch(&mut c, "play_stop", &json!({}));
}

#[test]
fn no_operation_an_agent_can_call_grants_trust() {
    for op in ops::registry() {
        let name = op.name();
        assert!(!name.contains("trust") && !name.contains("allow"), "`{name}` would let an agent trust itself; a grant is a person's answer to the question");
    }
}

// ---- the question and its three answers ----

fn editor(tag: &str) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    (app, root)
}

fn tap(app: &mut App, logical: Key, code: KeyCode) {
    for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: logical.clone(),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

/// Raises the question the way Send or Play leaves it, and returns the store it will write to.
fn raise(app: &mut App, root: &Path) -> (PathBuf, TrustRequest) {
    let store = empty_store(root);
    let request = TrustRequest { root: root.to_path_buf(), spec: PlayConfig::parse("version: 1\ncommand: [\"game\", \"{map}\"]\n").unwrap().spec() };
    {
        let mut state = app.world_mut().resource_mut::<EditorState>();
        state.trust.file = Some(store.clone());
        state.trust.pending = Some(request.clone());
    }
    app.update();
    app.update();
    assert!(app.world().resource::<Confirm>().asking(Asked::TrustCommand), "the question is on screen");
    (store, request)
}

fn decision(store: &Path, request: &TrustRequest) -> Decision {
    TrustStore::load(store).unwrap().decision(&request.root, &request.spec).unwrap()
}

#[test]
fn y_allows_this_command_only() {
    let (mut app, root) = editor(&format!("trust_q_y_{}", std::process::id()));
    let (store, request) = raise(&mut app, &root);
    tap(&mut app, Key::Character("y".into()), KeyCode::KeyY);
    assert_eq!(decision(&store, &request), Decision::Trusted(Scope::Command));
    let mut edited = request.clone();
    edited.spec.args.push("--other".into());
    assert_eq!(decision(&store, &edited), Decision::Untrusted, "the grant is for the command that was shown");
    assert!(!app.world().resource::<Confirm>().is_open());
    let note = app.world().resource::<EditorState>().status.note_text().to_owned();
    assert!(note.contains("Allowed that command"), "the status line says what happened; it says {note:?}");
}

#[test]
fn p_trusts_the_whole_project() {
    let (mut app, root) = editor(&format!("trust_q_p_{}", std::process::id()));
    let (store, request) = raise(&mut app, &root);
    tap(&mut app, Key::Character("p".into()), KeyCode::KeyP);
    assert_eq!(decision(&store, &request), Decision::Trusted(Scope::Project));
    let mut edited = request.clone();
    edited.spec.program = "anything-else".into();
    assert_eq!(decision(&store, &edited), Decision::Trusted(Scope::Project));
}

#[test]
fn n_and_escape_allow_nothing() {
    for (name, logical, code) in [("n", Key::Character("n".into()), KeyCode::KeyN), ("esc", Key::Escape, KeyCode::Escape)] {
        let (mut app, root) = editor(&format!("trust_q_{name}_{}", std::process::id()));
        let (store, request) = raise(&mut app, &root);
        tap(&mut app, logical, code);
        assert_eq!(decision(&store, &request), Decision::Untrusted, "{name}");
        assert!(!app.world().resource::<Confirm>().is_open());
        let note = app.world().resource::<EditorState>().status.note_text().to_owned();
        assert!(note.contains("Not allowed"), "{name}: the status line says what happened; it says {note:?}");
    }
}

#[test]
fn the_question_names_the_command_and_the_project() {
    let (mut app, root) = editor(&format!("trust_q_text_{}", std::process::id()));
    raise(&mut app, &root);
    let state = app.world().resource::<EditorState>();
    let awaiting = state.trust.awaiting.as_ref().expect("a request awaits the answer");
    assert_eq!(awaiting.root, root);
    assert_eq!(awaiting.spec.display(), "game {map}");
}

#[test]
fn a_damaged_store_is_a_problem_on_the_status_line_and_grants_nothing() {
    let (mut app, root) = editor(&format!("trust_q_bad_{}", std::process::id()));
    let (store, _request) = raise(&mut app, &root);
    std::fs::write(&store, "(version: 1, projects: [").unwrap();
    tap(&mut app, Key::Character("y".into()), KeyCode::KeyY);
    assert!(app.world().resource::<EditorState>().status.problem_text().contains("trusted") || app.world().resource::<EditorState>().status.problem_text().contains("nothing-trusted"));
    assert_eq!(std::fs::read_to_string(&store).unwrap(), "(version: 1, projects: [", "a damaged store is left as it is");
}
