//! **The Projects home lists the projects the editor opened as cards and opens one by its name** (UI-10, #216; #295).
//!
//! A bare `bevy_cauldron` shows `Screen::Launcher`: one card per recent project (its name, its folder and when it was opened).
//! A first press on a card chooses it and a second opens that folder in this process. A folder that is gone is a card that
//! does not open and offers Locate… and Remove; a folder that is not a project is a toast that offers to make it one. A first
//! run with nothing opened yet is a welcome whose one accent button is **Open the example**.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_cauldron::project_assets::PointedAt;
use bevy_cauldron::launcher::{Launcher, LauncherPlugin, MissingCard, OpenRequest, ProjectCard};
use bevy_cauldron::picker::{PickFor, Picked};
use bevy_gooey::ButtonVariant;
use bevy_gooey::{Toast, ToastAction, ToastKind};
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
    let file = scratch.0.join("recent.yaml");
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

/// The buttons that are a screen's primary action: gooey's `ButtonVariant::Submit` (UI-R03, #333; they were `AccentButton`).
fn submit_buttons(app: &mut App) -> Vec<Entity> {
    let world = app.world_mut();
    world.query::<(Entity, &ButtonVariant)>().iter(world).filter(|(_, variant)| **variant == ButtonVariant::Submit).map(|(entity, _)| entity).collect()
}

fn screen_of(app: &App) -> Screen {
    *app.world().resource::<State<Screen>>().get()
}

/// The folder the asset server and the chooser are pointed at.
fn pointed_at(app: &App) -> (Option<PathBuf>, PathBuf) {
    (app.world().resource::<ProjectRoot>().current(), app.world().resource::<PointedAt>().root.clone())
}

/// Every `Text` on screen.
fn all_text(app: &mut App) -> Vec<String> {
    let mut q = app.world_mut().query::<&Text>();
    q.iter(app.world()).map(|t| t.0.clone()).collect()
}

/// Every toast up: its words, its kind and its buttons' ids.
fn toasts(app: &mut App) -> Vec<(Entity, String, ToastKind, Vec<String>)> {
    let mut q = app.world_mut().query::<(Entity, &Toast)>();
    q.iter(app.world()).map(|(e, t)| (e, t.text().to_owned(), t.kind(), t.actions().iter().map(|(l, _)| l.clone()).collect())).collect()
}

#[test]
fn the_home_lists_two_recent_projects_as_cards_and_opens_the_second_by_its_name() {
    let s = Scratch::new("two");
    let (alpha, beta) = (project(&s.0, "alpha"), project(&s.0, "beta"));
    let mut app = on_the_launcher("two", &s, &[&alpha, &beta], &[]);

    let names = texts(&mut app, Role::ButtonText);
    assert!(names.contains(&"alpha".to_owned()) && names.contains(&"beta".to_owned()), "each recent project is a card named for its folder: {names:?}");
    let launcher = app.world().resource::<Launcher>();
    assert_eq!(launcher.rows.iter().map(|r| r.project.path.clone()).collect::<Vec<_>>(), [beta.clone(), alpha.clone()], "the last opened is first");
    let said = all_text(&mut app);
    assert!(said.iter().any(|t| t.starts_with("Opened ")), "a card says when the project was opened: {said:?}");
    assert!(said.iter().any(|t| t.contains("alpha")), "and where it is: {said:?}");
    assert_eq!(screen_of(&app), Screen::Launcher, "nothing opens until a card is chosen");

    // The first press chooses the card; the second opens it.
    press(&mut app, "alpha");
    assert_eq!(app.world().resource::<Launcher>().selected.as_ref(), Some(&alpha), "one press chooses");
    assert_eq!(screen_of(&app), Screen::Launcher, "and opens nothing");
    press(&mut app, "alpha");
    assert_eq!(pointed_at(&app), (Some(alpha.clone()), alpha.clone()), "the window opens that folder, in this process");
    assert_eq!(screen_of(&app), Screen::Project, "and goes on to its kits and maps");
    let again = recent::Recent::load(&s.0.join("recent.yaml")).unwrap().listing();
    assert_eq!(again[0].project.path, alpha, "opening a project moves it to the top of the list");
}

#[test]
fn a_missing_projects_card_does_not_open_and_offers_locate_and_remove() {
    let s = Scratch::new("gone");
    let (alpha, beta) = (project(&s.0, "alpha"), project(&s.0, "beta"));
    let mut app = on_the_launcher("gone", &s, &[&alpha, &beta], &[&alpha]);

    let mut missing = app.world_mut().query::<&MissingCard>();
    assert_eq!(missing.iter(app.world()).map(|c| c.path.clone()).collect::<Vec<_>>(), std::slice::from_ref(&alpha), "the moved folder's card is the missing kind");
    let mut openable = app.world_mut().query::<&ProjectCard>();
    assert_eq!(openable.iter(app.world()).map(|c| c.path.clone()).collect::<Vec<_>>(), std::slice::from_ref(&beta), "and only the folder that is there opens");
    assert!(all_text(&mut app).iter().any(|t| t == "Folder not found"), "it says so");
    assert!(!texts(&mut app, Role::ButtonText).contains(&"alpha".to_owned()), "there is no button named for it to press");
    button(&mut app, "Locate…");
    button(&mut app, "Remove");

    // Asked to open anyway (an agent, a script), it is refused where a person is looking.
    app.world_mut().write_message(OpenRequest(alpha.clone()));
    app.update();
    app.update();
    assert_eq!(screen_of(&app), Screen::Launcher, "a folder that is gone is not opened");
    let up = toasts(&mut app);
    assert!(up.iter().any(|(_, text, kind, _)| *kind == ToastKind::Alert && text.contains("is not a folder") && text.contains(alpha.to_str().unwrap())), "{up:?}");

    // Remove takes it out of the list, and the folder is not touched.
    press(&mut app, "Remove");
    let list = recent::Recent::load(&s.0.join("recent.yaml")).unwrap().listing();
    assert_eq!(list.iter().map(|r| r.project.path.clone()).collect::<Vec<_>>(), std::slice::from_ref(&beta));
    let mut missing = app.world_mut().query::<&MissingCard>();
    assert_eq!(missing.iter(app.world()).count(), 0, "and its card is gone");
}

#[test]
fn locate_points_a_missing_card_at_where_the_folder_went() {
    let s = Scratch::new("locate");
    let (alpha, beta) = (project(&s.0, "alpha"), project(&s.0, "beta"));
    let mut app = on_the_launcher("locate", &s, &[&alpha, &beta], &[&alpha]);
    let moved = project(&s.0, "alpha_moved");
    app.world_mut().write_message(Picked { for_what: PickFor::Locate(alpha.clone()), paths: vec![moved.clone()] });
    app.update();
    app.update();
    let mut missing = app.world_mut().query::<&MissingCard>();
    assert_eq!(missing.iter(app.world()).count(), 0, "the card is no longer missing");
    let names = texts(&mut app, Role::ButtonText);
    assert!(names.contains(&"alpha_moved".to_owned()), "it opens from where it is now: {names:?}");
}

#[test]
fn a_first_run_is_a_welcome_with_one_accent_button_that_opens_the_example() {
    let s = Scratch::new("first");
    let mut app = on_the_launcher("first", &s, &[], &[]);
    assert!(app.world().resource::<Launcher>().example.is_some(), "this checkout has an example beside it");

    let accents = submit_buttons(&mut app);
    let found: Vec<_> = accents.into_iter().map(|e| (app.world().get::<Children>(e).map(|kids| kids.to_vec()).unwrap_or_default(), e)).collect();
    assert_eq!(found.len(), 1, "exactly one accent button on the first run");
    let mut texts_q = app.world_mut().query::<(&Role, &Text)>();
    let label = found[0].0.iter().find_map(|k| texts_q.get(app.world(), *k).ok().map(|(_, t)| t.0.clone()));
    assert_eq!(label.as_deref(), Some("Open the example"));
    let said = all_text(&mut app).join("\n");
    assert!(said.contains("Welcome to Cauldron"), "{said:?}");
    // **The first run says what this thing is, and how it works** (design review, 2026-11-05). The old
    // sentence named a grimoire and defined nothing, the screen never said *Bevy*, and the tour that
    // would teach a first-timer lived two clicks away under `Learn`.
    assert!(said.contains("Bevy"), "the engine is named: {said:?}");
    // **No path.** A first draft said `assets/grimoire` and `no_internal_words_on_screen` refused it —
    // correctly: a path under `assets/` is the editor's word, not the person's. The *property* a first
    // run needs is that there is nothing to export.
    assert!(!said.contains("assets/"), "no path on the first screen: {said:?}");
    assert!(said.contains("no export step"), "the property is named: {said:?}");
    assert!(!said.contains("hand your game a grimoire it can load"), "the sentence that defined nothing is gone: {said:?}");
    assert!(said.contains("1. Open the example"), "the first step is spelled out: {said:?}");
    // The footnote had no referent — nothing on the home is a setting — so it is now the one thing a
    // person with a folder on disk needs to know before pressing Open folder….
    assert!(said.contains("Nothing on disk changes until you save"), "{said:?}");
    assert!(!said.contains("Everything here can be changed later in Settings"), "the referentless footnote is gone: {said:?}");
    // **`Take the tour` is on this page as well as under `Learn`**, which is the point of the finding:
    // the onboarding used to be reachable only from the second tab. Two buttons say it now, by design,
    // so this asserts it is here rather than that it is unique.
    assert!(all_text(&mut app).iter().any(|t| t == "Take the tour"), "the tour is offered on the first screen");
    button(&mut app, "New project");
    button(&mut app, "Open folder…");
}

#[test]
fn open_the_example_lands_on_the_examples_map_in_this_process() {
    let s = Scratch::new("example");
    let mut app = on_the_launcher("example", &s, &[], &[]);
    let example = app.world().resource::<Launcher>().example.clone().expect("this checkout has an example");
    // The welcome's accent button (the Learn page offers a plain one too).
    let accents = submit_buttons(&mut app);
    assert_eq!(accents.len(), 1, "exactly one accent button on the first run");
    let accent = accents[0];
    app.world_mut().commands().trigger(Activate { entity: accent });
    app.update();
    app.update();
    assert_eq!(screen_of(&app), Screen::Loading, "a map is opened through the loading screen, in this process");
    let argv = app.world().resource::<bevy_cauldron::loading::LoadRequest>().0.clone();
    assert_eq!(argv, [example.canonicalize().unwrap().to_string_lossy().into_owned(), "bedroom".to_owned()], "on the example's own map, not on a list of maps");
}

#[test]
fn a_home_with_projects_has_new_project_as_its_one_accent_button() {
    let s = Scratch::new("accent");
    let alpha = project(&s.0, "alpha");
    let mut app = on_the_launcher("accent", &s, &[&alpha], &[]);
    let accents = submit_buttons(&mut app);
    assert_eq!(accents.len(), 1);
    assert_eq!(button(&mut app, "New project"), accents[0]);
}

#[test]
fn a_chosen_folder_that_is_not_a_project_offers_to_make_it_one_and_keeps_what_it_held() {
    let s = Scratch::new("typed");
    let alpha = project(&s.0, "alpha");
    let plain = s.0.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    std::fs::write(plain.join("notes.txt"), "mine").unwrap();
    let mut app = on_the_launcher("typed", &s, &[&alpha], &[]);

    app.world_mut().write_message(Picked { for_what: PickFor::OpenFolder, paths: vec![plain.clone()] });
    app.update();
    app.update();
    app.update();
    assert_eq!(screen_of(&app), Screen::Launcher);
    let up = toasts(&mut app);
    let (toast, text, _, actions) = up.first().cloned().expect("the refusal is a toast");
    assert_eq!(text, "That folder is not a Cauldron project. Make it one?");
    assert_eq!(actions, ["Make it a project"]);
    assert!(!text.contains("assets/grimoire"), "no internal word");

    app.world_mut().commands().trigger(ToastAction { toast, id: format!("make-project:{}", plain.display()) });
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(std::fs::read_to_string(plain.join("notes.txt")).unwrap(), "mine", "the folder's own files stay");
    assert!(plain.join("assets/grimoire/kits.yaml").is_file() && plain.join("assets/grimoire/vocab.ron").is_file());
    assert_eq!(pointed_at(&app).0, Some(plain.canonicalize().unwrap()), "and it opens");
}

#[test]
fn a_folder_chosen_with_open_folder_that_is_a_project_opens() {
    let s = Scratch::new("pick");
    let (alpha, beta) = (project(&s.0, "alpha"), project(&s.0, "beta"));
    let mut app = on_the_launcher("pick", &s, &[&alpha], &[]);
    app.world_mut().write_message(Picked { for_what: PickFor::OpenFolder, paths: vec![beta.clone()] });
    app.update();
    app.update();
    assert_eq!(pointed_at(&app), (Some(beta.clone()), beta.clone()));
    assert_eq!(recent::Recent::load(&s.0.join("recent.yaml")).unwrap().listing()[0].project.path, beta);
}

#[test]
fn the_filter_hides_the_cards_it_does_not_match_by_name_or_folder() {
    let s = Scratch::new("filter");
    let (alpha, beta) = (project(&s.0, "alpha"), project(&s.0, "beta"));
    let mut app = on_the_launcher("filter", &s, &[&alpha, &beta], &[]);
    let shown = |app: &mut App| {
        let mut q = app.world_mut().query::<(&ProjectCard, &Node)>();
        q.iter(app.world()).filter(|(_, n)| n.display != Display::None).map(|(c, _)| c.path.clone()).collect::<Vec<_>>()
    };
    assert_eq!(shown(&mut app).len(), 2);
    app.world_mut().resource_mut::<Launcher>().filter = "alp".to_owned();
    app.update();
    assert_eq!(shown(&mut app), std::slice::from_ref(&alpha));
    app.world_mut().resource_mut::<Launcher>().filter = String::new();
    app.update();
    assert_eq!(shown(&mut app).len(), 2, "an empty filter shows every project");
}

#[test]
fn a_launcher_run_with_live_keeps_live_for_the_project_it_opens() {
    let s = Scratch::new("live");
    let alpha = project(&s.0, "alpha");
    let mut app = on_the_launcher("live", &s, &[&alpha], &[]);
    app.insert_resource(bevy_cauldron::live::LiveWrite);
    app.world_mut().write_message(OpenRequest(alpha.clone()));
    app.update();
    app.update();
    assert_eq!(pointed_at(&app).1, alpha);
    assert!(
        app.world().contains_resource::<bevy_cauldron::live::LiveWrite>(),
        "`bevy_cauldron --live` asked for live writes; the same process opens the project, so it still has them"
    );
}
