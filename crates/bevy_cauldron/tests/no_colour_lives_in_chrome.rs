//! **`chrome.rs` declares no colour of its own** (UI-02, #210).
//!
//! #210's acceptance is *"`chrome.rs` declares no colour constants, and the editor skin's
//! `problems()` is empty"*. Two slices took the skin half (`src/editor_skin.ron`, the state roles of
//! G-33); this is the other half: the 28 constants are gone and every painter asks
//! [`chrome::Ink`], which reads the editor's own skin.
//!
//! # Why a scan and not a compile error
//!
//! A constant that comes back is not caught by the type system — a fresh
//! `pub const TEXT: Color = Color::srgb(…)` beside the ink it duplicates compiles and draws. The scan
//! is the check the ratchet in `the_colour_table_only_shrinks` used to be, at the state it always
//! wanted: **zero**, so there is nothing left to count.
//!
//! # What "unchanged to the eye" is pinned to
//!
//! [`Ink::editor`] is the editor's measured palette, and the second test states every one of its 28
//! colours as the value the old table held. So moving the painters onto the ink is provably a
//! refactor: a retuned colour fails here with its name, which is the half of the second acceptance
//! item a test can hold while the clip is not yet recorded.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_cauldron::chrome::{self, Ink};
use bevy_cauldron::ui::editor_skin;

use crate::fixtures;

/// Every `pub const NAME: Color` in `chrome.rs`. The same scan `the_colour_table_only_shrinks` ran,
/// at the number #210 asks for.
fn colour_constants() -> Vec<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/chrome.rs");
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let mut names = Vec::new();
    for line in src.lines() {
        let Some(rest) = line.strip_prefix("pub const ") else { continue };
        let Some((name, tail)) = rest.split_once(':') else { continue };
        if !tail.trim_start().starts_with("Color") {
            continue;
        }
        if name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
            names.push(name.to_string());
        }
    }
    names
}

#[test]
fn chrome_declares_no_colour_constant() {
    let names = colour_constants();
    assert!(
        names.is_empty(),
        "chrome.rs still declares {} colour constant(s): {names:?}. Every colour the chrome paints \
         comes from `chrome::Ink`, which is the editor's skin (`src/editor_skin.ron`) — a constant \
         beside it is a second palette, and a second palette is a colour a theme can never reach.",
        names.len()
    );
}

/// The 28 colours, as the table held them on 2026-11-03. **This is the "unchanged to the eye"
/// assertion**: the ink is read from the skin, and the skin still holds these measured values.
#[test]
fn the_ink_holds_the_colours_the_table_held() {
    let ink = Ink::editor();
    let want: [(&str, Color, Color); 28] = [
        ("panel_bg", ink.panel_bg, Color::srgb(0.115, 0.107, 0.093)),
        ("row_bg", ink.row_bg, Color::srgb(0.165, 0.153, 0.134)),
        ("row_hover", ink.row_hover, Color::srgb(0.212, 0.197, 0.172)),
        ("row_selected", ink.row_selected, Color::srgb(0.248, 0.231, 0.201)),
        ("row_pressed", ink.row_pressed, Color::srgb(0.110, 0.102, 0.089)),
        ("text", ink.text, Color::srgb(0.880, 0.865, 0.835)),
        ("dim", ink.dim, Color::srgb(0.700, 0.685, 0.655)),
        ("accent", ink.accent, Color::srgb(0.950, 0.710, 0.300)),
        ("key", ink.key, Color::srgb(0.790, 0.770, 0.735)),
        ("label", ink.label, Color::srgb(0.615, 0.600, 0.575)),
        ("danger", ink.danger, Color::srgb(0.980, 0.520, 0.460)),
        ("scrim", ink.scrim, Color::srgba(0.0, 0.0, 0.0, 0.55)),
        ("veil", ink.veil, Color::srgba(0.0, 0.0, 0.0, 0.35)),
        ("muted", ink.muted, Color::srgb(0.575, 0.560, 0.540)),
        ("suggest", ink.suggest, Color::srgb(0.530, 0.710, 0.810)),
        ("unchecked", ink.unchecked, Color::srgb(0.7685, 1.0295, 1.1745)),
        ("labeled", ink.labeled, Color::srgb(0.550, 0.830, 0.500)),
        ("slot_bg", ink.slot_bg, Color::srgb(0.185, 0.172, 0.150)),
        ("overlay_bg", ink.overlay_bg, Color::srgb(0.195, 0.181, 0.158)),
        ("focus_bg", ink.focus_bg, Color::srgb(0.088, 0.082, 0.071)),
        ("header_bg", ink.header_bg, Color::srgb(0.140, 0.130, 0.113)),
        ("grid_line", ink.grid_line, Color::srgb(0.300, 0.279, 0.243)),
        ("bounds_line", ink.bounds_line, Color::srgb(0.520, 0.470, 0.370)),
        ("void", ink.void, Color::srgb(0.075, 0.070, 0.061)),
        ("bar_bg", ink.bar_bg, Color::srgb(0.1443, 0.13416, 0.117)),
        ("panel_edge", ink.panel_edge, Color::srgb(0.325, 0.302, 0.263)),
        ("edge_strong", ink.edge_strong, Color::srgb(0.440, 0.409, 0.356)),
        ("snip_wash", ink.snip_wash, Color::srgba(0.950, 0.710, 0.300, 0.28)),
    ];
    let mut wrong = Vec::new();
    for (name, got, expected) in want {
        if got.to_srgba() != expected.to_srgba() {
            wrong.push(format!("{name}: {got:?} is not {expected:?}"));
        }
    }
    assert!(wrong.is_empty(), "the ink is not the measured palette, so the chrome changed to the eye:\n{}", wrong.join("\n"));
}

/// **The ink is the running skin, not a copy of it.** A skin edited in the editor's file reaches
/// [`Ink::of`] with no second table in between — the failure this guards is a constant list that
/// agrees with the shipped file on the day it is written and drifts the first time either moves.
#[test]
fn the_ink_is_read_from_the_skin_it_is_given() {
    let mut skin = editor_skin();
    skin.accent = Color::srgb(0.1, 0.2, 0.3);
    skin.text_muted = Color::srgb(0.4, 0.5, 0.6);
    skin.tones.insert("key".to_owned(), Color::srgb(0.7, 0.8, 0.9));
    let ink = Ink::of(&skin);
    assert_eq!(ink.accent, Color::srgb(0.1, 0.2, 0.3), "the accent is the skin's");
    assert_eq!(ink.dim, Color::srgb(0.4, 0.5, 0.6), "`dim` is the skin's muted ink");
    assert_eq!(ink.key, Color::srgb(0.7, 0.8, 0.9), "a tone is the skin's tone");
    assert_eq!(ink.text, skin.text, "and the body ink is the skin's");
}

/// **The editor's own chrome never takes the project's skin** (UI-03, #211): `Ink::editor()` is the
/// editor's shipped skin whatever a project ships, which is what keeps a game's palette inside the
/// Game UI preview panel.
#[test]
fn the_editor_ink_is_the_editors_skin_whatever_the_project_ships() {
    let root = fixtures::Fixture::new(&format!("ink_editor_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    std::fs::create_dir_all(root.join("family")).expect("a family folder");
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_gooey\"])").expect("a family.ron");
    std::fs::write(root.join("family/bevy_gooey.ron"), "(accent: Srgba((red: 0.2, green: 0.6, blue: 0.9, alpha: 1.0)))")
        .expect("a project skin");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // The family settings are read over the first frames, which is when `ui::_skin::apply_ui_skin`
    // puts the project's file into the running skin.
    for _ in 0..12 {
        app.update();
    }
    let running = app.world().resource::<bevy_gooey::GooeySkin>();
    assert_eq!(running.accent, Color::srgba(0.2, 0.6, 0.9, 1.0), "the project skin is running");
    assert_eq!(
        Ink::editor().accent,
        editor_skin().accent,
        "the editor's own ink is its own skin, not the project's accent"
    );
}

/// The chrome module still exists and still names its ink — a scan that found nothing because the
/// file moved would pass the first test for the wrong reason.
#[test]
fn the_scan_reads_the_right_file() {
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/chrome.rs"))
        .expect("chrome.rs is where this crate's furniture lives");
    assert!(src.contains("pub fn panel_root"), "chrome.rs is the module this test scans");
    // And `chrome` is reachable under the name the chrome itself uses.
    assert_eq!(chrome::Ink::editor().panel_bg, editor_skin().surface, "the panel's ground is the skin's surface");
}
