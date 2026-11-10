//! **The editor's colours are a gooey skin, and the skin is data** (UI-02, #210).
//!
//! `chrome.rs` drew the editor with its own colour table and its own elevation test. The same
//! colours are now `src/editor_skin.ron`, read as `bevy_gooey`'s `GooeySkin`, and the library's own
//! `problems()` is the check that the ladder is a ladder and the ink is readable. These tests hold
//! the file and the table to one another while the table still exists, so neither can drift.

use bevy::prelude::*;
use bevy_cauldron::chrome;
use bevy_cauldron::ui::editor_skin;

use crate::fixtures;

#[test]
fn the_editor_skin_breaks_none_of_the_librarys_own_rules() {
    let skin = editor_skin();
    assert_eq!(skin.problems(), vec![], "the editor's skin must pass GooeySkin::problems()");
    assert_eq!(skin.surfaces.len(), 8, "the ladder has eight steps, lowest first");
}

#[test]
fn the_skin_and_the_palette_say_the_same_colours() {
    let skin = editor_skin();
    let ladder = [
        ("VOID", chrome::ink().void),
        ("PANEL_BG", chrome::ink().panel_bg),
        ("HEADER_BG", chrome::ink().header_bg),
        ("ROW_BG", chrome::ink().row_bg),
        ("SLOT_BG", chrome::ink().slot_bg),
        ("ROW_HOVER", chrome::ink().row_hover),
        ("ROW_SELECTED", chrome::ink().row_selected),
        ("PANEL_EDGE", chrome::ink().panel_edge),
    ];
    for (step, (name, colour)) in ladder.into_iter().enumerate() {
        assert_eq!(skin.surface_at(step as u8), colour, "ladder step {step} is {name}");
    }
    let roles = [
        ("background is VOID", skin.background, chrome::ink().void),
        ("surface is PANEL_BG", skin.surface, chrome::ink().panel_bg),
        ("surface_raised is ROW_BG", skin.surface_raised, chrome::ink().row_bg),
        ("text is TEXT", skin.text, chrome::ink().text),
        ("text_muted is DIM", skin.text_muted, chrome::ink().dim),
        ("accent is ACCENT", skin.accent, chrome::ink().accent),
        ("danger is DANGER", skin.danger, chrome::ink().danger),
        ("outline is EDGE_STRONG", skin.outline, chrome::ink().edge_strong),
        ("scrim is SCRIM", skin.scrim, chrome::ink().scrim),
    ];
    for (what, from_skin, from_palette) in roles {
        assert_eq!(from_skin, from_palette, "{what}");
    }
    assert_eq!(skin.radius, chrome::RADIUS_PANEL, "the skin's corner is the panel's");
    assert_eq!(skin.padding, chrome::PAD, "the skin's padding is the panel's");
    assert_eq!(skin.gap, chrome::GAP_ROW, "the skin's gap is a row's");
}

#[test]
fn the_running_editor_draws_gooey_from_the_editor_skin() {
    let root = fixtures::Fixture::new(&format!("editor_skin_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let running = app.world().resource::<bevy_gooey::GooeySkin>();
    assert_eq!(*running, editor_skin(), "with no project skin, the running skin is the editor's, not the library's default");
}
