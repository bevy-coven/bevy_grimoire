//! **The composition panel draws in the editor skin's ink** (UI-02, #210, slice 9).
//!
//! The panel's prose is four inks: what is read, the quieter counts and hints, the accent on the row
//! being acted on, and the budget's refusal. All four were `chrome.rs` constants, so a project's skin
//! file could not reach the panel that says what will be built.
//!
//! `compose::PanelInk` reads them from `GooeySkin` and keeps each constant as the fallback for a
//! harness with no skin. This test asks it directly rather than driving the whole plugin: what the
//! panel *says* is `compose`'s own business, and what colour it says it in is this.

use bevy::prelude::*;
use bevy_cauldron::chrome;
use bevy_cauldron::compose::PanelInk;
use bevy_gooey::GooeySkin;

/// **A skin a person could write**, on colours no preset uses, so a passing assertion cannot be the
/// constant coinciding with the field.
fn a_skin_of_its_own() -> GooeySkin {
    let mut skin = GooeySkin::tool();
    skin.text = Color::srgb(0.71, 0.81, 0.91);
    skin.text_muted = Color::srgb(0.31, 0.41, 0.51);
    skin.accent = Color::srgb(0.91, 0.51, 0.11);
    skin.danger = Color::srgb(0.81, 0.11, 0.21);
    skin
}

/// **All four are the skin's**, field by field so a failure says which one stopped following it.
#[test]
fn the_panel_draws_in_the_skins_ink() {
    let skin = a_skin_of_its_own();
    let ink = PanelInk::of(Some(&skin));

    assert_eq!(ink.read, skin.text, "a row's words are the skin's `text`");
    assert_eq!(ink.quiet, skin.text_muted, "a count or a hint is the skin's `text_muted`");
    assert_eq!(ink.accent, skin.accent, "the row being acted on is the skin's `accent`");
    assert_eq!(ink.danger, skin.danger, "the budget's refusal is the skin's `danger`");
}

/// **With no skin it is the shipped palette**, so a harness and every protected test written before
/// this seam reads the colour it always read.
#[test]
fn with_no_skin_the_panel_is_the_shipped_palette() {
    let ink = PanelInk::of(None);
    assert_eq!(ink.read, chrome::ink().text);
    assert_eq!(ink.quiet, chrome::ink().dim);
    assert_eq!(ink.accent, chrome::ink().accent);
    assert_eq!(ink.danger, chrome::ink().danger);
}

/// **The editor's own skin is the shipped palette**, which is what lets the running editor and a
/// harness draw the same panel. A skin retuned without its constant (or the reverse) fails here.
#[test]
fn the_editor_skin_draws_the_shipped_palette() {
    let ink = PanelInk::of(Some(&bevy_cauldron::ui::editor_skin()));
    assert_eq!(ink.read, chrome::ink().text, "the editor skin's `text` is `TEXT`");
    assert_eq!(ink.quiet, chrome::ink().dim, "its `text_muted` is `DIM`");
    assert_eq!(ink.accent, chrome::ink().accent, "its `accent` is `ACCENT`");
    assert_eq!(ink.danger, chrome::ink().danger, "its `danger` is `DANGER`");
}
