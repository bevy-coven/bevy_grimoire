//! **Every raw `Node` literal of the editor is counted, file by file, and the count may only fall** (#260, UI-99).
//!
//! #260 retires the hand-built widgets: the editor is laid out with bevy_gooey's `row`, `column`, `cell`, `image_box`,
//! `overlay`, `glyph_button`, `edge_tab`, `band` and friends, and a raw `Node` is a second layout path beside them. The
//! per-module ratchets (`the_chrome_helpers_keep_their_raw_nodes_counted.rs`, `the_kit_panels_are_built_from_gooey_widgets.rs`)
//! hold single modules; this one holds the whole crate, so a literal cannot leave a counted module for an uncounted one.
//!
//! # The counting rule
//!
//! The same spellings as those two (`Node {`, `Node{` and `Node::default()` are one construction each), counted by the word,
//! not the substring: `TreeNode {`, `ComputedNode {` and a signature's `-> Node {` are not a `Node` being built, and a
//! substring count would hold a number up that no conversion can lower. Two things are not counted at all, because they are not
//! constructions: a comment or doc line (its first non-blank characters are `//`), and the test code at the foot of a file
//! (everything from a `#[cfg(test)]` line that is followed by a `mod`). The viewport surface (`surface.rs`, `view.rs`) is the
//! one place a raw `Node` stays for good, so it is not counted.
//!
//! # How a number goes down
//!
//! A file's cap is in [`NODES_LEFT`], and the sum of the caps is [`TOTAL_NODES_LEFT`]. The test asserts exact equality for each
//! file and for the total, so a conversion that removes a node must lower its file's cap and the total in the same commit, and a
//! file that is not in the table has none. A cap is never raised: a node that bevy_gooey cannot express is a GAP recorded on
//! the issue, not a new literal.

use crate::fixtures;

/// The files that may still carry raw `Node` literals, with the number each carries (production code only). Sorted by path.
const NODES_LEFT: &[(&str, usize)] = &[
    ("activity_bar.rs", 1),
    ("agent_view.rs", 2),
    ("app_verbs.rs", 1),
    ("badges.rs", 6),
    ("balloon.rs", 1),
    ("chrome.rs", 35),
    ("compass.rs", 3),
    ("compose.rs", 1),
    ("design_areas.rs", 1),
    ("editor.rs", 35),
    ("empty_state.rs", 1),
    ("environment.rs", 1),
    ("familiar.rs", 10),
    ("family_panel.rs", 1),
    ("file_outline.rs", 1),
    ("files.rs", 2),
    ("files_panel.rs", 2),
    ("follow.rs", 1),
    ("game_ui_preview.rs", 1),
    ("jobs.rs", 3),
    ("keymap_screen.rs", 2),
    ("launcher.rs", 23),
    ("loading.rs", 3),
    ("new_project.rs", 1),
    ("problems_view.rs", 2),
    ("project_config.rs", 1),
    ("project_page.rs", 10),
    ("report.rs", 8),
    ("session.rs", 2),
    ("settings_screen.rs", 3),
    ("snip.rs", 2),
    ("terminal_pane.rs", 9),
    ("terminal_view.rs", 1),
    ("ui.rs", 1),
    ("wish_view.rs", 20),
];

/// The sum of [`NODES_LEFT`]: the crate's raw `Node` literals outside the viewport surface. #260 closes when it is 0.
const TOTAL_NODES_LEFT: usize = 197;

/// The viewport surface: the files where a raw `Node` is allowed to stay and is not counted.
const SURFACE_FILES: [&str; 2] = ["surface.rs", "view.rs"];

/// How many raw `Node` constructions `line` holds: the word `Node` (not the tail of `TreeNode`) opening a literal or calling
/// `default()`, and not a return type.
fn constructions(line: &str) -> usize {
    line.match_indices("Node")
        .filter(|(at, _)| {
            let before = &line[..*at];
            let word_start = !before.chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_');
            let rest = &line[at + "Node".len()..];
            let builds = rest.trim_start().starts_with('{') || rest.starts_with("::default()");
            word_start && builds && !before.trim_end().ends_with("->")
        })
        .count()
}

/// How many raw `Node` literals `text` constructs outside comments and the trailing test module.
fn count(text: &str) -> usize {
    let lines: Vec<&str> = text.lines().collect();
    let mut found = 0;
    for (i, line) in lines.iter().enumerate() {
        let opens_tests = line.starts_with("#[cfg(test)]") && lines.get(i + 1).is_some_and(|next| next.trim_start().contains("mod "));
        if opens_tests {
            break;
        }
        if line.trim_start().starts_with("//") {
            continue;
        }
        found += constructions(line);
    }
    found
}

/// Every `.rs` file under `dir`, as a path relative to `root` with `/` separators.
fn sources(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(root, &path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            out.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

#[test]
fn the_editor_carries_no_more_raw_nodes_than_it_is_counted_to() {
    let _ = fixtures::DEFAULT_KIT;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&root, &root, &mut files);
    files.sort();

    let mut total = 0;
    for file in &files {
        if SURFACE_FILES.contains(&file.as_str()) {
            continue;
        }
        let text = std::fs::read_to_string(root.join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        let found = count(&text);
        let left = NODES_LEFT.iter().find(|(name, _)| name == file).map_or(0, |(_, left)| *left);
        assert_eq!(
            found, left,
            "{file} carries {found} raw `Node` literals and is counted at {left}: a raw `Node` is a second layout path beside \
             gooey's `row`, `column`, `cell`, `image_box` and `overlay`; lower the cap (and TOTAL_NODES_LEFT) when one goes, and \
             do not add one: a node gooey cannot express is a GAP recorded on #260"
        );
        total += found;
    }
    assert_eq!(
        total, TOTAL_NODES_LEFT,
        "the editor carries {total} raw `Node` literals outside the viewport surface and is counted at {TOTAL_NODES_LEFT}"
    );
}

#[test]
fn the_table_is_the_total() {
    let sum: usize = NODES_LEFT.iter().map(|(_, left)| left).sum();
    assert_eq!(sum, TOTAL_NODES_LEFT, "NODES_LEFT sums to {sum}, TOTAL_NODES_LEFT says {TOTAL_NODES_LEFT}");
    let names: Vec<&str> = NODES_LEFT.iter().map(|(name, _)| *name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "NODES_LEFT is kept sorted by path");
    assert!(NODES_LEFT.iter().all(|(_, left)| *left > 0), "a file at 0 leaves the table: it has none");
}
