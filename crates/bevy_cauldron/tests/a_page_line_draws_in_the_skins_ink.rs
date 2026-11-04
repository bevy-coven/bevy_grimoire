//! **The home's and the project page's quiet lines draw in the editor skin's ink** (UI-02, #210; #296).
//!
//! A card's folder, a map's size and a kit's piece count are *quiet* lines: the words under a name. They were a `chrome.rs` constant, so
//! a project's skin file could not reach the first screen anyone sees. [`launcher::quiet_ink`] reads the skin's `text_muted` and keeps
//! the shipped palette's `LABEL` as the answer for a harness with no skin, which is why every test that reads the colour of one still
//! reads the colour it always read.

use bevy::prelude::*;
use bevy_cauldron::chrome;
use bevy_cauldron::launcher::{self, Quiet};
use bevy_gooey::GooeySkin;

use crate::fixtures;

/// **A skin a person could write**, on a colour no preset uses.
fn a_skin_of_its_own() -> GooeySkin {
    let mut skin = GooeySkin::tool();
    skin.text_muted = Color::srgb(0.25, 0.35, 0.45);
    skin
}

#[test]
fn a_quiet_line_is_the_skins_muted_ink_and_the_shipped_palettes_with_no_skin() {
    let skin = a_skin_of_its_own();
    assert_eq!(launcher::quiet_ink(Some(&skin)), skin.text_muted, "a quiet line is the skin's `text_muted`");
    assert_ne!(launcher::quiet_ink(Some(&skin)), chrome::LABEL, "and the skin's, not the palette's, is what it draws");
    assert_eq!(launcher::quiet_ink(None), chrome::LABEL, "with no skin it is the shipped palette, so tests written before the seam read the colour they always read");
}

/// **On the page itself**: every quiet line of a drawn project page is in the skin's ink, including the ones a later skin change reaches.
#[test]
fn the_project_pages_quiet_lines_follow_the_skin() {
    let root = fixtures::Fixture::new("page_ink").descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let skin = a_skin_of_its_own();
    app.insert_resource(skin.clone());
    bevy_cauldron::harness::stand_on_the_project_page(&mut app, &root);

    let mut lines = app.world_mut().query_filtered::<(&Text, &TextColor), With<Quiet>>();
    let drawn: Vec<(String, Color)> = lines.iter(app.world()).map(|(t, c)| (t.0.clone(), c.0)).collect();
    assert!(drawn.iter().any(|(t, _)| t.contains("× ") && t.contains(" m")), "the map's size is a quiet line: {drawn:?}");
    assert!(drawn.iter().all(|(_, c)| *c == skin.text_muted), "every quiet line is the skin's ink: {drawn:?}");

    let mut changed = skin;
    changed.text_muted = Color::srgb(0.9, 0.1, 0.1);
    app.insert_resource(changed.clone());
    app.update();
    let mut lines = app.world_mut().query_filtered::<&TextColor, With<Quiet>>();
    assert!(lines.iter(app.world()).all(|c| c.0 == changed.text_muted), "and a skin change repaints them");
}
