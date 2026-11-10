//! **The theme, the text size and the interface size reach the editor's own chrome** (UI-73, #251).
//!
//! Pass 183 landed the half where the theme and the text size reach **gooey's** running skin
//! (`tests/the_theme_is_a_skin_and_the_text_fits.rs`: High contrast applies, Parchment is refused with
//! its measured pair, `problems()` is empty, 200% text fits every panel). What it could not do was
//! reach the chrome's **own** labels and sizes, and said so on the issue: "`chrome::font` labels do not
//! yet follow the text size; interface size is not wired."
//!
//! # The two items, as tests
//!
//! - **`chrome::font` follows the text size.** `crates/bevy_cauldron/src/chrome.rs` has 117 call sites
//!   drawing the editor's hand-made labels, and every one of them asks `chrome::font(role)`. A role's
//!   size is a fact of the type scale and the person's multiple is a fact about the editor, so the
//!   running scale is one atomic — [`follow_the_text_size`] is its only writer — and a label is drawn
//!   at `role.px() * scale`. The property is stated both ways: the accessor moves, and a `TextFont`
//!   the crate makes through `chrome::font` is that many times larger.
//! - **Interface size is wired.** `UiScale` multiplies every `Val::Px` and every font size, so the
//!   setting is a multiple of the fit the editor already computes: 125% is 1.25 times the interface
//!   the window would otherwise draw. [`crate::surface::ui_scale_for_a_person`] is that product, and
//!   the test holds it to the arithmetic rather than to a number it invented.
//!
//! # Why the atomic and not a parameter
//!
//! `chrome::font` takes a `text::Role` — a type that says "a role, never a number" — and nothing else.
//! Adding a scale parameter would be 117 call sites plus every helper that calls them, and every one
//! would have to thread a setting through a spawn closure it cannot see. The atomic keeps the type
//! scale a type and makes the person's multiple global, which is what it is: there is one editor.
//!
//! **A test that leaves it set restores it**, and `scale_is_restored` is the guard, because a value
//! shared by every test in this binary is exactly the sort of thing one test can break for another.

use bevy::prelude::*;
use bevy_cauldron::chrome::{self, text};
use bevy_cauldron::settings::{Theme, UserSettingsFile};
use bevy_cauldron::surface::ui_scale_for_a_person;

use crate::fixtures;

/// A `TextFont`'s size in logical pixels. `chrome::font` always makes a `FontSize::Px`, so a role
/// that came back as anything else would be a change this test should fail on rather than a number.
fn font_size_px(font: bevy::text::TextFont) -> f32 {
    match font.font_size {
        bevy::text::FontSize::Px(px) => px,
        other => panic!("chrome::font makes a FontSize::Px, not {other:?}"),
    }
}

/// A scale, set and restored around whatever a test asserts, so one test cannot break another.
fn with_scale<T>(scale: f32, body: impl FnOnce() -> T) -> T {
    let was = chrome::text_scale();
    chrome::set_text_scale(scale);
    let out = body();
    chrome::set_text_scale(was);
    out
}

/// **`chrome::font` is the role's size times the person's multiple.** The whole of the first item, at
/// the one function every hand-drawn label goes through.
#[test]
fn a_chrome_label_is_drawn_at_the_persons_text_size() {
    let at_design = with_scale(1.0, || font_size_px(chrome::font(text::BODY)));
    assert!(
        (at_design - text::BODY.px()).abs() < f32::EPSILON,
        "at 100% a role is drawn at its designed size: {at_design} vs {}",
        text::BODY.px()
    );
    let at_double = with_scale(2.0, || font_size_px(chrome::font(text::BODY)));
    assert!(
        (at_double - text::BODY.px() * 2.0).abs() < f32::EPSILON,
        "at 200% the same role is twice as large: {at_double} vs {}",
        text::BODY.px() * 2.0
    );
    // **Every role moves, and none of them changes into another**: the ratio between two roles is the
    // type scale's, whatever a person chose, which is what keeps the hierarchy the same one.
    let (body, hint) = with_scale(1.5, || (font_size_px(chrome::font(text::BODY)), font_size_px(chrome::font(text::HINT))));
    assert!((body / hint - text::BODY.px() / text::HINT.px()).abs() < 1e-5, "1.5x keeps the hierarchy: {body} / {hint}");
}

/// **The scale is clamped to the range a settings file may hold**, so a file that asks for 50x draws
/// a label of a sane size rather than one taller than the window.
#[test]
fn the_scale_a_label_is_drawn_at_is_the_range_a_file_may_hold() {
    let was = chrome::text_scale();
    chrome::set_text_scale(50.0);
    let high = chrome::text_scale();
    chrome::set_text_scale(-1.0);
    let low = chrome::text_scale();
    chrome::set_text_scale(was);
    assert!((high - bevy_cauldron::settings::SCALE_RANGE.1).abs() < 1e-3, "clamped high: {high}");
    assert!((low - bevy_cauldron::settings::SCALE_RANGE.0).abs() < 1e-3, "clamped low: {low}");
}

/// **A settings file's text size reaches the stored scale through the running editor.**
///
/// **What this proves, and what it does not.** `follow_the_text_size` copies the setting into the atomic
/// `chrome::font` reads, and this is that copy, made by the running editor from a file. It does **not**
/// prove a label already on screen resized: gooey and this crate write a `TextFont` once, when a widget
/// is spawned, so a label built in `OnEnter(Editor)` keeps the size it was made at. Live resize needs a
/// marker on every label and a system that rewrites its `TextFont`, which is owed (see #251).
#[test]
fn the_settings_files_text_size_reaches_the_stored_scale() {
    let root = fixtures::Fixture::new(&format!("text_scale_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let settings_file = root.join(format!("user_config_text_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(settings_file.parent().expect("a settings folder")).expect("a settings folder");
    std::fs::write(&settings_file, "{version: 1, text_scale: 1.75}").expect("a settings file");

    let was = chrome::text_scale();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(settings_file));
    for _ in 0..12 {
        app.update();
    }
    let drawn = chrome::text_scale();
    chrome::set_text_scale(was);
    assert!((drawn - 1.75).abs() < 1e-3, "the file's 175% is what a hand-drawn label is scaled by: {drawn}");
}

/// **Interface size is wired**: the person's multiple is a factor of the same `UiScale` fit, so 125%
/// enlarges the whole interface and 100% leaves exactly what the editor drew before.
#[test]
fn the_interface_size_multiplies_the_ui_scale() {
    let (w, sf) = (1600.0_f32, 2.0_f32);
    let at_design = ui_scale_for_a_person(w, sf, 1.0);
    assert_eq!(at_design, bevy_cauldron::surface::ui_scale_for(w, sf), "100% is the fit the editor already had");
    let larger = ui_scale_for_a_person(w, sf, 1.25);
    assert!((larger - at_design * 1.25).abs() < 1e-5, "125% is 1.25 times the interface: {larger} vs {at_design}");
}

/// **A window's interface size setting reaches `UiScale`**, which is the value every `Val::Px` and
/// every font size is multiplied by — so this is "the panels grew", not "a number moved".
///
/// **Through the settings file, not the resource.** `sync_settings` re-resolves `EditorSettings` from
/// the two files every frame, so a test that assigned the resource would have it overwritten before
/// the fit read it and would prove nothing — the same reason the theme tests write a file.
#[test]
fn the_settings_files_interface_size_reaches_the_ui_scale() {
    use bevy::ui::UiScale;
    use bevy::window::{PrimaryWindow, Window};

    let at = |tag: &str, contents: &str| {
        let root = fixtures::Fixture::new(&format!("ui_scale_{tag}_{}", std::process::id()))
            .descriptor("site/wall", "alpha")
            .build("m");
        let settings_file = root.join(format!("user_config_ui_{tag}_{}/settings.yaml", std::process::id()));
        std::fs::create_dir_all(settings_file.parent().expect("a settings folder")).expect("a settings folder");
        std::fs::write(&settings_file, contents).expect("a settings file");
        let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
        app.insert_resource(UserSettingsFile(settings_file));
        // A window, because the fit is where the scale is written and it returns without one — the
        // headless harness has no window on purpose.
        app.world_mut().spawn((
            Window { resolution: bevy::window::WindowResolution::new(1400, 900), ..default() },
            PrimaryWindow,
        ));
        for _ in 0..12 {
            app.update();
        }
        app.world().resource::<UiScale>().0
    };
    let at_design = at("design", "{version: 1}");
    let larger = at("large", "{version: 1, interface_scale: 1.25}");
    assert!(at_design > 0.0, "the fit ran at all: {at_design}");
    assert!(
        (larger - at_design * 1.25).abs() < 1e-4,
        "the file's 125% multiplies the interface the window would have drawn: {larger} vs {at_design}"
    );
}

/// **High contrast still breaks no rule of the library's own**, which is the other half of #251's
/// acceptance and the reason a theme is a preset rather than a palette this crate derives.
#[test]
fn the_high_contrast_theme_keeps_passing_the_librarys_rules() {
    let skin = bevy_cauldron::ui::skin_for(Theme::HighContrast);
    assert_eq!(skin.problems(), vec![], "a theme the editor offers must pass `problems()`");
}
