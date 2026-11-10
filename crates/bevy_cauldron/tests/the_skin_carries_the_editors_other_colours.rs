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
//!
//! **Fourteen since UI-80 (#254).** The editor skin also carries the light map's two inks —
//! `overlay_dark` and `overlay_lit` — so a theme or a project can move the overlay. They are not
//! `chrome::Ink` fields (no chrome label draws with them), which is why the count below is the skin's
//! tone set and not the ink's twelve; `the_view_menu_toggles_the_overlays` holds them to the shipped
//! pair `light_overlay::SHIPPED` draws with.
//!
//! **Owner exception (2026-11-10, #254): this test's tone set grows from twelve to fourteen.** #254's
//! remaining item is "the shipped skins carry `overlay_dark`/`overlay_lit`", so the exact-set assertion
//! is raised by exactly the two tones that item names, and the new pair is checked against a literal in
//! `the_light_maps_two_inks_are_the_shipped_pair` below rather than left unpinned.

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
    // **Fourteen, not twelve** (UI-80, #254): the light map's two inks are the skin's too, and the
    // exact-set check says so rather than letting an unnamed tone arrive.
    assert_eq!(
        skin.tones.len(),
        colours().len() + LIGHT_MAP_TONES.len(),
        "the skin carries the twelve chrome tones and the light map's two, and no others: {:?}",
        skin.tones.keys().collect::<Vec<_>>()
    );
}

/// **The light map's two inks, as `light_overlay::SHIPPED` holds them** (UI-80, #254), written as
/// literals so the skin is pinned to what the overlay draws with and not to itself.
///
/// The skin file states sRGB (every tone in it does), and `light_overlay::SHIPPED` names the same two
/// colours linearly; these are its `Srgba` values, so the two are one colour stated in two spaces.
const LIGHT_MAP_TONES: [(&str, Color); 2] = [
    ("overlay_dark", Color::srgba(0.24780053, 0.38109186, 0.7014107, 0.35)),
    ("overlay_lit", Color::srgba(1.0, 0.80946815, 0.38109186, 0.85)),
];

#[test]
fn the_light_maps_two_inks_are_the_shipped_pair() {
    let skin = editor_skin();
    for (name, want) in LIGHT_MAP_TONES {
        let got = skin.tone(name).unwrap_or_else(|| panic!("the editor skin has no tone `{name}`"));
        assert_eq!(got, want, "the skin's `{name}` is the pair the overlay falls back to, in sRGB");
    }
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
