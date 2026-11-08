//! **A chord the editor draws is spelled with glyphs its face has** — `feedback-1794097488318`,
//! oracle `other:face-coverage`.
//!
//! The note, from the editor at 2560x1406 on a Mac: *"One of the characters in here is a "?", why?"* —
//! pointing at the Undo tooltip's keycap, which draws a box with a `?` in it where its text, `⌘Z`, has the
//! command-key symbol. It is the same keycap `feedback-1793927744511` pointed at; that capture was blocked on
//! `bevy-coven/bevy_gooey#45`, which closed by giving the host a setting (`BindingSettings::chord_glyphs`)
//! to spell chords in words, and this editor had not set it.
//!
//! # The relation
//!
//! A picture has no expected value, so the oracle is a relation between two things one build holds: **the
//! words the UI draws, and the face it draws them with**. Text is legible exactly when its codepoints are
//! mapped by the face (Segura, Fraser, Sánchez & Ruiz-Cortés, *A Survey on Metamorphic Testing*,
//! `10.1109/TSE.2016.2532875`: the effective relations are those drawn from what the program means). The
//! face's own character map is the independent measurement; nothing here reads gooey's spelling table.
//!
//! It is asserted over **every** chord text on screen (`ChordText`: a tooltip's keycap, a menu row, a
//! badge), not the one the person pointed at, and **as a Mac spells them**: the headless editor on Linux
//! would otherwise be spelling for Windows, in words, and pass for the wrong reason.

use bevy::prelude::*;
use bevy::text::FontSource;
use bevy_cauldron::harness;
use bevy_gooey::{BindingSettings, ChordText, Modifiers, Platform, Shortcut};

use crate::fixtures;

/// Frames for the chord texts to be rewritten under the new platform.
const SETTLE_FRAMES: usize = 4;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..6 {
        app.update();
    }
    app
}

/// Does the face in `bytes` have a glyph for `c`, read from its own character map?
fn covers(bytes: &[u8], c: char) -> bool {
    ttf_parser::Face::parse(bytes, 0)
        .unwrap_or_else(|e| panic!("the face does not parse: {e}"))
        .glyph_index(c)
        .is_some()
}

/// **Every chord the editor draws, spelled for a Mac, uses only codepoints its face maps.**
#[test]
fn every_chord_the_editor_draws_on_a_mac_is_made_of_glyphs_its_face_has() {
    let mut app = editor("chord-glyphs");
    app.world_mut().resource_mut::<BindingSettings>().platform = Some(Platform::Mac);
    // **A Mac build's primary modifier is Command; this run is a Linux build, whose primary is Ctrl.** The editor's
    // chords are made with `Modifiers::PRIMARY`, so the capture's `⌘Z` is on screen here as `Ctrl+Z` until the chords
    // are put as the Mac build holds them.
    {
        let world = app.world_mut();
        for mut chord in world.query::<&mut ChordText>().iter_mut(world) {
            if chord.0.modifiers == Modifiers::CTRL {
                chord.0 = Shortcut::new(Modifiers::META, chord.0.key);
            }
        }
    }
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }

    let world = app.world_mut();
    let drawn: Vec<(String, FontSource)> = world
        .query_filtered::<(&Text, &TextFont), With<ChordText>>()
        .iter(world)
        .map(|(text, font)| (text.0.clone(), font.font.clone()))
        .collect();
    assert!(
        drawn.iter().any(|(words, _)| words.ends_with('Z') && (words.contains('⌘') || words.contains("Cmd"))),
        "no chord text is a Command chord on Z, so the toolbar's Undo keycap was not found and this test proves nothing: {drawn:?}"
    );

    let fonts = world.resource::<Assets<Font>>();
    let mut missing: Vec<String> = Vec::new();
    for (words, source) in &drawn {
        let FontSource::Handle(handle) = source else {
            panic!("a chord text is drawn in {source:?}, not a face this test can read the character map of");
        };
        let face = fonts.get(handle).unwrap_or_else(|| panic!("the face of a chord text ({handle:?}) is not loaded"));
        for c in words.chars().filter(|c| !covers(face.data.as_ref(), *c)) {
            missing.push(format!("{words:?} has U+{:04X} {c}, which the face has no glyph for", c as u32));
        }
    }
    assert!(
        missing.is_empty(),
        "a chord drawn on a Mac shows a missing-glyph box (the owner's \"?\") for:\n  {}",
        missing.join("\n  ")
    );
}
