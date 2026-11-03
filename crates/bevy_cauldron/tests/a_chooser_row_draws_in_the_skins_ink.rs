//! **The menu's rows draw in the editor skin's ink** (UI-02, #210, slice 10).
//!
//! A row in the Cauldron's menu is one of six inks: the row the arrows are on, the value under the
//! caret, a stocked kit, an empty one, an ordinary row and a problem. All six were `chrome.rs`
//! constants, so a project's skin file could not reach the first screen anyone sees.
//!
//! `chooser::colour` reads them from `GooeySkin` and keeps each constant as the fallback for a
//! harness with no skin — which is why every protected test that reads a row's ink still reads the
//! colour it always read.

use bevy::prelude::*;
use bevy_cauldron::chooser::{colour, Tone};
use bevy_cauldron::chrome;
use bevy_gooey::GooeySkin;

/// **A skin a person could write**, on colours no preset uses.
fn a_skin_of_its_own() -> GooeySkin {
    let mut skin = GooeySkin::tool();
    skin.text = Color::srgb(0.75, 0.65, 0.55);
    skin.text_muted = Color::srgb(0.25, 0.35, 0.45);
    skin.accent = Color::srgb(0.15, 0.85, 0.35);
    skin.danger = Color::srgb(0.95, 0.15, 0.05);
    skin.tones.insert("labeled".to_owned(), Color::srgb(0.55, 0.95, 0.15));
    skin.tones.insert("label".to_owned(), Color::srgb(0.45, 0.25, 0.65));
    skin
}

/// **All six tones are the skin's**, one assertion per tone so a failure names the one that stopped
/// following it.
#[test]
fn a_row_draws_in_the_skins_ink() {
    let skin = a_skin_of_its_own();
    let of = |tone| colour(tone, Some(&skin));

    assert_eq!(of(Tone::Selected), skin.text, "the row the arrows are on is the skin's `text`");
    assert_eq!(of(Tone::Editing), skin.accent, "the value under the caret is the skin's `accent`");
    assert_eq!(of(Tone::Stocked), Color::srgb(0.55, 0.95, 0.15), "a stocked kit is the `labeled` tone");
    assert_eq!(of(Tone::Empty), Color::srgb(0.45, 0.25, 0.65), "an empty one is the `label` tone");
    assert_eq!(of(Tone::Row), skin.text_muted, "an ordinary row is the skin's `text_muted`");
    assert_eq!(of(Tone::Problem), skin.danger, "a problem is the skin's `danger`");
}

/// **The six stay distinct**, which is the property that makes them tones rather than decoration: a
/// skin that answered all six with one colour would make a stocked kit unreadable against a blank one.
#[test]
fn the_six_tones_are_six_colours() {
    let skin = a_skin_of_its_own();
    let mut seen: Vec<Color> = Vec::new();
    for tone in [Tone::Selected, Tone::Editing, Tone::Stocked, Tone::Empty, Tone::Row, Tone::Problem] {
        let c = colour(tone, Some(&skin));
        assert!(!seen.contains(&c), "two tones drew the same colour {c:?}: they would be one state on screen");
        seen.push(c);
    }
}

/// **With no skin it is the shipped palette**, so every protected test written before this seam reads
/// the colour it always read.
#[test]
fn with_no_skin_a_row_is_the_shipped_palette() {
    assert_eq!(colour(Tone::Selected, None), chrome::TEXT);
    assert_eq!(colour(Tone::Editing, None), chrome::ACCENT);
    assert_eq!(colour(Tone::Stocked, None), chrome::LABELED);
    assert_eq!(colour(Tone::Empty, None), chrome::LABEL);
    assert_eq!(colour(Tone::Row, None), chrome::DIM);
    assert_eq!(colour(Tone::Problem, None), chrome::DANGER);
}
