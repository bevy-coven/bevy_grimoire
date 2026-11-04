//! **The launcher lists the projects the editor opened and opens one by its name** (UI-10, #216).
//!
//! A bare `bevy_cauldron` shows `Screen::Launcher`: one row per recent project (its name as a button, its
//! folder and when it was opened beside it). Choosing a row opens that folder in this process; a folder that
//! is gone or is not a project is refused on the screen, and nothing is opened.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_cauldron::chooser::MenuOpening;
use bevy_cauldron::launcher::{Launcher, LauncherPlugin, OpenRequest};
use bevy_cauldron::project_assets::ProjectRoot;
use bevy_cauldron::recent::{self, RecentProjects};
use bevy_cauldron::screen::Screen;
use bevy_gooey::Role;

use crate::fixtures;

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("grimoire_launcher_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A project folder named `name` under `dir`: `assets/grimoire` is all the launcher asks for.
fn project(dir: &Path, name: &str) -> PathBuf {
    let root = dir.join(name);
    std::fs::create_dir_all(root.join("assets/grimoire")).unwrap();
    root.canonicalize().unwrap()
}

/// An editor app on the launcher, whose recent list holds `projects` (the last one opened first); each of
/// `gone` is deleted after it was listed, as a folder a person moved.
fn on_the_launcher(tag: &str, scratch: &Scratch, projects: &[&Path], gone: &[&Path]) -> App {
    let root = fixtures::Fixture::new(&format!("launcher_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let file = scratch.0.join("recent.ron");
    for p in projects {
        recent::record(&file, p).unwrap();
    }
    for p in gone {
        std::fs::remove_dir_all(p).unwrap();
    }
    app.insert_resource(RecentProjects(file)).add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..4 {
        app.update();
    }
    app
}

fn texts(app: &mut App, role: Role) -> Vec<String> {
    let mut q = app.world_mut().query::<(&Role, &Text)>();
    q.iter(app.world()).filter(|(r, _)| **r == role).map(|(_, t)| t.0.clone()).collect()
}

/// The button whose text is `text`.
fn button(app: &mut App, text: &str) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &Children)>();
    let mut texts = world.query::<(&Role, &Text)>();
    let found: Vec<Entity> = q
        .iter(world)
        .filter_map(|(e, kids)| kids.iter().find_map(|k| texts.get(world, k).ok().filter(|(r, t)| **r == Role::ButtonText && t.0 == text).map(|_| e)))
        .collect();
    assert_eq!(found.len(), 1, "exactly one button says `{text}`: {found:?}");
    found[0]
}

fn press(app: &mut App, text: &str) {
    let b = button(app, text);
    app.world_mut().commands().trigger(Activate { entity: b });
    app.update();
    app.update();
}

fn screen_of(app: &App) -> Screen {
    *app.world().resource::<State<Screen>>().get()
}

/// The folder the asset server and the chooser are pointed at.
fn pointed_at(app: &App) -> (Option<PathBuf>, PathBuf) {
    (app.world().resource::<ProjectRoot>().current(), app.world().resource::<MenuOpening>().root.clone())
}

#[test]
fn the_launcher_lists_two_recent_projects_and_opens_the_second_by_its_name() {
    let s = Scratch::new("two");
    let (alpha, beta) = (project(&s.0, "alpha"), project(&s.0, "beta"));
    let mut app = on_the_launcher("two", &s, &[&alpha, &beta], &[]);

    let buttons = texts(&mut app, Role::ButtonText);
    assert!(buttons.contains(&"alpha".to_owned()) && buttons.contains(&"beta".to_owned()), "each recent project is a button named for its folder: {buttons:?}");
    let launcher = app.world().resource::<Launcher>();
    assert_eq!(launcher.rows.iter().map(|r| r.project.path.clone()).collect::<Vec<_>>(), [beta.clone(), alpha.clone()], "the last opened is first");
    let labels = texts(&mut app, Role::Label);
    assert!(labels.iter().any(|t| t.contains(alpha.to_str().unwrap()) && t.contains("opened")), "a row says where the project is and when it was opened: {labels:?}");
    assert_eq!(screen_of(&app), Screen::Launcher, "nothing opens until a row is chosen");

    // The second row of the list is `alpha`; it is opened by its label.
    press(&mut app, "alpha");
    assert_eq!(pointed_at(&app), (Some(alpha.clone()), alpha.clone()), "the window opens that folder, in this process");
    assert_eq!(screen_of(&app), Screen::Menu, "and goes on to its kits and maps");
    let again = recent::Recent::load(&s.0.join("recent.ron")).unwrap().listing();
    assert_eq!(again[0].project.path, alpha, "opening a project moves it to the top of the list");
}

#[test]
fn a_recent_project_whose_folder_is_gone_is_said_to_be_missing_and_is_not_opened() {
    let s = Scratch::new("gone");
    let (alpha, beta) = (project(&s.0, "alpha"), project(&s.0, "beta"));
    let mut app = on_the_launcher("gone", &s, &[&alpha, &beta], &[&alpha]);
    let labels = texts(&mut app, Role::Label);
    assert!(labels.iter().any(|t| t.starts_with("missing:") && t.contains(alpha.to_str().unwrap())), "the row says the folder is not there: {labels:?}");

    press(&mut app, "alpha");
    assert_eq!(screen_of(&app), Screen::Launcher, "a folder that is gone is not opened");
    let problem = app.world().resource::<Launcher>().problem.clone().expect("the refusal is kept");
    assert!(problem.contains("is not a folder") && problem.contains(alpha.to_str().unwrap()), "{problem}");
    assert!(texts(&mut app, Role::Label).contains(&problem), "and it is on the screen");
}

#[test]
fn a_typed_folder_that_is_not_a_project_is_refused_by_name_and_a_project_is_opened() {
    let s = Scratch::new("typed");
    let alpha = project(&s.0, "alpha");
    let plain = s.0.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let mut app = on_the_launcher("typed", &s, &[&alpha], &[]);

    app.world_mut().write_message(OpenRequest(plain.clone()));
    app.update();
    app.update();
    assert_eq!(screen_of(&app), Screen::Launcher);
    let problem = app.world().resource::<Launcher>().problem.clone().unwrap();
    assert!(problem.contains("is not a project") && problem.contains("assets/grimoire"), "the refusal names what is missing: {problem}");

    app.world_mut().write_message(OpenRequest(alpha.clone()));
    app.update();
    app.update();
    assert_eq!(pointed_at(&app), (Some(alpha.clone()), alpha.clone()));
    assert_eq!(app.world().resource::<Launcher>().problem, None, "a choice that works clears the last refusal");
}

#[test]
fn a_launcher_run_with_live_keeps_live_for_the_project_it_opens() {
    let s = Scratch::new("live");
    let alpha = project(&s.0, "alpha");
    let mut app = on_the_launcher("live", &s, &[&alpha], &[]);
    app.insert_resource(bevy_cauldron::live::LiveWrite);
    press(&mut app, "alpha");
    assert_eq!(pointed_at(&app).1, alpha);
    assert!(
        app.world().contains_resource::<bevy_cauldron::live::LiveWrite>(),
        "`bevy_cauldron --live` asked for live writes; the same process opens the project, so it still has them"
    );
}
