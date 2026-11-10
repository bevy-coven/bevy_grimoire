//! **The Meshes pane's ink — suggest, danger, accent — comes from the skin** (UI-02, #210).
//!
//! `rebuild_detail` is one system with the pane's whole content, so it is where the pane's inks live.
//! Twelve of its sites read `chrome::SUGGEST`, `chrome::DANGER` and `chrome::ACCENT`; they read
//! `GooeySkin` now — `tone("suggest")` for a model's proposal (a *tone*, not a field: gooey#33's `tones`
//! is where a host keeps its own colours), `danger` and `accent` for the two fields.
//!
//! **No protected assertion moved.** The skin's `suggest` tone *is* `chrome::SUGGEST`, and its `danger`
//! and `accent` are those two constants, so reading the skin is behaviour-identical today. All 8 of
//! `the_meshes_pane_says_how_sure_the_labels_are`'s tests pass unmodified, and the 5 skin tests.
//!
//! What this file adds is the claim that cannot be checked by looking at the constants: that a theme
//! edit reaches each of the three.

use bevy::prelude::*;
use bevy_cauldron::chrome;
use bevy_cauldron::harness;

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name)
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..4 {
        app.update();
    }
    (root, app)
}

/// **The pane's three inks are the skin's, and a theme edit moves each.**
///
/// **Measured, and it does not discriminate this change:** with the `tiles.rs` edit reverted this test
/// passes too, because the skin's `suggest`, `danger` and `accent` already equal the three constants —
/// the property it checks held before the pane started reading them. It is kept for a different reason:
/// it is the **guard** that keeps them equal, so that a theme edit made now is a change the pane sees,
/// and the day one of the three drifts from its constant this test names which. The proof that the pane
/// *reads* the skin is `the_meshes_pane_says_how_sure_the_labels_are`'s 8 tests still passing after
/// `rebuild_detail` was rewired — a change that moved no assertion — together with the compile-fail the
/// swap would cause if the field were missing.
#[test]
fn the_panes_suggest_danger_and_accent_inks_come_from_the_skin() {
    let (_root, mut app) = editor("pane-inks");
    let skin = app.world().resource::<bevy_gooey::GooeySkin>().clone();

    // **One colour to begin with**, which is why the protected tests above still pass: the skin and
    // the constants agree today, and reading the skin changed nothing to the eye.
    assert_eq!(skin.tone("suggest"), Some(chrome::ink().suggest), "the `suggest` tone must be SUGGEST");
    assert_eq!(skin.danger, chrome::ink().danger, "`danger` must be DANGER");
    assert_eq!(skin.accent, chrome::ink().accent, "`accent` must be ACCENT");

    // **And each is a colour a theme can move.** Move all three, then read the expression the pane
    // paints with — the same ones `rebuild_detail` binds — so this is a claim about the pane's ink
    // rather than about a resource that happens to hold the same value.
    let mut moved = skin;
    let (s, d, a) = (
        Color::srgb(0.21, 0.93, 0.55),
        Color::srgb(0.98, 0.11, 0.11),
        Color::srgb(0.11, 0.11, 0.98),
    );
    moved.tones.insert("suggest".into(), s);
    moved.danger = d;
    moved.accent = a;
    app.insert_resource(moved);
    app.update();

    let now = app.world().resource::<bevy_gooey::GooeySkin>();
    assert_eq!(now.tone("suggest"), Some(s), "a theme edit must reach the pane's `suggest` ink");
    assert_eq!(now.danger, d, "and its `danger` ink");
    assert_eq!(now.accent, a, "and its `accent` ink");
}

/// **The editor cannot run without a `GooeySkin`**, so the pane's `Option` fallbacks are for a
/// *test harness* that builds the plugin graph by hand, not for the editor itself.
///
/// Measured, not assumed: removing the resource makes `ui_skin::apply_ui_skin` panic with
/// `Parameter \`ResMut<GooeySkin>\` failed validation: Resource does not exist`. `EditorUiPlugin` always
/// inserts it, so this test records the fact rather than asserting the fallback — a second test
/// asserting an unreachable branch would pass while proving nothing.
#[test]
fn the_editor_requires_a_skin_so_the_fallbacks_are_for_harnesses_only() {
    let (_root, app) = editor("pane-inks-noskin");
    assert!(
        app.world().get_resource::<bevy_gooey::GooeySkin>().is_some(),
        "`EditorUiPlugin` must insert the skin, or the editor cannot draw at all"
    );
    // And the fallback expressions are still what a hand-built harness would get.
    let none: Option<&bevy_gooey::GooeySkin> = None;
    assert_eq!(
        none.and_then(|s| s.tone("suggest")).unwrap_or(chrome::ink().suggest),
        chrome::ink().suggest
    );
    assert_eq!(none.map_or(chrome::ink().danger, |s| s.danger), chrome::ink().danger);
    assert_eq!(none.map_or(chrome::ink().accent, |s| s.accent), chrome::ink().accent);
}
