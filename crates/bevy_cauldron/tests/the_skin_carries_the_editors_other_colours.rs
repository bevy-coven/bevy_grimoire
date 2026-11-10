//! **The editor skin carries the twelve colours no `GooeySkin` field has, as tones** (UI-02, #210).
//!
//! `chrome::Ink` has twelve fields that are not a field of `GooeySkin` — `key`, `label`, `muted`,
//! `suggest`, `labeled`, `grid_line`, `bounds_line`, `veil`, `focus_bg`, `row_pressed`, `snip_wash`,
//! `overlay_bg` — so they live in the skin's `tones` map (`src/editor_skin.ron`), where a project's
//! file can change them and `GooeySkin::tone_problems` can check the text pairs among them.
//!
//! **Pinned to literal values, not to `chrome::ink()`** (review, 2026-11-10): `Ink::of` reads a skin's
//! tones, so the ink cannot be the thing that proves the tones — the values the palette held before it
//! moved into the skin are. `the_ink_holds_the_colours_the_table_held`
//! (`tests/no_colour_lives_in_chrome.rs`) is the other end of this pin: that one says the ink is those
//! same values, and this one says the skin carries them, so the two together say the ink and the skin
//! are one palette without either comparing a value to itself.

use bevy::prelude::*;
use bevy_cauldron::ui::editor_skin;

/// **The twelve tones as the old `chrome.rs` table held them**, written as literals on purpose.
///
/// **Why literals and not `chrome::ink()`** (review, 2026-11-10): `Ink::of` *reads* a skin's tones, so
/// comparing `skin.tone(name)` to `ink().<field>` compares a value with itself and passes whatever the
/// file says. The value that makes this test mean something is the one the palette held before it moved
/// into the skin, so a retuned tone fails here with its name and its two numbers.
fn colours() -> [(&'static str, Color); 12] {
    [
        ("bounds_line", Color::srgb(0.52, 0.47, 0.37)),
        ("focus_bg", Color::srgb(0.088, 0.082, 0.071)),
        ("grid_line", Color::srgb(0.3, 0.279, 0.243)),
        ("key", Color::srgb(0.79, 0.77, 0.735)),
        ("label", Color::srgb(0.615, 0.6, 0.575)),
        ("labeled", Color::srgb(0.55, 0.83, 0.5)),
        ("muted", Color::srgb(0.575, 0.56, 0.54)),
        ("overlay_bg", Color::srgb(0.195, 0.181, 0.158)),
        ("row_pressed", Color::srgb(0.11, 0.102, 0.089)),
        ("snip_wash", Color::srgba(0.95, 0.71, 0.3, 0.28)),
        ("suggest", Color::srgb(0.53, 0.71, 0.81)),
        ("veil", Color::srgba(0.0, 0.0, 0.0, 0.35)),
    ]
}

#[test]
fn each_tone_is_the_colour_chrome_draws_with() {
    let skin = editor_skin();
    for (name, constant) in colours() {
        let tone = skin.tone(name).unwrap_or_else(|| panic!("the editor skin has no tone `{name}`"));
        assert_eq!(tone.to_srgba(), constant.to_srgba(), "tone `{name}` is not the constant of that name in chrome.rs");
    }
    assert_eq!(skin.tones.len(), colours().len(), "the skin carries these tones and no others: {:?}", skin.tones.keys().collect::<Vec<_>>());
}

/// The text tones read on the panel's ground. The ratios are what the constants measure today, so a retuned
/// tone that loses contrast fails here with the pair named.
#[test]
fn the_text_tones_read_on_the_panel() {
    let skin = editor_skin();
    let pairs: [(&'static str, &str, &str, f32); 5] = [
        ("key on surface", "key", "surface", 4.5),
        ("label on surface", "label", "surface", 4.5),
        ("muted on surface", "muted", "surface", 4.5),
        ("suggest on surface", "suggest", "surface", 4.5),
        ("labeled on surface", "labeled", "surface", 4.5),
    ];
    assert_eq!(skin.tone_problems(&pairs), vec![], "the editor's text tones must read on its panel");
}
