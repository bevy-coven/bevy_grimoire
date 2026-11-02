//! **A theme edit reaches the "a machine decided this" ink** (UI-02, #210).
//!
//! `chrome::UNCHECKED` is what the Meshes pane paints a model-written label with, and it is drawn from
//! `rebuild_detail` — one system, both draw sites (the labels row and the kind chips). It reads
//! `GooeySkin::unchecked` now, through `DetailInputs`, so a theme edit reaches every place the editor
//! says *a machine wrote this and nobody has confirmed it*.
//!
//! **The protected test still passes untouched.** `the_meshes_pane_says_how_sure_the_labels_are` asserts
//! the ink equals `chrome::UNCHECKED`, and the skin's `unchecked` is that same colour
//! (`SUGGEST x 1.45`), so reading the skin is behaviour-identical today and no assertion moved. This
//! file is what makes the *next* change — a theme that moves `unchecked` — visible.
//!
//! What is claimed: the ink the pane would use comes from the skin. What is not: that every colour in
//! the pane does. `TEXT`, `LABEL` and `SUGGEST` still read the constants, and that is the rest of #210.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

/// **The pane's unchecked ink is the skin's**, so a theme that moves it moves the pane.
#[test]
fn the_model_written_ink_comes_from_the_skin() {
    let root = fixtures::Fixture::new("theme-unchecked-ink")
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..4 {
        app.update();
    }

    // **The editor's own skin is the constant today**, which is why the protected test above still
    // passes: reading it changed nothing to the eye.
    let skin = app
        .world()
        .resource::<bevy_gooey::GooeySkin>()
        .clone();
    assert_eq!(
        skin.unchecked,
        Some(bevy_cauldron::chrome::UNCHECKED),
        "the skin's `unchecked` and `chrome::UNCHECKED` must be one colour, or the pane's ink moves \
         the moment it starts reading the skin — a change nobody asked for"
    );

    // **And it is a colour a theme can move**, through the one seam the pane reads.
    let mut moved = skin;
    let distinct = Color::srgb(0.97, 0.42, 0.13);
    moved.unchecked = Some(distinct);
    app.insert_resource(moved);
    app.update();

    // **The pane's own seam reads that skin.** `tiles::unchecked_ink` is the expression
    // `rebuild_detail` paints with, so a theme edit that reaches the skin reaches the label — the
    // claim, rather than the weaker "the resource changed".
    let painted = bevy_cauldron::tiles::unchecked_ink(
        app.world().get_resource::<bevy_gooey::GooeySkin>(),
    );
    assert_eq!(
        painted,
        distinct,
        "a theme edit must reach the ink a model-written label is painted in; it is still the \
         constant, so the pane reads a second palette beside the skin"
    );
    // And with no skin at all, the constant — a harness that never ran `EditorUiPlugin` still draws.
    assert_eq!(
        bevy_cauldron::tiles::unchecked_ink(None),
        bevy_cauldron::chrome::UNCHECKED,
        "a build with no skin must still have an ink for a model-written label"
    );
}
