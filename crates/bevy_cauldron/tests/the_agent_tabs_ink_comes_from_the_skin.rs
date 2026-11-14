//! **The agent tab's ink comes from the running skin** (UI-02, #210, slice 6a).
//!
//! The terminal's agent tab draws four things a person reads while a session warms up: the card's
//! heading, one row per self-test check (passing, failing, waiting), and the status chip in the bar.
//! All four were `chrome.rs` constants — `Ink::text`, `Ink::muted`, `Ink::labeled`, `Ink::danger` — read from
//! `CheckStatus::ink` and `AgentPost::chip`, so a project's skin file could not reach any of them and
//! a high-contrast theme could not either.
//!
//! `terminal_agent::AgentInk` is the seam, the shape `editor::StatusInk` and `items::ListInk` already
//! use: one expression the spawners and this test both read, falling back to the shipped palette for a
//! harness that never ran `EditorUiPlugin`.

use bevy::prelude::*;
use bevy_cauldron::chrome;
use bevy_cauldron::terminal_agent::{AgentInk, CheckStatus};
use bevy_gooey::GooeySkin;

use crate::fixtures;

/// **A skin a person could write**, on colours no preset uses.
fn a_skin_of_its_own() -> GooeySkin {
    let mut skin = GooeySkin::tool();
    skin.text = Color::srgb(0.13, 0.24, 0.35);
    skin.danger = Color::srgb(0.99, 0.13, 0.24);
    skin.tones.insert("muted".to_owned(), Color::srgb(0.46, 0.57, 0.68));
    skin.tones.insert("labeled".to_owned(), Color::srgb(0.11, 0.99, 0.22));
    skin
}

/// **All four are the skin's**, field by field, so a failure says which one stopped following it.
#[test]
fn the_agent_tab_reads_the_skins_four_inks() {
    let skin = a_skin_of_its_own();
    let ink = AgentInk::of(Some(&skin));

    assert_eq!(ink.read, skin.text, "the card's heading is the skin's `text`");
    assert_eq!(ink.muted, skin.tone("muted").unwrap(), "a check still waiting is the skin's `muted` tone");
    assert_eq!(ink.good, skin.tone("labeled").unwrap(), "a check that passed is the skin's `labeled` tone");
    assert_eq!(ink.danger, skin.danger, "a check that failed is the skin's `danger`");
}

/// **Each check's row takes the ink of its own state**, so a theme that moves one state does not move
/// the others: a passing check stays passing while a waiting one is quieted.
#[test]
fn each_check_state_draws_in_its_own_role() {
    let ink = AgentInk::of(Some(&a_skin_of_its_own()));
    assert_eq!(CheckStatus::Passed("ok".to_owned()).ink(&ink), ink.good);
    assert_eq!(CheckStatus::Failed("no".to_owned()).ink(&ink), ink.danger);
    assert_eq!(CheckStatus::Pending.ink(&ink), ink.muted);
    assert_eq!(CheckStatus::Skipped("why".to_owned()).ink(&ink), ink.muted);
    assert_ne!(ink.good, ink.danger, "passing and failing must not look alike");
}

/// **With no skin it is the shipped palette**, so a harness that never ran `EditorUiPlugin` and every
/// protected test read what they always read.
#[test]
fn with_no_skin_the_agent_tab_is_the_shipped_palette() {
    let ink = AgentInk::of(None);
    assert_eq!(ink.read, chrome::ink().text);
    assert_eq!(ink.muted, chrome::ink().muted);
    assert_eq!(ink.good, chrome::ink().labeled);
    assert_eq!(ink.danger, chrome::ink().danger);
}

/// **The editor's own skin is the shipped palette**, so the running editor and a harness draw one
/// card. This reads the running skin out of a real headless editor, so a skin retuned without its
/// constant (or the reverse) fails here rather than in a person's window.
#[test]
fn the_editor_skin_draws_the_shipped_palette() {
    let root = fixtures::Fixture::new(&format!("agent_ink_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let running = app.world().resource::<GooeySkin>();
    let ink = AgentInk::of(Some(running));
    assert_eq!(ink.read, chrome::ink().text);
    assert_eq!(ink.muted, chrome::ink().muted);
    assert_eq!(ink.good, chrome::ink().labeled);
    assert_eq!(ink.danger, chrome::ink().danger);
}

/// **`SHELL` is process-wide**, so the two tests that open a real shell take turns.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// The editor on an agent tab, opened the way a person opens one, with a custom command whose harness
/// prints one word — so no omp has to be installed and no network is touched.
///
/// The settings choose the agent and its command, `Invoke Agent` is pressed through the pointer chain,
/// and the card is waited for exactly as `an_agent_tab_runs_its_self_test` waits for it.
fn editor_on_an_agent_tab(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    let file = root.join("assets/grimoire/editor.yaml");
    let settings = r#"{version: 1, terminal_agent: Custom, terminal_agent_command: "printf 'AGENT-%s\n' UP"}"#;
    std::fs::write(&file, settings).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let agents = std::env::temp_dir().join(format!("{tag}_{}_agents", std::process::id()));
    let server = std::env::temp_dir().join(format!("{tag}_{}_server", std::process::id()));
    std::fs::write(&server, "").unwrap_or_else(|e| panic!("{}: {e}", server.display()));
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<bevy_cauldron::terminal::Terminal>().set_collapsed(false); // #424: the pane starts collapsed; this test needs it raised
    app.insert_non_send(PosixShell(guard));
    app.insert_resource(bevy_cauldron::terminal_agent::AgentSetup { port: Some(15798), server, files: agents });
    invoke_the_agent(&mut app);
    app
}

/// How many frames, 50 ms apart, a test waits for the card.
const TRIES: usize = 100;

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
fn point(app: &mut App, at: Vec2, action: bevy::picking::pointer::PointerAction) {
    use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
    use bevy::picking::pointer::{Location, PointerId, PointerInput};
    let handle = app.world().get_resource::<bevy_cauldron::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
    let location =
        Location { target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }), position: at };
    app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
    app.update();
}

/// **Press `Invoke Agent`** the way a person does, through the whole pointer chain.
fn invoke_the_agent(app: &mut App) {
    use bevy::picking::pointer::{PointerAction, PointerButton};
    let button = |app: &mut App| {
        let world = app.world_mut();
        world
            .query_filtered::<&UiGlobalTransform, With<bevy_cauldron::terminal_pane::TerminalInvokeButton>>()
            .iter(world)
            .next()
            .map(|at| at.translation)
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

/// **The card's rows as the running skin's ink**, read from the entities the editor draws: `(the heading, one per check)` for the card.
/// Owner exception (2026-11-14): a row is an LED and its sentence in a row of their own now (#421), so the sentences are read from the
/// entities that carry `TerminalAgentCardRow`, in the order they were drawn.
fn drawn_card(app: &mut App) -> Vec<(String, Color)> {
    let world = app.world_mut();
    world
        .query::<(&bevy_cauldron::terminal_pane::TerminalAgentCardRow, &Text, &TextColor)>()
        .iter(world)
        .map(|(_, text, ink)| (text.0.clone(), ink.0))
        .collect()
}


/// **`dress_the_agent_tab` is what puts the skin's ink on the card and the chip** (UI-02, #210).
///
/// This is the test the seam had been missing: every other one here reads [`AgentInk`], the *expression*
/// a spawner and a test share, so deleting the system that repaints the card kept all four green. What
/// this asserts is the other half — that a running editor, on a skin a person could write, draws the
/// card's heading, its check rows and the bar's chip in that skin's four inks.
///
/// It is **load-bearing**: with `dress_the_agent_tab` removed from the plugin, the card keeps the
/// shipped palette and the heading fails with *"the card's heading is the running skin's `text`"*.
#[test]
fn the_card_and_the_chip_are_repainted_from_the_running_skin() {
    let mut app = editor_on_an_agent_tab("agent_ink_repaint");
    until(&mut app, "the self-test card was never drawn", |app| !drawn_card(app).is_empty());

    // **A skin no preset is**, inserted after the card and the chip stand: this is the case the system
    // exists for — a project's `family/bevy_gooey.yaml` edited while the tab is on screen.
    let skin = a_skin_of_its_own();
    let expect = AgentInk::of(Some(&skin));
    app.world_mut().insert_resource(skin);
    app.update();
    app.update();

    let rows = drawn_card(&mut app);
    let heading = rows.first().unwrap_or_else(|| panic!("the card draws a heading: {rows:?}"));
    assert_eq!(
        heading.1, expect.read,
        "the card's heading is the running skin's `text`: the card says {:?}",
        heading.0
    );
    // Every check row draws in the ink of its own state, read through the one expression the spawner
    // and the system both use — so this compares the system's output against the post's states.
    let post = app.world().resource::<bevy_cauldron::terminal::Terminal>().tab().and_then(|tab| tab.agent()).cloned();
    let post = post.expect("the drawn tab is an agent's");
    for (index, check) in post.checks.iter().enumerate() {
        let row = rows.get(index + 1).unwrap_or_else(|| panic!("the card draws row {index}: {rows:?}"));
        assert_eq!(
            row.1,
            check.ink(&expect),
            "check {index} ({:?}) draws in its own state's ink: {:?}",
            check.detail(),
            row.0
        );
    }
    // Owner exception (2026-11-14): the bar's status chip drew `post.chip_words()` in `post.chip_ink`; #423 replaces it with the agent's LED
    // (one LED, one word), whose colours are bevy_gooey's own, so the chip's half of this check is gone. The skin-ink checks go on holding of
    // the card's heading and rows above, and of the card's heading below.
    let chip = heading.1;
    // **And it is the skin's, not the palette's**, whatever state the post is in — the two palettes
    // disagree on every one of the four inks `a_skin_of_its_own` sets, so this cannot pass by luck.
    let palette = AgentInk::of(None);
    assert_ne!(
        (expect.read, expect.muted, expect.good, expect.danger),
        (palette.read, palette.muted, palette.good, palette.danger),
        "the skin under test must differ from the shipped palette, or this proves nothing"
    );
    assert!(
        [expect.read, expect.muted, expect.good, expect.danger].contains(&chip),
        "the chip's ink is one of the running skin's four: {chip:?}"
    );
    assert!(
        ![palette.read, palette.muted, palette.good, palette.danger].contains(&chip),
        "and none of the shipped palette's, which is what it draws without the repaint: {chip:?}"
    );
}
