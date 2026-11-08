//! **A button says what it does** (UI-R03, #333): the screen's main action is gooey's `ButtonVariant::Submit` (the editor's own
//! `AccentButton` and its repaint are gone), the toolbar's icon buttons are `Ghost`, a destructive confirmation's agreeing
//! answer is `Danger` and an ordinary one is not, and a dialog with a text field names the button Return presses.

use std::path::PathBuf;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_cauldron::confirm::{Asked, Confirm};
use bevy_cauldron::launcher::LauncherPlugin;
use bevy_cauldron::new_project::{NameField, NewProjectForm};
use bevy_cauldron::screen::Screen;
use bevy_cauldron::toolbar::ToolbarCommand;
use bevy_gooey::{ButtonVariant, Role};

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("buttons_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    app
}

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

/// A keystroke the way a keyboard sends it: a press and a release, then a frame.
fn tap(app: &mut App, logical: Key, code: KeyCode) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput { key_code: code, logical_key: logical.clone(), state, text: None, repeat: false, window: Entity::PLACEHOLDER });
    }
    app.update();
}

/// The button that says `text`.
fn button(app: &mut App, text: &str) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &Children)>();
    let mut t = world.query::<(&Role, &Text)>();
    q.iter(world)
        .find_map(|(e, kids)| kids.iter().find_map(|k| t.get(world, k).ok().filter(|(r, t)| **r == Role::ButtonText && t.0 == text).map(|_| e)))
        .unwrap_or_else(|| panic!("no button says `{text}`"))
}

/// The `Submit` button that says `text`: the page's main action, and not a plain one with the same words on another page.
fn submit_button_named(app: &mut App, text: &str) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &ButtonVariant, &Children)>();
    let mut t = world.query::<(&Role, &Text)>();
    q.iter(world)
        .filter(|(_, variant, _)| **variant == ButtonVariant::Submit)
        .find_map(|(e, _, kids)| kids.iter().find_map(|k| t.get(world, k).ok().filter(|(r, t)| **r == Role::ButtonText && t.0 == text).map(|_| e)))
        .unwrap_or_else(|| panic!("no Submit button says `{text}`"))
}

fn variant_of(app: &App, entity: Entity) -> Option<ButtonVariant> {
    app.world().get::<ButtonVariant>(entity).copied()
}

#[test]
fn return_in_the_new_project_name_field_creates_the_project() {
    let scratch: PathBuf = std::env::temp_dir().join(format!("grimoire_default_button_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap_or_else(|e| panic!("{}: {e}", scratch.display()));

    let mut app = editor("create");
    // Keys arrive on a window, which the headless editor has not spawned.
    app.world_mut().spawn((Window { resolution: bevy::window::WindowResolution::new(1280, 720), ..default() }, bevy::window::PrimaryWindow));
    // A home with a project on it, so the header's `New project` is the page's main action (a first run's is `Open the example`).
    let listed = scratch.join("listed");
    std::fs::create_dir_all(listed.join("assets/grimoire")).unwrap_or_else(|e| panic!("{}: {e}", listed.display()));
    let file = scratch.join("recent.ron");
    bevy_cauldron::recent::record(&file, &listed.canonicalize().unwrap_or_else(|e| panic!("{e}"))).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(bevy_cauldron::recent::RecentProjects(file));
    app.add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    settle(&mut app, 4);
    // The page's main action is gooey's Submit.
    let open = submit_button_named(&mut app, "New project");
    app.world_mut().commands().trigger(Activate { entity: open });
    settle(&mut app, 4);

    // The form is up with its Name field holding the keyboard; put it somewhere that is ours, and type a name.
    app.world_mut().resource_mut::<NewProjectForm>().0.location = scratch.to_string_lossy().into_owned();
    // The text box inside the Name row is what takes characters: put the keyboard on it, as a click on the field does.
    let row = app.world_mut().query_filtered::<Entity, With<NameField>>().single(app.world()).unwrap_or_else(|e| panic!("the form has its Name field: {e}"));
    let text_box = {
        let world = app.world_mut();
        let mut boxes = world.query_filtered::<Entity, With<bevy::ui_widgets::TextInput>>();
        let found: Vec<Entity> = boxes
            .iter(world)
            .filter(|e| std::iter::successors(Some(*e), |at| world.get::<ChildOf>(*at).map(ChildOf::parent)).any(|at| at == row))
            .collect();
        assert_eq!(found.len(), 1, "one text box in the Name row: {found:?}");
        found[0]
    };
    app.world_mut().resource_mut::<bevy::input_focus::InputFocus>().set(text_box, bevy::input_focus::FocusCause::Navigated);
    app.world_mut().insert_resource(bevy::input_focus::InputFocusVisible(true));
    settle(&mut app, 3);
    bevy_gooey::testing::type_text(&mut app, "harbour");
    eprintln!("DEBUG box={text_box:?} value={:?} focused_is_box={}", app.world().get::<bevy_gooey::TextInputValue>(text_box).map(|v| v.0.clone()), bevy_gooey::testing::focused(&app) == Some(text_box));
    eprintln!("DEBUG name={:?} focus={:?} label={:?}", app.world().resource::<NewProjectForm>().0.name, bevy_gooey::testing::focused(&app), bevy_gooey::testing::focused_label(&app));
    assert!(!scratch.join("harbour").exists(), "typing a name creates nothing by itself");

    bevy_gooey::testing::press(&mut app, KeyCode::Enter);
    settle(&mut app, 4);
    eprintln!("DEBUG after enter: form={} problem={:?} done={}", app.world().get_resource::<NewProjectForm>().is_some(), app.world().resource::<bevy_cauldron::launcher::Launcher>().problem, app.world().resource::<bevy_cauldron::launcher::Launcher>().new_project_done);
    assert!(scratch.join("harbour").is_dir(), "Return in the Name field pressed Create: the project's folder is made");
    assert!(app.world().get_resource::<NewProjectForm>().is_none(), "and the form is gone");
    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn every_toolbar_icon_button_is_ghost() {
    let mut app = editor("toolbar");
    let found: Vec<(&'static str, Option<ButtonVariant>)> = {
        let world = app.world_mut();
        world.query::<(&ToolbarCommand, Option<&ButtonVariant>)>().iter(world).map(|(command, variant)| (command.0, variant.copied())).collect()
    };
    assert!(!found.is_empty(), "the editor's toolbar has command buttons");
    let others: Vec<_> = found.iter().filter(|(_, variant)| *variant != Some(ButtonVariant::Ghost)).collect();
    assert!(others.is_empty(), "a toolbar button is a Ghost, no fill at rest: {others:?}");
}

#[test]
fn a_destructive_answer_is_danger_and_an_ordinary_one_is_not() {
    let mut app = editor("danger");
    app.world_mut().resource_mut::<Confirm>().ask(Asked::DeleteEntry, "Delete the map `harbour`?", "The map file is removed.", "Delete map `harbour`", "Keep it");
    settle(&mut app, 3);
    let yes = button(&mut app, "Delete map `harbour`");
    assert_eq!(variant_of(&app, yes), Some(ButtonVariant::Danger), "deleting a map is the danger colour");
    let no = button(&mut app, "Keep it");
    assert_eq!(variant_of(&app, no), None, "the safe answer is a plain button");

    // Answer it, then ask an ordinary question: quitting keeps everything saved.
    tap(&mut app, Key::Character("n".into()), KeyCode::KeyN);
    settle(&mut app, 3);
    app.world_mut().resource_mut::<Confirm>().ask(Asked::QuitApp, "Quit Cauldron?", "Anything already saved stays saved.", "Quit", "Stay");
    settle(&mut app, 3);
    let quit = button(&mut app, "Quit");
    assert_eq!(variant_of(&app, quit), None, "an ordinary answer is not red");
}
