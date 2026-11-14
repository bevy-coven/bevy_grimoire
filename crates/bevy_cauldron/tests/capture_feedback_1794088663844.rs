//! **The terminal's bar does not offer a second door to the agent the empty pane already offers** (feedback-1794088663844;
//! oracle `spec`).
//!
//! The note, from the person who had chosen `omp`: *"What are these chips? Why does having a default agent bring on "omp" and
//! then there is an "Agent" and another "omp". We should only have agent panel."* With an agent chosen the empty terminal
//! holds one control that starts it, the `Invoke Agent` button in the middle of the pane; the bar had a second, an `Agent`
//! chip, that started the same agent. The chip is for adding *another* agent tab, so it appears once something is open and
//! goes when the last tab is closed and the button is back.
//!
//! (The default for the agent setting is `Off`, and `the_terminal_agent_is_a_setting` pins it; the note's other request, that
//! the default be an `omp` agent, needs the owner's word to change that assertion and is not part of this fix.)

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_agent::AgentSetup;
use bevy_cauldron::terminal_pane::{TerminalAgentChip, TerminalCloseTab, TerminalInvokeButton, TerminalNewTab};

use crate::fixtures;

/// How many frames, 50 ms apart, a test waits for something to happen.
const TRIES: usize = 100;
/// Frames a test lets the editor run to be sure something does *not* happen: two seconds, far longer than
/// the first frame that raised the pane and opened its shell before this change.
const QUIET_FRAMES: usize = 40;

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


fn chips(app: &mut App) -> usize {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<TerminalAgentChip>>().iter(world).count()
}

#[test]
fn an_empty_pane_with_an_agent_chosen_has_the_button_and_no_second_agent_control() {
    let mut app = editor_with_an_agent("chips_empty");
    until(&mut app, "the Invoke Agent button was never drawn", |app| the_button(app).is_some());
    quiet(&mut app);
    assert!(app.world().resource::<Terminal>().invites_the_agent());
    assert_eq!(chips(&mut app), 0, "the one control that starts the agent is the button; the bar offers no `Agent` chip beside it");
}

#[test]
fn the_agent_chip_comes_with_the_first_tab_and_goes_when_the_last_is_closed() {
    let mut app = editor_with_an_agent("chips_tab");
    until(&mut app, "the Invoke Agent button was never drawn", |app| the_button(app).is_some());
    let button = the_button(&mut app).expect("the button stands");
    click(&mut app, button);
    until(&mut app, "the press never opened a tab", |app| app.world().resource::<Terminal>().len() == 1);
    until(&mut app, "the bar never offered another agent", |app| chips(app) == 1);
    assert!(the_button(&mut app).is_none(), "and the button is gone");

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
    until(&mut app, "the `Agent` chip stayed beside the button", |app| chips(app) == 0);
}

#[test]
fn with_a_plain_shell_open_the_chip_offers_the_first_agent() {
    let mut app = editor_with_an_agent("chips_shell");
    until(&mut app, "the Invoke Agent button was never drawn", |app| the_button(app).is_some());
    // The `+` chip opens a plain shell, as it always did.
    let plus = {
        let world = app.world_mut();
        world.query_filtered::<&UiGlobalTransform, With<TerminalNewTab>>().iter(world).next().map(|at| at.translation).expect("the bar has a +")
    };
    click(&mut app, plus);
    until(&mut app, "the + never opened a shell", |app| app.world().resource::<Terminal>().len() == 1);
    until(&mut app, "the bar offered no agent beside a plain shell", |app| chips(app) == 1);
}
