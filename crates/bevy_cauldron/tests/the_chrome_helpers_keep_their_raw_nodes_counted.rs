//! **The raw `Node` literals of `chrome.rs` are counted, so layout cannot hide in a helper** (UI-59, #245).
//!
//! `the_kit_panels_are_built_from_gooey_widgets.rs` counts the Kit modules' literals. The Kit panels reach gooey's rows
//! through `chrome` helpers (`spaced`, `spaced_row`, `gapped_row`, `wrapping_row`, `fill_text`), and a literal moved into
//! `chrome.rs` would leave that count and appear here instead: it is a number that may only go down.

use crate::fixtures;

/// The number of raw `Node` literals `chrome.rs` may carry, counted as the Kit ratchet counts (`Node {`, `Node{`,
/// `Node::default()`). A new one needs a reason in the commit that adds it and this number raised by the same amount.
// 42 -> 38: #260 moved the heading and title margins and the row helpers' margins onto gooey's `Spacing`. Owner exception (2026-11-14): a ratchet tightened to the real count.
// 38 -> 34: #260 moved the problem line, the row labels and the wrapping text onto gooey's `TextFlow`, `Sizing` and `Spacing`. Owner exception (2026-11-14): a ratchet tightened to the real count.
// 34 -> 33: #260 moved the frame root's height onto gooey's `Extent::Vh`. Owner exception (2026-11-14): a ratchet tightened to the real count.
// 33 -> 31: #260 moved the modal card, the severity rail and the frame root onto gooey's column, px border and Extent::Vh. Owner exception (2026-11-14): a ratchet tightened to the real count.
// 31 -> 13: #260 round 8 moved the frame, the chips, the buttons, the rows, the text field, the name box and the toast strip onto gooey rows, columns and placed nodes (G-87). Owner exception (2026-11-14): a ratchet tightened to the real count.
// 13 -> 10: #260 round 8: the back button and the bars' spacers are gooey's (G-87). Owner exception (2026-11-14): a ratchet tightened to the real count.
const CHROME_NODES_LEFT: usize = 10;

#[test]
fn chrome_rs_carries_no_more_raw_nodes_than_it_is_counted_to() {
    let _ = fixtures::DEFAULT_KIT;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/chrome.rs");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let found = text
        .lines()
        .filter(|line| line.contains("Node {") || line.contains("Node{") || line.contains("Node::default()"))
        .count();
    assert_eq!(
        found, CHROME_NODES_LEFT,
        "chrome.rs carries {found} raw `Node` literals and is counted at {CHROME_NODES_LEFT}: a helper that lays out with a raw \
         `Node` is a second layout path beside gooey's `row`, `column` and `cell`; lower the number when one goes"
    );
}
