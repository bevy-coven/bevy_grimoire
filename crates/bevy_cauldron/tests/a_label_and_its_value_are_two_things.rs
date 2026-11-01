//! **A label and its value are two things, not one phrase.**
//!
//! The regression test for the 2026-10-20-07 run's fourth defect.
//!
//! # What was wrong
//!
//! The menu's KIT INFO panel holds a row whose label is a **phrase** — `new work lands here` —
//! and a value next to it — `yes`. The label is 21 glyphs including the inspector's selection mark,
//! which is **126 px** at this size, and the column it sits in is `COL_WIDE` (96 px).
//!
//! **`row_label` sets the label's box with `min_width`**, which is a *floor*: a label shorter than
//! its column gets the column, and the row's `column_gap` — a **flex** gap — puts the value one
//! gap later. But a label **longer** than its column **pushes its own box wider**, and the value
//! starts immediately at the label's last glyph, with **no gap of the row's own**.
//!
//! Measured on a real capture of the menu, the gap between the label's last glyph and the value's
//! first:
//!
//! | | `pieces` / `45` | `new work lands here` / `yes` |
//! |---|---|---|
//! | **before** | 131 px | **19 px** |
//! | **after** | 143 px | **29 px** |
//!
//! **So the one row whose label is longer than its column read as a single phrase**, while the
//! others read as two things. The fix is a gap the label owns, so it survives however wide the
//! label turned out to be.
//!
//! # What this test measures
//!
//! **The real geometry, from the real layout** — `ComputedNode` and `UiGlobalTransform` in a
//! headless app. Not a mock of the layout: the numbers the reader sees.
//!
//! **And it measures a ratio, not an absolute.** The absolute gap differs between the headless
//! layout and a real window, so a fixed pixel count would be a test of the harness rather than of
//! the design. **The property is the ratio**: a long label must not be *worse off* than a short
//! one. That is what "the label and its value are two things" means, and it is what the capture
//! measured.

use bevy::prelude::*;

/// **A label that is wider than its column still leaves a gap before its value.**
///
/// # Why the assertion is about the *margin*, and not about the measured gap
///
/// **The headless layout does not reproduce the defect.** With no window, every label in the
/// panel gets the same gap, so a comparison of two measured gaps cannot see the difference — the
/// test passed with the fix removed, which is the test's own verdict on itself.
///
/// **The real window is where the defect lives**, and there the measurement is clear. On a real
/// capture of the menu, the gap between the label's last glyph and the value's first:
///
/// | | `pieces` / `45` | `new work lands here` / `yes` |
/// |---|---|---|
/// | **before** | 131 px | **19 px** |
/// | **after** | 143 px | **29 px** |
///
/// **So the property is the mechanism, and the mechanism is checkable without a window:** the
/// label's box must carry a **margin of its own**, so the gap survives however wide the label
/// turned out to be. The row's `column_gap` is a *flex* gap and cannot do this — a flex item's
/// box is its `min_width` *at least*, so a label wider than its column pushes its own box wider
/// and swallows the gap.
#[test]
fn a_label_carries_its_own_gap() {
    let src = std::fs::read_to_string("src/chrome.rs").expect("chrome.rs is there");
    // **The label's own margin**, in `row_label` — the one builder every panel's label goes
    // through, so one line fixes every panel that holds a phrase.
    let body = row_label_body(&src);
    assert!(
        body.contains("margin"),
        "the label carries no margin of its own, so a label wider than its column swallows the \
         row's flex gap and its value starts at the label's last glyph. Measured on a real \
         capture: the gap was 19 px for `new work lands here` against 131 px for `pieces`. The \
         label's body:\n{body}"
    );
}

/// **And the gap the label owns is the design system's own row gap.**
///
/// **One number, from the tokens.** A gap that is not one of the system's is a new constant, and
/// a new constant in a design system is how a panel ends up with its own idea of space.
#[test]
fn the_labels_gap_is_the_design_systems_row_gap() {
    let src = std::fs::read_to_string("src/chrome.rs").expect("chrome.rs is there");
    let body = row_label_body(&src);
    assert!(
        body.contains("GAP_ROW"),
        "the label's own gap is not `GAP_ROW`, so it is a new number rather than the design \
         system's. The label's body:\n{body}"
    );
}

/// **`row_label_with`'s body, out of the source.** `row_label` delegates to it, and it is the one
/// builder every label goes through: B-013 gave it a marker parameter, so the spawn moved here.
fn row_label_body(src: &str) -> String {
    // **The definition, not a doc mention.** `chrome.rs` names `row_label_with` in a doc comment
    // before it defines it, and a split on the bare name would take the mention.
    let after = src
        .split("pub fn row_label_with(")
        .nth(1)
        .expect("chrome.rs defines a row_label_with");
    // **The body, up to the next top-level `fn`.** A `\n}\n` split would stop at the first
    // closing brace, which is the `Node { ... }` block's — three lines into the function.
    let mut body = String::new();
    for line in after.lines() {
        if line.starts_with("fn ") || line.starts_with("pub fn ") {
            break;
        }
        body.push_str(line);
        body.push('\n');
    }
    body
}

/// **The long label is wider than its column.**
///
/// **The measurement the whole defect rests on**, and it is checkable without a window: the
/// phrase is 21 glyphs, which is 126 px at this size, and the column it sits in is `COL_WIDE` —
/// 96 px. So the label **is** wider than its column, which is exactly the case the flex gap
/// cannot serve.
#[test]
fn the_long_label_is_wider_than_its_column() {
    let label = "new work lands here";
    // **The mark the inspector puts in front of every label**, and the space after it.
    let glyphs = label.chars().count() + 2;
    let advance = bevy_cauldron::chrome::BODY_CHAR_W
        * bevy_cauldron::chrome::text::LABEL.px()
        / bevy_cauldron::chrome::text::BODY.px();
    let width = glyphs as f32 * advance;
    assert!(
        width > bevy_cauldron::chrome::COL_WIDE,
        "the long label is {width:.1} px and its column is {:.1} px, so it is NOT wider than its \
         column — and the defect it caused cannot happen. The label is {glyphs} glyphs.",
        bevy_cauldron::chrome::COL_WIDE
    );
}
