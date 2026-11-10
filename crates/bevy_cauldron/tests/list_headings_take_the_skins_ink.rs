//! **A list heading draws in the skin's ink** (UI-02, #210).
//!
//! `chrome::list_heading` is the one builder behind the PLACE palette's heading, the rig list's and the
//! kit's piece list, and it drew `chrome::LABEL` — the constant — so a project's skin could not reach the
//! name of any of the three lists. `ListHeadingInk` is the seam: the shipped `Ink::label` where there is no
//! skin (which keeps every protected test and the editor itself byte-identical), the skin's own `label`
//! tone where a skin names one.

use bevy::prelude::*;
use bevy_cauldron::chrome::{self, ListHeadingInk};

/// **With no skin the heading is the shipped constant.**
#[test]
fn with_no_skin_the_heading_is_the_shipped_label() {
    assert_eq!(
        ListHeadingInk::of(None),
        ListHeadingInk::shipped(),
        "a harness that never ran the UI plugin draws what it always drew"
    );
    assert_eq!(ListHeadingInk::shipped().0, chrome::ink().label);
}

/// **A skin that names the `label` tone changes it**; the editor's own skin does not, by design.
#[test]
fn a_skins_label_tone_is_the_headings_ink() {
    let mut tones = std::collections::BTreeMap::new();
    tones.insert("label".to_owned(), Color::srgb(0.2, 0.7, 0.3));
    let skin = bevy_gooey::GooeySkin { tones, ..Default::default() };
    assert_eq!(
        ListHeadingInk::of(Some(&skin)).0,
        Color::srgb(0.2, 0.7, 0.3),
        "a project's skin reaches all three list headings"
    );

    let plain = bevy_gooey::GooeySkin::default();
    assert_eq!(
        ListHeadingInk::of(Some(&plain)),
        ListHeadingInk::shipped(),
        "the editor's own skin carries no label tone, so its headings are unchanged"
    );
}
