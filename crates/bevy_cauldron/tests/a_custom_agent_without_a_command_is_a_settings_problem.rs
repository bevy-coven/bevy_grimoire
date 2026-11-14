//! **A saved Custom agent with no command is a settings problem, and the picker never saves one** (#425).
//!
//! The owner's `settings.yaml` held `terminal_agent: Custom` and no `terminal_agent_command`, so the agent card read
//! `FAIL Agent started: Custom command is not set`, and the Settings row (which offered `Custom command` among its choices)
//! made that state one press of an arrow away.
//!
//! - the Terminal agent row does not offer `Custom command` until a command is set, and no step of its choices saves it;
//! - a file that already holds `Custom` with no command is one settings problem, naming the file and the fix, and the row
//!   shows it with the one-press fix (Oh My Pi, when it is installed);
//! - with nothing saved the default stays #341's: the agent is the machine's.

use std::path::PathBuf;
use std::time::Duration;

use bevy::prelude::*;
use bevy_cauldron::live::EditorReloads;
use bevy_cauldron::setting_catalogue::{self, Value};
use bevy_cauldron::settings::{EditorSettings, MachineAgent, SettingsLayer, TerminalAgent, UserSettingsFile};
use bevy_cauldron::settings_screen::OpenSettings;
use bevy_gooey::testing::{click, find_by_label};
use bevy_gooey::{Role, TextHint};
use bevy_grimoire_core::live::Outcome;

use crate::fixtures;

/// The editor on a project of its own, its settings screen closed.
struct Rig {
    app: App,
    user: PathBuf,
    project: PathBuf,
}

/// A project whose `editor.yaml` is `project_settings` (when given), a user settings file that does not exist yet, and a machine
/// that `machine` says has the agent or not.
fn rig(tag: &str, project_settings: Option<&str>, machine: TerminalAgent) -> Rig {
    let tag = format!("{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("site/wall", "alpha").build("m");
    let project = root.join("assets/grimoire/editor.yaml");
    if let Some(text) = project_settings {
        std::fs::write(&project, text).unwrap_or_else(|e| panic!("{}: {e}", project.display()));
    }
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(MachineAgent(machine));
    let user = root.join(format!("user_config_{tag}/settings.yaml"));
    std::fs::create_dir_all(user.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(user.clone()));
    for _ in 0..4 {
        app.update();
    }
    Rig { app, user, project }
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// Runs frames until `done`, because the watchers look at a file a few times a second.
fn until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        updates(app, 2);
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("never saw: {what}");
}

/// Opens the screen and narrows it to the Terminal agent row, so the row stands at the top of the list where a click can reach it
/// (the list scrolls, and the row is the last of the Terminal tab).
fn open(app: &mut App) {
    app.world_mut().write_message(OpenSettings);
    updates(app, 4);
    let search = find_by_label(app, "Search settings").expect("the screen has a search field");
    click(app, search);
    let row = app
        .world_mut()
        .query::<(Entity, &bevy_gooey::TextInputValue)>()
        .iter(app.world())
        .filter(|(_, v)| v.0.is_empty())
        .map(|(e, _)| e)
        .next()
        .expect("the search row holds no text");
    app.world_mut().commands().trigger(bevy::ui_widgets::ValueChange { source: row, value: "terminal agent".to_owned(), is_final: false });
    updates(app, 4);
}

fn layer(path: &std::path::Path) -> SettingsLayer {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    SettingsLayer::read("test", &text).unwrap_or_else(|r| panic!("{}", r.reason))
}

/// The options of the Terminal agent row's choice, as the screen draws them: `(the option's entity, its words)`.
fn options(app: &mut App) -> Vec<(Entity, String)> {
    let row = find_by_label(app, "Terminal agent").unwrap_or_else(|e| panic!("the Terminal tab lists the terminal's agent: {e}"));
    let world = app.world_mut();
    let mut found = Vec::new();
    let mut stack = vec![row];
    while let Some(at) = stack.pop() {
        if world.get::<Role>(at) == Some(&Role::ChoiceOption) {
            let words = world.get::<Children>(at).and_then(|kids| kids.iter().find_map(|kid| world.get::<Text>(kid).map(|t| t.0.clone()))).or_else(|| world.get::<Text>(at).map(|t| t.0.clone()));
            found.push((at, words.unwrap_or_default()));
        }
        if let Some(kids) = world.get::<Children>(at) {
            stack.extend(kids.iter());
        }
    }
    found
}

fn reload_problems(app: &App, file_part: &str) -> Vec<(String, String)> {
    app.world()
        .resource::<EditorReloads>()
        .kinds()
        .iter()
        .filter(|k| k.outcome == Outcome::Refused && k.file.contains(file_part))
        .map(|k| (k.file.clone(), k.reason.clone()))
        .collect()
}

/// **A choice without a command is never saved**: the write itself refuses it, whatever drew the choice.
#[test]
fn writing_custom_with_no_command_is_refused_and_leaves_the_file_alone() {
    let r = rig("custom_write", None, TerminalAgent::Off);
    let custom = Value::Option(setting_catalogue::agent_index(TerminalAgent::Custom));
    let refused = setting_catalogue::write_key(&r.user, "terminal_agent", Some(custom));
    assert!(refused.as_ref().is_err_and(|why| why.contains("command")), "Custom with no command is refused, saying why: {refused:?}");
    assert!(!r.user.exists(), "nothing was written: {}", r.user.display());

    // With a command in the file it is the choice a person made.
    std::fs::write(&r.user, "{version: 1, terminal_agent_command: \"claude\"}").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(setting_catalogue::write_key(&r.user, "terminal_agent", Some(custom)), Ok(()));
    assert_eq!(layer(&r.user).terminal_agent, Some(TerminalAgent::Custom));

    // And a file that already has the problem can still change every other key.
    std::fs::write(&r.user, "{version: 1, terminal_agent: Custom}").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(setting_catalogue::write_key(&r.user, "show_grid", Some(Value::On(false))), Ok(()), "only the agent key is held to the rule");
}

/// **Stepping the Terminal agent row's choices never saves `Custom` without a command.**
#[test]
fn the_row_offers_custom_command_only_once_a_command_is_set_and_no_step_saves_it_without_one() {
    let mut r = rig("custom_row", None, TerminalAgent::Off);
    open(&mut r.app);
    let offered: Vec<String> = options(&mut r.app).into_iter().map(|(_, words)| words).collect();
    assert_eq!(offered.len(), 2, "with no command the row offers Off and Oh My Pi, not Custom command: {offered:?}");
    assert!(!offered.iter().any(|words| words.contains("Custom")), "{offered:?}");

    // Press every choice the row offers, in order, as a person stepping through them does.
    // The rows are built again after every write, so each option is found afresh before it is pressed.
    for index in 0..offered.len() {
        updates(&mut r.app, 4);
        let found = options(&mut r.app);
        let Some((option, words)) = found.get(index).cloned() else { break };
        click(&mut r.app, option);
        updates(&mut r.app, 4);
        if r.user.exists() {
            assert_ne!(layer(&r.user).terminal_agent, Some(TerminalAgent::Custom), "stepping onto {words:?} saved Custom with no command");
        }
    }
    assert!(r.user.exists(), "the choices that are offered do save");

    // A command makes it a choice.
    std::fs::write(&r.user, "{version: 1, terminal_agent: OhMyPi, terminal_agent_command: \"claude\"}").unwrap_or_else(|e| panic!("{e}"));
    until(&mut r.app, "the row to offer Custom command once the file sets a command", |app| options(app).len() == 3);
    assert!(options(&mut r.app).iter().any(|(_, words)| words.contains("Custom")));
}

/// **Choosing it asks for the command in the same row**, and the command and the choice are saved together.
#[test]
fn a_command_typed_in_the_row_is_saved_with_the_choice() {
    let mut r = rig("custom_type", None, TerminalAgent::Off);
    open(&mut r.app);
    let ask = find_by_label(&mut r.app, "Custom command…").unwrap_or_else(|e| panic!("the row offers a way to set a command: {e}"));
    click(&mut r.app, ask);
    updates(&mut r.app, 4);
    let field = {
        let world = r.app.world_mut();
        world
            .query::<(Entity, &TextHint)>()
            .iter(world)
            .find(|(_, hint)| hint.0.starts_with("a program"))
            .map(|(entity, _)| entity)
            .expect("the row opened a field for the command")
    };
    r.app.world_mut().commands().trigger(bevy::ui_widgets::ValueChange { source: field, value: "claude --mcp-config $CAULDRON_MCP_CONFIG".to_owned(), is_final: true });
    updates(&mut r.app, 4);
    let saved = layer(&r.user);
    assert_eq!(saved.terminal_agent, Some(TerminalAgent::Custom), "the choice is saved with its command");
    assert_eq!(saved.terminal_agent_command.as_deref(), Some("claude --mcp-config $CAULDRON_MCP_CONFIG"));
}

/// **Booting with `terminal_agent: Custom` and no command shows one settings problem naming the file, and the fix sets omp.**
#[test]
fn a_file_with_custom_and_no_command_is_one_problem_naming_the_file_and_the_fix_sets_omp() {
    let mut r = rig("custom_boot", Some("{version: 1, terminal_agent: Custom}"), TerminalAgent::OhMyPi);
    until(&mut r.app, "the settings problem", |app| !reload_problems(app, "editor.yaml").is_empty());
    let problems = reload_problems(&r.app, "editor.yaml");
    assert_eq!(problems.len(), 1, "one settings problem, not one per frame: {problems:?}");
    let (file, reason) = &problems[0];
    assert!(file.ends_with("editor.yaml"), "it names the file: {file}");
    assert!(reason.contains("terminal_agent_command") && reason.contains("Fix"), "and the fix: {reason}");
    assert_eq!(r.app.world().resource::<EditorReloads>().kinds().iter().filter(|k| k.outcome == Outcome::Refused).count(), 1, "and nothing else is refused");
    assert_eq!(r.app.world().resource::<EditorSettings>().terminal_agent, TerminalAgent::Custom, "the file is never silently replaced");

    open(&mut r.app);
    let fix = find_by_label(&mut r.app, "Use Oh My Pi, which is installed").unwrap_or_else(|e| panic!("the row shows the one-press fix: {e}"));
    click(&mut r.app, fix);
    updates(&mut r.app, 4);
    assert_eq!(layer(&r.project).terminal_agent, Some(TerminalAgent::OhMyPi), "the fix is written to the file that had the problem");
    until(&mut r.app, "the problem to clear", |app| reload_problems(app, "editor.yaml").is_empty());
    assert_eq!(r.app.world().resource::<EditorSettings>().terminal_agent, TerminalAgent::OhMyPi);
}

/// **A project that chooses Custom is not a problem when the user's file holds the command**, and with nothing saved the default is
/// the machine's, as #341 made it.
#[test]
fn a_command_in_the_other_file_is_no_problem_and_nothing_saved_is_the_machines_choice() {
    let mut r = rig("custom_layers", Some("{version: 1, terminal_agent: Custom}"), TerminalAgent::OhMyPi);
    std::fs::write(&r.user, "{version: 1, terminal_agent_command: \"claude\"}").unwrap_or_else(|e| panic!("{e}"));
    until(&mut r.app, "the user's command to settle the problem", |app| reload_problems(app, "editor.yaml").is_empty() && app.world().resource::<EditorSettings>().terminal_agent_command.is_some());
    assert_eq!(r.app.world().resource::<EditorSettings>().terminal_agent, TerminalAgent::Custom);

    let plain = rig("custom_default", None, TerminalAgent::OhMyPi);
    assert_eq!(plain.app.world().resource::<EditorSettings>().terminal_agent, TerminalAgent::OhMyPi, "nothing saved: the machine's agent");
    assert!(reload_problems(&plain.app, "settings.yaml").is_empty() && reload_problems(&plain.app, "editor.yaml").is_empty());
}
