//! **Edit one value in a project file and keep every comment** (GD-02, #364).
//!
//! An editor write and an agent's operation both change part of a file a person may have written notes
//! in, so an edit replaces the bytes of exactly one thing — a scalar, a sequence item, a mapping entry —
//! and every other byte of the file comes through as it was. A comment, a blank line, a neighbour's
//! alignment and the order of keys are all somebody's, and none of them is this edit's to move.
//!
//! # What an edit may change
//!
//! | call | changes |
//! |---|---|
//! | [`set`] | one scalar's bytes |
//! | [`add_item`] | one item at the end of a sequence |
//! | [`remove_item`] | one item of a sequence |
//! | [`add_entry`] | one entry at the end of a mapping |
//! | [`remove_entry`] | one entry of a mapping |
//!
//! # The kind of a leaf never changes
//!
//! As with `data_files::set_leaf`: a number stays a number, a string a string, a bool a bool, and a
//! null a null. An editor that wrote `bed` into `turn_deg` would make a file whose reader says
//! *invalid type: string, expected f32* — a refusal this module exists to prevent rather than cause. A
//! value outside a caller's registered [`Bounds`] is refused too. Both refusals name the leaf's path.
//!
//! The one thing the *spelling* follows is the text: a string that YAML would misread is quoted where
//! it lands (`'a: b'`), because a value's kind is the kind, and its spelling is whatever the line needs
//! to mean that kind.
//!
//! # How the bytes are found
//!
//! [`crate::yaml::walk`] reports every node's path and bytes — the same walk the reader refuses a file
//! by, so an edit is only made to text the reader accepts. What the edit produces is checked by the
//! caller's own typed read ([`crate::yaml::from_str`]) in `tests/yaml_surgery_keeps_every_comment.rs`,
//! because a surgery that "looks right" and produces a file the type refuses is the one failure this
//! module could hide.
//!
//! A leaf is one span. **A collection's span is not its content's end**: `saphyr-parser` places a block
//! collection's end event at the token that follows it, so inserting at a `span`'s end would put a new
//! item below the next key. [`content_end`] walks to the last child's own end instead, which is where
//! an item or an entry is added and where a removal stops.
//!
//! # Two styles, no third
//!
//! A block sequence is written either indented under its key (`  - id: bed`) or at the key's own
//! indentation (`- id: bed`), and both are in the repository: the writer emits the second, a person may
//! write either. [`add_item`] copies the **siblings'** dash column rather than assuming a style, and
//! the fields of the new item one [`crate::yaml::INDENT_SPACES`] further in, so neither style is
//! rewritten by an edit.
//!
//! A flow collection (`[a, b]`) is one line: a leaf inside it is set in place (its span is exact), but
//! a list with an item in it cannot take another without rewriting the line it is on, which would move
//! bytes the edit was not asked for — so that is refused, naming the path. An **empty** flow list
//! (`placements: []`) is the one case with no sibling to copy, and it becomes a block, which is the
//! style this repository's writer emits anyway.

use crate::data_files::Bounds;
use crate::problem::Problem;
use crate::yaml::{self, Node, NodeKind, Style, Walk};

/// **A value an edit writes**, which is the shape's own kind and not a text: a caller says what it
/// means, and the kind check is this module's.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A whole number, for a leaf that holds one.
    Int(i64),
    /// A number with a fraction, for a leaf that holds one.
    Float(f64),
    /// A string, for a leaf that holds one.
    Str(String),
    /// A bool, for a leaf that holds one.
    Bool(bool),
    /// The null a leaf already holds, which is the only value a null leaf takes.
    Null,
}

impl Value {
    /// What this value is, for a refusal's sentence: `an integer`, `a string`.
    fn kind(&self) -> &'static str {
        match self {
            Value::Int(_) => "an integer",
            Value::Float(_) => "a float",
            Value::Str(_) => "a string",
            Value::Bool(_) => "a bool",
            Value::Null => "a null",
        }
    }

    /// Whether a leaf written as `leaf` holds this value's kind.
    fn fits(&self, leaf: &yaml::Leaf) -> bool {
        match self {
            Value::Null => leaf.style == Style::Plain && leaf.text.is_empty(),
            Value::Str(_) => leaf.style.is_text(&leaf.text),
            Value::Bool(_) => leaf.style == Style::Plain && matches!(leaf.text.as_str(), "true" | "false"),
            Value::Int(_) => leaf.style == Style::Plain && yaml::is_number(&leaf.text) && !leaf.text.contains(['.', 'e', 'E']),
            Value::Float(_) => leaf.style == Style::Plain && yaml::is_number(&leaf.text),
        }
    }

    /// **The bytes this value is written as**, and whether they would be plain or need no quotes.
    ///
    /// The text is quoted where YAML would otherwise read it as something else — a number, a bool, a
    /// null, or a string with YAML's syntax in it — so a string stays a string however it reads.
    fn text(&self) -> String {
        match self {
            Value::Int(n) => n.to_string(),
            Value::Float(x) => format_float(*x),
            Value::Bool(b) => b.to_string(),
            Value::Null => String::new(),
            Value::Str(s) => quote(s),
        }
    }
}

/// **`x` as this writer spells a float**: `90` is `90.0` (so it stays a float), and the shortest text
/// that reads back as the same `f32` is used otherwise, which is what `serde_yaml_ng` writes too.
fn format_float(x: f64) -> String {
    let x32 = x as f32;
    let text = if x32 as f64 == x { format!("{x32}") } else { format!("{x}") };
    if text.contains(['.', 'e', 'E', 'n', 'i']) { text } else { format!("{text}.0") }
}

/// **`s` as a YAML scalar that is a string**: plain when the text cannot be misread, single-quoted
/// otherwise. Single quotes, because a string with a `\` or a `"` in it needs no escaping there.
fn quote(s: &str) -> String {
    if s.is_empty() || !Style::Plain.is_text(s) || starts_with_indicator(s) || s.trim() != s || s.contains(": ") || s.ends_with(':') {
        return format!("'{}'", s.replace('\'', "''"));
    }
    s.to_owned()
}

/// Whether `s` starts with a character YAML reads as syntax when it begins a plain scalar.
fn starts_with_indicator(s: &str) -> bool {
    s.starts_with(['-', '?', ':', ',', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`'])
}

/// **Set the scalar at `path`**, keeping its kind, and answer the whole file's new text. Only that
/// leaf's bytes change. `bounds` is what a caller registered for a number, if anything.
#[allow(clippy::result_large_err)]
pub fn set(text: &str, path: &str, value: Value, bounds: Option<&Bounds>) -> Result<String, Problem> {
    let walk = walk_or_refuse(text, path)?;
    let node = walk.nodes.iter().find(|n| n.path == path).ok_or_else(|| {
        let near = nearest(&walk, path);
        yaml::problem_at(
            FILE,
            Some(path.to_owned()),
            1,
            &format!("`{path}` is not in this file"),
            &format!("the nearest paths are: {}", near.iter().map(|p| format!("`{p}`")).collect::<Vec<_>>().join(", ")),
            "check the path against the file's own keys",
        )
    })?;
    let Some(leaf) = &node.scalar else {
        return Err(yaml::problem_at(
            FILE,
            Some(path.to_owned()),
            1,
            &format!("`{path}` is not a value"),
            "it is a list or a mapping, which takes an item or an entry and not a value",
            "use `add_item` or `add_entry` for a list or a mapping",
        ));
    };
    if !value.fits(leaf) {
        return Err(yaml::problem_at(
            FILE,
            Some(path.to_owned()),
            1,
            &format!("`{path}` holds {}, which does not take {}", describe(leaf), value.kind()),
            // A `Str` over a null leaf is the one pairing a person expects to work, so it is named as
            // what it is: a null is not a string, and the caller decides.
            "the kind of a value never changes; write a value of the kind the leaf holds",
            "check the field's type",
        ));
    }
    if let (Some(bounds), Value::Int(n)) = (bounds, &value) {
        check_bounds(path, *n as f64, bounds)?;
    }
    if let (Some(bounds), Value::Float(x)) = (bounds, &value) {
        if !x.is_finite() {
            return Err(yaml::problem_at(FILE, Some(path.to_owned()), 1, &format!("`{path}`: {x} is not a finite number"), "a file holds no infinity or NaN", "write a finite number"));
        }
        check_bounds(path, *x, bounds)?;
    }
    let mut out = String::with_capacity(text.len() + 8);
    out.push_str(&text[..node.span.0]);
    out.push_str(&value.text());
    out.push_str(&text[node.span.1..]);
    Ok(out)
}

/// What a leaf holds, for a refusal's sentence.
fn describe(leaf: &yaml::Leaf) -> String {
    match leaf.style {
        Style::Quoted | Style::Block => "a string".to_owned(),
        Style::Plain => {
            if leaf.text.is_empty() {
                "a null".to_owned()
            } else if matches!(leaf.text.as_str(), "true" | "false") {
                "a bool".to_owned()
            } else if leaf.text.contains(['.', 'e', 'E']) && yaml::is_number(&leaf.text) {
                "a float".to_owned()
            } else if yaml::is_number(&leaf.text) {
                "an integer".to_owned()
            } else {
                "a string".to_owned()
            }
        }
    }
}

/// **`x` inside `bounds`**, or a refusal naming the leaf.
#[allow(clippy::result_large_err)]
fn check_bounds(path: &str, x: f64, bounds: &Bounds) -> Result<(), Problem> {
    if bounds.min.is_some_and(|m| x < m) || bounds.max.is_some_and(|m| x > m) {
        return Err(yaml::problem_at(
            FILE,
            Some(path.to_owned()),
            1,
            &format!("`{path}`: {x} is outside its bounds {}", range(bounds)),
            "the field is registered with a range, and this value is not in it",
            "write a value inside the bounds",
        ));
    }
    Ok(())
}

/// A [`Bounds`] as a refusal says it: `[-180, 180]`, `[1, inf]`.
fn range(b: &Bounds) -> String {
    format!("[{}, {}]", b.min.map_or("-inf".into(), |m| m.to_string()), b.max.map_or("inf".into(), |m| m.to_string()))
}

/// **Add `item` as the last item of the sequence at `path`**, copying the siblings' indentation and
/// style. `item` is YAML text: one or more lines, the first of which is the item's own first line.
#[allow(clippy::result_large_err)]
pub fn add_item(text: &str, path: &str, item: &str) -> Result<String, Problem> {
    let walk = walk_or_refuse(text, path)?;
    let node = find(&walk, path)?;
    if node.kind != NodeKind::Sequence {
        return Err(not_a(path, "a list"));
    }
    let item_lines: Vec<&str> = item.trim_end_matches('\n').lines().map(str::trim_end).collect();
    if item_lines.is_empty() || item_lines[0].trim().is_empty() {
        return Err(yaml::problem_at(FILE, Some(path.to_owned()), 1, "an item to add is empty", "there is nothing to write", "give the item's first line"));
    }
    let items: Vec<&Node> = walk.nodes.iter().filter(|n| is_item_of(&n.path, path)).collect();
    let flow = text[node.span.0..node.span.1].trim_start().starts_with('[');
    if flow && !items.is_empty() {
        return Err(yaml::problem_at(
            FILE,
            Some(path.to_owned()),
            1,
            &format!("`{path}` is a flow list with an item in it"),
            "an item added to it would rewrite the line the list is on, and this edit moves no byte it was not asked for",
            "write the list in block style (one item a line) first",
        ));
    }

    // The dash column and the field column come from the siblings; with none to copy (an empty
    // collection) the item goes at the key's own indentation, which is the style the writer emits.
    let dash_column = items
        .iter()
        .filter_map(|n| n.dash)
        .map(|dash| column_of(text, dash))
        .min()
        .unwrap_or_else(|| key_column(text, node));
    let field_column = dash_column + yaml::INDENT_SPACES;
    let mut block = String::new();
    for (n, line) in item_lines.iter().enumerate() {
        if n == 0 {
            block.push_str(&" ".repeat(dash_column));
            block.push_str("- ");
            block.push_str(line.trim_start());
        } else {
            block.push_str(&" ".repeat(field_column));
            block.push_str(line.trim());
        }
        block.push('\n');
    }
    Ok(insert_after_last_of(text, node, &walk, &block, flow))
}

/// **Add `entry` as the last entry of the mapping at `path`**, copying the siblings' indentation.
#[allow(clippy::result_large_err)]
pub fn add_entry(text: &str, path: &str, entry: &str) -> Result<String, Problem> {
    let walk = walk_or_refuse(text, path)?;
    let node = find(&walk, path)?;
    if node.kind != NodeKind::Mapping {
        return Err(not_a(path, "a mapping"));
    }
    if text[node.span.0..node.span.1].trim_start().starts_with('{') {
        return Err(yaml::problem_at(
            FILE,
            Some(path.to_owned()),
            1,
            &format!("`{path}` is a flow mapping"),
            "an entry added to it would rewrite the line the mapping is on, and this edit moves no byte it was not asked for",
            "write the mapping in block style first",
        ));
    }
    let entry_lines: Vec<&str> = entry.trim_end_matches('\n').lines().map(str::trim_end).collect();
    if entry_lines.is_empty() || entry_lines[0].trim().is_empty() {
        return Err(yaml::problem_at(FILE, Some(path.to_owned()), 1, "an entry to add is empty", "there is nothing to write", "give the entry's first line"));
    }
    let children: Vec<&Node> = walk.nodes.iter().filter(|n| is_entry_of(&n.path, path)).collect();
    // A mapping with no entry has no sibling to copy. Its own column plus one step is where a nested
    // mapping's keys go, which is the only style there is for one.
    let column = children
        .iter()
        .filter_map(|n| n.key.as_ref().map(|k| column_of(text, k.span.0)))
        .min()
        .unwrap_or_else(|| key_column(text, node) + yaml::INDENT_SPACES);
    let mut block = String::new();
    for (n, line) in entry_lines.iter().enumerate() {
        // The first line names the key, so its own indentation is the indent the caller wrote it with;
        // every later line keeps the indentation it was written with, relative to that.
        let body = if n == 0 { line.trim_start() } else { *line };
        block.push_str(&" ".repeat(column));
        block.push_str(body);
        block.push('\n');
    }
    Ok(insert_after_last_of(text, node, &walk, &block, false))
}

/// **Remove item `index` of the sequence at `path`**, taking only its own lines.
#[allow(clippy::result_large_err)]
pub fn remove_item(text: &str, path: &str, index: usize) -> Result<String, Problem> {
    let walk = walk_or_refuse(text, path)?;
    let node = find(&walk, path)?;
    if node.kind != NodeKind::Sequence {
        return Err(not_a(path, "a list"));
    }
    let item_path = yaml::join_path_index(path, index);
    let item = walk.nodes.iter().find(|n| n.path == item_path).ok_or_else(|| {
        yaml::problem_at(FILE, Some(item_path.clone()), 1, &format!("`{item_path}` is not in this file"), "the list has fewer items than that", "check the item's index")
    })?;
    // The item's own lines: from its dash (or its first character) through the end of its last line.
    let from = item.dash.unwrap_or(item.span.0);
    let start = text[..from].rfind('\n').map_or(from, |at| at + 1);
    let end = end_of_last_line(text, content_end(text, item, &walk));
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..start]);
    out.push_str(&text[end..]);
    Ok(out)
}

/// **Remove the entry named `key` from the mapping at `path`**, taking only its own lines.
#[allow(clippy::result_large_err)]
pub fn remove_entry(text: &str, path: &str, key: &str) -> Result<String, Problem> {
    let walk = walk_or_refuse(text, path)?;
    let node = find(&walk, path)?;
    if node.kind != NodeKind::Mapping {
        return Err(not_a(path, "a mapping"));
    }
    let entry_path = yaml::join_path_key(path, key);
    let entry = walk.nodes.iter().find(|n| n.path == entry_path).ok_or_else(|| {
        yaml::problem_at(FILE, Some(entry_path.clone()), 1, &format!("`{entry_path}` is not in this file"), "the mapping has no such key", "check the key's spelling")
    })?;
    let from_key = entry.key.as_ref().map_or(entry.span.0, |k| k.span.0);
    let start = text[..from_key].rfind('\n').map_or(from_key, |at| at + 1);
    let end = end_of_last_line(text, content_end(text, entry, &walk));
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..start]);
    out.push_str(&text[end..]);
    Ok(out)
}

/// **`text` with `block` inserted after the last line the node at `node` holds.**
///
/// Not the collection's own end: `saphyr-parser` places an end event at the next token, which for a
/// block collection is the line after its last item, so inserting there would put the new item below a
/// sibling that is not in the collection. The last byte of the node's *children* is where its content
/// really stops — and a trailing comment on that line is kept, because the insert follows its newline.
///
/// `flow` says the node was `[]`, which is replaced rather than added to: its two characters are
/// swapped for the block's newline and the key's own line loses the space before them.
fn insert_after_last_of(text: &str, node: &Node, walk: &Walk, block: &str, flow: bool) -> String {
    let at = end_of_last_line(text, content_end(text, node, walk));
    let mut out = String::with_capacity(text.len() + block.len());
    if flow {
        // `placements: []` becomes `placements:` and the block under it.
        let before = &text[..node.span.0];
        out.push_str(before.trim_end_matches(' '));
        out.push('\n');
        out.push_str(block.trim_end_matches('\n'));
        out.push('\n');
        out.push_str(text[node.span.1..].trim_start_matches('\n'));
        return out;
    }
    out.push_str(&text[..at]);
    out.push_str(block);
    out.push_str(&text[at..]);
    out
}

/// **The byte the node's own content reaches**, which is not its `span`'s end.
///
/// `saphyr-parser` places a block collection's end at the token that follows it, so a sequence's `span`
/// ends where the *next* key starts, and inserting there would put a new item after a line that is not
/// in the collection. A scalar's end is its own; a flow collection's end includes its bracket; and a
/// block collection's end is the end of its last child's content, one level at a time.
fn content_end(text: &str, node: &Node, walk: &Walk) -> usize {
    if node.kind == NodeKind::Scalar || is_flow(text, node) {
        return node.span.1;
    }
    let children: Vec<&Node> = walk.nodes.iter().filter(|n| is_direct_child(&n.path, &node.path)).collect();
    children.iter().map(|child| content_end(text, child, walk)).max().unwrap_or(node.span.0)
}

/// Whether the node is written in flow style, whose `span` includes its closing bracket.
fn is_flow(text: &str, node: &Node) -> bool {
    text[node.span.0..node.span.1].trim_start().starts_with(['[', '{'])
}

/// Whether `path` is one level under `parent`: `parent[3]`, `parent.key`, and not `parent[3].at`.
fn is_direct_child(path: &str, parent: &str) -> bool {
    let Some(rest) = path.strip_prefix(parent) else { return false };
    if rest.starts_with('[') {
        let Some(close) = rest.find(']') else { return false };
        return rest[1..close].chars().all(|c| c.is_ascii_digit()) && !rest[close + 1..].starts_with(['.', '[']);
    }
    let Some(key) = rest.strip_prefix('.') else { return false };
    !key.is_empty() && !key.contains(['.', '[', ']'])
}

/// The column a node's own key starts at, or the line's indentation when it has no key.
fn key_column(text: &str, node: &Node) -> usize {
    match &node.key {
        Some(key) => column_of(text, key.span.0),
        None => column_of(text, node.span.0),
    }
}

/// Whether `path` is one item of the sequence at `parent`.
fn is_item_of(path: &str, parent: &str) -> bool {
    let Some(rest) = path.strip_prefix(parent) else { return false };
    let Some(index) = rest.strip_prefix('[') else { return false };
    index.trim_end_matches(']').chars().all(|c| c.is_ascii_digit()) && index.ends_with(']') && !index.contains('.')
}

/// Whether `path` is one entry of the mapping at `parent`.
fn is_entry_of(path: &str, parent: &str) -> bool {
    let Some(rest) = path.strip_prefix(parent) else { return false };
    let Some(key) = rest.strip_prefix('.') else { return false };
    !key.is_empty() && !key.contains(['.', '[', ']'])
}

/// The file name a refusal carries: this module edits text a caller names, and the caller's own reader
/// names the file when it reads the result. A surgery refusal names the leaf's path, which is what the
/// issue asks for.
const FILE: &str = "a project file";

/// The walk of `text`, or the refusal it earned (the file says one of the things a reader refuses).
#[allow(clippy::result_large_err)]
fn walk_or_refuse(text: &str, path: &str) -> Result<Walk, Problem> {
    let walk = yaml::walk(text);
    if let Some(site) = yaml::indentation_tab_site(text, yaml::block_lines(&walk)) {
        return Err(yaml::refusal_of(FILE, site));
    }
    if let Some(site) = walk.site {
        return Err(yaml::refusal_of(FILE, site));
    }
    // The path is checked by the caller that uses it, so a file that does not read is refused before
    // any path is looked for: a wrong path in a broken file must not be the answer.
    let _ = path;
    Ok(walk)
}

/// The node at `path`, or a refusal naming it and its nearest neighbours.
#[allow(clippy::result_large_err)]
fn find<'a>(walk: &'a Walk, path: &str) -> Result<&'a Node, Problem> {
    walk.nodes.iter().find(|n| n.path == path).ok_or_else(|| {
        let near = nearest(walk, path);
        yaml::problem_at(
            FILE,
            Some(path.to_owned()),
            1,
            &format!("`{path}` is not in this file"),
            &format!("the nearest paths are: {}", near.iter().map(|p| format!("`{p}`")).collect::<Vec<_>>().join(", ")),
            "check the path against the file's own keys",
        )
    })
}

/// `path` is not a list or a mapping, as the call needed.
fn not_a(path: &str, wanted: &str) -> Problem {
    yaml::problem_at(
        FILE,
        Some(path.to_owned()),
        1,
        &format!("`{path}` is not {wanted}"),
        "the call needs a list or a mapping at this path",
        "check the path against the file's own keys",
    )
}

/// Up to five known paths sharing the longest prefix with `path`, so a wrong path is a typo a reader
/// can fix rather than a search.
fn nearest(walk: &Walk, path: &str) -> Vec<String> {
    let prefix = |p: &str| {
        let a: Vec<char> = p.chars().collect();
        let b: Vec<char> = path.chars().collect();
        let common = a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count();
        // A path that is a PREFIX of the other is nearer than one that shares a long middle.
        if common == a.len().min(b.len()) { 1000 } else { common }
    };
    let mut scored: Vec<(usize, &str)> = walk.nodes.iter().map(|n| (prefix(&n.path), n.path.as_str())).filter(|(_, p)| !p.is_empty()).collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    scored.into_iter().take(5).map(|(_, p)| p.to_owned()).collect()
}

/// The column of the byte at `at`, counted in characters from its line's start.
fn column_of(text: &str, at: usize) -> usize {
    let line_start = text[..at].rfind('\n').map_or(0, |n| n + 1);
    text[line_start..at].chars().count()
}

/// **The byte just past the newline that ends the line containing `at`**, so text inserted there starts
/// a line of its own.
fn end_of_last_line(text: &str, at: usize) -> usize {
    let from = at.max(1).min(text.len());
    match text[from..].find('\n') {
        Some(offset) => from + offset + 1,
        None => text.len(),
    }
}
