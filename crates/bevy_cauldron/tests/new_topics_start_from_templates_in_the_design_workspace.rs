//! **Create style and New topic, in the Design workspace** (SG-08, #407; the series #399).
//!
//! The wizard half landed with `a_new_project_starts_with_a_style.rs`. This is the other half: in the Design workspace **Create style** writes a
//! place's `style.md` from the template, and **New topic** writes `design/<topic>.md` from that topic's template on the game, and offers any
//! lower place only the topics the game already has. Opening the workspace on a project writes no file, and a template the editor writes is
//! allowed (the text is the editor's own, as the wizard's is), so `grimoire_style` serves it with no question.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::PrimaryWindow;
use bevy_cauldron::design_templates::{self, STYLE_H2S};
use bevy_cauldron::design_view::DesignView;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::keys::{Action, binding};
use bevy_cauldron::workspace::Workspace;
use bevy_grimoire_core::design;
use bevy_grimoire_core::trust::{Decision, TrustStore};

use crate::fixtures;

const SETTLE: usize = 14;
const FRAME_S: f32 = 1.0 / 60.0;

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn press(app: &mut App, action: Action) {
    let row = binding(action);
    let mut keys = vec![row.key];
    if row.needs_mod {
        keys.push(if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft });
    }
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for code in &keys {
            app.world_mut().write_message(KeyboardInput {
                key_code: *code,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        }
        app.update();
        app.update();
    }
    settle(app);
}

fn write(root: &std::path::Path, rel: &str, text: &str) {
    let full = root.join(rel);
    std::fs::create_dir_all(full.parent().expect("a folder")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&full, text).unwrap_or_else(|e| panic!("{e}"));
}

/// An editor in the Design workspace on a fresh fixture project, and the person's trust store of the test's own.
fn designer(tag: &str, files: &[(&str, &str)]) -> (App, std::path::PathBuf, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    for (rel, text) in files {
        write(&root, rel, text);
    }
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(FRAME_S)));
    let store = root.join("trusted.yaml");
    TrustStore::default().save(&store).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<EditorState>().trust.file = Some(store.clone());
    settle(&mut app);
    press(&mut app, Action::WorkspaceDesign);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Design);
    // The editor's state is rebuilt by a workspace switch, so the test's store is named again once Design is open.
    app.world_mut().resource_mut::<EditorState>().trust.file = Some(store.clone());
    (app, root, store)
}

/// The topics the New topic row offers, read off its buttons (a topic's name is also a row of the tree, so a label would be ambiguous).
fn offered_topics(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    world
        .query::<&bevy_cauldron::design_view::DesignAction>()
        .iter(world)
        .filter_map(|a| match a {
            bevy_cauldron::design_view::DesignAction::NewTopic(t) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

fn click(app: &mut App, label: &str) {
    if label == "m" {
        bevy_cauldron::design_view::show_file(app.world_mut(), "design/levels/m/style.md", None);
        settle(app);
        return;
    }
    let entity = bevy_gooey::testing::find_by_label(app, label).unwrap_or_else(|e| panic!("`{label}`: {e}"));
    app.world_mut().commands().trigger(bevy::ui_widgets::Activate { entity });
    settle(app);
}

fn visible(app: &mut App, label: &str) -> bool {
    bevy_gooey::testing::find_by_label(app, label).is_ok()
}

fn allowed(root: &std::path::Path, store: &std::path::Path, rel: &str) -> bool {
    let text = std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
    TrustStore::load(store).and_then(|s| s.text_decision(root, rel, &text)).is_ok_and(|d| d != Decision::Untrusted)
}

#[test]
fn opening_the_workspace_on_a_project_with_no_style_writes_no_file_and_create_style_writes_the_template() {
    let (mut app, root, store) = designer("create_style", &[]);
    assert!(!root.join("design").exists(), "entering the Design workspace writes nothing");
    assert!(visible(&mut app, "Create style"), "a game with no style offers to start one");

    click(&mut app, "Create style");
    let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let written = std::fs::read_to_string(root.join("design/style.md")).unwrap_or_else(|e| panic!("Create style wrote nothing: {e}"));
    assert_eq!(written, design_templates::style_text(&name), "the file is the template, with the project's name as its one H1");
    let h2s: Vec<String> = design::headings(&written).into_iter().filter(|h| h.level == 2).map(|h| h.title).collect();
    assert_eq!(h2s, STYLE_H2S, "the nine H2s, in order");
    assert!(allowed(&root, &store, "design/style.md"), "the template the editor wrote is allowed, so grimoire_style serves it with no question");
    assert_eq!(app.world().resource::<DesignView>().selected(), Some("design/style.md"));
    assert!(!visible(&mut app, "Create style"), "the page exists now");
    assert_eq!(std::fs::read_dir(root.join("design")).map(|d| d.count()).unwrap_or(0), 1, "only the one file was written");
}

#[test]
fn new_topic_dialogue_on_the_game_writes_design_dialogue_md_from_its_template() {
    let (mut app, root, store) = designer("new_topic_game", &[("design/style.md", "# The game\n\nGrey.\n")]);
    click(&mut app, "New topic");
    let mut offered = offered_topics(&mut app);
    offered.sort();
    assert_eq!(offered, ["dialogue", "interaction", "names", "sound", "ui"], "the game offers its five topics");
    click(&mut app, "dialogue");
    let written = std::fs::read_to_string(root.join("design/dialogue.md")).unwrap_or_else(|e| panic!("New topic wrote nothing: {e}"));
    assert_eq!(written, design_templates::topic_text("dialogue", "The game").expect("a template"), "the file is the topic's template");
    assert!(allowed(&root, &store, "design/dialogue.md"), "and it is allowed");
    assert_eq!(app.world().resource::<DesignView>().selected(), Some("design/dialogue.md"), "the new page is the one shown");
    // The game has `dialogue` now, so it is no longer offered; the other four still are.
    let offered = design_templates::topics_offered(&design::Tree::read(&root), "design");
    assert_eq!(offered, ["interaction", "sound", "ui", "names"], "the topic made is not offered again");
    click(&mut app, "New topic");
    assert!(visible(&mut app, "interaction"), "the rest are still offered");
}

#[test]
fn new_topic_on_a_level_lists_only_the_games_topics() {
    let (mut app, _root, _store) = designer(
        "new_topic_level",
        &[
            ("design/style.md", "# The game\n"),
            ("design/dialogue.md", "# Dialogue\n"),
            ("design/sound.md", "# Sound\n"),
            ("design/levels/m/style.md", "# Yard\n"),
        ],
    );
    click(&mut app, "m");
    click(&mut app, "New topic");
    let mut offered = offered_topics(&mut app);
    offered.sort();
    assert_eq!(offered, ["dialogue", "sound"], "a level is offered only the game's topics, and not the three the game has no file for");
}
