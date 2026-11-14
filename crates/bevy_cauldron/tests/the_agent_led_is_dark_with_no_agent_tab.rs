//! **The agent's LED is in the bar before any agent tab opens, dark** (#343, the last line of its acceptance: *"the LED's four
//! states each drawn"*; `Off` is the one the LED never showed).
//!
//! An agent is chosen in the settings but nothing has been invoked: the bar already holds the LED, as a hollow ring that says
//! `off`. It lights (`busy`) the moment the agent's tab opens, and it is one LED throughout, not a second one beside the first.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_agent::AgentSetup;
use bevy_cauldron::terminal_pane::{TerminalAgentCard, TerminalBar, TerminalInvokeButton};
use bevy_gooey::{Led, LedState, LedWord};

use crate::fixtures;

/// How many frames, 50 ms apart, a test waits.
const TRIES: usize = 200;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// The editor on a project whose settings choose a custom agent, with an agent setup whose server is a
/// file that exists.
fn editor_with_an_agent() -> App {
    let tag = format!("agent_led_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("floor", "alpha").build("m");
    let file = root.join("assets/grimoire/editor.yaml");
    let settings = r#"{version: 1, terminal_agent: Custom, terminal_agent_command: "printf 'AGENT-%s\\n' UP"}"#;
    std::fs::write(&file, settings).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
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

/// One pointer action at `at`, in the physical pixels of the image the editor draws its interface into.
fn point(app: &mut App, at: Vec2, action: PointerAction) {
    let handle = app.world().get_resource::<bevy_cauldron::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
    let location = Location { target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }), position: at };
    app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
    app.update();
}

/// **The terminal starts no agent by itself** (it costs tokens to warm up), so the test presses `Invoke
/// Agent` the way a person does, through the pointer chain, before it looks at the agent's tab.
fn invoke_the_agent(app: &mut App) {
    let button = |app: &mut App| {
        let world = app.world_mut();
        world.query_filtered::<&UiGlobalTransform, With<TerminalInvokeButton>>().iter(world).next().map(|at| at.translation)
    };
    until(app, "the Invoke Agent button was never drawn", |app| button(app).is_some());
    let at = button(app).expect("the button stands");
    for action in [
        PointerAction::Move { delta: Vec2::ZERO },
        PointerAction::Press(PointerButton::Primary),
        PointerAction::Release(PointerButton::Primary),
    ] {
        point(app, at, action);
    }
}

/// Where an LED stands: in the terminal's bar, or on the self-test card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Home {
    Bar,
    Card,
}

/// Is `entity` at or below an entity that has `M`?
fn is_under<M: Component>(world: &World, entity: Entity) -> bool {
    let mut at = Some(entity);
    while let Some(current) = at {
        if world.get::<M>(current).is_some() {
            return true;
        }
        at = world.get::<ChildOf>(current).map(ChildOf::parent);
    }
    false
}

/// Every LED the editor draws, with where it stands and what state it holds, in no order.
fn leds(app: &mut App) -> Vec<(Entity, Home, LedState)> {
    let world = app.world_mut();
    let found: Vec<(Entity, LedState)> = world.query::<(Entity, &Led)>().iter(world).map(|(entity, led)| (entity, led.0)).collect();
    found
        .into_iter()
        .filter_map(|(entity, state)| {
            if is_under::<TerminalAgentCard>(world, entity) {
                Some((entity, Home::Card, state))
            } else if is_under::<TerminalBar>(world, entity) {
                Some((entity, Home::Bar, state))
            } else {
                None
            }
        })
        .collect()
}

/// The LED standing in `home`, if one does.
fn led_in(app: &mut App, home: Home) -> Option<(Entity, LedState)> {
    leds(app).into_iter().find(|(_, at, _)| *at == home).map(|(entity, _, state)| (entity, state))
}

/// What an LED's state word says, drawn beside its dot.
fn word_of(app: &mut App, led: Entity) -> Option<String> {
    let world = app.world_mut();
    let children = world.get::<Children>(led)?.to_vec();
    children.iter().find_map(|child| world.get::<LedWord>(*child).and_then(|_| world.get::<Text>(*child)).map(|text| text.0.clone()))
}

/// What the shown tab's screen says. The agent's command prints `AGENT-UP`.
fn the_screen(app: &mut App) -> String {
    let mut terminal = app.world_mut().resource_mut::<Terminal>();
    let Some(tab) = terminal.tab_mut() else {
        return String::new();
    };
    tab.screen_mut().lines().iter().map(|line| line.cells.iter().map(|c| c.ch).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn bar_leds(app: &mut App) -> Vec<LedState> {
    leds(app).into_iter().filter(|(_, home, _)| *home == Home::Bar).map(|(_, _, state)| state).collect()
}

#[test]
fn the_bar_draws_a_dark_agent_led_before_any_agent_tab() {
    let mut app = editor_with_an_agent();
    until(&mut app, "the Invoke Agent button was never drawn", |app| {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<TerminalInvokeButton>>().iter(world).next().is_some()
    });
    assert_eq!(bar_leds(&mut app), vec![LedState::Off], "one LED in the bar, dark, before any agent tab");
    let (led, state) = led_in(&mut app, Home::Bar).expect("the bar's LED");
    assert_eq!(state, LedState::Off);
    assert_eq!(word_of(&mut app, led).as_deref(), Some("off"), "the word beside a dark LED");
}

#[test]
fn the_dark_led_lights_when_the_agent_tab_opens() {
    let mut app = editor_with_an_agent();
    invoke_the_agent(&mut app);
    until(&mut app, "the bar's LED never lit when the agent's tab opened", |app| bar_leds(app) == vec![LedState::Busy]);
    assert_eq!(bar_leds(&mut app).len(), 1, "still one LED in the bar");
    // **The shell has started before the test ends.** An `App` dropped while its agent's shell is still starting waits for a child
    // that the pty will not let go of (the test then never returns), so the test lets the command run first.
    until(&mut app, "the agent's command never ran", |app| the_screen(app).contains("AGENT-UP"));
}
