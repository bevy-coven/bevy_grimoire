//! **The status band draws in the editor skin's ink** (UI-02, #210, slice 12).
//!
//! The band is the line an author reads without looking for it — the armed piece, the map's name, the
//! cell under the cursor, a fault — and it was drawn from four `chrome.rs` constants, so a project's
//! skin file could not reach it.
//!
//! `editor::StatusInk` reads them from `GooeySkin` and keeps each constant as the fallback for a
//! harness with no skin.

use bevy::prelude::*;
use bevy_cauldron::chrome;
use bevy_cauldron::editor::StatusInk;
use bevy_gooey::GooeySkin;

/// **A skin a person could write**, on colours no preset uses.
fn a_skin_of_its_own() -> GooeySkin {
    let mut skin = GooeySkin::tool();
    skin.text = Color::srgb(0.61, 0.71, 0.81);
    skin.text_muted = Color::srgb(0.21, 0.31, 0.41);
    skin.accent = Color::srgb(0.91, 0.41, 0.01);
    skin.danger = Color::srgb(0.71, 0.01, 0.31);
    skin
}

/// **All four are the skin's**, field by field.
#[test]
fn the_status_band_draws_in_the_skins_ink() {
    let skin = a_skin_of_its_own();
    let ink = StatusInk::of(Some(&skin));

    assert_eq!(ink.read, skin.text, "an ordinary value is the skin's `text`");
    assert_eq!(ink.quiet, skin.text_muted, "a quiet value is the skin's `text_muted`");
    assert_eq!(ink.accent, skin.accent, "a live answer, or an unsaved map, is the skin's `accent`");
    assert_eq!(ink.danger, skin.danger, "a fault is the skin's `danger`");
}

/// **The live inks stay distinct from the settled ones**, which is the property the band relies on:
/// a value that is *happening* has to read differently from one that is simply true.
#[test]
fn a_live_answer_is_not_a_settled_one() {
    let skin = a_skin_of_its_own();
    let ink = StatusInk::of(Some(&skin));
    assert_ne!(ink.accent, ink.read, "an unsaved map must not look saved");
    assert_ne!(ink.danger, ink.read, "a fault must not look like a value");
}

/// **With no skin it is the shipped palette**, so every protected test reads what it always read.
#[test]
fn with_no_skin_the_band_is_the_shipped_palette() {
    let ink = StatusInk::of(None);
    assert_eq!(ink.read, chrome::ink().text);
    assert_eq!(ink.quiet, chrome::ink().dim);
    assert_eq!(ink.accent, chrome::ink().accent);
    assert_eq!(ink.danger, chrome::ink().danger);
}

/// **The editor's own skin is the shipped palette**, so the running editor and a harness draw one
/// band. A skin retuned without its constant (or the reverse) fails here.
#[test]
fn the_editor_skin_draws_the_shipped_palette() {
    let ink = StatusInk::of(Some(&bevy_cauldron::ui::editor_skin()));
    assert_eq!(ink.read, chrome::ink().text);
    assert_eq!(ink.quiet, chrome::ink().dim);
    assert_eq!(ink.accent, chrome::ink().accent);
    assert_eq!(ink.danger, chrome::ink().danger);
}
