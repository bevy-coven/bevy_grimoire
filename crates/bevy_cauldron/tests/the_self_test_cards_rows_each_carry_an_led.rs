//! **Each row of the agent's self-test card starts with a small LED, and the card says no `OK`, `FAIL` or `SKIP`** (#421; the owner:
//! *"No, the 'OK's need to be subtle LEDs."*).
//!
//! A row's LED is in the row's state: passed is live, failed is failed, skipped is off, and a check still running is busy. The check's
//! sentence stands beside it. The state word is not drawn, but the LED keeps its accessible state name (`bevy_gooey`'s `LedWord`), so a
//! screen reader and this test still read the state. The card's final line, the agent's overall LED, stays.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_agent::{AgentSetup, CHECK_LABELS, CheckStatus};
use bevy_cauldron::terminal_pane::{TerminalAgentCard, TerminalAgentCardLed, TerminalAgentCardRow, TerminalCardRowLed, TerminalInvokeButton};
use bevy_gooey::{Led, LedState, LedWord};

use crate::fixtures;

const TRIES: usize = 200;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn editor() -> App {
    let tag = format!("card_leds_{}", std::process::id());
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
    app.insert_non_send(PosixShell(guard));
    app.insert_resource(AgentSetup { port: Some(15799), server, files });
    app.world_mut().resource_mut::<Terminal>().set_collapsed(false);
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

fn point(app: &mut App, at: Vec2, action: PointerAction) {
    let handle = app.world().get_resource::<bevy_cauldron::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
    let location = Location { target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }), position: at };
    app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
    app.update();
}

fn start_the_agent(app: &mut App) {
    let button = |app: &mut App| {
        let world = app.world_mut();
        world.query_filtered::<&UiGlobalTransform, With<TerminalInvokeButton>>().iter(world).next().map(|at| at.translation)
    };
    until(app, "the Start agent button was never drawn", |app| button(app).is_some());
    let at = button(app).expect("the button stands");
    for action in [PointerAction::Move { delta: Vec2::ZERO }, PointerAction::Press(PointerButton::Primary), PointerAction::Release(PointerButton::Primary)] {
        point(app, at, action);
    }
}

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

/// A row's LED: its state, and the state word it keeps for a screen reader.
fn row_led(app: &mut App, index: usize) -> Option<(LedState, String)> {
    let world = app.world_mut();
    let led = world.query::<(Entity, &TerminalCardRowLed)>().iter(world).find(|(_, row)| row.0 == index).map(|(entity, _)| entity)?;
    let state = world.get::<Led>(led)?.0;
    let kids = world.get::<Children>(led)?.to_vec();
    let word = kids.iter().find_map(|kid| world.get::<LedWord>(*kid).and_then(|_| world.get::<Text>(*kid)).map(|text| text.0.clone()))?;
    Some((state, word))
}

/// **Every word the card draws**: the texts under it that are not hidden.
fn drawn_words(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let texts: Vec<(Entity, String)> = world.query::<(Entity, &Text)>().iter(world).map(|(entity, text)| (entity, text.0.clone())).collect();
    texts
        .into_iter()
        .filter(|(entity, words)| is_under::<TerminalAgentCard>(world, *entity) && !words.is_empty() && world.get::<Node>(*entity).is_none_or(|node| node.display != Display::None))
        .map(|(_, words)| words)
        .collect()
}

fn rows(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut rows: Vec<(usize, String)> = world.query::<(&TerminalAgentCardRow, &Text)>().iter(world).filter_map(|(row, text)| row.0.map(|at| (at, text.0.clone()))).collect();
    rows.sort();
    rows.into_iter().map(|(_, words)| words).collect()
}

/// **A card with one passed, one failed, one skipped and one running row finds an LED of each state on its row, and no `OK`, `FAIL` or
/// `SKIP` text.**
#[test]
fn each_row_carries_an_led_in_its_state_and_the_card_says_no_ok_fail_or_skip() {
    let mut app = editor();
    start_the_agent(&mut app);
    until(&mut app, "the card to be drawn", |app| row_led(app, 0).is_some() && row_led(app, 3).is_some());

    // Pin one check in each state, as a card in a real run would show them.
    {
        let mut terminal = app.world_mut().resource_mut::<Terminal>();
        let post = terminal.agent_posts_mut().next().expect("the agent's tab");
        post.checks[0] = CheckStatus::Passed("bevy_cauldron_mcp on port 15799".to_owned());
        post.checks[1] = CheckStatus::Failed("the shell stopped before the agent was typed".to_owned());
        post.checks[2] = CheckStatus::Skipped("nothing was started".to_owned());
        post.checks[3] = CheckStatus::Pending;
        post.revision += 1;
    }
    until(&mut app, "the card to be drawn again from the pinned checks", |app| row_led(app, 1).is_some_and(|(state, _)| state == LedState::Failed));

    let wanted = [(LedState::Live, "live"), (LedState::Failed, "failed"), (LedState::Off, "off"), (LedState::Busy, "busy")];
    for (index, (state, word)) in wanted.iter().enumerate() {
        let (got, kept) = row_led(&mut app, index).unwrap_or_else(|| panic!("row {index} ({}) has no LED", CHECK_LABELS[index]));
        assert_eq!(got, *state, "row {index} ({}) lights its LED in its state", CHECK_LABELS[index]);
        assert_eq!(&kept, word, "the LED keeps its accessible state name: row {index}");
    }

    // The sentence stands beside the LED, with the check's own words and no state word before it.
    let sentences = rows(&mut app);
    assert_eq!(sentences.len(), 4, "{sentences:?}");
    assert_eq!(sentences[0], "Tools server: bevy_cauldron_mcp on port 15799");
    assert_eq!(sentences[1], "Agent started: the shell stopped before the agent was typed");
    assert_eq!(sentences[2], "Agent connected: nothing was started");
    assert_eq!(sentences[3], "Agent self test: waiting");

    // No `OK`, `FAIL` or `SKIP`, and no state word on any row: only the card's final LED draws its word.
    let words = drawn_words(&mut app);
    for word in &words {
        let first = word.split_whitespace().next().unwrap_or("");
        assert!(!["OK", "FAIL", "SKIP"].contains(&first), "the card says {word:?}: {words:?}");
    }
    let verdict = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<TerminalAgentCardLed>>().iter(world).count()
    };
    assert_eq!(verdict, 1, "the card's final line, the agent's overall LED, stays");
    assert!(words.iter().any(|word| word == "failed"), "the overall LED still draws its word: {words:?}");
    assert_eq!(words.iter().filter(|word| word.as_str() == "live").count(), 0, "a row's LED draws no word: {words:?}");
}
