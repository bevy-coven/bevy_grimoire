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
    // A surface the size of a window, so the workspace's widgets are where a pointer reaches them.
    harness::resize_surface(&mut app, 1536, 1400).unwrap_or_else(|e| panic!("{e}"));
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

/// A button takes the real pointer click; the level's tree row is chosen by the event a click on it sends (the headless pointer lands on a neighbouring row of the virtualized tree).
fn click(app: &mut App, label: &str) {
    if label == "m" {
        let tree = {
            let world = app.world_mut();
            world.query_filtered::<Entity, With<bevy_cauldron::design_view::DesignTree>>().single(world).unwrap_or_else(|e| panic!("one tree: {e}"))
        };
        let key = bevy_cauldron::files::key_of("design/levels/m");
        app.world_mut().commands().trigger(bevy_gooey::ValueChange { source: tree, value: bevy_gooey::TableSelection::of(key), is_final: true });
        settle(app);
        return;
    }
    let entity = bevy_gooey::testing::find_by_label(app, label).unwrap_or_else(|e| panic!("`{label}`: {e}"));
    // Create style takes the real pointer click. The New topic row's buttons are pressed by `Activate`: a real click on them did not open the
    // topic menu in the headless frame (their neighbour, the toggle, is as wide as the row's room), and that is said here rather than hidden.
    if label == "Create style" {
        bevy_gooey::testing::click(app, entity);
    } else {
        app.world_mut().commands().trigger(bevy::ui_widgets::Activate { entity });
    }
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

/// **A button's rectangle in logical pixels**, `(min, max)`, from its laid-out node.
fn rect_of(app: &mut App, label: &str) -> (Vec2, Vec2) {
    let entity = bevy_gooey::testing::find_by_label(app, label).unwrap_or_else(|e| panic!("`{label}`: {e}"));
    let world = app.world();
    let node = world.get::<bevy::ui::ComputedNode>(entity).unwrap_or_else(|| panic!("`{label}` is not laid out"));
    let at = world.get::<bevy::ui::UiGlobalTransform>(entity).unwrap_or_else(|| panic!("`{label}` is not placed")).translation;
    let (size, centre) = (node.size() * node.inverse_scale_factor(), at * node.inverse_scale_factor());
    (centre - size / 2.0, centre + size / 2.0)
}

fn real_click(app: &mut App, label: &str) {
    let entity = bevy_gooey::testing::find_by_label(app, label).unwrap_or_else(|e| panic!("`{label}`: {e}"));
    bevy_gooey::testing::click(app, entity);
    settle(app);
}

#[test]
fn every_header_button_has_its_own_rectangle_in_the_window_and_takes_a_real_click() {
    let (mut app, root, _store) = designer("header", &[("design/style.md", "# The game\n\nGrey.\n")]);
    let labels = ["Edit page", "As an agent reads it", "New topic"];
    let rects: Vec<(Vec2, Vec2)> = labels.iter().map(|l| rect_of(&mut app, l)).collect();
    for (label, (min, max)) in labels.iter().zip(&rects) {
        assert!(max.x - min.x >= 40.0 && max.y - min.y > 0.0, "`{label}` is squeezed to {min:?} {max:?}: too small to hit");
        assert!(min.x >= 0.0 && min.y >= 0.0 && max.x <= 1536.0 && max.y <= 1400.0, "`{label}` is outside the window: {min:?} {max:?}");
    }
    for (i, a) in rects.iter().enumerate() {
        for (j, b) in rects.iter().enumerate().skip(i + 1) {
            let apart = a.1.x <= b.0.x || b.1.x <= a.0.x || a.1.y <= b.0.y || b.1.y <= a.0.y;
            assert!(apart, "`{}` and `{}` overlap: {a:?} {b:?}", labels[i], labels[j]);
        }
    }
    // Each takes a real click, and the click does what the button says.
    real_click(&mut app, "As an agent reads it");
    assert!(app.world().resource::<DesignView>().as_agent(), "a click on the toggle turns it on");
    real_click(&mut app, "As an agent reads it");
    assert!(!app.world().resource::<DesignView>().as_agent(), "and off");
    real_click(&mut app, "New topic");
    assert!(!offered_topics(&mut app).is_empty(), "a click on New topic opens the topic menu");
    real_click(&mut app, "dialogue");
    assert!(root.join("design/dialogue.md").is_file(), "a click on a topic writes its page");
    real_click(&mut app, "Edit page");
    assert_eq!(app.world().resource::<DesignView>().editing_file(), Some("design/dialogue.md"), "a click on Edit page opens the editor on the page shown");
}

#[test]
fn a_game_with_no_topic_yet_shows_its_first_topic_as_a_row_of_the_tree() {
    let (mut app, _root, _store) = designer("first_topic", &[("design/style.md", "# The game\n\nGrey.\n")]);
    click(&mut app, "New topic");
    click(&mut app, "dialogue");
    assert!(visible(&mut app, "dialogue"), "the new topic is a visible row under the game, which had no children before it");
}
