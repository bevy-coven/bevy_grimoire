//! **A new project starts with an agent guide, written by the wizard itself** (#349, AG-04).
//!
//! The New project wizard writes `<root>/.dark_factory/AGENTS.md` from a template with four headed
//! sections — **This world**, **Style**, **Rules** and **Lessons** — so a person finds the file and its
//! shape without reading docs.
//!
//! **The test drives the wizard, not a function beside it**: it opens the New project form, types a name,
//! presses the form's own **Create**, and then asks `grimoire/agent_guide` what an agent would be given.
//! That is what proves the acceptance item — "a headless test runs the wizard; the new project has the
//! file, and `grimoire/agent_guide` serves it with no trust question" — and deleting the Create observer's
//! grant fails it.
//!
//! **The store is left at its default.** A real editor's `trust.file` is `None`, which means the person's
//! own `<data_dir>/Grimoire/trusted.ron` (`crate::trusted::resolve`); the test inserts a scratch
//! `trusted::StoreRoot`, so the grant lands in a store of the test's making and not the developer's.
//!
//! **A project the wizard did not create gets no file.** Opening an existing game must write nothing into
//! it; the project page's Create guide is how such a project gets one, and it calls the same writer.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::guide_template::{self, LESSONS_HEADING, SECTIONS};
use bevy_cauldron::harness;
use bevy_cauldron::launcher::LauncherPlugin;
use bevy_cauldron::loading::LoadingPlugin;
use bevy_cauldron::new_project::{self, CreateButton, LocationField, NameField, NewProjectForm};
use bevy_cauldron::ops_brp::OpsBridgePlugin;
use bevy_cauldron::project::Project;
use bevy_cauldron::screen::Screen;
use bevy_grimoire_core::agent_guide::PROJECT_PATH;
use bevy_grimoire_core::trust::{self, TrustStore};
use bevy_gooey::testing;
use serde_json::json;

use crate::fixtures;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire_new_guide_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn settle(app: &mut App) {
    for _ in 0..6 {
        std::thread::sleep(std::time::Duration::from_millis(60));
        app.update();
    }
}

fn call(app: &mut App) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get("grimoire/agent_guide") {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => panic!("`grimoire/agent_guide` is not registered"),
    };
    let answer = app
        .world_mut()
        .run_system_with(id, Some(json!({})))
        .unwrap_or_else(|e| panic!("grimoire/agent_guide: {e:?}"))
        .unwrap_or_else(|e| panic!("grimoire/agent_guide: {e:?}"));
    settle(app);
    answer
}

fn project_text(answer: &serde_json::Value) -> Option<String> {
    answer["layers"]
        .as_array()?
        .iter()
        .find(|layer| layer["source"] == "project")
        .and_then(|layer| layer["text"].as_str())
        .map(str::to_owned)
}

/// **The person's own store**, where a `None` override resolves to: the `StoreRoot`'s `trusted.ron`.
fn user_store(holder: &Path) -> PathBuf {
    holder.join(trust::STORE_FILE)
}

/// **The wizard, driven through its own form**: the home, New project, a name and a folder, Create.
fn run_the_wizard(tag: &str, holder: &Path) -> PathBuf {
    let holder = holder.to_path_buf();
    // A fixture project to stand the editor up on; the wizard makes its own folder beside it.
    let start = fixtures::Fixture::new(&format!("wizard_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = harness::build_headless(&start, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(LoadingPlugin);
    app.add_plugins(LauncherPlugin);
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(OpsBridgePlugin);
    // **The store's folder is the test's**, as `settings::UserSettingsFile` is for settings: only the binary
    // names the real one, so no test reads or writes the developer's `trusted.ron`.
    app.insert_resource(bevy_cauldron::trusted::StoreRoot(holder.clone()));
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    settle(&mut app);

    // **The person's store is the default one** — `trust.file` stays `None` — and the store's folder is a
    // scratch folder of the test's (`trusted::StoreRoot`), so nothing here touches the real `trusted.ron`.
    assert!(
        app.world().resource::<EditorState>().trust.file.is_none(),
        "a real editor names no store: `None` is the person's own file"
    );
    let store = user_store(&holder);
    assert!(store.starts_with(&holder), "the store is the scratch one: {}", store.display());

    let button = testing::find_by_label(&mut app, "New project").unwrap_or_else(|e| panic!("{e:?}"));
    app.world_mut().commands().trigger(Activate { entity: button });
    settle(&mut app);
    assert!(app.world().get_resource::<NewProjectForm>().is_some(), "the form is open");

    let name = app.world_mut().query_filtered::<Entity, With<NameField>>().single(app.world()).expect("the Name field");
    let folder = holder.to_string_lossy().into_owned();
    app.world_mut().commands().trigger(bevy::ui_widgets::ValueChange::<String> { source: name, value: tag.to_owned(), is_final: true });
    let location = app.world_mut().query_filtered::<Entity, With<LocationField>>().single(app.world()).expect("the Location field");
    app.world_mut().commands().trigger(bevy::ui_widgets::ValueChange::<String> { source: location, value: folder.clone(), is_final: true });
    settle(&mut app);

    let create = app.world_mut().query_filtered::<Entity, With<CreateButton>>().iter(app.world()).next().expect("Create is on the form");
    app.world_mut().commands().trigger(Activate { entity: create });
    settle(&mut app);
    settle(&mut app);

    let root = Path::new(&folder).join(tag);
    assert!(root.join("assets/grimoire").is_dir(), "the wizard made the project at {}", root.display());
    root
}

/// **The wizard's Create writes the four-section guide and its grant, and an agent is asked nothing.**
#[test]
fn the_wizard_writes_the_four_section_guide_and_grants_it_so_an_agent_is_asked_nothing() {
    let holder = scratch(&format!("guide_wizard_{}", std::process::id()));
    let root = run_the_wizard(&format!("guide_wizard_{}", std::process::id()), &holder);
    let file = root.join(PROJECT_PATH);
    let text = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    for (heading, line) in SECTIONS {
        assert!(text.contains(heading), "the new project's guide has no `{heading}`:\n{text}");
        assert!(text.contains(line), "`{heading}`'s prompt line is missing:\n{text}");
    }
    assert!(text.contains(LESSONS_HEADING), "the Lessons heading the next slice appends under");

    // **The grant the Create observer wrote**, into the person's own store.
    let store = TrustStore::load(&user_store(&holder)).expect("the person's store reads");
    assert_eq!(
        store.guide_decision(&root, &text).expect("resolves"),
        trust::Decision::Trusted(trust::Scope::Command),
        "Create wrote a guide grant for the text it wrote"
    );

    // **And an agent is served it, with no question raised.**
    let mut app = harness::build_headless(&root, new_project::FIRST_MAP, None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(OpsBridgePlugin);
    app.insert_resource(bevy_cauldron::trusted::StoreRoot(holder.clone()));
    settle(&mut app);
    assert!(app.world().resource::<EditorState>().trust.file.is_none(), "the store is still the default one");
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
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..6 {
        app.update();
    }
    assert!(!root.join(PROJECT_PATH).exists(), "a project the editor did not create gets no guide written into it");
    assert_eq!(
        std::fs::read_dir(&root).map(|e| e.count()).unwrap_or_default(),
        before,
        "and nothing else was written"
    );
}

/// **One writer**, idempotent, and its grant is for the content on disk.
#[test]
fn the_writer_is_the_one_function_and_it_is_idempotent() {
    let dir = scratch(&format!("writer_{}", std::process::id()));
    let store = dir.join("trusted.yaml");
    TrustStore::default().save(&store).expect("an empty store");
    let root = dir.join("project");
    std::fs::create_dir_all(&root).expect("the project folder");
    let first = guide_template::write(&root, Some(&bevy_cauldron::trusted::StoreRoot(dir.clone())), None).expect("the guide is written");
    let written = std::fs::read_to_string(&first).expect("it reads");
    let second = guide_template::write(&root, Some(&bevy_cauldron::trusted::StoreRoot(dir.clone())), None).expect("writing it again is fine");
    assert_eq!(first, second, "one path");
    assert_eq!(written, guide_template::TEMPLATE, "and the template is what it writes");

    let trusted = TrustStore::load(&store).expect("the store reads");
    assert_eq!(
        trusted.guide_decision(&root, &written).expect("resolves"),
        trust::Decision::Trusted(trust::Scope::Command)
    );
}

/// **A store that will not read says so** rather than landing as a file nobody has allowed.
#[test]
fn a_store_that_will_not_read_is_an_error_and_not_a_silent_ungranted_guide() {
    let dir = scratch(&format!("badstore_{}", std::process::id()));
    let store = dir.join("trusted.yaml");
    std::fs::write(&store, "(version: 2, projects: [").expect("a damaged store");
    let store_root = bevy_cauldron::trusted::StoreRoot(dir.clone());
    let root = dir.join("project");
    std::fs::create_dir_all(&root).expect("the project folder");
    let said = guide_template::write(&root, Some(&store_root), None).expect_err("a damaged store is refused");
    assert!(said.contains("trusted.yaml"), "the refusal names the file: {said}");
}

#[test]
fn the_written_path_is_the_one_the_guide_reader_looks_at() {
    assert_eq!(guide_template::path(Path::new("/a/project")), Path::new("/a/project").join(PROJECT_PATH));
}
