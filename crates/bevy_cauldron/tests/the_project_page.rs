//! **The project page: a project's maps and kits, in the window's own widgets** (#296).
//!
//! It replaced the chooser. A project opened from the home, or named on the command line, lands here: a card per map, a row per kit, and
//! the dialogs the chooser's name fields were. What is proved, each by the widgets a person presses:
//!
//! 1. **A map card opens the editor on its map**, in this process.
//! 2. **The New map dialog makes the map through `map_create` and opens it**, and says as it is typed what file name the name becomes.
//! 3. **Esc goes back to the home's grid, and nothing quits.**
//! 4. **Delete asks first**, naming what goes; refusing keeps the file, agreeing removes it.
//! 5. **A map's settings and a kit's policy are editable here**, the two things the chooser's panels did that no other screen does.
//! 6. **The page follows its project's files**: a map an agent made appears.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, ValueChange};
use bevy_cauldron::catalog::Catalog;
use bevy_cauldron::confirm::{Asked, Confirm};
use bevy_cauldron::harness;
use bevy_cauldron::launcher::{Launcher, LauncherPlugin};
use bevy_cauldron::loading::{LoadRequest, LoadingPlugin};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::project_page::{CreateButton, KitRow, MapCard, NameField, NoteField, OriginField, PageAction, PageDialog, ProjectPage, SavedAs, SizeField};
use bevy_cauldron::screen::Screen;
use bevy_gooey::{FieldProblem, Role};

use crate::fixtures::Fixture;

/// A stepped editor standing on the page of a fixture project that has the map `m`.
fn on_the_page(tag: &str) -> (PathBuf, App) {
    let root = Fixture::new(&format!("page_{tag}")).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(LoadingPlugin);
    harness::stand_on_the_project_page(&mut app, &root);
    (root, app)
}

fn screen_of(app: &App) -> Screen {
    *app.world().resource::<State<Screen>>().get()
}

fn settle(app: &mut App) {
    for _ in 0..3 {
        app.update();
    }
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
    settle(app);
}

fn type_into<M: Component>(app: &mut App, text: &str) {
    let field = app.world_mut().query_filtered::<Entity, With<M>>().single(app.world()).expect("the dialog has the field");
    app.world_mut().commands().trigger(ValueChange::<String> { source: field, value: text.to_owned(), is_final: false });
    settle(app);
}

fn every_text(app: &mut App) -> Vec<String> {
    app.world_mut().query::<&Text>().iter(app.world()).map(|t| t.0.clone()).collect()
}

fn saved_as(app: &mut App) -> String {
    app.world_mut().query_filtered::<&Text, With<SavedAs>>().single(app.world()).expect("the dialog says what the name is saved as").0.clone()
}

fn dialogs(app: &mut App) -> usize {
    app.world_mut().query_filtered::<(), With<PageDialog>>().iter(app.world()).count()
}

/// A keystroke as a keyboard sends it, then a frame.
fn tap(app: &mut App, logical: Key, code: KeyCode) {
    for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput { key_code: code, logical_key: logical.clone(), state, text: None, repeat: false, window: Entity::PLACEHOLDER });
    }
    app.update();
}

/// Steps in real time until `done`, or five seconds pass.
fn until(app: &mut App, mut done: impl FnMut(&mut App) -> bool) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        app.update();
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the app never got there: it is on {:?}", screen_of(app));
}

fn map_file(root: &Path, name: &str) -> PathBuf {
    root.join("assets/grimoire/maps").join(format!("{name}.map.ron"))
}

#[test]
fn the_page_names_the_project_and_draws_its_maps_and_kits() {
    let (root, mut app) = on_the_page("draws");
    assert_eq!(screen_of(&app), Screen::Project);
    let said = every_text(&mut app);
    let name = root.file_name().unwrap().to_string_lossy().into_owned();
    assert!(said.contains(&name), "the breadcrumb names the project: {said:?}");
    assert!(said.iter().any(|t| t == "10 × 10 m, 4 m tall" || t.ends_with(" m tall")), "a map's size reads in metres: {said:?}");
    assert!(said.iter().any(|t| t == "Default for new pieces"), "and the kit new work lands in says so: {said:?}");
    let cards = app.world_mut().query::<&MapCard>().iter(app.world()).map(|c| c.0.clone()).collect::<Vec<_>>();
    assert_eq!(cards, ["m"]);
    let rows = app.world_mut().query::<&KitRow>().iter(app.world()).count();
    assert_eq!(rows, 1);
    let visible_dividers = app.world_mut().query::<(&bevy_gooey::SplitDivider, &Node)>().iter(app.world()).filter(|(_, n)| n.display != Display::None).count();
    assert_eq!(visible_dividers, 0, "the sidebar is not a thing to resize, so its divider is not drawn here either");
    for word in ["bevy_cauldron", "assets/grimoire"] {
        assert!(!said.iter().any(|t| t.contains(word)), "the page says no internal word `{word}`: {said:?}");
    }
}

/// **A map card opens the editor on that map**, through the loading screen, in this process.
#[test]
fn a_map_card_opens_the_editor_on_its_map() {
    let (root, mut app) = on_the_page("open");
    let card = app.world_mut().query::<(Entity, &MapCard)>().iter(app.world()).find(|(_, c)| c.0 == "m").map(|(e, _)| e).expect("a card for the map");
    app.world_mut().commands().trigger(Activate { entity: card });
    app.update();
    app.update();
    assert_eq!(screen_of(&app), Screen::Loading, "a map opens through the loading screen");
    let request = app.world().resource::<LoadRequest>().0.clone();
    assert_eq!(Path::new(&request[0]).canonicalize().unwrap(), root.canonicalize().unwrap(), "on that project");
    assert_eq!(request[1], "m", "and that map");

    until(&mut app, |app| screen_of(app) == Screen::Editor);
    assert_eq!(app.world().resource::<OpenMap>().map.name, "m", "the editor is on the map that was pressed");
    assert_eq!(app.world().resource::<Project>().open_map_name.as_deref(), Some("m"));
}

/// **The New map dialog makes the map and opens it**, and shows the file name the typed name becomes as it is typed.
#[test]
fn the_new_map_dialog_makes_the_map_and_opens_it_and_shows_what_the_name_is_saved_as() {
    let (root, mut app) = on_the_page("new_map");
    press(&mut app, "+ New map");
    assert_eq!(dialogs(&mut app), 1, "one dialog");
    assert!(!press_is_ready(&mut app), "Create waits for a name");

    type_into::<NameField>(&mut app, "First Level");
    assert_eq!(saved_as(&mut app), "Saved as first_level", "the helper shows the snake_case name the map will have");
    assert!(press_is_ready(&mut app));

    // A name that is taken is a problem on the field and nothing is made.
    type_into::<NameField>(&mut app, "m");
    let problem = app.world_mut().query_filtered::<&FieldProblem, With<NameField>>().iter(app.world()).next().map(|p| p.text.clone());
    assert!(problem.is_some_and(|p| p.contains("already here")), "the name in use is said beside the field");
    assert!(!press_is_ready(&mut app));

    type_into::<NameField>(&mut app, "First Level");
    press_create(&mut app);
    assert!(map_file(&root, "first_level").is_file(), "the map is on disk under its snake_case name");
    let made = Catalog::scan(&root).unwrap().maps.into_iter().find(|m| m.name == "first_level").expect("and the project lists it");
    assert!(matches!(made.summary, bevy_cauldron::catalog::MapSummary::Read { bounds: (10.0, 4.0, 10.0), .. }), "ten metres square and four tall by default");
    assert_eq!(dialogs(&mut app), 0, "the dialog is gone");

    until(&mut app, |app| screen_of(app) == Screen::Editor);
    assert_eq!(app.world().resource::<OpenMap>().map.name, "first_level", "and the new map is opened at once");
}

fn press_is_ready(app: &mut App) -> bool {
    let create = app.world_mut().query_filtered::<Entity, With<CreateButton>>().single(app.world()).expect("Create");
    app.world().get::<bevy::ui::InteractionDisabled>(create).is_none()
}

fn press_create(app: &mut App) {
    let create = app.world_mut().query_filtered::<Entity, With<CreateButton>>().single(app.world()).expect("Create");
    app.world_mut().commands().trigger(Activate { entity: create });
    settle(app);
}

/// **Esc goes back to the home's grid and nothing quits.**
#[test]
fn esc_on_the_page_goes_back_to_the_projects_grid_and_nothing_quits() {
    let (_root, mut app) = on_the_page("esc");
    assert_eq!(screen_of(&app), Screen::Project);
    tap(&mut app, Key::Escape, KeyCode::Escape);
    settle(&mut app);
    assert_eq!(screen_of(&app), Screen::Launcher, "Esc is back to the home");
    assert!(app.world().get_resource::<Messages<AppExit>>().is_none_or(Messages::is_empty), "and nothing asked the application to exit");
    assert_eq!(app.world().resource::<Launcher>().tab, bevy_cauldron::launcher::HomeTab::Projects, "on the Projects grid");
}

/// **Esc with a dialog up belongs to the dialog**, and with a question up to the question: neither goes back.
#[test]
fn esc_with_a_dialog_or_a_question_up_does_not_go_back() {
    let (_root, mut app) = on_the_page("esc_modal");
    press(&mut app, "+ New map");
    tap(&mut app, Key::Escape, KeyCode::Escape);
    assert_eq!(screen_of(&app), Screen::Project, "a dialog is up: Esc is its own");

    // A question up: the page asks before deleting, and Esc answers it.
    let mut app2 = on_the_page("esc_question").1;
    app2.world_mut().write_message(PageAction::DeleteMap("m".to_owned()));
    settle(&mut app2);
    assert!(app2.world().resource::<Confirm>().is_open());
    tap(&mut app2, Key::Escape, KeyCode::Escape);
    assert_eq!(screen_of(&app2), Screen::Project, "a question is up: Esc refuses it and stays");
}

/// **Delete asks first**, naming what goes; refusing keeps the file and agreeing removes it.
#[test]
fn delete_asks_first_and_only_agreeing_removes_the_map() {
    let (root, mut app) = on_the_page("delete");
    let second = map_file(&root, "second");
    std::fs::copy(map_file(&root, "m"), &second).unwrap();
    let text = std::fs::read_to_string(&second).unwrap().replacen("name: \"m\"", "name: \"second\"", 1);
    std::fs::write(&second, text).unwrap();
    app.world_mut().resource_mut::<ProjectPage>().dirty = true;
    settle(&mut app);

    app.world_mut().write_message(PageAction::DeleteMap("second".to_owned()));
    settle(&mut app);
    assert!(app.world().resource::<Confirm>().asking(Asked::DeleteEntry), "the page asks");
    assert!(second.is_file(), "asking changes nothing");
    let said = every_text(&mut app);
    assert!(said.iter().any(|t| t.contains("Delete the map `second`")), "the question names what goes: {said:?}");

    tap(&mut app, Key::Character("n".into()), KeyCode::KeyN);
    settle(&mut app);
    assert!(second.is_file(), "refusing keeps the map");

    app.world_mut().write_message(PageAction::DeleteMap("second".to_owned()));
    settle(&mut app);
    tap(&mut app, Key::Character("y".into()), KeyCode::KeyY);
    settle(&mut app);
    assert!(!second.exists(), "agreeing removes it");
    assert!(map_file(&root, "m").is_file(), "and only it");
    let cards = app.world_mut().query::<&MapCard>().iter(app.world()).map(|c| c.0.clone()).collect::<Vec<_>>();
    assert_eq!(cards, ["m"], "and the page draws what is left");
}

/// **A map's settings are edited from its card**: its size, origin and note, through `map_set_settings`, which keeps its name.
#[test]
fn a_maps_settings_are_changed_from_the_page_through_the_operation() {
    let (root, mut app) = on_the_page("settings");
    app.world_mut().write_message(PageAction::MapSettings("m".to_owned()));
    settle(&mut app);
    assert_eq!(dialogs(&mut app), 1);
    let said = every_text(&mut app);
    assert!(said.iter().any(|t| t == "Map: m"), "the name is shown and not editable: {said:?}");
    assert!(app.world_mut().query_filtered::<(), With<NameField>>().iter(app.world()).next().is_none(), "there is no name field to edit");

    // The three sizes and the origin's x are number fields; the note is the one text field.
    for (axis, value) in [(0, 20.0), (1, 14.0), (2, 6.0)] {
        let source = app.world_mut().query::<(Entity, &SizeField)>().iter(app.world()).find(|(_, f)| f.0 == axis).map(|(e, _)| e).expect("a size field");
        app.world_mut().commands().trigger(ValueChange::<f32> { source, value, is_final: true });
        settle(&mut app);
    }
    let origin_x = app.world_mut().query::<(Entity, &OriginField)>().iter(app.world()).find(|(_, f)| f.0 == 0).map(|(e, _)| e).expect("an origin field");
    app.world_mut().commands().trigger(ValueChange::<f32> { source: origin_x, value: 3.0, is_final: true });
    settle(&mut app);
    type_into::<NoteField>(&mut app, "a courtyard");
    press(&mut app, "Save");

    let text = std::fs::read_to_string(map_file(&root, "m")).unwrap();
    let map = bevy_grimoire_core::map::Map::parse(&text).unwrap();
    assert_eq!(map.bounds, (20.0, 6.0, 14.0), "width, height and depth in metres");
    assert_eq!(map.origin.0, 3.0);
    assert_eq!(map.note.as_deref(), Some("a courtyard"));
    assert_eq!(map.name, "m", "its name is untouched");
    assert_eq!(map.placements.len(), 1, "and so is everything it holds");
    assert_eq!(dialogs(&mut app), 0);
}

/// **A kit's policy is listed in plain words and an entry is removed through the question**, keeping the file's comments.
#[test]
fn a_kits_policy_lists_and_an_entry_is_removed_after_the_question() {
    let (root, mut app) = on_the_page("policy");
    let policy = root.join("assets/grimoire/furniture/project.ron");
    std::fs::write(
        &policy,
        "(\n    version: 2,\n    // A comment that must survive.\n    note: None,\n    exclude: [\"characters\"],\n    patches: [],\n)\n",
    )
    .unwrap();

    app.world_mut().write_message(PageAction::KitPolicy("furniture".to_owned()));
    settle(&mut app);
    let said = every_text(&mut app);
    assert!(said.iter().any(|t| t == "Leaves out `characters`"), "the entry is listed in words: {said:?}");

    press(&mut app, "Remove");
    assert!(app.world().resource::<Confirm>().asking(Asked::DeleteEntry), "removing asks");
    assert!(std::fs::read_to_string(&policy).unwrap().contains("characters"), "asking changes nothing");
    tap(&mut app, Key::Character("y".into()), KeyCode::KeyY);
    settle(&mut app);
    let after = std::fs::read_to_string(&policy).unwrap();
    assert!(!after.contains("characters"), "agreeing removes the entry:\n{after}");
    assert!(after.contains("// A comment that must survive."), "and keeps the file's comment:\n{after}");
    assert_eq!(dialogs(&mut app), 1, "the policy dialog comes back with what is left");
    assert!(every_text(&mut app).iter().any(|t| t == "This kit leaves nothing out and changes nothing."));
}

/// **A map an agent writes while the page is up appears on it.**
#[test]
fn the_page_follows_a_map_an_agent_makes() {
    let (root, mut app) = on_the_page("follows");
    let before = app.world_mut().query::<&MapCard>().iter(app.world()).count();
    let text = std::fs::read_to_string(map_file(&root, "m")).unwrap().replacen("name: \"m\"", "name: \"annexe\"", 1);
    std::fs::write(map_file(&root, "annexe"), text).unwrap();
    until(&mut app, |app| app.world_mut().query::<&MapCard>().iter(app.world()).count() == before + 1);
}

/// `LauncherPlugin` is the page's host: adding it twice is the harness's concern, never a test's.
#[test]
fn the_harness_adds_the_home_plugins_once() {
    let (root, mut app) = on_the_page("twice");
    harness::stand_on_the_project_page(&mut app, &root);
    assert!(app.is_plugin_added::<LauncherPlugin>());
    assert_eq!(screen_of(&app), Screen::Project);
}
