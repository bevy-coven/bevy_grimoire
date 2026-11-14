//! **The terminal is on every screen from the moment the application opens, and Primary+J collapses it**
//! (#314, #315, #316 — the owner's ask of 2026-11-06: "the terminal is the centerpiece of work. It should
//! be on every screen from the moment you open the application, but it should be easily collapsible by
//! hitting Command J or Control J").
//!
//! These drive the real screens — the projects home, the project page, the loading screen and the
//! editor — through their own plugins, and read the pane's node, the shell's screen and gooey's focus.
//! The shell is a POSIX `sh`, pinned, so a `fish` greeting or terminal query never stands in front of
//! the assertion.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::keys::MOD_KEYS;
use bevy_cauldron::launcher::{LauncherPlugin, LauncherRoot};
use bevy_cauldron::screen::Screen;
use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_pane::{TerminalBody, TerminalFocus, TerminalPaneRoot};

use crate::fixtures;

/// How many 50 ms checks a test waits for the shell before calling it absent.
const TRIES: usize = 80;

/// Frames for a screen change to tear down one screen and build the next.
const SCREEN_FRAMES: usize = 6;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// The application as it opens on a project, with a window and every screen's plugin.
fn the_application(tag: &str) -> App {
    the_application_and_its_project(tag).0
}

/// The same, and the project's folder.
fn the_application_and_its_project(tag: &str) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .build("m");
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(PosixShell(guard));
    app.add_plugins((LauncherPlugin, bevy_cauldron::loading::LoadingPlugin));
    app.world_mut().spawn((
        Window {
            resolution: bevy::window::WindowResolution::new(1280, 720),
            ..default()
        },
        PrimaryWindow,
    ));
    settle(&mut app, 10);
    (app, root)
}

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

fn go_to(app: &mut App, screen: Screen) {
    app.world_mut().resource_mut::<NextState<Screen>>().set(screen);
    settle(app, SCREEN_FRAMES);
}

fn window(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("one primary window")
}

fn send(app: &mut App, key_code: KeyCode, logical_key: Key, state: ButtonState) {
    let window = window(app);
    app.world_mut().write_message(KeyboardInput {
        key_code,
        text: match (&logical_key, state) {
            (Key::Character(c), ButtonState::Pressed) => Some(c.clone()),
            _ => None,
        },
        logical_key,
        state,
        repeat: false,
        window,
    });
}

/// One key, down then up, a frame each.
fn tap(app: &mut App, key_code: KeyCode, logical_key: Key) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, key_code, logical_key.clone(), state);
        app.update();
    }
}

/// **The platform's primary modifier and `key`**, as a hand makes it: Command on a mac, Ctrl elsewhere.
/// A letter carries its character, as a keyboard reports it, so a chord that leaked would reach the
/// shell as that letter's control code (Ctrl+J is a newline).
fn primary(app: &mut App, key: KeyCode) {
    let unidentified = || Key::Unidentified(NativeKey::Unidentified);
    let logical = match key {
        KeyCode::KeyJ => Key::Character("j".into()),
        _ => unidentified(),
    };
    for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, MOD_KEYS[0], unidentified(), state);
        send(app, key, logical.clone(), state);
        app.update();
    }
    settle(app, 2);
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        let s = c.to_string();
        tap(app, KeyCode::Space, Key::Character(s.as_str().into()));
    }
}

/// The one pane on screen, and whether it is raised.
fn pane(app: &mut App) -> (Entity, bool) {
    let world = app.world_mut();
    let panes: Vec<(Entity, bool)> = world
        .query_filtered::<(Entity, &Node), With<TerminalPaneRoot>>()
        .iter(world)
        .map(|(entity, node)| (entity, node.display != Display::None))
        .collect();
    assert_eq!(panes.len(), 1, "every screen has exactly one terminal pane: {panes:?}");
    panes[0]
}

fn is_under(world: &World, mut entity: Entity, root: Entity) -> bool {
    while let Some(parent) = world.get::<ChildOf>(entity) {
        entity = parent.parent();
        if entity == root {
            return true;
        }
    }
    false
}

/// The pane is a row of `screen`'s own root — the home's, the page's, the loader's or the editor frame's.
fn pane_is_under_a_root_named(app: &mut App, name: &str) -> bool {
    let (pane, _) = pane(app);
    let world = app.world_mut();
    let roots: Vec<Entity> = world
        .query_filtered::<(Entity, &Name), Without<ChildOf>>()
        .iter(world)
        .filter(|(_, n)| n.as_str() == name)
        .map(|(e, _)| e)
        .collect();
    roots.iter().any(|root| is_under(app.world(), pane, *root))
}

/// The shell's screen as text, one row a line.
fn screen_text(app: &App) -> String {
    let Some(tab) = app.world().resource::<Terminal>().tab() else {
        return String::new();
    };
    tab.screen()
        .built_lines()
        .iter()
        .map(|line| line.cells.iter().map(|c| c.ch).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Everything the pane draws, as the spans' words — what a person reads.
fn drawn_text(app: &mut App) -> String {
    let world = app.world_mut();
    let Some(body) = world.query_filtered::<Entity, With<TerminalBody>>().iter(world).next() else {
        return String::new();
    };
    let spans: Vec<(Entity, String)> = world
        .query::<(Entity, &TextSpan)>()
        .iter(world)
        .map(|(e, s)| (e, s.0.clone()))
        .collect();
    spans
        .into_iter()
        .filter(|(e, _)| is_under(app.world(), *e, body))
        .map(|(_, s)| s)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Wait until `test` holds of the app, stepping it, or panic with `why` and what the shell shows.
fn until(app: &mut App, why: &str, test: impl Fn(&mut App) -> bool) {
    for _ in 0..TRIES {
        settle(app, 2);
        if test(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("{why}. The shell shows: {:?}", screen_text(app));
}

fn holds(app: &App) -> bool {
    app.world().resource::<TerminalFocus>().holds()
}

/// **The pane starts collapsed** (#424, replacing #314's raised default), so a test that works in the shell raises it first, as a
/// person does with Primary+J.
fn raise(app: &mut App) {
    app.world_mut().resource_mut::<Terminal>().set_collapsed(false);
    settle(app, 3);
}

#[test]
fn a_raised_terminal_is_on_the_home_with_a_shell_running() {
    let mut app = the_application("terminal_on_the_home");
    // Owner exception (2026-11-14): this test pinned "raised on start" (#314); #424 starts the pane collapsed (asserted on the home by
    // `primary_j_collapses_the_terminal_on_every_screen...`), so here the person raises it on the door, as they would, and the rest holds.
    raise(&mut app);
    until(&mut app, "the door's terminal never started a shell", |app| !app.world().resource::<Terminal>().is_empty());
    go_to(&mut app, Screen::Launcher);
    assert!(
        pane_is_under_a_root_named(&mut app, "projects home"),
        "the projects home has no terminal docked in it"
    );
    assert!(pane(&mut app).1, "a raised terminal is raised on the home, not waiting for a chord");
    until(&mut app, "the home's terminal never started a shell", |app| {
        !app.world().resource::<Terminal>().is_empty() && screen_text(app).contains('$')
    });
    assert!(!holds(&app), "the terminal opens without the keyboard, so the home's own keys work");
    assert!(
        app.world_mut().query::<&LauncherRoot>().iter(app.world()).count() == 1,
        "the home is still the home"
    );
}

#[test]
fn the_editor_opens_with_the_terminal_collapsed_and_every_screen_shows_the_same_scrollback() {
    let (mut app, project) = the_application_and_its_project("terminal_every_screen");
    // The harness stands on a door: the editor's own frame has the pane, collapsed, with no chord pressed (#424; this test pinned "raised").
    assert!(!pane(&mut app).1, "the editor's terminal starts collapsed");
    raise(&mut app);
    assert!(pane(&mut app).1, "and is raised once the person raises it");
    until(&mut app, "the editor's terminal never started a shell", |app| screen_text(app).contains('$'));
    assert!(!holds(&app), "a door opens without the terminal holding the keyboard");

    app.world_mut().resource_mut::<Terminal>().tab_mut().expect("a tab").screen_mut().feed(b"\r\nMARK-314\r\n");
    settle(&mut app, 4);
    for (screen, root) in [(Screen::Project, "projects home"), (Screen::Launcher, "projects home"), (Screen::Loading, "loading")] {
        if screen == Screen::Project {
            bevy_cauldron::harness::stand_on_the_project_page(&mut app, &project);
            settle(&mut app, SCREEN_FRAMES);
        } else {
            go_to(&mut app, screen);
        }
        assert!(pane_is_under_a_root_named(&mut app, root), "{screen:?} has no terminal docked in its root");
        assert!(pane(&mut app).1, "the terminal is raised on {screen:?}");
        until(&mut app, &format!("{screen:?}'s pane never drew the scrollback the editor's had"), |app| {
            drawn_text(app).contains("MARK-314")
        });
    }
}

#[test]
fn primary_j_collapses_the_terminal_on_every_screen_and_brings_it_back_with_the_keys() {
    let mut app = the_application("terminal_primary_j");
    go_to(&mut app, Screen::Launcher);
    assert!(!pane(&mut app).1, "collapsed to begin with (#424; this test pinned raised)");
    raise(&mut app);
    assert!(pane(&mut app).1, "raised");

    primary(&mut app, KeyCode::KeyJ);
    assert!(!pane(&mut app).1, "Primary+J collapses the terminal on the home");
    assert!(!holds(&app), "a collapsed terminal does not keep the keyboard");

    // The collapse is the person's, not the screen's: the next screen's pane is built collapsed.
    go_to(&mut app, Screen::Loading);
    assert!(!pane(&mut app).1, "a terminal collapsed on the home is collapsed on the next screen too");

    primary(&mut app, KeyCode::KeyJ);
    assert!(pane(&mut app).1, "Primary+J brings the terminal back");
    assert!(holds(&app), "and puts the keyboard in the shell, because a person who restores it means to type");

    primary(&mut app, KeyCode::KeyJ);
    assert!(!pane(&mut app).1, "and collapses it again, keyboard in the shell or not");
}

#[test]
fn primary_j_never_reaches_the_shell() {
    let mut app = the_application("terminal_j_not_a_newline");
    raise(&mut app);
    until(&mut app, "no shell", |app| screen_text(app).contains('$'));
    app.world_mut().resource_mut::<TerminalFocus>().take();
    settle(&mut app, 2);
    type_text(&mut app, "echo A");
    // Collapse and restore in the middle of the line. Had Ctrl+J reached the shell it would be a newline,
    // and `echo A` would run on its own before the `B` arrived.
    primary(&mut app, KeyCode::KeyJ);
    primary(&mut app, KeyCode::KeyJ);
    assert!(holds(&app), "restored with the keys");
    type_text(&mut app, "B");
    tap(&mut app, KeyCode::Enter, Key::Enter);
    until(&mut app, "`echo AB` never answered", |app| {
        screen_text(app).lines().any(|line| line.trim_end() == "AB")
    });
    assert!(
        !screen_text(&app).lines().any(|line| line.trim_end() == "A"),
        "the shell ran `echo A` on its own: the collapse chord reached it as a newline"
    );
}

#[test]
fn the_home_does_not_act_on_keys_the_shell_has() {
    let mut app = the_application("terminal_holds_the_keys");
    raise(&mut app);
    go_to(&mut app, Screen::Launcher);
    until(&mut app, "no shell", |app| screen_text(app).contains('$'));
    app.world_mut().resource_mut::<TerminalFocus>().take();
    settle(&mut app, 2);
    type_text(&mut app, "cat -v");
    tap(&mut app, KeyCode::Enter, Key::Enter);
    until(&mut app, "`cat -v` was never echoed", |app| screen_text(app).contains("cat -v"));
    std::thread::sleep(std::time::Duration::from_millis(300));

    let body = app
        .world_mut()
        .query_filtered::<Entity, With<TerminalBody>>()
        .iter(app.world())
        .next()
        .expect("the home's terminal has a body");
    for (key, logical) in [(KeyCode::ArrowDown, Key::ArrowDown), (KeyCode::Tab, Key::Tab), (KeyCode::ArrowUp, Key::ArrowUp)] {
        tap(&mut app, key, logical);
        let focus = app.world().resource::<InputFocus>().get();
        assert!(
            focus.is_none_or(|f| f == body),
            "{key:?} typed into the shell moved the home's focus to {focus:?}: the screen's navigation acted on a \
             key the terminal held"
        );
        assert!(holds(&app), "the terminal still has the keyboard after {key:?}");
    }
    tap(&mut app, KeyCode::Enter, Key::Enter);
    // The shell heard them: `cat -v` prints Down, Tab and Up as it received them.
    until(&mut app, "the shell never received the arrows", |app| {
        let text = screen_text(app);
        text.contains("^[[B") && text.contains("^[[A")
    });
    let focus = app.world().resource::<InputFocus>().get();
    assert!(
        focus.is_none_or(|f| f == body),
        "Enter typed into the shell landed on the home's widget {focus:?}"
    );
}
