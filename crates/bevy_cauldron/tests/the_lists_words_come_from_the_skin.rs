//! **A theme edit reaches the shared list's words** (UI-02, #210, the `tiles.rs` seam).
//!
//! `draw_ink` was the largest remaining reader of `chrome::ink().text` and `chrome::ink().dim` in the editor — 181
//! colour uses in `tiles.rs`, most of them through this one function and its builders — and none of
//! them could see a project's skin file. The three inks it draws a row's own words in now come from
//! `ListInk`, which reads the running `GooeySkin` and falls back to the constants when no skin is up (a
//! headless harness that never added `EditorUiPlugin`).
//!
//! **What is claimed:** the ink the list *would* use comes from the skin, and the fallbacks are exactly
//! the constants, so nothing moves to the eye until a project edits its skin. **What is not:** that
//! every colour in `tiles.rs` does — `Ink::panel_bg`, `Ink::row_selected`, the severity rail and the rest still
//! read the constants, and that is the rest of #210. The count is in the report.

use bevy::prelude::*;
use bevy_cauldron::chrome;
use bevy_cauldron::harness;
use bevy_cauldron::tiles::ListInk;

use crate::fixtures;

fn skin(app: &App) -> bevy_gooey::GooeySkin {
    app.world().resource::<bevy_gooey::GooeySkin>().clone()
}

/// **The editor's own skin is the constants today**, which is why no protected assertion moved when
/// the list started reading it: `Ink::text`, `Ink::dim`, `Ink::accent` and `Ink::danger` are the skin's own four values.
///
/// This is the load-bearing half. If the skin and the constants disagreed, this change would have
/// altered the editor's appearance without anybody asking, and the protected tests that compare ink to
/// a constant would have caught it — this says so before they have to.
#[test]
fn the_lists_ink_is_the_skins_and_the_skin_is_the_palette() {
    let root = fixtures::Fixture::new("list-ink-is-the-skin")
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }

    let running = skin(&app);
    let words = ListInk::of(Some(&running));
    assert_eq!(words.read, chrome::ink().text, "a row's own words are `TEXT`");
    assert_eq!(words.quiet, chrome::ink().dim, "a quiet part is `DIM`");
    assert_eq!(words.accent, chrome::ink().accent, "the cursor's row is `ACCENT`");
    assert_eq!(words.danger, chrome::ink().danger, "a refusal is `DANGER`");
}

/// **With no skin at all the list falls back to the constants**, which is the harness case: a test that
/// builds the panel without `EditorUiPlugin` must paint the shipped palette, not the library's default.
#[test]
fn a_list_with_no_skin_paints_the_shipped_palette() {
    let words = ListInk::of(None);
    assert_eq!(words.read, chrome::ink().text);
    assert_eq!(words.quiet, chrome::ink().dim);
    assert_eq!(words.accent, chrome::ink().accent);
    assert_eq!(words.danger, chrome::ink().danger);
}

/// **And a theme that moves `text` moves the list's words** — the change this seam exists for. Asked of
/// the same expression `draw_ink` reads, so an assertion here is an assertion about what is painted.
#[test]
fn a_theme_that_moves_the_text_moves_the_lists_words() {
    let root = fixtures::Fixture::new("list-ink-moves")
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }

    let mut edited = skin(&app);
    let moved = Color::srgb(0.20, 0.90, 0.40);
    edited.text = moved;
    let words = ListInk::of(Some(&edited));
    assert_eq!(words.read, moved, "a project's `text` reaches a row's words");
    assert_eq!(
        words.quiet, chrome::ink().dim,
        "and the fields the theme did not touch keep the editor's own values"
    );
}
