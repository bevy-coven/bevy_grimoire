//! **A theme edit reaches every list row in the editor** (UI-02, #210).
//!
//! `chrome.rs`'s colour constants are drawn by 131 sites outside that module, so the theme cannot be
//! a skin until those read one. `style_list_rows` is the first: it is the single chokepoint every list
//! row's fill passes through — it keys on `RowRest`, so a row spawned by any tab is painted there —
//! which makes it the one place where a skin reaches the whole editor at once.
//!
//! Two claims, and each can fail:
//!
//! 1. **A hovered row takes the skin's `hover` colour**, not `Ink::row_hover`. This is what a person sees
//!    change when they edit `editor_skin.yaml`: the row under their pointer.
//! 2. **A pressed row takes the skin's `pressed` colour.** The pair gooey's `problems()` refuses to let
//!    collapse (ΔL\* under 4) is exactly this one against hover, so a theme that moves one and not the
//!    other would be a theme the crate already calls broken.
//!
//! What is **not** claimed: that *every* colour is a skin role. `selected` has no gooey role and keeps
//! the constant; so do the 131 other sites. This is the first group, and a small one.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy_cauldron::chrome;
use bevy_cauldron::harness;

use crate::fixtures;

/// **A row's hover colour comes from the skin, not the constant.**
///
/// `Hovered` is bevy's `Immutable` picking component: only the picker may write it, and a headless app
/// has no pointer, so a test cannot put a row in the hovered state — measured, `row hovered
/// Some(false)` after an insert. What a test *can* ask is the question underneath: is the colour
/// `style_list_rows` would paint for hover the skin's? That is read from the same expression the
/// system uses, through the one function both go through, so a theme edit reaches the row's hover
/// colour by construction rather than by a pointer the harness cannot place.
///
/// This is weaker than a picture of a hovered row and stronger than nothing: the failure it catches is
/// the real one — `hover` read from `chrome::ink().row_hover` instead of the skin.
#[test]
fn the_hover_colour_the_editor_paints_comes_from_the_skin() {
    let root = fixtures::Fixture::new("theme-hover-row")
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..4 {
        app.update();
    }
    let mut skin = bevy_cauldron::ui::editor_skin();
    let distinct = Color::srgb(0.11, 0.87, 0.23);
    skin.hover = Some(distinct);
    app.insert_resource(skin);
    app.update();

    let painted = bevy_cauldron::chrome::row_fills(app.world());
    assert_eq!(
        painted.hover, distinct,
        "the colour the editor paints a hovered row must be the skin's `hover`; it is still the \
         constant, so a theme edit does not reach the row a person's pointer is on"
    );
    assert_eq!(
        painted.pressed,
        bevy_cauldron::ui::editor_skin().pressed.unwrap(),
        "and `pressed` likewise, the other half of the pair gooey's `problems()` keeps apart"
    );
}

/// **A pressed row takes the skin's `pressed` colour**, the other half of the pair `problems()` checks.
#[test]
fn a_pressed_row_takes_its_colour_from_the_skin() {
    let root = fixtures::Fixture::new("theme-pressed-row")
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..4 {
        app.update();
    }
    let mut skin = bevy_cauldron::ui::editor_skin();
    let distinct = Color::srgb(0.93, 0.21, 0.44);
    skin.pressed = Some(distinct);
    app.insert_resource(skin);

    let row = app
        .world_mut()
        .spawn((
            chrome::RowRest(chrome::ink().row_bg),
            Hovered::default(),
            BackgroundColor(chrome::ink().row_bg),
            // The pointer down on it, which is what `style_list_rows` reads as pressed.
            bevy::ui::Pressed,
        ))
        .id();

    for _ in 0..3 {
        app.update();
    }

    let painted = app
        .world()
        .get::<BackgroundColor>(row)
        .expect("a row keeps its fill")
        .0;
    assert_eq!(
        painted, distinct,
        "a pressed row must take the skin's `pressed` colour, or a theme moves hover and leaves press \
         behind — the exact pair gooey's `problems()` refuses to let collapse"
    );
}
