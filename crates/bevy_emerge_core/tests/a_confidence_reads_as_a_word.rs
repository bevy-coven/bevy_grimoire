//! **A confidence reads as one word, the same everywhere it is shown (cycle 8, item 3).**
//!
//! The Tiles proposal line and the Meshes labels row both show a model's confidence. Before this,
//! the proposal line printed the Rust variant (`High`) and the labels row said nothing at all. The
//! one spelling lives on `Confidence`'s `Display`, so the two surfaces cannot disagree.

use bevy_emerge_core::descriptor::Confidence;

#[test]
fn each_confidence_is_one_lower_case_word() {
    assert_eq!(Confidence::High.to_string(), "high");
    assert_eq!(Confidence::Medium.to_string(), "medium");
    assert_eq!(Confidence::Low.to_string(), "low");
}
