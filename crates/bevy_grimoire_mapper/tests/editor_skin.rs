//! **The editor's colours are a gooey skin, and the skin is data** (UI-02, #210).
//!
//! `chrome.rs` drew the editor with its own colour table and its own elevation test. The same
//! colours are now `src/editor_skin.ron`, read as `bevy_gooey`'s `GooeySkin`, and the library's own
//! `problems()` is the check that the ladder is a ladder and the ink is readable. These tests hold
//! the file and the table to one another while the table still exists, so neither can drift.

use bevy::prelude::*;
use bevy_grimoire_mapper::chrome;
use bevy_grimoire_mapper::ui::editor_skin;

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
        ("VOID", chrome::VOID),
        ("PANEL_BG", chrome::PANEL_BG),
        ("HEADER_BG", chrome::HEADER_BG),
        ("ROW_BG", chrome::ROW_BG),
        ("SLOT_BG", chrome::SLOT_BG),
        ("ROW_HOVER", chrome::ROW_HOVER),
        ("ROW_SELECTED", chrome::ROW_SELECTED),
        ("PANEL_EDGE", chrome::PANEL_EDGE),
    ];
    for (step, (name, colour)) in ladder.into_iter().enumerate() {
        assert_eq!(skin.surface_at(step as u8), colour, "ladder step {step} is {name}");
    }
    let roles = [
        ("background is VOID", skin.background, chrome::VOID),
        ("surface is PANEL_BG", skin.surface, chrome::PANEL_BG),
        ("surface_raised is ROW_BG", skin.surface_raised, chrome::ROW_BG),
        ("text is TEXT", skin.text, chrome::TEXT),
        ("text_muted is DIM", skin.text_muted, chrome::DIM),
        ("accent is ACCENT", skin.accent, chrome::ACCENT),
        ("danger is DANGER", skin.danger, chrome::DANGER),
        ("outline is EDGE_STRONG", skin.outline, chrome::EDGE_STRONG),
        ("scrim is SCRIM", skin.scrim, chrome::SCRIM),
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
    let app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let running = app.world().resource::<bevy_gooey::GooeySkin>();
    assert_eq!(*running, editor_skin(), "with no project skin, the running skin is the editor's, not the library's default");
}
