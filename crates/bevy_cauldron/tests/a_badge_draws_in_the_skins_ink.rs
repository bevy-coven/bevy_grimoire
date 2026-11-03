//! **The keyboard legend draws in the editor skin's ink** (UI-02, #210, slice 7).
//!
//! A badge is four colours: the ground a labelled cluster stands on, the chord's ink and the box's
//! hairline (one colour, `key`), the description's quieter ink, and the veil the whole layer paints.
//! All four were `chrome.rs` constants, so a project's skin file could not reach the legend — the one
//! surface a person reads *while* learning the editor, and the last place a stale palette should be.
//!
//! `BadgeInk::of` reads them from `GooeySkin` and keeps the constant as the fallback for a harness
//! with no skin. This test says what a skin can change it changes, and what it cannot leaves alone:
//! a skin with every field moved draws a badge in the skin's colours, and no skin still draws in the
//! shipped palette.

use bevy::prelude::*;
use bevy_cauldron::badges::BadgeInk;
use bevy_cauldron::chrome;
use bevy_gooey::GooeySkin;

/// **A skin a person could write:** every field a badge reads, moved somewhere no preset puts it.
fn a_skin_of_its_own() -> GooeySkin {
    let mut skin = GooeySkin::tool();
    skin.surface = Color::srgb(0.31, 0.11, 0.19);
    skin.text_muted = Color::srgb(0.41, 0.81, 0.21);
    skin.tones.insert("key".to_owned(), Color::srgb(0.11, 0.61, 0.91));
    skin.tones.insert("veil".to_owned(), Color::srgba(0.2, 0.4, 0.6, 0.31));
    skin
}

/// **Each of the four is the skin's, not the constant's.** Written field by field rather than as one
/// `assert_eq!` on the struct so a failure says which colour stopped following the skin.
#[test]
fn a_badge_draws_in_the_skins_ink() {
    let skin = a_skin_of_its_own();
    let ink = BadgeInk::of(Some(&skin));

    assert_eq!(ink.panel, skin.surface, "a labelled cluster stands on the skin's `surface`");
    assert_eq!(ink.key, Color::srgb(0.11, 0.61, 0.91), "`key` is a tone, and the skin's is what draws");
    assert_eq!(ink.dim, skin.text_muted, "a description is the skin's `text_muted`");
    assert_eq!(ink.veil, Color::srgba(0.2, 0.4, 0.6, 0.31), "`veil` is a tone, and the skin's is what draws");
}

/// **With no skin it is the shipped palette**, which is what every harness and every test that was
/// written before this seam already expects — so moving a colour here re-pins nothing.
#[test]
fn with_no_skin_a_badge_is_the_shipped_palette() {
    let ink = BadgeInk::of(None);
    assert_eq!(ink, BadgeInk::SHIPPED);
    assert_eq!(ink.panel, chrome::PANEL_BG);
    assert_eq!(ink.key, chrome::KEY);
    assert_eq!(ink.dim, chrome::DIM);
    assert_eq!(ink.veil, chrome::VEIL);
}

/// **The shipped palette and the editor's own skin are one palette**, which is the claim that lets a
/// harness and the running editor draw the same badge. If someone retunes `editor_skin.ron` without
/// its constant (or the reverse) this fails, and it is the pair the tone test does not cover:
/// `BadgeInk` maps `dim` to `text_muted`, not to a tone.
#[test]
fn the_editor_skin_draws_the_shipped_palette() {
    let ink = BadgeInk::of(Some(&bevy_cauldron::ui::editor_skin()));
    assert_eq!(ink.panel, chrome::PANEL_BG, "the editor skin's `surface` is `PANEL_BG`");
    assert_eq!(ink.key, chrome::KEY, "its `key` tone is `KEY`");
    assert_eq!(ink.dim, chrome::DIM, "its `text_muted` is `DIM`");
    assert_eq!(ink.veil, chrome::VEIL, "its `veil` tone is `VEIL`");
}
