//! **The rig bench's staging chips take the skin's ink too** (UI-02, #210).
//!
//! `anim_stage::spawn_chips` drew a slot chip's words and the "click: solo" line from
//! `chrome::TEXT` and `chrome::LABEL`, so a project's skin could not reach the one row of the bench an
//! author *drives*. `BenchInk` already carried both colours (slice 4d moved the column labels onto
//! it); this file pins that the chips read the same struct, with the shipped palette where there is
//! no skin — which is what keeps a harness byte-identical to the editor.

use bevy::prelude::*;
use bevy_cauldron::anim_tab::BenchInk;
use bevy_cauldron::chrome;

/// **With no skin, the shipped palette.** The fallback every protected test already reads.
#[test]
fn with_no_skin_the_chips_read_the_shipped_table() {
    let ink = BenchInk::of(None);
    assert_eq!(ink.read, chrome::TEXT, "a chip's words are TEXT with no skin");
    assert_eq!(ink.label, chrome::LABEL, "the quiet line under them is LABEL");
}

/// **A skin's `text` reaches the chips**, and a skin that names the `label` tone quietens the line
/// under them — the same two seams the bench's column labels already use.
#[test]
fn a_skins_text_and_label_tone_reach_the_chips() {
    let text = Color::srgb(0.9, 0.1, 0.2);
    let mut tones = std::collections::BTreeMap::new();
    tones.insert("label".to_owned(), Color::srgb(0.1, 0.9, 0.2));
    let skin = bevy_gooey::GooeySkin { text, tones, ..Default::default() };
    let ink = BenchInk::of(Some(&skin));
    assert_eq!(ink.read, text, "the chips' words follow the skin's text");
    assert_eq!(
        ink.label,
        Color::srgb(0.1, 0.9, 0.2),
        "a skin naming the label tone quietens the line under the chips"
    );
}
