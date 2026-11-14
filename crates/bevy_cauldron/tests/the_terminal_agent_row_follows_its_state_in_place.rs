//! **The Terminal agent row's extras follow their state, though the list is no longer built again on every change** (#425 x #429).
//!
//! #425 gave the Terminal agent row a problem line, a `Use Oh My Pi` button, the command and a field for it (opened by
//! `SettingsDialog::command_editing`), and a choice list that offers `Custom command` only with a command set. #429 made the
//! settings list stand while a setting changes, built again only for a search. Both are right, and together they left the extras
//! built once: the field never opened, and the problem never appeared or went.
//!
//! Each test changes only what the row is built from, and reads what the person would see. The last one pins the other half of
//! #429: a setting that is not the agent's still changes in place, in the very rows the agent's change built.

use std::path::PathBuf;
use std::time::Duration;

use bevy::prelude::*;
use bevy::input_focus::InputFocus;
use bevy::ui::{Checked, ComputedNode, ScrollPosition};
use bevy::ui_widgets::ValueChange;
use bevy_cauldron::settings::{MachineAgent, TerminalAgent, UserSettingsFile};
use bevy_cauldron::settings_screen::OpenSettings;
use bevy_gooey::testing::{click, find_by_label};
use bevy_gooey::{ChoiceValue, ListView, TextHint};

use crate::fixtures;

struct Rig {
    app: App,
    user: PathBuf,
}

/// The editor on a project of its own, on a machine that has `machine`, its settings screen open on the Terminal tab's list.
fn rig(tag: &str, machine: TerminalAgent) -> Rig {
    let tag = format!("{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(MachineAgent(machine));
    let user = root.join(format!("user_config_{tag}/settings.yaml"));
    std::fs::create_dir_all(user.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(user.clone()));
    for _ in 0..4 {
        app.update();
    }
    app.world_mut().write_message(OpenSettings);
    for _ in 0..4 {
        app.update();
    }
    Rig { app, user }
}

/// Runs frames until `done`, because the watchers look at a file a few times a second.
fn until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        app.update();
        app.update();
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("never saw: {what}");
}

/// The words of every `Text` on screen that start with `start`.
fn texts_starting(app: &mut App, start: &str) -> Vec<String> {
    let world = app.world_mut();
    world.query::<&Text>().iter(world).filter(|t| t.0.starts_with(start)).map(|t| t.0.clone()).collect()
}

/// The problem line the Terminal agent row shows, if it shows one.
fn problem(app: &mut App) -> Vec<String> {
    texts_starting(app, "Problem:").into_iter().filter(|words| words.contains("terminal_agent_command")).collect()
}

/// The field the row opens for the command, if it is open **and shown**: the All page and the Terminal page each have the row, and
/// the page that is not shown has its own field, which is not the one a person types in.
fn command_field(app: &mut App) -> Option<Entity> {
    let world = app.world_mut();
    world
        .query::<(Entity, &TextHint, &InheritedVisibility)>()
        .iter(world)
        .find(|(_, hint, shown)| hint.0.starts_with("a program") && shown.get())
        .map(|(entity, ..)| entity)
}

/// How many `Custom command` options the agent choice draws now.
fn custom_options(app: &mut App) -> usize {
    let row = find_by_label(app, "Terminal agent").unwrap_or_else(|e| panic!("the Terminal tab lists the terminal's agent: {e}"));
    let world = app.world_mut();
    let mut found = 0;
    let mut stack = vec![row];
    while let Some(at) = stack.pop() {
        let words = world.get::<Children>(at).and_then(|kids| kids.iter().find_map(|kid| world.get::<Text>(kid).map(|t| t.0.clone()))).or_else(|| world.get::<Text>(at).map(|t| t.0.clone()));
        if world.get::<bevy_gooey::Role>(at) == Some(&bevy_gooey::Role::ChoiceOption) && words.is_some_and(|words| words.contains("Custom")) {
            found += 1;
        }
        if let Some(kids) = world.get::<Children>(at) {
            stack.extend(kids.iter());
        }
    }
    found
}

/// Narrows the list to the Terminal agent row, so the row stands at the top where a click can reach it (the list scrolls, and the
/// row is the last of the Terminal tab).
fn narrow_to_the_agent_row(app: &mut App) {
    let search = find_by_label(app, "Search settings").expect("the screen has a search field");
    click(app, search);
    let row = {
        let world = app.world_mut();
        world.query::<(Entity, &bevy_gooey::TextInputValue)>().iter(world).filter(|(_, v)| v.0.is_empty()).map(|(e, _)| e).next().expect("the search row holds no text")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: "terminal agent".to_owned(), is_final: false });
    for _ in 0..4 {
        app.update();
    }
}

/// **Choosing Custom with no command says what is wrong, and a command clears it.** The file is what the person's hand, or an older
/// release, left: `terminal_agent: Custom` and nothing else.
#[test]
fn a_custom_agent_with_no_command_shows_the_problem_and_setting_a_command_clears_it() {
    let mut r = rig("seam_problem", TerminalAgent::OhMyPi);
    assert!(problem(&mut r.app).is_empty(), "nothing is wrong at the start");

    std::fs::write(&r.user, "{version: 1, terminal_agent: Custom}").unwrap_or_else(|e| panic!("{e}"));
    until(&mut r.app, "the row to say Custom has no command", |app| !problem(app).is_empty());
    assert!(find_by_label(&mut r.app, "Use Oh My Pi, which is installed").is_ok(), "and to offer the one-press fix, omp being installed");

    std::fs::write(&r.user, "{version: 1, terminal_agent: Custom, terminal_agent_command: \"claude\"}").unwrap_or_else(|e| panic!("{e}"));
    until(&mut r.app, "the problem to clear once a command is set", |app| problem(app).is_empty());
    assert!(find_by_label(&mut r.app, "Use Oh My Pi, which is installed").is_err(), "the fix goes with the problem");
    assert!(!texts_starting(&mut r.app, "Command: claude").is_empty(), "the row says the command it holds");
    assert!(custom_options(&mut r.app) == 1, "and Custom command is a choice now");
}

/// **`Custom command…` opens the command field** in the row, which is `command_editing` being read after the list was built.
#[test]
fn the_custom_command_button_opens_the_field_in_the_row() {
    let mut r = rig("seam_field", TerminalAgent::OhMyPi);
    narrow_to_the_agent_row(&mut r.app);
    assert!(command_field(&mut r.app).is_none(), "the field is not there until it is asked for");
    assert_eq!(custom_options(&mut r.app), 0, "and without a command Custom is not a choice");
    let ask = find_by_label(&mut r.app, "Custom command…").unwrap_or_else(|e| panic!("the row offers a way to set a command: {e}"));
    click(&mut r.app, ask);
    for _ in 0..4 {
        r.app.update();
    }
    assert!(command_field(&mut r.app).is_some(), "the row opened a field for the command");
    assert!(find_by_label(&mut r.app, "Custom command…").is_err(), "in place of the button that asked for it");
}

/// The settings list on screen.
fn shown_list(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut lists = world.query_filtered::<(Entity, &ComputedNode, &InheritedVisibility), With<ListView>>();
    let dialog = world.query_filtered::<Entity, With<bevy_cauldron::settings_screen::SettingsDialog>>().single(world).expect("the settings dialog is open");
    lists
        .iter(world)
        .filter(|(list, node, shown)| {
            let mut at = *list;
            while let Some(parent) = world.get::<ChildOf>(at) {
                at = parent.parent();
                if at == dialog {
                    return node.size().y > 0.0 && shown.get();
                }
            }
            false
        })
        .map(|(e, ..)| e)
        .next()
        .expect("a settings list is on screen")
}

/// The rows of the list on screen, in order.
fn rows(app: &mut App) -> Vec<Entity> {
    let list = shown_list(app);
    rows_of(app, list)
}

fn rows_of(app: &mut App, list: Entity) -> Vec<Entity> {
    let world = app.world_mut();
    let column = world.get::<Children>(list).and_then(|c| c.first().copied()).expect("the list has its column: it was despawned, so the list was built again");
    world.get::<Children>(column).map(|c| c.iter().collect()).unwrap_or_default()
}

/// Where the list is drawn scrolled to, in logical px, and where it says it is scrolled to.
fn scroll_of(app: &mut App, list: Entity) -> (f32, f32) {
    let world = app.world();
    let drawn = world.get::<ComputedNode>(list).map_or(0.0, |n| n.scroll_position.y * n.inverse_scale_factor());
    (drawn, world.get::<ScrollPosition>(list).map_or(0.0, |s| s.y))
}

/// The Terminal agent row's own choice: the one control under `row` that holds a choice's value.
fn choice_under(app: &mut App, row: Entity) -> Entity {
    let world = app.world();
    world.get::<Children>(row).and_then(|kids| kids.iter().find(|kid| world.get::<ChoiceValue>(*kid).is_some())).expect("the agent row has its choice")
}

/// **Stepping through the agent row's own states never moves the list, its other rows or the focus** (#429's promise, kept by #425's
/// row): the list is scrolled to its end, where the agent row is, and a person asks for a command, types it and presses Enter.
#[test]
fn the_agent_row_follows_its_state_without_moving_the_list_or_losing_the_focus() {
    let mut r = rig("seam_scroll", TerminalAgent::OhMyPi);
    let list = shown_list(&mut r.app);
    r.app.world_mut().get_mut::<ScrollPosition>(list).expect("a list scrolls").y = 100_000.0;
    for _ in 0..10 {
        r.app.update();
    }
    let before = rows_of(&mut r.app, list);
    let (drawn, stored) = scroll_of(&mut r.app, list);
    assert!(drawn > 0.0, "the list is scrolled to its end ({drawn} px), so a rebuild would show");
    let agent_row = *before.last().expect("the Terminal agent row is the last of the list");
    let choice = choice_under(&mut r.app, agent_row);

    // Asking for a command: the button goes and the field comes, in the row that was there.
    let ask = find_by_label(&mut r.app, "Custom command…").unwrap_or_else(|e| panic!("the row offers a way to set a command: {e}"));
    r.app.world_mut().commands().trigger(bevy_gooey::Activate { entity: ask });
    for _ in 0..6 {
        r.app.update();
    }
    let field = command_field(&mut r.app).expect("the row opened a field for the command");
    assert_eq!(rows_of(&mut r.app, list), before, "asking for a command built the list again");
    let (drawn_now, stored_now) = scroll_of(&mut r.app, list);
    assert!(drawn_now > 0.0 && stored_now == stored, "asking for a command moved the list: drawn {drawn} -> {drawn_now}, stored {stored} -> {stored_now}");
    // The widget hands the focus on to the editor inside it, so the focus is the field or something under it.
    let held = r.app.world().resource::<InputFocus>().get().expect("something has the focus");
    let under_the_field = held == field || r.app.world().get::<ChildOf>(held).is_some_and(|_| {
        let mut at = held;
        while let Some(parent) = r.app.world().get::<ChildOf>(at) {
            at = parent.parent();
            if at == field {
                return true;
            }
        }
        false
    });
    let world = r.app.world();
    assert!(
        under_the_field,
        "the command field has the keyboard focus: it is {held:?} ({:?}, {:?}), the field {field:?} (size {:?}, shown {:?})",
        world.get::<bevy_gooey::Role>(held),
        world.get::<Text>(held).map(|t| t.0.clone()),
        world.get::<ComputedNode>(field).map(|n| n.size()),
        world.get::<InheritedVisibility>(field).map(|v| v.get())
    );
    assert_eq!(choice_under(&mut r.app, agent_row), choice, "the choice is the control that was there: no command is set, so its options did not change");

    // Typing is not an end; Enter is.
    r.app.world_mut().commands().trigger(ValueChange { source: field, value: "claude".to_owned(), is_final: false });
    r.app.world_mut().commands().trigger(ValueChange { source: field, value: "claude".to_owned(), is_final: true });
    for _ in 0..8 {
        r.app.update();
    }
    assert!(std::fs::read_to_string(&r.user).unwrap_or_default().contains("claude"), "Enter saved the command");
    assert_eq!(rows_of(&mut r.app, list), before, "saving the command built the list again");
    let (drawn_end, stored_end) = scroll_of(&mut r.app, list);
    assert!(drawn_end > 0.0 && stored_end == stored, "saving the command moved the list: drawn {drawn} -> {drawn_end}, stored {stored} -> {stored_end}");
    let choice_now = choice_under(&mut r.app, agent_row);
    assert_eq!(r.app.world().resource::<InputFocus>().get(), Some(choice_now), "the focus went to the row's control, not to the field that went");
    assert!(r.app.world().get_entity(choice_now).is_ok(), "and it exists");
    assert_eq!(custom_options(&mut r.app), 1, "Custom command is a choice now that a command is set");
    assert!(command_field(&mut r.app).is_none(), "and the field is gone");
}

/// **A setting that is not the agent's still changes in place**, after the agent's row was built again.
#[test]
fn another_setting_still_changes_in_place_after_the_agent_row_changed() {
    let mut r = rig("seam_in_place", TerminalAgent::OhMyPi);
    std::fs::write(&r.user, "{version: 1, terminal_agent: Custom}").unwrap_or_else(|e| panic!("{e}"));
    until(&mut r.app, "the problem to show", |app| !problem(app).is_empty());
    for _ in 0..4 {
        r.app.update();
    }
    let before = rows(&mut r.app);
    assert!(before.len() > 4, "the list holds the settings: {}", before.len());
    let grid = find_by_label(&mut r.app, "Grid lines").expect("the Grid lines switch is listed");
    assert!(r.app.world().get::<Checked>(grid).is_some(), "the grid is on by default");

    r.app.world_mut().commands().trigger(ValueChange { source: grid, value: false, is_final: true });
    for _ in 0..8 {
        r.app.update();
    }
    assert_eq!(rows(&mut r.app), before, "the rows are the same rows: stepping Grid lines built nothing again");
    assert!(r.app.world().get::<Checked>(grid).is_none(), "and the switch turned off in the control that was there");
    assert!(!problem(&mut r.app).is_empty(), "the agent's problem stands, it is not Grid lines's to change");
}
