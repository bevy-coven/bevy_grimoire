//! **A new project starts with an agent guide** (#349, AG-04).
//!
//! The New project wizard writes `<root>/.dark_factory/AGENTS.md` from a template with four headed
//! sections — **This world**, **Style**, **Rules** and **Lessons** — so a person finds the file and its
//! shape without reading docs. **The person who created the project authored that template**, so the
//! wizard writes a guide grant for the exact content in the same step: a new project's guide is never
//! withheld from the person who made it, and the test proves it by asking `grimoire/agent_guide` with no
//! question raised.
//!
//! **A project the wizard did not create gets no file.** Opening an existing game must write nothing into
//! it; the project page's Create guide (AG-05) is how such a project gets one, and it calls the same
//! `guide_template::write`.
//!
//! **One writer.** The wizard, the project page and any later caller go through `guide_template::write`,
//! so the template, its grant and the file's path cannot drift apart.

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::guide_template::{self, LESSONS_HEADING, SECTIONS};
use bevy_cauldron::new_project::{self, NewProject, Template};
use bevy_cauldron::ops_brp::OpsBridgePlugin;
use bevy_cauldron::project::Project;
use bevy_grimoire_core::agent_guide::PROJECT_PATH;
use bevy_grimoire_core::trust::TrustStore;
use serde_json::json;
use std::path::{Path, PathBuf};

use crate::fixtures;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire_new_guide_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn call(app: &mut App) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get("grimoire/agent_guide") {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => panic!("`grimoire/agent_guide` is not registered"),
    };
    app.world_mut()
        .run_system_with(id, Some(json!({})))
        .unwrap_or_else(|e| panic!("grimoire/agent_guide: {e:?}"))
        .unwrap_or_else(|e| panic!("grimoire/agent_guide: {e:?}"))
}

fn project_text(answer: &serde_json::Value) -> Option<String> {
    answer["layers"]
        .as_array()?
        .iter()
        .find(|layer| layer["source"] == "project")
        .and_then(|layer| layer["text"].as_str())
        .map(str::to_owned)
}

/// **The wizard's own path**: the form, then `create`, with a trust store of the test's own beside it.
fn make(tag: &str) -> (PathBuf, PathBuf) {
    let dir = scratch(tag);
    let store = dir.join("trusted.ron");
    TrustStore::default().save(&store).expect("an empty store");
    let form = NewProject { name: tag.to_owned(), location: dir.to_string_lossy().into_owned(), template: Template::Empty };
    // **The whole act the form's Create performs**: the project, then its guide's grant to the person's
    // store. `create` writes the file (it has no store), and `grant` is the second half of the same call.
    let root = new_project::create(&form).unwrap_or_else(|e| panic!("{e}"));
    guide_template::grant(&root, &store).unwrap_or_else(|e| panic!("{e}"));
    (root, store)
}

#[test]
fn the_wizard_writes_the_four_section_guide_and_grants_it_so_an_agent_is_asked_nothing() {
    let (root, store) = make(&format!("wizard_{}", std::process::id()));
    let file = root.join(PROJECT_PATH);
    let text = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    for (heading, line) in SECTIONS {
        assert!(text.contains(heading), "the new project's guide has no `{heading}`:\n{text}");
        assert!(text.contains(line), "`{heading}`'s prompt line is missing:\n{text}");
    }
    assert!(text.contains(LESSONS_HEADING), "the Lessons heading the next slice appends under");

    // **Served with no question**, because the person who made the project wrote that text.
    let mut app = bevy_cauldron::harness::build_headless(&root, new_project::FIRST_MAP, None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(bevy_cauldron::agent_guide::UserGuideFile(root.join("no-such-user-guide.md")));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(OpsBridgePlugin);
    app.world_mut().resource_mut::<EditorState>().trust.file = Some(store);
    for _ in 0..6 {
        app.update();
    }
    let answer = call(&mut app);
    assert_eq!(project_text(&answer).as_deref(), Some(text.as_str()), "the new project's guide is served: {answer}");
    assert_eq!(answer["withheld"], json!(null), "and nothing is withheld: {answer}");
    assert!(
        !app.world().resource::<bevy_cauldron::confirm::Confirm>().is_open(),
        "a new project's guide is never withheld from its author, so nothing is asked"
    );
    assert!(app.world().resource::<Project>().root.exists());
}

#[test]
fn opening_an_existing_project_writes_no_guide() {
    let root = fixtures::Fixture::new(&format!("existing_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let before = std::fs::read_dir(&root).map(|e| e.count()).unwrap_or_default();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }
    assert!(
        !root.join(PROJECT_PATH).exists(),
        "a project the editor did not create gets no guide written into it"
    );
    assert_eq!(std::fs::read_dir(&root).map(|e| e.count()).unwrap_or_default(), before, "and nothing else was written");
}

/// **One writer**: the wizard's call and the project page's Create guide are the same function, so the
/// template a person reads in a new project is the one an existing project gets.
#[test]
fn the_writer_is_the_one_function_and_it_is_idempotent() {
    let dir = scratch(&format!("writer_{}", std::process::id()));
    let store = dir.join("trusted.ron");
    TrustStore::default().save(&store).expect("an empty store");
    let root = dir.join("project");
    std::fs::create_dir_all(&root).expect("the project folder");
    let first = guide_template::write(&root, &store).expect("the guide is written");
    let written = std::fs::read_to_string(&first).expect("it reads");
    let second = guide_template::write(&root, &store).expect("writing it again is fine");
    assert_eq!(first, second, "one path");
    assert_eq!(written, guide_template::TEMPLATE, "and the template is what it writes");

    // **The grant is for the content on disk**, so an agent is served it without a question.
    let trusted = TrustStore::load(&store).expect("the store reads");
    assert_eq!(
        bevy_grimoire_core::trust::guide_decision(&trusted, &root, &written).expect("resolves"),
        bevy_grimoire_core::trust::Decision::Trusted(bevy_grimoire_core::trust::Scope::Command)
    );
}

/// **A guide that cannot be granted says so** rather than landing as a file nobody has allowed: a store
/// that will not read is an `Err` naming it.
#[test]
fn a_store_that_will_not_read_is_an_error_and_not_a_silent_ungranted_guide() {
    let dir = scratch(&format!("badstore_{}", std::process::id()));
    let store = dir.join("trusted.ron");
    std::fs::write(&store, "(version: 2, projects: [").expect("a damaged store");
    let root = dir.join("project");
    std::fs::create_dir_all(&root).expect("the project folder");
    let said = guide_template::write(&root, &store).expect_err("a damaged store is refused");
    assert!(said.contains("trusted.ron"), "the refusal names the file: {said}");
}

/// **The path is the one AG-02 reads**, so a guide written here is a guide an agent is given.
#[test]
fn the_written_path_is_the_one_the_guide_reader_looks_at() {
    assert_eq!(guide_template::path(Path::new("/a/project")), Path::new("/a/project").join(PROJECT_PATH));
}
