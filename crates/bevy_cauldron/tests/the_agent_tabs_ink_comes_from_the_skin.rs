//! **The agent tab's ink comes from the running skin** (UI-02, #210, slice 6a).
//!
//! The terminal's agent tab draws four things a person reads while a session warms up: the card's
//! heading, one row per self-test check (passing, failing, waiting), and the status chip in the bar.
//! All four were `chrome.rs` constants — `TEXT`, `MUTED`, `LABELED`, `DANGER` — read from
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
    assert_eq!(ink.read, chrome::TEXT);
    assert_eq!(ink.muted, chrome::MUTED);
    assert_eq!(ink.good, chrome::LABELED);
    assert_eq!(ink.danger, chrome::DANGER);
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
    assert_eq!(ink.read, chrome::TEXT);
    assert_eq!(ink.muted, chrome::MUTED);
    assert_eq!(ink.good, chrome::LABELED);
    assert_eq!(ink.danger, chrome::DANGER);
}
