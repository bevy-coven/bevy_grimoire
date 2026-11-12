//! **The Agent guide is edited on the project page the way a person edits it**: read the label over the
//! field, click into the field, type, and press Save (AG-05, #350).
//!
//! `the_agent_guide_on_the_project_page.rs` proves the section's *rules* — which state offers which
//! button, what a conflict does, that a guide shown in part is read-only — by sending the change event
//! the widget would send. That is the right shape for a rule, and it is not the same claim as **a person
//! can type here**. So this file takes the acceptance's first line literally and goes through the
//! widget: a press at the field's own place and the platform's key presses, then Save's own activation.
//!
//! Four things this file is written around, because each cost a pass before:
//!
//! - **A pointer and a keyboard want different windows.** The editor draws into an image, and once a
//!   primary window exists gooey's pointer resolves widgets against a window nothing is drawn into,
//!   so a click lands nowhere; yet Bevy dispatches a focused field's keys only while a primary window
//!   exists. So the one press, the one that focuses the field, is sent before any window is spawned,
//!   the window is spawned for the keys, and Save is activated by its own event (the press the pointer
//!   would have resolved to), which is what every other test of this page does.
//! - **The field is below the fold.** The page does not scroll and the terminal pane is drawn under
//!   it, so at the harness's default surface the area's centre lies beneath the terminal and a press
//!   there would land on the terminal. The surface is made tall enough to hold the page, which is
//!   what a person with a tall window has.
//! - **A gooey text row's field is the label's sibling, not the label.** `find_by_label` walks *up* from
//!   a `Text` to the nearest focusable ancestor, which for a row's label is the page, so the field is
//!   found by reading the label and then taking the field under it — [`the_field_under`], which is the
//!   one place this file spells that out.
//! - **A press puts the caret where it lands**, in the middle of the text, so the person moves to the
//!   end (End) before typing and what is typed is appended. The test does the same rather than typing
//!   a replacement.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_cauldron::agent_guide::UserGuideFile;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::launcher::LauncherPlugin;
use bevy_cauldron::loading::LoadingPlugin;
use bevy_cauldron::project_page::{AgentGuideStatus, AgentGuideText};
use bevy_grimoire_core::agent_guide::PROJECT_PATH;
use bevy_grimoire_core::trust::{self, TrustStore};
use bevy_gooey::testing;
use bevy_gooey::Activate;
use bevy_gooey::{Role, TextInputOf};
use std::path::{Path, PathBuf};

use crate::fixtures::Fixture;

/// The words gooey draws over the section's area, which is how a person finds it.
const GUIDE_LABEL: &str = "The guide";

fn on_the_page(tag: &str) -> (PathBuf, PathBuf, App) {
    let root = Fixture::new(&format!("guide_by_label_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let store = root.join("trusted.ron");
    TrustStore::default().save(&store).expect("an empty store");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(LoadingPlugin);
    app.add_plugins(LauncherPlugin);
    app.insert_resource(UserGuideFile(root.join("no-user-guide.md")));
    harness::stand_on_the_project_page(&mut app, &root);
    app.world_mut().resource_mut::<EditorState>().trust.file = Some(store.clone());
    settle(&mut app);
    (root, store, app)
}

fn settle(app: &mut App) {
    for _ in 0..6 {
        app.update();
    }
}

fn guide_path(root: &Path) -> PathBuf {
    root.join(PROJECT_PATH)
}

fn write_guide(root: &Path, text: &str) {
    let path = guide_path(root);
    std::fs::create_dir_all(path.parent().expect("it has a folder")).expect("the folder is made");
    std::fs::write(&path, text).expect("the guide is written");
}

fn status(app: &mut App) -> String {
    app.world_mut()
        .query_filtered::<&Text, With<AgentGuideStatus>>()
        .iter(app.world())
        .map(|t| t.0.clone())
        .next()
        .unwrap_or_else(|| panic!("the page draws no agent-guide status line"))
}

/// **The field under the label that reads `label`**: what a person points at when they read the words
/// over a text row and click the box beneath them.
///
/// gooey draws a text row as a `Text` label and, beside it, the field that takes the keystrokes. The
/// label is not the field and `find_by_label` walks the wrong way for one, so this finds the label's
/// row and then the field whose [`TextInputOf`] names that row.
fn the_field_under(app: &mut App, label: &str) -> Entity {
    let world = app.world_mut();
    let row = world
        .query::<(Entity, &Text)>()
        .iter(world)
        .find(|(_, text)| text.0.trim() == label)
        .map(|(entity, _)| world.get::<ChildOf>(entity).map(ChildOf::parent))
        .flatten()
        .unwrap_or_else(|| panic!("no row on screen reads `{label}`"));
    let world = app.world_mut();
    world
        .query::<(Entity, &TextInputOf)>()
        .iter(world)
        .find(|(_, of)| of.0 == row)
        .map(|(entity, _)| entity)
        .unwrap_or_else(|| panic!("the row labelled `{label}` has no field under it"))
}

/// **Type into the guide's field, press Save, and read the file.**
///
/// The acceptance's first line, through the widget: a press where the field is drawn, the platform's
/// own key presses, and a press on the button labelled Save. Nothing here sends a `ValueChange`; the
/// change the editor acts on is the one the field itself produced.
#[test]
fn the_guide_is_typed_into_by_its_label_and_saved() {
    let (root, store, mut app): (PathBuf, PathBuf, App) = on_the_page("typed");
    write_guide(&root, "the guide as it read");
    app.world_mut().resource_mut::<bevy_cauldron::project_page::ProjectPage>().dirty = true;
    settle(&mut app);
    assert!(status(&mut app).starts_with("waiting for you"), "{}", status(&mut app));

    // **A press where the field is drawn**, and the focus is read off the field that is drawn after
    // it: the page redraws its body when it has read the guide, which is a new field entity, so the
    // claim is that the keyboard is in the guide's own field, not that a redraw did not happen.
    harness::resize_surface(&mut app, 1536, 2400).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    let field = the_field_under(&mut app, GUIDE_LABEL);
    testing::click(&mut app, field);
    let field = the_field_under(&mut app, GUIDE_LABEL);
    assert_eq!(
        testing::focused(&app),
        Some(field),
        "a press on the field puts the keyboard in it, on a label of {:?}",
        testing::focused_label(&app)
    );
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    testing::press(&mut app, KeyCode::End);
    testing::type_text(&mut app, "\nand a line typed on the page");

    let save = testing::find_by_label(&mut app, "Save")
        .unwrap_or_else(|e| panic!("the section offers no Save once the text changed: {e}"));
    app.world_mut().commands().trigger(Activate { entity: save });
    settle(&mut app);

    let saved = std::fs::read_to_string(guide_path(&root)).expect("the file reads");
    assert_eq!(
        saved,
        "the guide as it read\nand a line typed on the page",
        "what was typed on the page is what the file holds"
    );
    let trusted = TrustStore::load(&store).expect("the store reads");
    assert_eq!(
        trusted.guide_decision(&root, &saved).expect("resolves"),
        trust::Decision::Trusted(trust::Scope::Command),
        "the person wrote it, so they have read all of it: the save carries its grant"
    );
    assert!(status(&mut app).starts_with("allowed"), "{}", status(&mut app));
}

/// **The field under the label is the guide's own**, not another text field on the page.
///
/// The row carries [`AgentGuideText`] and holds the file's text, which is what makes typing there
/// typing into the guide — and it is drawn as a `Role::TextArea`, the multi-line field, not a
/// single-line input.
#[test]
fn the_field_under_the_label_is_the_guides_own_text_area() {
    let (root, _store, mut app) = on_the_page("holds");
    write_guide(&root, "every word of it");
    app.world_mut().resource_mut::<bevy_cauldron::project_page::ProjectPage>().dirty = true;
    settle(&mut app);
    let field = the_field_under(&mut app, GUIDE_LABEL);
    let world = app.world();
    let row = world.get::<TextInputOf>(field).expect("a field has a row").0;
    assert!(world.entity(row).contains::<AgentGuideText>(), "the labelled field is not the guide's own");
    // **The ROLE is the row's, not the field's**: gooey's `text_area` puts `Role::TextArea` on the
    // row it spawns and every field under a row is a `Role::TextField`, so asking the field would be
    // asking the wrong entity and answering `TextField` for a text area and a one-line input alike.
    assert_eq!(
        world.get::<Role>(row),
        Some(&Role::TextArea),
        "the guide is edited in a multi-line area, not a one-line input"
    );
    assert_eq!(
        world.get::<Role>(field),
        Some(&Role::TextField),
        "the row's field is a text field, whatever kind of row it is in"
    );
    assert_eq!(
        world.get::<bevy_gooey::TextInputValue>(row).map(|v| v.0.as_str()),
        Some("every word of it"),
        "and it holds the file's text"
    );
    // **And pressing Save with nothing typed writes nothing**, so the button is the section's Save and
    // not some other one: a Save that always wrote would pass the first test for the wrong reason.
    let before = std::fs::metadata(guide_path(&root)).expect("the file is there").modified().expect("a time");
    let save = testing::find_by_label(&mut app, "Save").expect("the section's Save");
    testing::click(&mut app, save);
    settle(&mut app);
    let after = std::fs::metadata(guide_path(&root)).expect("the file is there").modified().expect("a time");
    assert_eq!(before, after, "nothing was typed, so nothing was written");
}
