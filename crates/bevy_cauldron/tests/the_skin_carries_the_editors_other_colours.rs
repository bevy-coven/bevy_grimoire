//! **The editor skin carries the twelve colours no `GooeySkin` field has, as tones** (UI-02, #210, slice 2).
//!
//! `chrome.rs` draws with `Ink::key`, `Ink::label`, `Ink::muted`, `Ink::suggest`, `Ink::labeled`, `Ink::grid_line`, `Ink::bounds_line`, `Ink::veil`,
//! `Ink::focus_bg`, `Ink::row_pressed`, `Ink::snip_wash` and `Ink::overlay_bg`, none of which is a field of `GooeySkin`. They are
//! the skin's `tones` now (`src/editor_skin.ron`), so a project's skin file can change them and gooey can check
//! the text pairs among them. Each constant is still what `chrome.rs` draws with until UI-99 moves its readers to
//! `skin.tone(..)`, so this says the two are one colour: a tone edited without its constant (or the reverse) fails here.

use bevy::prelude::*;
use bevy_cauldron::chrome;
use bevy_cauldron::ui::editor_skin;

fn colours() -> [(&'static str, Color); 12] {
    [
        ("bounds_line", chrome::ink().bounds_line),
        ("focus_bg", chrome::ink().focus_bg),
        ("grid_line", chrome::ink().grid_line),
        ("key", chrome::ink().key),
        ("label", chrome::ink().label),
        ("labeled", chrome::ink().labeled),
        ("muted", chrome::ink().muted),
        ("overlay_bg", chrome::ink().overlay_bg),
        ("row_pressed", chrome::ink().row_pressed),
        ("snip_wash", chrome::ink().snip_wash),
        ("suggest", chrome::ink().suggest),
        ("veil", chrome::ink().veil),
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
