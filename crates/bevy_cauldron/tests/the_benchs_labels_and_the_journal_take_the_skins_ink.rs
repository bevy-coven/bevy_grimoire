//! **The bench's column label and its chip's on-ground take the editor skin's ink** (UI-02, #210,
//! slice 12).
//!
//! Three modules drew themselves from `chrome.rs` constants picked at the call site rather than
//! through a seam, so a project's skin file could not reach them: `anim_tab`'s slot table (its column
//! labels, the kind column, the skip chip's on/off pair, a staleness verdict, the plot captions and
//! the receipt line), `notice`'s journal (the newest refusal and every older one) and `labels`' own
//! line. Each now reads a seam — `anim_tab::BenchInk` grew `label` and `on`, and the other two read
//! `compose::PanelInk`, the editor's existing three-ink seam rather than a second palette — with the
//! constant it replaced as the no-skin fallback.
//!
//! **What this file asks that the earlier slices could not:** whether a skin's *tone map* and its
//! *elevation ladder* reach the two places that need one of each. `the_rig_bench_draws_in_the_skins_ink.rs`
//! owns the five field inks and is not repeated here.

use bevy::prelude::*;
use bevy_cauldron::anim_tab::BenchInk;
use bevy_gooey::GooeySkin;

/// **A skin a person could write**, with its `label` tone and its ladder moved to colours no preset
/// uses — so a passing assertion cannot be a constant coinciding with the field.
fn a_skin_of_its_own() -> GooeySkin {
    let mut skin = GooeySkin::tool();
    skin.tones.insert("label".to_owned(), Color::srgb(0.13, 0.14, 0.15));
    // A ladder whose seventh rung is nothing like the constant it stands in for.
    skin.surfaces = (0..8).map(|i| Color::srgb(0.02 * i as f32, 0.5, 0.03)).collect();
    skin
}

/// **A column's label is the skin's `label` tone, and a chip that is on stands on the ladder's
/// seventh rung** — the two fields this slice added, separately so a failure names the one that
/// stopped following the skin.
#[test]
fn the_benchs_label_and_on_ground_are_the_skins() {
    let ink = BenchInk::of(Some(&a_skin_of_its_own()));
    assert_eq!(ink.label, Color::srgb(0.13, 0.14, 0.15), "a column's label is the skin's `label` tone");
    assert_eq!(ink.on, Color::srgb(0.12, 0.5, 0.03), "a chip that is on stands on rung 6");
}

/// **A skin with no `label` tone still draws one**, from the constant the call site used, so a
/// project that restyles only the fields does not lose the column labels to `Color::NONE`.
#[test]
fn a_skin_without_the_tone_keeps_the_editors_label() {
    let mut skin = GooeySkin::tool();
    skin.tones.remove("label");
    let ink = BenchInk::of(Some(&skin));
    assert_eq!(ink.label, BenchInk::of(None).label, "no tone means the shipped label, not nothing");
}

/// **With no skin at all the two new fields are the shipped table**, so a harness that never ran
/// `EditorUiPlugin` draws exactly what it drew before this slice — which is what keeps every
/// protected assertion written earlier true.
#[test]
fn with_no_skin_the_new_fields_are_the_shipped_table() {
    let ink = BenchInk::of(None);
    let shipped = BenchInk::shipped();
    assert_eq!(ink.label, shipped.label);
    assert_eq!(ink.on, shipped.on);
}

/// **The editor's own skin carries both**, so the running editor and a harness draw one bench: the
/// tone it names and the rung it reads are the two constants the call sites used.
#[test]
fn the_editor_skin_carries_both() {
    let skin = bevy_cauldron::ui::editor_skin();
    let ink = BenchInk::of(Some(&skin));
    assert_eq!(ink.label, BenchInk::of(None).label, "the editor skin's `label` tone is the shipped one");
    assert_eq!(ink.on, BenchInk::of(None).on, "and its seventh rung is the shipped ground");
}
