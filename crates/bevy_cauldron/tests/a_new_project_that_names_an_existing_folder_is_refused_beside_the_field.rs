//! **The new-project wizard refuses an existing folder beside the field, keeps Create disabled with the
//! reason, and creates the project once the name is free** (UI-12, #218).

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::{Activate, ValueChange};
use bevy_cauldron::project_assets::PointedAt;
use bevy_cauldron::launcher::{ClipboardOff, Launcher, LauncherPlugin};
use bevy_cauldron::new_project::{self, CreateButton, LocationField, NameField, NewProject, ReasonLine, Template, TemplateChoice};
use bevy_cauldron::recent::{self, RecentProjects};
use bevy_cauldron::screen::Screen;
use bevy_gooey::{FieldProblem, Role};

use crate::fixtures::Fixture;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire_wizard_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn on_the_launcher(tag: &str, dir: &std::path::Path) -> App {
    let root = Fixture::new(&format!("wizard_{tag}_{}", std::process::id())).descriptor("bench", "props").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(RecentProjects(dir.join("recent.ron"))).insert_resource(ClipboardOff).add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..4 {
        app.update();
    }
    app
}

fn button(app: &mut App, text: &str) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &Children)>();
    let mut t = world.query::<(&Role, &Text)>();
    q.iter(world)
        .find_map(|(e, kids)| kids.iter().find_map(|k| t.get(world, k).ok().filter(|(r, t)| **r == Role::ButtonText && t.0 == text).map(|_| e)))
        .unwrap_or_else(|| panic!("no button says `{text}`"))
}

fn press(app: &mut App, text: &str) {
    let b = button(app, text);
    app.world_mut().commands().trigger(Activate { entity: b });
    for _ in 0..3 {
        app.update();
    }
}

fn type_into<M: Component>(app: &mut App, text: &str) {
    let field = app.world_mut().query_filtered::<Entity, With<M>>().single(app.world()).unwrap();
    app.world_mut().commands().trigger(ValueChange::<String> { source: field, value: text.to_owned(), is_final: false });
    for _ in 0..3 {
        app.update();
    }
}

fn problem_on<M: Component>(app: &mut App) -> Option<String> {
    app.world_mut().query_filtered::<&FieldProblem, With<M>>().iter(app.world()).next().map(|p| p.text.clone())
}

fn create_is_disabled(app: &mut App) -> bool {
    let create = app.world_mut().query_filtered::<Entity, With<CreateButton>>().single(app.world()).unwrap();
    app.world().get::<InteractionDisabled>(create).is_some()
}

fn reason(app: &mut App) -> String {
    app.world_mut().query_filtered::<&Text, With<ReasonLine>>().single(app.world()).unwrap().0.clone()
}

#[test]
fn an_existing_folder_is_a_problem_on_the_name_and_create_stays_disabled_until_the_name_is_free() {
    let dir = scratch("taken");
    let places = dir.join("projects");
    std::fs::create_dir_all(places.join("taken")).unwrap();
    let mut app = on_the_launcher("taken", &dir);
    assert!(app.world_mut().query_filtered::<(), With<NameField>>().iter(app.world()).next().is_none(), "no form until it is asked for");

    press(&mut app, "New project");
    assert!(create_is_disabled(&mut app), "an empty form cannot be created");
    assert_eq!(reason(&mut app), "Type a name for the project.", "and says why, before anything is typed");
    assert_eq!(problem_on::<NameField>(&mut app), None, "an untouched name is not an error");

    type_into::<LocationField>(&mut app, places.to_str().unwrap());
    type_into::<NameField>(&mut app, "taken");
    let problem = problem_on::<NameField>(&mut app).expect("the name has a problem");
    assert!(problem.contains("`taken` already exists") && problem.contains(places.to_str().unwrap()), "{problem}");
    assert!(create_is_disabled(&mut app));
    assert_eq!(reason(&mut app), problem, "Create's reason is the field's sentence");

    // A script can activate a disabled button; nothing is made and the refusal is shown.
    press(&mut app, "Create");
    assert_eq!(*app.world().resource::<State<Screen>>().get(), Screen::Launcher, "nothing is opened");
    assert!(app.world().resource::<Launcher>().problem.as_deref().is_some_and(|p| p.contains("already exists")));
    let said = app.world().resource::<Launcher>().problem.clone().unwrap();
    let mut shown = app.world_mut().query::<(&Role, &Text, &bevy::ui::ComputedNode)>();
    assert!(
        shown.iter(app.world()).any(|(r, t, _)| *r == Role::Label && t.0 == said),
        "a refusal raised by Create is on the form's own card, not only on the list card the form replaced: {said}"
    );

    type_into::<NameField>(&mut app, "harbour");
    assert_eq!(problem_on::<NameField>(&mut app), None, "a free name clears the problem");
    assert!(!create_is_disabled(&mut app));
    assert_eq!(reason(&mut app), "");

    press(&mut app, "Create");
    let made = places.join("harbour");
    assert!(made.join("assets/grimoire/kits.ron").is_file() && made.join("assets/grimoire/vocab.ron").is_file(), "the project is written");
    assert_eq!(app.world().resource::<PointedAt>().root, made.canonicalize().unwrap(), "and opened the way a recent row is, in this process");
    assert_eq!(*app.world().resource::<State<Screen>>().get(), Screen::Project);
    assert!(app.world_mut().query_filtered::<(), With<NameField>>().iter(app.world()).next().is_none(), "the form is gone");
    let listed = recent::Recent::load(&dir.join("recent.ron")).unwrap().listing();
    assert_eq!(listed[0].project.path, made.canonicalize().unwrap(), "and it is at the top of the recent list");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_location_that_is_not_a_folder_and_a_name_with_nothing_usable_are_each_said_on_their_own_field() {
    let dir = scratch("fields");
    let mut app = on_the_launcher("fields", &dir);
    press(&mut app, "New project");
    type_into::<LocationField>(&mut app, dir.join("nowhere").to_str().unwrap());
    assert!(problem_on::<LocationField>(&mut app).is_some_and(|p| p.contains("is not a folder")));
    type_into::<LocationField>(&mut app, dir.to_str().unwrap());
    assert_eq!(problem_on::<LocationField>(&mut app), None);
    type_into::<NameField>(&mut app, "123");
    assert!(problem_on::<NameField>(&mut app).is_some_and(|p| p.contains("leaves nothing usable")), "a name that snake_cases to nothing is refused");
    assert!(create_is_disabled(&mut app));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_one_kit_template_makes_a_project_with_a_first_kit_and_cancel_makes_nothing() {
    let dir = scratch("kit");
    let mut app = on_the_launcher("kit", &dir);
    press(&mut app, "New project");
    type_into::<LocationField>(&mut app, dir.to_str().unwrap());
    type_into::<NameField>(&mut app, "meadow");
    let choice = app.world_mut().query_filtered::<Entity, With<TemplateChoice>>().single(app.world()).unwrap();
    app.world_mut().commands().trigger(ValueChange::<usize> { source: choice, value: 1, is_final: true });
    app.update();
    press(&mut app, "Create");
    let made = dir.join("meadow");
    let catalog = bevy_cauldron::catalog::Catalog::scan(&made).unwrap();
    assert_eq!(catalog.kits.len(), 1, "one empty kit, bound");
    assert_eq!(catalog.authoring.as_deref(), Some("meadow"));

    let mut app = on_the_launcher("kit_cancel", &dir);
    press(&mut app, "New project");
    type_into::<LocationField>(&mut app, dir.to_str().unwrap());
    type_into::<NameField>(&mut app, "never");
    press(&mut app, "Cancel");
    assert!(!dir.join("never").exists(), "Cancel writes nothing");
    assert!(app.world_mut().query_filtered::<(), With<NameField>>().iter(app.world()).next().is_none());
    assert_eq!(new_project::check(&NewProject { name: "x".into(), location: dir.to_string_lossy().into_owned(), template: Template::Empty }).reason(), None);
    let _ = std::fs::remove_dir_all(&dir);
}
