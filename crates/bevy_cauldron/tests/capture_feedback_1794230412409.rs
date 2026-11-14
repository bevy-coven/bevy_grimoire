//! **The agent's connection has an LED, in the bar and on the self-test card** (feedback-1794230412409; oracle `spec`).
//!
//! The note, on the card an `omp` agent's self test draws over the terminal, a second time (the first, feedback-1794089256357,
//! asked for *"a glowing indicator virtual LED stating whether something is live"* and got the card's rows on one line only):
//! *"This still needs work, where are the LEDs?"* `bevy_gooey` has had the widget since v0.45.0 (`led(label, state)`, #80,
//! asked for by bevy_grimoire#343): a dot with four silhouettes (a ring, a disc, a rounded square, a diamond) and its state in
//! words, so colour is never the only signal.
//!
//! What this holds, as a person sees it:
//!
//! * a lit LED stands in the terminal's bar beside the status chip, and another ends the card, while an agent tab is drawn;
//! * it says **busy** while a check is still waiting, **live** once every check passed or was skipped, **failed** the moment one
//!   failed: the same three answers the status chip's words give;
//! * each is drawn, not merely present: the dot has a size and a glow, and the word stands beside it;
//! * the bar's LED stays when the card has gone, which is the point of an indicator.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use serde_json::json;

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_agent::{self, AgentSetup};
use bevy_cauldron::terminal_pane::{TerminalAgentCard, TerminalAgentCardLed, TerminalAgentStatusChip, TerminalBar, TerminalInvokeButton};
use bevy_gooey::{Led, LedState, LedWord, Role};

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

fn token(app: &App) -> String {
    let terminal = app.world().resource::<Terminal>();
    terminal.tab().and_then(|tab| tab.agent()).map(|post| post.token.clone()).expect("the first tab is the agent's")
}

fn now(app: &App) -> f64 {
    app.world().resource::<Time<Real>>().elapsed_secs_f64()
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
            // Owner exception (2026-11-14): every card row carries its own LED now (#421), so "the card's LED" is the verdict LED at its foot.
            if world.get::<TerminalAgentCardLed>(entity).is_some() {
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

/// Do both homes hold an LED in `state`?
fn both_are(app: &mut App, state: LedState) -> bool {
    led_in(app, Home::Bar).is_some_and(|(_, bar)| bar == state) && led_in(app, Home::Card).is_some_and(|(_, card)| card == state)
}

/// What an LED's state word says, drawn beside its dot.
fn word_of(app: &mut App, led: Entity) -> Option<String> {
    let world = app.world_mut();
    let children = world.get::<Children>(led)?.to_vec();
    children.iter().find_map(|child| world.get::<LedWord>(*child).and_then(|_| world.get::<Text>(*child)).map(|text| text.0.clone()))
}

/// **The dot as drawn**: its laid-out size in physical px, and how many glow layers it carries.
fn dot_of(app: &mut App, led: Entity) -> Option<(Vec2, usize)> {
    let world = app.world_mut();
    let children = world.get::<Children>(led)?.to_vec();
    let dot = children.into_iter().find(|child| world.get::<Role>(*child) == Some(&Role::LedDot))?;
    let size = world.get::<ComputedNode>(dot)?.size();
    let glow = world.get::<BoxShadow>(dot).map_or(0, |shadow| shadow.0.len());
    Some((size, glow))
}

fn centre_of(app: &mut App, entity: Entity) -> Vec2 {
    app.world().get::<UiGlobalTransform>(entity).expect("placed").translation
}

fn chip_entity(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<TerminalAgentStatusChip>>().iter(world).next().expect("the bar's status chip")
}

/// An agent tab's three results, with the self test passing or failing.
fn hello_and_self_test(app: &mut App, passing: bool) {
    let token = token(app);
    let at = now(app);
    let known = terminal_agent::agent_hello(
        &mut app.world_mut().resource_mut::<Terminal>(),
        &json!({"token": token, "client": {"name": "omp-coding-agent", "version": "1.0.0"}, "tools": 22}),
        at,
    );
    assert_eq!(known, Ok(json!({"known": true})));
    let checks = json!([
        {"name": "state", "ok": true, "detail": "the editor is on launcher"},
        {"name": "operations", "ok": true, "detail": "88 operations"},
        {"name": "picture", "ok": passing, "detail": if passing { "a 2560x1406 picture" } else { "the picture came back empty" }},
    ]);
    let known = terminal_agent::self_test_result(&mut app.world_mut().resource_mut::<Terminal>(), &json!({"token": token, "checks": checks}));
    assert_eq!(known, Ok(json!({"known": true})));
}

#[test]
fn the_agents_led_is_busy_while_it_checks_then_live_in_the_bar_and_on_the_card() {
    let mut app = editor_with_an_agent();
    invoke_the_agent(&mut app);

    // The card is up and a check is still waiting: both LEDs are there, and both say so.
    until(&mut app, "no LED stood in both the bar and on the card while the agent was checking", |app| both_are(app, LedState::Busy));
    let (bar, _) = led_in(&mut app, Home::Bar).expect("the bar's LED");
    assert_eq!(word_of(&mut app, bar).as_deref(), Some("busy"), "the word beside a busy LED");

    // The agent connects and its self test passes.
    hello_and_self_test(&mut app, true);
    until(&mut app, "the LEDs never went live after every check passed", |app| both_are(app, LedState::Live));
    for _ in 0..4 {
        app.update();
    }

    for home in [Home::Bar, Home::Card] {
        let (led, state) = led_in(&mut app, home).unwrap_or_else(|| panic!("no LED in {home:?}"));
        assert_eq!(state, LedState::Live, "the LED in {home:?}");
        assert_eq!(word_of(&mut app, led).as_deref(), Some("live"), "the word beside the LED in {home:?}");
        let (size, glow) = dot_of(&mut app, led).unwrap_or_else(|| panic!("the LED in {home:?} has no dot"));
        assert!(size.x > 0.0 && size.y > 0.0, "the dot of the LED in {home:?} is laid out: {size:?}");
        assert!(glow > 0, "a live LED in {home:?} glows: it has {glow} glow layers");
    }

    // Owner exception (2026-11-14): the bar's LED stood on the status chip's row, before it; #423 makes the LED the status control itself (one
    // LED with one word, no chip of words), so the LED now stands inside the control that brings the self test back.
    let (bar, _) = led_in(&mut app, Home::Bar).expect("the bar's LED");
    let chip = chip_entity(&mut app);
    let (led_at, chip_at) = (centre_of(&mut app, bar), centre_of(&mut app, chip));
    let chip_size = app.world().get::<ComputedNode>(chip).expect("laid out").size();
    assert!((led_at.x - chip_at.x).abs() <= chip_size.x / 2.0, "the LED {led_at:?} stands inside the status control {chip_at:?} ({chip_size:?})");
    assert!((led_at.y - chip_at.y).abs() <= chip_size.y / 2.0, "the LED {led_at:?} stands on the status control's row {chip_at:?} ({chip_size:?})");

    // The card goes after its linger; the bar's LED is what stays.
    until(&mut app, "the card never went after every check passed", |app| {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<TerminalAgentCard>>().iter(world).next().is_none()
    });
    let left = leds(&mut app);
    assert_eq!(left.len(), 1, "only the bar's LED is left: {left:?}");
    assert_eq!((left[0].1, left[0].2), (Home::Bar, LedState::Live));
}

#[test]
fn a_failed_self_test_turns_both_leds_to_failed() {
    let mut app = editor_with_an_agent();
    invoke_the_agent(&mut app);
    until(&mut app, "no LED stood in both the bar and on the card while the agent was checking", |app| both_are(app, LedState::Busy));

    hello_and_self_test(&mut app, false);
    until(&mut app, "the LEDs never said the self test failed", |app| both_are(app, LedState::Failed));
    for _ in 0..4 {
        app.update();
    }
    for home in [Home::Bar, Home::Card] {
        let (led, _) = led_in(&mut app, home).unwrap_or_else(|| panic!("no LED in {home:?}"));
        assert_eq!(word_of(&mut app, led).as_deref(), Some("failed"), "the word beside the LED in {home:?}");
    }
}
