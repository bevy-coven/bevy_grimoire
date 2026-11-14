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
//! substring count would hold a number up that no conversion can lower. **A hand patch counts too**: `entry::<Node>()` and
//! `get_mut::<Node>()` rewrite a field of a spawned node (the alignment over a gooey row, say), which is raw layout the
//! constructions' count cannot see, so each is one more. What is not counted: a comment (a whole line, a trailing `// ..`, a
//! `/* .. */` block) and a test module. **A test module is skipped as an item, not to the end of the file**: production code
//! follows a mid-file test module (`anim_stage.rs`, `chrome.rs`), and an earlier version of this scan stopped at the first one
//! and never counted it. The viewport surface (`surface.rs`, `view.rs`) is the one place a raw `Node` stays for good, so it is
//! not counted.
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
    ("chrome.rs", 7),
    ("terminal_pane.rs", 6),
    ("terminal_view.rs", 1),
];

/// The sum of [`NODES_LEFT`]: the crate's raw `Node` literals outside the viewport surface. #260 closes when it is 0.
const TOTAL_NODES_LEFT: usize = 14;

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

/// How many times `line` reaches into a spawned entity's `Node` to rewrite a field (`entry::<Node>()`, `get_mut::<Node>()`): a hand
/// patch over a gooey bundle, which is raw layout all the same.
fn patches(line: &str) -> usize {
    line.matches("entry::<Node>").count() + line.matches("get_mut::<Node>").count()
}

/// **`text` without its comments**, line for line: a `//` comment (whole line or trailing) and a `/* .. */` block (which may span
/// lines) are dropped; a string or a char literal is kept whole, so a `//` inside one (a URL) cuts nothing.
fn without_comments(text: &str) -> Vec<String> {
    let mut in_block = false;
    let mut in_string = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        let chars: Vec<char> = line.chars().collect();
        let mut code = String::new();
        let mut at = 0;
        while at < chars.len() {
            let c = chars[at];
            let next = chars.get(at + 1).copied();
            if in_block {
                if c == '*' && next == Some('/') {
                    in_block = false;
                    at += 2;
                } else {
                    at += 1;
                }
            } else if in_string {
                code.push(c);
                if c == '\\' {
                    if let Some(escaped) = next {
                        code.push(escaped);
                    }
                    at += 2;
                } else {
                    if c == '"' {
                        in_string = false;
                    }
                    at += 1;
                }
            } else if c == '/' && next == Some('/') {
                break;
            } else if c == '/' && next == Some('*') {
                in_block = true;
                at += 2;
            } else if c == '"' {
                in_string = true;
                code.push(c);
                at += 1;
            } else if c == '\'' {
                // A char literal (`'"'`, `'\\n'`) is kept whole; a lifetime has no closing quote two or three chars on.
                let width = if next == Some('\\') { chars[at + 1..].iter().position(|ch| *ch == '\'').map(|p| p + 2) } else if chars.get(at + 2) == Some(&'\'') { Some(3) } else { None };
                let width = width.unwrap_or(1);
                code.extend(&chars[at..(at + width).min(chars.len())]);
                at += width;
            } else {
                code.push(c);
                at += 1;
            }
        }
        lines.push(code);
    }
    lines
}

/// **How many raw `Node` constructions and patches `text` holds in production code**: comments are not counted, and a test module
/// is skipped *as an item*, not for the rest of the file (production code follows a mid-file test module in `anim_stage.rs` and
/// `chrome.rs`): a `#[cfg(test)]` line that is, or is followed by, a `mod` skips that module (the one line when it ends in `;`,
/// else down to the next line that is exactly `}` at column 0), and counting goes on after it.
fn count(text: &str) -> usize {
    const ATTRIBUTE: &str = "#[cfg(test)]";
    let raw: Vec<&str> = text.lines().collect();
    let code = without_comments(text);
    let mut found = 0;
    let mut at = 0;
    while at < raw.len() {
        if let Some(after) = raw[at].strip_prefix(ATTRIBUTE) {
            let on_this_line = after.trim_start().starts_with("mod ");
            let on_next_line = after.trim().is_empty() && raw.get(at + 1).is_some_and(|next| next.trim_start().starts_with("mod "));
            if on_this_line || on_next_line {
                let module = if on_this_line { at } else { at + 1 };
                at = if raw[module].trim_end().ends_with(';') {
                    module + 1
                } else {
                    raw[module + 1..].iter().position(|line| *line == "}").map_or(raw.len(), |end| module + 1 + end + 1)
                };
                continue;
            }
        }
        found += constructions(&code[at]) + patches(&code[at]);
        at += 1;
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

/// **The scan itself**, on a source made up for it: production code after a test module is counted, the module's own nodes are
/// not, a `mod tests;` declaration skips only its own line, comments of every kind are dropped, a `//` inside a string cuts
/// nothing, and a patch is one more.
#[test]
fn the_scan_counts_production_code_after_a_test_module_and_nothing_else() {
    let source = r#"
fn before() { Node { ..default() } }
#[cfg(test)]
mod tests {
    fn inside() { Node { ..default() } }
}
fn after() { Node::default(); line.entry::<Node>().and_modify(|n| n.x = 1); }
// Node { in a comment }
fn trailing() { let _ = 1; } // Node { in a trailing comment }
/* Node { in a block
   Node { still in it */ fn closes() { Node{ ..default() } }
fn url() { let _ = "https://example.com//"; Node { ..default() } }
fn signature() -> Node { Node { ..default() } }
fn tree() -> TreeNode { TreeNode { key: 1 } }
#[cfg(test)]
mod declared;
fn last() { node.get_mut::<Node>(); }
"#;
    // before 1, after 2 (a construction and a patch), closes 1, url 1, signature 1 (its body, not its return type), last 1.
    assert_eq!(count(source), 7);
    let attribute_and_module_on_one_line = "fn a() { Node { } }\n#[cfg(test)] mod tests {\n    fn t() { Node { } }\n}\nfn b() { Node { } }\n";
    assert_eq!(count(attribute_and_module_on_one_line), 2, "a test module opened on the attribute's own line is skipped too");
}
