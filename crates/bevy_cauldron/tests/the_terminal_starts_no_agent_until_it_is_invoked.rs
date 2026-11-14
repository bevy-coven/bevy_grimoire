//! **The terminal starts no agent by itself** (`terminal_agent`). A session costs tokens to warm up, so
//! with an agent chosen the terminal opens empty and shows an `Invoke Agent` button in the middle of its
//! area; the press starts the agent, and a person who is only looking around the game pays nothing.
//!
//! The project's settings choose a custom command, so the harness is a `printf` the shell really runs: if
//! anything started it, its output would be on a screen. The press goes through `bevy_picking`'s own
//! input on the image the editor draws into, as `a_click_puts_the_keyboard_in_the_terminal` does.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_agent::AgentSetup;
use bevy::window::PrimaryWindow;
use bevy_cauldron::terminal_pane::{TerminalBody, TerminalCloseTab, TerminalFocus, TerminalInvokeButton};

use crate::fixtures;

/// How many frames, 50 ms apart, a test waits for something to happen.
const TRIES: usize = 100;
/// Frames a test lets the editor run to be sure something does *not* happen: two seconds, far longer than
/// the first frame that raised the pane and opened its shell before this change.
const QUIET_FRAMES: usize = 40;
/// What the harness prints. Its command line spells it apart (`AGENT-%s`), so the echo of the typed line
/// is not mistaken for its output.
const SAID: &str = "AGENT-UP";
/// How far the button's centre may be from the body's, logical px.
const CENTRE_TOLERANCE_PX: f32 = 1.5;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// The editor on a project, whose settings are `settings` when given.
fn editor(tag: &str, settings: Option<&str>) -> App {
    let tag = format!("{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("floor", "alpha").build("m");
    if let Some(settings) = settings {
        let file = root.join("assets/grimoire/editor.yaml");
        std::fs::write(&file, settings).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    }
    let files = std::env::temp_dir().join(format!("{tag}_agents"));
    let server = std::env::temp_dir().join(format!("{tag}_server"));
    std::fs::write(&server, "").unwrap_or_else(|e| panic!("{}: {e}", server.display()));
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<bevy_cauldron::terminal::Terminal>().set_collapsed(false); // #424: the pane starts collapsed; this test needs it raised
    app.insert_non_send(PosixShell(guard));
    app.insert_resource(AgentSetup { port: Some(15799), server, files });
    app
}

/// The editor on a project that chooses an agent whose command prints `AGENT-UP`.
fn editor_with_an_agent(tag: &str) -> App {
    editor(
        tag,
        Some(r#"{version: 1, terminal_agent: Custom, terminal_agent_command: "printf 'AGENT-%s\\n' UP"}"#),
    )
}

fn until(app: &mut App, why: &str, test: impl Fn(&mut App) -> bool) {
    for _ in 0..TRIES {
        for _ in 0..2 {
            app.update();
        }
        if test(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("{why}");
}

fn quiet(app: &mut App) {
    for _ in 0..QUIET_FRAMES {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

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

/// One pointer action at `at`, in the physical pixels of the image the editor draws its interface into.
fn point(app: &mut App, at: Vec2, action: PointerAction) {
    let handle = app.world().get_resource::<bevy_cauldron::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
    let location = Location { target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }), position: at };
    app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
    app.update();
}

/// A primary click at `at`.
fn click(app: &mut App, at: Vec2) {
    for action in [
        PointerAction::Move { delta: Vec2::ZERO },
        PointerAction::Press(PointerButton::Primary),
        PointerAction::Release(PointerButton::Primary),
    ] {
        point(app, at, action);
    }
}

/// Where the `Invoke Agent` button stands, once it is laid out; `None` while there is none.
fn the_button(app: &mut App) -> Option<Vec2> {
    let world = app.world_mut();
    world.query_filtered::<&UiGlobalTransform, With<TerminalInvokeButton>>().iter(world).next().map(|at| at.translation)
}

/// The middle of the terminal's body, where the rows are drawn.
fn the_body_centre(app: &mut App) -> Vec2 {
    let world = app.world_mut();
    world
        .query_filtered::<&UiGlobalTransform, With<TerminalBody>>()
        .iter(world)
        .next()
        .map(|at| at.translation)
        .expect("the terminal has a body")
}

#[test]
fn an_agent_in_the_settings_starts_nothing_and_the_press_starts_it() {
    let mut app = editor_with_an_agent("invoke_agent");
    until(&mut app, "the Invoke Agent button was never drawn", |app| the_button(app).is_some());
    quiet(&mut app);
    {
        let terminal = app.world().resource::<Terminal>();
        assert_eq!(terminal.len(), 0, "no shell, and so no agent, opened by itself");
        assert!(terminal.invites_the_agent());
    }
    assert!(!screen_text(&app).contains(SAID), "nothing ran the harness");

    // **In the middle of the terminal's area.**
    let button = the_button(&mut app).expect("the button is still there after the quiet");
    let centre = the_body_centre(&mut app);
    assert!(
        (button - centre).abs().max_element() <= CENTRE_TOLERANCE_PX,
        "the button {button:?} stands in the middle of the body {centre:?}"
    );

    click(&mut app, button);
    until(&mut app, "the press never opened a tab", |app| app.world().resource::<Terminal>().len() == 1);
    until(&mut app, "the custom command was never typed into the new tab's shell", |app| {
        screen_text(app).lines().any(|line| line.trim() == SAID)
    });
    let terminal = app.world().resource::<Terminal>();
    assert!(terminal.tab().is_some_and(|tab| tab.agent().is_some()), "the tab the press opened is an agent tab");
    assert!(!terminal.invites_the_agent());
    assert!(the_button(&mut app).is_none(), "the button goes once there is an agent to talk to");
}

#[test]
fn closing_the_agent_tab_brings_the_button_back_and_starts_no_shell() {
    let mut app = editor_with_an_agent("invoke_agent_closed");
    until(&mut app, "the Invoke Agent button was never drawn", |app| the_button(app).is_some());
    let button = the_button(&mut app).expect("the button stands");
    click(&mut app, button);
    until(&mut app, "the press never opened a tab", |app| app.world().resource::<Terminal>().len() == 1);

    let close = {
        let world = app.world_mut();
        world
            .query_filtered::<&UiGlobalTransform, With<TerminalCloseTab>>()
            .iter(world)
            .next()
            .map(|at| at.translation)
            .expect("the bar has a ×")
    };
    click(&mut app, close);
    until(&mut app, "closing the only tab never emptied the terminal", |app| app.world().resource::<Terminal>().is_empty());
    until(&mut app, "the button never came back", |app| the_button(app).is_some());
    quiet(&mut app);
    assert_eq!(app.world().resource::<Terminal>().len(), 0, "an empty terminal with an agent chosen opens no shell by itself");
}

#[test]
fn with_no_agent_chosen_the_terminal_opens_its_shell_and_offers_no_button() {
    let mut app = editor("invoke_no_agent", None);
    until(&mut app, "the terminal never opened its shell", |app| app.world().resource::<Terminal>().len() == 1);
    let terminal = app.world().resource::<Terminal>();
    assert!(terminal.tab().is_some_and(|tab| tab.agent().is_none()), "a plain shell");
    assert!(!terminal.invites_the_agent());
    assert!(the_button(&mut app).is_none(), "no agent is chosen, so there is nothing to invoke");
}

/// One key, pressed and let go, on the primary window.
fn tap(app: &mut App, key_code: KeyCode, logical_key: Key) {
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("one primary window");
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key: logical_key.clone(),
            state,
            text: None,
            repeat: false,
            window,
        });
        app.update();
    }
}

#[test]
fn enter_starts_the_agent_for_a_person_at_the_keyboard_and_only_when_the_terminal_has_the_keys() {
    let mut app = editor_with_an_agent("invoke_agent_enter");
    // Keys arrive on a window, which the headless editor has not spawned.
    app.world_mut().spawn((Window { resolution: bevy::window::WindowResolution::new(1280, 720), ..default() }, PrimaryWindow));
    until(&mut app, "the Invoke Agent button was never drawn", |app| the_button(app).is_some());

    // Keys that are not the terminal's start nothing, whatever they are.
    app.world_mut().resource_mut::<TerminalFocus>().release();
    tap(&mut app, KeyCode::Enter, Key::Enter);
    quiet(&mut app);
    assert_eq!(app.world().resource::<Terminal>().len(), 0, "Enter outside the terminal starts nothing");

    app.world_mut().resource_mut::<TerminalFocus>().take();
    tap(&mut app, KeyCode::Space, Key::Unidentified(NativeKey::Unidentified));
    assert_eq!(app.world().resource::<Terminal>().len(), 0, "only Enter presses the button");

    tap(&mut app, KeyCode::Enter, Key::Enter);
    until(&mut app, "Enter in the terminal never opened the agent's tab", |app| app.world().resource::<Terminal>().len() == 1);
    until(&mut app, "the custom command was never typed into the new tab's shell", |app| {
        screen_text(app).lines().any(|line| line.trim() == SAID)
    });
    assert!(app.world().resource::<Terminal>().tab().is_some_and(|tab| tab.agent().is_some()));
}
