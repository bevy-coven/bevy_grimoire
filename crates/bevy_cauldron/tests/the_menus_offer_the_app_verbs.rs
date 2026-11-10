//! **The menus offer the verbs of an application** (#299, and #257's Help half).
//!
//! File: New project…, Open project…, Open recent, Add meshes…, Save, Close project, Quit. Help: the keys, the written guide,
//! What's new, Report a problem, About. The bar keeps its height while a question is up, and the Map menu is grouped. Each test
//! presses the row a person presses (an `Activate` on it) and asserts on what they would see.

use std::path::PathBuf;

use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy_cauldron::app_verbs::{AboutDialog, AppVerb, LinksOff, RECENT_SHOWN};
use bevy_cauldron::commands;
use bevy_cauldron::confirm::{Asked, Confirm};
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::keys::{self, Action, Context};
use bevy_cauldron::menu_bar::{EditorMenuBar, MAP_GROUPS, MenuCommand};
use bevy_cauldron::new_project::NewProjectForm;
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::project_assets::PointedAt;
use bevy_cauldron::screen::Screen;
use bevy_gooey::Activate;
use bevy_gooey::model::shortcut_conflicts;

use crate::fixtures;

/// The keys to hold down in the next frame, and the ones held in this one.
#[derive(Resource, Default)]
struct Script {
    next: Vec<KeyCode>,
    held: Vec<KeyCode>,
}

/// What the app asked to exit with, so a message that is gone in two frames is still a fact.
#[derive(Resource, Default)]
struct Exits(Vec<AppExit>);

fn drive(mut script: ResMut<Script>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    for key in script.held.drain(..) {
        keys.release(key);
    }
    let pressed = std::mem::take(&mut script.next);
    for key in &pressed {
        keys.press(*key);
    }
    script.held = pressed;
}

fn collect_exits(mut messages: MessageReader<AppExit>, mut exits: ResMut<Exits>) {
    exits.0.extend(messages.read().cloned());
}

fn editor_at(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // The home's plugins, as the binary has them: the verbs that leave for it set its state.
    app.add_plugins(bevy_cauldron::launcher::LauncherPlugin);
    app.init_resource::<Script>().init_resource::<Exits>();
    app.add_systems(PreUpdate, drive.after(InputSystems));
    app.add_systems(Last, collect_exits);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("barrel", "alpha")
        .place_as("barrel@0", "barrel", (0.0, 0.0))
        .build("m");
    editor_at(&root)
}

/// One frame with `keys` down, then a few to settle.
fn press(app: &mut App, keys: &[KeyCode]) {
    app.world_mut().resource_mut::<Script>().next = keys.to_vec();
    for _ in 0..5 {
        app.update();
    }
}

fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

fn dirty_the_map(app: &mut App) {
    let mut open = app.world_mut().resource_mut::<OpenMap>();
    let mut extra = open.map.placements[0].clone();
    extra.id = "barrel@1".to_owned();
    extra.at = (2.0, 2.0);
    open.map.placements.push(extra);
    assert!(open.dirty(), "the map holds an edit the file does not");
}

fn verb_row(app: &mut App, verb: &AppVerb) -> Entity {
    let found: Vec<Entity> = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &AppVerb)>();
        q.iter(world).filter(|(_, v)| *v == verb).map(|(e, _)| e).collect()
    };
    assert_eq!(found.len(), 1, "exactly one row is {verb:?}: {found:?}");
    found[0]
}

fn command_row(app: &mut App, action: Action) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &MenuCommand)>();
    let found: Vec<Entity> = q.iter(world).filter(|(_, r)| r.0 == action).map(|(e, _)| e).collect();
    assert_eq!(found.len(), 1, "exactly one row is {action:?}: {found:?}");
    found[0]
}

fn choose(app: &mut App, row: Entity) {
    app.world_mut().commands().trigger(Activate { entity: row });
    for _ in 0..4 {
        app.update();
    }
}

fn screen_of(app: &App) -> Screen {
    *app.world().resource::<State<Screen>>().get()
}

fn verbs(app: &mut App) -> Vec<AppVerb> {
    let world = app.world_mut();
    let mut q = world.query::<&AppVerb>();
    q.iter(world).cloned().collect()
}

#[test]
fn the_file_menu_offers_the_verbs_of_an_application() {
    let mut app = editor("menus_file");
    let rows = verbs(&mut app);
    for verb in [AppVerb::NewProject, AppVerb::OpenProject, AppVerb::AddMeshes, AppVerb::Quit] {
        assert!(rows.contains(&verb), "File offers {verb:?}");
    }
    for action in [Action::Save, Action::MainMenu] {
        command_row(&mut app, action);
    }
    assert_eq!(commands::command(Action::MainMenu).title, "Close project", "the way back is Close project");
    assert_eq!(commands::command(Action::Save).shortcut, bevy_gooey::Shortcut::primary(KeyCode::KeyS), "Save is Ctrl+S");
}

#[test]
fn the_help_menu_offers_the_keys_the_guide_and_a_report() {
    let mut app = editor("menus_help");
    let rows = verbs(&mut app);
    for verb in [AppVerb::KeyboardShortcuts, AppVerb::GettingStarted, AppVerb::WhatsNew, AppVerb::ReportProblem, AppVerb::About] {
        assert!(rows.contains(&verb), "Help offers {verb:?}");
    }
    command_row(&mut app, Action::Shortcuts);
}

/// **Every map command is in a group, and in one.** The groups are the Map menu's lines; a command in none would only be
/// appended after them.
#[test]
fn the_map_menu_names_every_map_command() {
    let in_groups: Vec<Action> = MAP_GROUPS.iter().flat_map(|g| g.iter().copied()).collect();
    for command in commands::all().iter().filter(|c| c.category() == "map") {
        let times = in_groups.iter().filter(|a| **a == command.action).count();
        assert_eq!(times, 1, "{} is in {times} groups of the Map menu", command.id);
    }
    for action in &in_groups {
        assert_eq!(commands::command(*action).category(), "map", "{action:?} is in the Map menu's groups but is not a map command");
    }
}

/// **An arrow means one thing in the Map context** (#299). The Map menu used to show Up twice, once for the palette and once for the
/// composition picker. They never fire together (one is `Idle`, the other `Picking`), but a menu that says Up on two rows is a menu a
/// person cannot trust, so the picker walks on Shift+Up and Shift+Down and the palette keeps the bare arrows.
#[test]
fn an_arrow_means_one_thing_in_the_map_context() {
    let map: Vec<(&str, bevy_gooey::Shortcut)> = keys::BINDINGS
        .iter()
        .filter(|b| keys::fires_in(b.context, Context::Map))
        .filter(|b| matches!(b.key, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::ArrowLeft | KeyCode::ArrowRight))
        .map(|b| (commands::identity(b.action).0, commands::shortcut_of(b)))
        .collect();
    let clashes: Vec<String> = shortcut_conflicts(&map).into_iter().map(|(a, b)| format!("{a} and {b}")).collect();
    assert!(clashes.is_empty(), "an arrow is bound to two commands in the Map context:\n  {}", clashes.join("\n  "));
}

#[test]
fn quit_from_a_clean_map_quits_at_once() {
    let mut app = editor("menus_quit_clean");
    let quit = verb_row(&mut app, &AppVerb::Quit);
    choose(&mut app, quit);
    assert!(!app.world().resource::<Confirm>().is_open(), "nothing is unsaved, so nothing is asked");
    assert_eq!(app.world().resource::<Exits>().0.len(), 1, "the app exits");
}

#[test]
fn quit_from_a_map_with_edits_asks_the_leave_questions_three_answers() {
    let mut app = editor("menus_quit_dirty");
    dirty_the_map(&mut app);
    let quit = verb_row(&mut app, &AppVerb::Quit);
    choose(&mut app, quit);
    assert!(app.world().resource::<Confirm>().asking(Asked::QuitApp), "unsaved edits are asked about before quitting");
    assert!(app.world().resource::<Exits>().0.is_empty(), "and nothing has quit yet");

    // Stay.
    press(&mut app, &[KeyCode::KeyN]);
    assert!(!app.world().resource::<Confirm>().is_open(), "N answers it");
    assert!(app.world().resource::<Exits>().0.is_empty(), "Stay keeps the app open");
    assert!(app.world().resource::<OpenMap>().dirty(), "and keeps the edit");
    assert!(!app.world().resource::<EditorState>().leaving, "the editor is not left waiting on a question that is gone");
}

#[test]
fn quit_discarding_leaves_the_file_as_it_was() {
    let mut app = editor("menus_quit_discard");
    dirty_the_map(&mut app);
    let in_file = std::fs::read_to_string(&app.world().resource::<OpenMap>().map_path).expect("the map's file");
    let quit = verb_row(&mut app, &AppVerb::Quit);
    choose(&mut app, quit);
    press(&mut app, &[KeyCode::KeyD]);
    assert_eq!(app.world().resource::<Exits>().0.len(), 1, "Discard quits");
    let after = std::fs::read_to_string(&app.world().resource::<OpenMap>().map_path).expect("the map's file");
    assert_eq!(in_file, after, "and writes nothing");
}

#[test]
fn quit_saving_writes_the_map_and_quits() {
    let mut app = editor("menus_quit_save");
    dirty_the_map(&mut app);
    let quit = verb_row(&mut app, &AppVerb::Quit);
    choose(&mut app, quit);
    press(&mut app, &[KeyCode::KeyY]);
    assert_eq!(app.world().resource::<Exits>().0.len(), 1, "Save and quit quits");
    assert!(!app.world().resource::<OpenMap>().dirty(), "after saving what was open");
}

/// **Cmd/Ctrl+Q is File → Quit**, from the keyboard.
#[test]
fn the_quit_chord_asks_the_same_question() {
    let mut app = editor("menus_quit_chord");
    dirty_the_map(&mut app);
    press(&mut app, &[primary(), KeyCode::KeyQ]);
    assert!(app.world().resource::<Confirm>().asking(Asked::QuitApp), "the chord asks about the unsaved edit");
}

#[test]
fn close_project_goes_back_to_the_project_page() {
    let mut app = editor("menus_close");
    let row = command_row(&mut app, Action::MainMenu);
    choose(&mut app, row);
    assert_eq!(screen_of(&app), Screen::Project, "Close project lands on the project page");
}

#[test]
fn new_project_leaves_for_the_home_and_its_form() {
    let mut app = editor("menus_new_project");
    let row = verb_row(&mut app, &AppVerb::NewProject);
    choose(&mut app, row);
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(screen_of(&app), Screen::Launcher, "New project… leaves for the home");
    assert!(app.world().get_resource::<NewProjectForm>().is_some(), "and the form is open on it");
}

#[test]
fn a_menu_that_leaves_saves_first_and_a_map_that_will_not_save_stays() {
    let mut app = editor("menus_leave_saves");
    dirty_the_map(&mut app);
    // A map that cannot be written: two placements with one id.
    {
        let mut open = app.world_mut().resource_mut::<OpenMap>();
        let twin = open.map.placements[0].clone();
        open.map.placements.push(twin);
    }
    let row = verb_row(&mut app, &AppVerb::OpenProject);
    choose(&mut app, row);
    assert_eq!(screen_of(&app), Screen::Editor, "a refused save keeps the person on the map");
    assert!(app.world().resource::<EditorState>().status.problem_text().contains("NOT SAVED"), "and says why");
}

/// **Open recent lists the last five** that are still there, newest first.
#[test]
fn open_recent_lists_at_most_five_projects() {
    let mut app = editor("menus_recent_list");
    let dir = std::env::temp_dir().join(format!("grimoire-recent-{}", std::process::id()));
    let mut projects: Vec<PathBuf> = Vec::new();
    for n in 0..7 {
        let folder = dir.join(format!("p{n}"));
        std::fs::create_dir_all(&folder).expect("a folder to list");
        projects.push(folder);
    }
    let file = dir.join("recent.yaml");
    let mut recent = bevy_cauldron::recent::Recent::default();
    for (n, folder) in projects.iter().enumerate() {
        recent.opened(folder, 1_000 + n as u64);
    }
    recent.save(&file).expect("the list");
    app.insert_resource(bevy_cauldron::recent::RecentProjects(file));
    // The bar is built again, as it is on every door.
    let bars: Vec<Entity> = app.world_mut().query_filtered::<Entity, With<EditorMenuBar>>().iter(app.world()).collect();
    for bar in bars {
        app.world_mut().entity_mut(bar).despawn();
    }
    for _ in 0..3 {
        app.update();
    }
    let listed: Vec<PathBuf> = verbs(&mut app)
        .into_iter()
        .filter_map(|v| match v {
            AppVerb::OpenRecent(path) => Some(path),
            _ => None,
        })
        .collect();
    assert_eq!(listed.len(), RECENT_SHOWN, "seven were opened and {RECENT_SHOWN} are listed: {listed:?}");
    let newest = std::fs::canonicalize(&projects[6]).expect("the newest folder");
    assert!(listed.contains(&newest), "the newest is in the list: {listed:?}");
    let oldest = std::fs::canonicalize(&projects[0]).expect("the oldest folder");
    assert!(!listed.contains(&oldest), "the oldest is not: {listed:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **Choosing a recent project saves what is open and opens that project in this process.**
#[test]
fn a_recent_project_opens_from_the_menu() {
    let other = fixtures::Fixture::new(&format!("menus_recent_open_other_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = editor("menus_recent_open");
    let dir = std::env::temp_dir().join(format!("grimoire-recent-open-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a folder");
    let file = dir.join("recent.yaml");
    let mut recent = bevy_cauldron::recent::Recent::default();
    recent.opened(&other, 1_000);
    recent.save(&file).expect("the list");
    app.insert_resource(bevy_cauldron::recent::RecentProjects(file));
    let bars: Vec<Entity> = app.world_mut().query_filtered::<Entity, With<EditorMenuBar>>().iter(app.world()).collect();
    for bar in bars {
        app.world_mut().entity_mut(bar).despawn();
    }
    for _ in 0..3 {
        app.update();
    }
    let row = verb_row(&mut app, &AppVerb::OpenRecent(std::fs::canonicalize(&other).expect("the folder")));
    choose(&mut app, row);
    assert_eq!(screen_of(&app), Screen::Project, "the editor's door closes and the project's page opens, in this process");
    assert_eq!(
        app.world().get_resource::<PointedAt>().map(|p| p.root.clone()),
        Some(std::fs::canonicalize(&other).expect("the folder")),
        "and it is the project that was chosen"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// **The bar is the same bar with a question up**: it was taken down and built again, which moved everything under it by its height.
#[test]
fn the_bar_keeps_its_place_and_height_while_a_question_is_up() {
    let mut app = editor("menus_bar_height");
    let bar_of = |app: &mut App| -> (Entity, f32) {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(Entity, &ComputedNode), With<EditorMenuBar>>();
        let (entity, node) = q.single(world).expect("the editor has one menu bar");
        (entity, node.size.y)
    };
    let (before, height) = bar_of(&mut app);
    assert!(height > 0.0, "the bar has a height to keep");
    app.world_mut().resource_mut::<Confirm>().ask(Asked::SwitchDoor, "A question?", "", "Yes", "No");
    for _ in 0..4 {
        app.update();
    }
    assert!(app.world().resource::<Confirm>().is_open());
    let (during, still) = bar_of(&mut app);
    assert_eq!(during, before, "the bar is the one that was there, not one built again");
    assert_eq!(still, height, "and it is as tall as it was");
}

#[test]
fn getting_started_opens_the_written_guide() {
    let mut app = editor("menus_guide");
    app.init_resource::<LinksOff>();
    let row = verb_row(&mut app, &AppVerb::GettingStarted);
    choose(&mut app, row);
    assert_eq!(app.world().resource::<LinksOff>().0, vec![bevy_cauldron::launcher::GUIDE_URL.to_owned()]);
}

#[test]
fn report_a_problem_opens_a_new_issue_that_names_the_version() {
    let mut app = editor("menus_report");
    app.init_resource::<LinksOff>();
    let row = verb_row(&mut app, &AppVerb::ReportProblem);
    choose(&mut app, row);
    let opened = app.world().resource::<LinksOff>().0.clone();
    assert_eq!(opened.len(), 1, "one address is opened");
    assert!(opened[0].starts_with("https://github.com/bevy-coven/bevy_grimoire/issues/new?body="), "{}", opened[0]);
    assert!(opened[0].contains(&format!("Cauldron%20{}", env!("CARGO_PKG_VERSION"))), "the version is in the body: {}", opened[0]);
}

#[test]
fn about_says_the_version_and_the_licence() {
    let mut app = editor("menus_about");
    let row = verb_row(&mut app, &AppVerb::About);
    choose(&mut app, row);
    let open = app.world_mut().query::<&AboutDialog>().iter(app.world()).count();
    assert_eq!(open, 1, "About opens a dialog");
    for words in [format!("Version {}", env!("CARGO_PKG_VERSION")), "Licence: MIT OR Apache-2.0".to_owned()] {
        assert!(bevy_gooey::testing::find_by_label(&mut app, &words).is_ok(), "the dialog says {words:?}");
    }
    // Esc closes it, and is not also the editor's back-out.
    press(&mut app, &[KeyCode::Escape]);
    let open = app.world_mut().query::<&AboutDialog>().iter(app.world()).count();
    assert_eq!(open, 0, "Esc closes About");
}
