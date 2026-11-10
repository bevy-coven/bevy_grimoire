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
//! adding to or removing from one would rewrite the line it shares with its key, so both are refused,
//! naming the path. An **empty** flow collection (`placements: []`) is the one case with no sibling to
//! copy, and it becomes a block, which is the style this repository's writer emits anyway.

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

// **A bool is `true` or `false` and a [`Value`] cannot say which, on purpose.** A caller that means to
// flip a switch says so with [`Value::Bool`], and the value it writes is the one it holds: an edit here
// changes a number, a string or a null, and swapping a `true` for a `false` is a different operation a
// caller states with `Value::Bool(false)`.

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

/// **`s` as a YAML scalar that reads back as exactly `s`**, quoted as little as the text allows.
///
/// - **Single quotes** for a string YAML would otherwise misread but that needs no escape: a `#` after a
///   space (which starts a comment), a leading or trailing space, `: ` or a trailing `:`, a leading
///   indicator (`-`, `?`, `[`, `&`, `!` and the rest), or a word the reader would take for a number, a
///   bool or a null.
/// - **Double quotes** when the text holds a character no plain or single-quoted scalar can carry: a
///   newline, a tab, a control character, or a `\` (which a single-quoted scalar cannot escape and a
///   plain one reads as itself only in some positions). The escapes are YAML's own.
fn quote(s: &str) -> String {
    if needs_double(s) {
        let mut out = String::with_capacity(s.len() + 2);
        out.push('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                '\r' => out.push_str("\\r"),
                c if c.is_control() => out.push_str(&format!("\\x{:02x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
        return out;
    }
    if s.is_empty()
        || !Style::Plain.is_text(s)
        || starts_with_indicator(s)
        || s.trim() != s
        || s.contains(" #")
        || s.contains(": ")
        || s.ends_with(':')
        // **A flow indicator ends a flow scalar wherever it sits.** `{id: x, y}` reads `x` and then `y`
        // as a key, and a bracket closes the collection early — so a string holding one is quoted even
        // where the plain spelling would be fine in block style.
        || s.chars().any(|c| matches!(c, ',' | '[' | ']' | '{' | '}'))
    {
        return format!("'{}'", s.replace('\'', "''"));
    }
    s.to_owned()
}

/// Whether `s` holds something only a double-quoted scalar can carry: a line break, a tab, a control
/// character, or a backslash.
fn needs_double(s: &str) -> bool {
    s.chars().any(|c| c == '\\' || c.is_control())
}

/// Whether `s` starts with a character YAML reads as syntax when it begins a plain scalar.
fn starts_with_indicator(s: &str) -> bool {
    s.starts_with(['-', '?', ':', ',', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`'])
}

/// **Format an `f32` at the fewest decimal places that still round-trip to the same value.**
///
/// Round-tripping through `parse` is what decides, rather than a tolerance, so a written file always reloads
/// to the same `f32` the writer was holding. One decimal is exact for anything snapped to a half-metre or a
/// whole degree; the wider forms are only reached by a value that came from elsewhere.
pub fn fmt_number(v: f32) -> String {
    for places in 1..=6 {
        let s = format!("{v:.places$}");
        if s.parse::<f32>() == Ok(v) {
            return s;
        }
    }
    format!("{v}")
}

/// **Set the scalar at `path`**, keeping its kind, and answer the whole file's new text. Only that
/// leaf's bytes change. `bounds` is what a caller registered for a number, if anything.
#[allow(clippy::result_large_err)]
pub fn set(text: &str, path: &str, value: Value, bounds: Option<&Bounds>) -> Result<String, Problem> {
    let walk = walk_or_refuse(text)?;
    let node = find(&walk, path)?;
    let Some(leaf) = &node.scalar else {
        return Err(at_node(text,
            node,
            &format!("`{path}` is not a value"),
            "it is a list or a mapping, which takes an item or an entry and not a value",
            "use `add_item` or `add_entry` for a list or a mapping",
        ));
    };

    // **A float is finite whether or not a caller registered bounds.** A file holds no infinity and no
    // NaN: both are written as a word the reader takes for a string, which would change the leaf's kind.
    if let Value::Float(x) = &value
        && !x.is_finite()
    {
        return Err(at_node_named(text, node, path, &format!("`{path}`: {x} is not a finite number"), "a file holds no infinity and no NaN", "write a finite number"));
    }
    // **A number with a fraction needs a float leaf, and a whole value may fill an integer one.** `9.5`
    // into an integer leaf would be a file the reader refuses (`9.5` is not a `u32`), and `9.0` is
    // exactly the integer `9`.
    let value = match (&value, &leaf.style) {
        (Value::Float(_), _) if !leaf.style.accepts_number(&leaf.text) => return Err(wrong_kind(text, node, leaf, &value, path)),
        (Value::Float(x), _) if is_integer_leaf(leaf) => {
            if x.fract() != 0.0 {
                return Err(at_node_named(text,
                    node,
                    path,
                    &format!("`{path}` holds an integer, and {x} is not a whole number"),
                    "a float leaf is the only one that takes a fraction",
                    "write a whole number",
                ));
            }
            // A cast saturates, which would write a number nobody asked for: `1e30` is not an integer any
            // file holds, and it is refused rather than turned into `i64::MAX`.
            // A cast saturates, which would write a number nobody asked for, so the cast's own range is
            // checked first — the leaf's own range (a `u32` version, a `u64` seed) is its reader's
            // business, and refuses what it cannot hold with its own words.
            if !(*x > i64::MIN as f64 && *x < i64::MAX as f64) {
                return Err(at_node_named(text, node, path, &format!("`{path}`: {x} is not a number an integer holds"), "a whole number too large for any integer cannot be written", "write a number the field holds"));
            }
            Value::Int(*x as i64)
        }
        _ => value.clone(),
    };
    if !value.fits(leaf) {
        return Err(wrong_kind(text, node, leaf, &value, path));
    }
    if let Some(bounds) = bounds {
        let number = match &value {
            Value::Int(n) => *n as f64,
            Value::Float(x) => *x,
            _ => f64::NAN,
        };
        if number.is_finite() {
            check_bounds(text, node, path, number, bounds)?;
        }
    }

    // **A block scalar's span is its content, not its indicator.** `saphyr-parser` reports a `|` scalar
    // from its first content character to the start of the line after its last, so a replacement runs
    // from the `|` itself (which says the value was a block) to the end of the content.
    let mut out = String::with_capacity(text.len() + 8);
    if leaf.style == Style::Block {
        let indicator = block_indicator(text, node);
        let end = content_end(text, node, &walk);
        // **The header line's comment is a person's**, and it stays after the new value: `note: |  # why`
        // becomes `note: new  # why`. The comment is what follows the indicator on its own line.
        let header_end = text[indicator..].find('\n').map_or(text.len(), |at| indicator + at);
        // **The header may carry chomping and indentation indicators** — `|-`, `|+`, `|2`, `|2-` — and
        // `serde_yaml_ng` writes a multi-line string as `|-`, so they are the common case: they are
        // skipped before the comment is looked for, or the comment is silently dropped.
        let header = &text[indicator + 1..header_end];
        let rest = header.trim_start_matches(|c: char| c == '+' || c == '-' || c.is_ascii_digit()).trim_start();
        let comment = rest.strip_prefix('#').map(|rest| format!("  #{}", rest));
        out.push_str(&text[..indicator]);
        out.push_str(&value.text());
        if let Some(comment) = comment {
            out.push_str(&comment);
        }
        out.push_str(&text[end..]);
    } else {
        out.push_str(&text[..node.span.0]);
        out.push_str(&value.text());
        out.push_str(&text[node.span.1..]);
    }
    verified(out, path)
}

/// **The `|` or `>` that makes a scalar a block**, which is where a replacement of one starts.
///
/// **The first one after the key's `:`**, not the last: a header line may carry a comment — `note: |  #
/// a -> b` — and the `>` in it is not the indicator. `saphyr-parser`'s span begins at the first content
/// character, so everything between the key and that character is the indicator, its options and its
/// comment, and the indicator is the first of them.
fn block_indicator(text: &str, node: &Node) -> usize {
    let from = node.key.as_ref().map_or(node.span.0, |key| key.span.1);
    let up_to = node.span.0.min(text.len());
    let colon = text[from..up_to].find(':').map_or(from, |at| from + at);
    text[colon..up_to].find(['|', '>']).map_or(node.span.0, |at| colon + at)
}

/// Whether a leaf's number is a whole number, so a `Float` that is whole may fill it.
fn is_integer_leaf(leaf: &yaml::Leaf) -> bool {
    leaf.style == Style::Plain && yaml::is_number(&leaf.text) && !leaf.text.contains(['.', 'e', 'E'])
}

/// **A flow collection cannot be edited line by line.** `at: [1.0, 2.0]` is one line, and adding to or
/// removing from it would rewrite that line and whatever sits around it, which is bytes this edit was
/// not asked to move.
fn flow_refusal(text: &str, node: &Node, path: &str, verb: &str) -> Problem {
    at_node_named(
        text,
        node,
        path,
        &format!("`{path}` is a flow collection"),
        &format!("it is written on one line, so an item {verb} it would rewrite the line it shares with its key"),
        "write the collection in block style (one item a line) first",
    )
}

/// **An item's first key removed without losing the item**: the `-` stays, and the item's own next entry
/// is promoted onto its line, so what is left is still an item of the list.
///
/// `next` is the byte the next entry **of this item** starts at, and `after_removed` the byte the removed
/// entry's own lines end at: everything between them — a comment, a blank line — is kept, because it is
/// somebody's.
fn promote_first_key(text: &str, dash: usize, line_start: usize, next: usize, after_removed: usize) -> String {
    // The promoted entry's own line, without its indentation, which the `- ` replaces.
    let next_line_end = text[next..].find('\n').map_or(text.len(), |at| next + at + 1);
    let promoted = text[next..next_line_end].trim();
    // Between the removed entry and the promoted one: whatever a person wrote there — a comment, a blank
    // line — is kept, and the whitespace that indented the promoted key is not (the `- ` is its indent).
    // **A blank line keeps its newline and loses its indentation**: the indentation belonged to the entry
    // that was removed, and a line of nothing but spaces is not a thing a person wrote.
    // **Each whole line a person wrote between the two keys is kept**; the last segment, when it has no
    // newline, is the promoted key's own indentation, which the `- ` replaces.
    let between: String = text[after_removed..next]
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
        .map(|line| if line.trim().is_empty() { "\n".to_owned() } else { line.to_owned() })
        .collect();

    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..line_start]);
    // **The `-` stays and the promoted entry takes its own place on that line**, which is what keeps the
    // row an item of the list.
    out.push_str(&" ".repeat(dash - line_start));
    out.push_str("- ");
    out.push_str(promoted);
    out.push('\n');
    out.push_str(&between);
    // The promoted key's line is re-emitted above, so it is skipped here rather than repeated; its own
    // value, when that is a block starting on a later line, follows as it was.
    out.push_str(&text[next_line_end..]);
    out
}

/// **The `-` that starts the line `byte` is on**, when there is one: an item's first key's line.
fn dash_on_line(text: &str, byte: usize) -> Option<usize> {
    let line_start = text[..byte].rfind('\n').map_or(0, |at| at + 1);
    let prefix = &text[line_start..byte];
    prefix.trim_start().starts_with('-').then(|| line_start + prefix.len() - prefix.trim_start().len())
}

/// **The item path a `parent[i].key` path belongs to**: `placements[0]`.
fn item_of_path(path: &str) -> Option<String> {
    let key = path.rfind('.')?;
    let parent = &path[..key];
    parent.ends_with(']').then(|| parent.to_owned())
}

/// **The next entry of the item at `item_path`**, whose own lines start at or after `from`: the next
/// key of that item, never the next item's first key.
fn next_entry_of(text: &str, walk: &Walk, item_path: &str, from: usize) -> Option<usize> {
    walk.nodes
        .iter()
        .filter(|n| is_entry_of(&n.path, item_path))
        .filter_map(|n| n.key.as_ref().map(|k| k.span.0))
        .filter(|at| *at >= from)
        // The item's own entries only: a key of the item's `parent[1]` has a longer path.
        .filter(|at| {
            let line_start = text[..*at].rfind('\n').map_or(0, |n| n + 1);
            let prefix = &text[line_start..*at];
            !prefix.trim_start().starts_with('-')
        })
        .min()
}

/// A refusal at a node: its own line and column, never a made-up `line 1`.
fn at_node(text: &str, node: &Node, what: &str, why: &str, fix: &str) -> Problem {
    yaml::problem_at_node(FILE, None, text, node.span.0, what, why, fix)
}

/// A refusal at a node, naming the field's path: what every refusal about a leaf says.
fn at_node_named(text: &str, node: &Node, path: &str, what: &str, why: &str, fix: &str) -> Problem {
    yaml::problem_at_node(FILE, Some(path.to_owned()), text, node.span.0, what, why, fix)
}

/// A refusal that a leaf does not take a value.
fn wrong_kind(text: &str, node: &Node, leaf: &yaml::Leaf, value: &Value, path: &str) -> Problem {
    yaml::problem_at_node(
        FILE,
        Some(path.to_owned()),
        text,
        node.span.0,
        &format!("`{path}` holds {}, which does not take {}", describe(leaf), value.kind()),
        "the kind of a value never changes; write a value of the kind the leaf holds",
        "check the field's type",
    )
}

/// **The edited text, once the reader is asked about it.** An edit that produced a file its own reader
/// refuses is refused here instead of written — a duplicate key, an anchor, a broken line — with the
/// path it was asked about. `data_files::set_leaf` has the same rule for RON, and this is that rule.
#[allow(clippy::result_large_err)]
fn verified(text: String, path: &str) -> Result<String, Problem> {
    let walk = yaml::walk(&text);
    if let Some(site) = yaml::first_fault(&text, &walk) {
        return Err(yaml::problem_naming(FILE, Some(path.to_owned()), site));
    }
    Ok(text)
}

/// What a leaf holds, for a refusal's sentence.
fn describe(leaf: &yaml::Leaf) -> String {
    match leaf.style {
        Style::Quoted | Style::Block => "a string".to_owned(),
        Style::Plain => {
            if leaf.text.is_empty() || matches!(leaf.text.as_str(), "~" | "null" | "Null" | "NULL") {
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
fn check_bounds(text: &str, node: &Node, path: &str, x: f64, bounds: &Bounds) -> Result<(), Problem> {
    if bounds.min.is_some_and(|m| x < m) || bounds.max.is_some_and(|m| x > m) {
        return Err(yaml::problem_at_node(
            FILE,
            Some(path.to_owned()),
            text,
            node.span.0,
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
    let walk = walk_or_refuse(text)?;
    let node = find(&walk, path)?;
    if node.kind != NodeKind::Sequence {
        return Err(not_a_at(text, node, path, "a list"));
    }
    let item_lines: Vec<&str> = item.trim_end_matches('\n').lines().map(str::trim_end).collect();
    if item_lines.is_empty() || item_lines[0].trim().is_empty() {
        return Err(at_node(text, node, "an item to add is empty", "there is nothing to write", "give the item's first line"));
    }
    let items: Vec<&Node> = walk.nodes.iter().filter(|n| is_item_of(&n.path, path)).collect();
    let flow = text[node.span.0..node.span.1].trim_start().starts_with('[');
    if flow && !items.is_empty() {
        return Err(at_node_named(
            text,
            node,
            path,
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
    let first_indent = item_lines[0].len() - item_lines[0].trim_start().len();
    let mut block = String::new();
    for (n, line) in item_lines.iter().enumerate() {
        if n == 0 {
            block.push_str(&" ".repeat(dash_column));
            block.push_str("- ");
            block.push_str(line.trim_start());
        } else {
            // **Every later line keeps its indentation relative to the item's first line**, so a nested
            // mapping stays nested: `patch:` then `  scale: 2` is two levels under the item, however
            // deeply the text the caller wrote was itself indented.
            let relative = (line.len() - line.trim_start().len()).saturating_sub(first_indent);
            block.push_str(&" ".repeat(field_column + relative));
            block.push_str(line.trim_start());
        }
        block.push('\n');
    }
    verified(insert_after_last_of(text, node, &walk, &block, flow), path)
}

/// **Add `entry` as the last entry of the mapping at `path`**, copying the siblings' indentation.
#[allow(clippy::result_large_err)]
pub fn add_entry(text: &str, path: &str, entry: &str) -> Result<String, Problem> {
    let walk = walk_or_refuse(text)?;
    let node = find(&walk, path)?;
    if node.kind != NodeKind::Mapping {
        return Err(not_a_at(text, node, path, "a mapping"));
    }
    // An **empty** `{}` has no entry to rewrite around, so it becomes a block exactly as `[]` does.
    let flow = is_flow(text, node);
    if flow && !text[node.span.0..node.span.1].trim_end().ends_with("{}") {
        return Err(flow_refusal(text, node, path, "added to"));
    }
    let entry_lines: Vec<&str> = entry.trim_end_matches('\n').lines().map(str::trim_end).collect();
    if entry_lines.is_empty() || entry_lines[0].trim().is_empty() {
        return Err(at_node_named(text, node, path, "an entry to add is empty", "there is nothing to write", "give the entry's first line"));
    }
    let flow = text[node.span.0..node.span.1].trim_start().starts_with('{');
    let children: Vec<&Node> = walk.nodes.iter().filter(|n| is_entry_of(&n.path, path)).collect();
    // A mapping with no entry has no sibling to copy. Its own column plus one step is where a nested
    // mapping's keys go, which is the only style there is for one.
    // **The document's own mapping has no key to stand beside**, so an entry added to it goes at column 0 —
    // `key_column` answers where the file's first `key:` is, which *is* 0 for every file the writer emits, and
    // adding a step to that would nest the entry under the last key instead of writing it at the root.
    let column = if node.path.is_empty() {
        0
    } else {
        children
            .iter()
            .filter_map(|n| n.key.as_ref().map(|k| column_of(text, k.span.0)))
            .min()
            .unwrap_or_else(|| key_column(text, node) + yaml::INDENT_SPACES)
    };
    // **A block value is indented one step under its key**, which is what YAML requires of a mapping or a
    // sequence written under one: `provenance:` then its fields at `column + INDENT_SPACES`. A single-line
    // entry (`exclude: []`) is just the key's own line at `column`.
    let mut block = String::new();
    let first = entry_lines[0].trim_start();
    block.push_str(&" ".repeat(column));
    block.push_str(first);
    block.push('\n');
    for line in entry_lines.iter().skip(1) {
        block.push_str(&" ".repeat(column + yaml::INDENT_SPACES));
        block.push_str(line.trim_start());
        block.push('\n');
    }
    verified(insert_after_last_of(text, node, &walk, &block, flow), path)
}

/// **Set the entry `key` of the mapping at `path`, or add it if the mapping does not have it.**
///
/// **One call, because a caller that has to ask which case it is has to search the file itself** — which is
/// the bookkeeping this module exists to remove. `entry` is the key and its value as YAML (`provenance:` then
/// its block, `scale: 1.0`), indented however the caller wrote it; [`add_entry`]'s own rules decide where it
/// goes when it is new, so a first stamp and a re-stamp cannot disagree about the file's shape.
#[allow(clippy::result_large_err)]
pub fn set_entry(text: &str, path: &str, entry: &str) -> Result<String, Problem> {
    let walk = walk_or_refuse(text)?;
    let node = find(&walk, path)?;
    let key = entry
        .trim_start()
        .split_once(':')
        .map(|(k, _)| k.trim())
        .filter(|k| !k.is_empty())
        .ok_or_else(|| at_node_named(text, node, path, "an entry needs a `key:` and a value", "there is nothing to set", "write the entry's first line as `key: value`"))?;
    let at = yaml::join_path_key(path, key);
    let exists = walk.nodes.iter().any(|n| n.path == at);
    if !exists {
        return add_entry(text, path, entry);
    }
    // **An edit of an entry is a set of its leaf's value**, which keeps the key's own line, its comments and
    // its neighbours — so this is `set` on the entry's own path, and a caller whose value is a block writes
    // the entry afresh through `remove_entry` then `add_entry` (the two calls that exist for a shape change).
    let value = entry.trim_start().split_once(':').map(|(_, v)| v.trim()).unwrap_or_default();
    if value.is_empty() {
        return Err(at_node_named(text, node, path, &format!("`{key}:` has no value on its line"), "an entry whose value is a block cannot be set through this call", "remove the entry and add it again"));
    }
    // **What stands there decides how it is written.** A scalar takes `set`; a collection takes the collection's
    // own replacement, which is `set` for a flow list (`exclude: [a]` on one line) and otherwise a refusal —
    // because a block list's items are lines a person wrote and one call cannot rewrite them without
    // guessing which is which.
    let existing = walk.nodes.iter().find(|n| n.path == at);
    match existing.map(|n| n.kind) {
        Some(NodeKind::Sequence) => {
            // A list's items are replaced by rewriting the entry's own line, which `set` does for a flow list
            // and which `replace_lines` does for a block one.
            replace_entry_lines(text, &at, entry)
        }
        Some(NodeKind::Mapping) => replace_entry_lines(text, &at, entry),
        _ => set(text, &at, scalar_of(value), None),
    }
}

/// **A scalar as the value a written line spells**: a number, a bool, a quoted string or a plain word.
fn scalar_of(value: &str) -> Value {
    let plain = value.trim();
    if let Some(inner) = plain.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        return Value::Str(inner.to_owned());
    }
    if let Some(inner) = plain.strip_prefix('\'').and_then(|t| t.strip_suffix('\'')) {
        return Value::Str(inner.replace("''", "'"));
    }
    match plain {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    if let Ok(i) = plain.parse::<i64>() {
        return Value::Int(i);
    }
    if let Ok(f) = plain.parse::<f64>()
        && f.is_finite()
    {
        return Value::Float(f);
    }
    Value::Str(plain.to_owned())
}

/// **Replace the entry at `path` with `entry`, taking exactly its own lines.**
///
/// The shape-changing sibling of [`set`], which writes one scalar's bytes: this is for a value that *is* a
/// collection (`exclude: [a, b]`, or a block list a person wrote), where a replacement changes how many lines
/// the entry occupies. What is kept is everything the entry did not own: the key's own line comment stands
/// above the new value, and so does every comment and blank line around it.
#[allow(clippy::result_large_err)]
fn replace_entry_lines(text: &str, path: &str, entry: &str) -> Result<String, Problem> {
    let walk = walk_or_refuse(text)?;
    let node = find(&walk, path)?;
    let key_at = node.key.as_ref().map_or(node.span.0, |k| k.span.0);
    let line_start = text[..key_at].rfind('\n').map_or(0, |at| at + 1);
    let column = column_of(text, key_at);
    let end = end_of_last_line(text, content_end(text, node, &walk));
    // The new value, at the entry's own column: the key's line is replaced by the caller's first line, and a
    // block value's lines follow one step in, which is what YAML needs of a mapping under a key.
    let lines: Vec<&str> = entry.trim_end_matches('\n').lines().collect();
    let mut block = String::new();
    block.push_str(&" ".repeat(column));
    block.push_str(lines[0].trim_start());
    block.push('\n');
    for line in lines.iter().skip(1) {
        block.push_str(&" ".repeat(column + yaml::INDENT_SPACES));
        block.push_str(line.trim_start());
        block.push('\n');
    }
    let mut out = String::with_capacity(text.len() + block.len());
    out.push_str(&text[..line_start]);
    out.push_str(&block);
    out.push_str(&text[end..]);
    verified(out, path)
}

/// **Remove item `index` of the sequence at `path`**, taking only its own lines.
#[allow(clippy::result_large_err)]
pub fn remove_item(text: &str, path: &str, index: usize) -> Result<String, Problem> {
    let walk = walk_or_refuse(text)?;
    let node = find(&walk, path)?;
    if node.kind != NodeKind::Sequence {
        return Err(not_a_at(text, node, path, "a list"));
    }
    // **A flow list is one line, and this cannot edit one.** Removing an item from `at: [1.0, 2.0]` would
    // take the line the key is on, so it is refused exactly as adding to a non-empty flow list is.
    if is_flow(text, node) {
        return Err(flow_refusal(text, node, path, "removed from"));
    }
    let item_path = yaml::join_path_index(path, index);
    let item = walk.nodes.iter().find(|n| n.path == item_path).ok_or_else(|| {
        at_node_named(text, node, &item_path, &format!("`{item_path}` is not in this file"), "the list has fewer items than that", "check the item's index")
    })?;
    // The item's own lines: from its dash (or its first character) through the end of its last line.
    let from = item.dash.unwrap_or(item.span.0);
    let start = text[..from].rfind('\n').map_or(from, |at| at + 1);
    let end = end_of_last_line(text, content_end(text, item, &walk));
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..start]);
    out.push_str(&text[end..]);
    verified(out, path)
}

/// **Remove the entry named `key` from the mapping at `path`**, taking only its own lines.
#[allow(clippy::result_large_err)]
pub fn remove_entry(text: &str, path: &str, key: &str) -> Result<String, Problem> {
    let walk = walk_or_refuse(text)?;
    let node = find(&walk, path)?;
    if node.kind != NodeKind::Mapping {
        return Err(not_a_at(text, node, path, "a mapping"));
    }
    if is_flow(text, node) {
        return Err(flow_refusal(text, node, path, "removed from"));
    }
    let entry_path = yaml::join_path_key(path, key);
    let entry = walk.nodes.iter().find(|n| n.path == entry_path).ok_or_else(|| {
        at_node_named(text, node, &entry_path, &format!("`{entry_path}` is not in this file"), "the mapping has no such key", "check the key's spelling")
    })?;

    // **An entry on the line its item's `-` starts is the item's first key**, which is decided from the
    // KEY's own line and not from where its value begins: a first key whose value is a block mapping
    // (`- patch:`) or a block scalar (`- note: |`) starts its value on a later line.
    let key_at = entry.key.as_ref().map_or(entry.span.0, |k| k.span.0);
    if let Some(dash) = dash_on_line(text, key_at) {
        let line_start = text[..key_at].rfind('\n').map_or(0, |at| at + 1);
        // The item this key belongs to, and the next entry **of that item** — never the next item's key.
        let item_path = item_of_path(&entry_path).unwrap_or_else(|| entry_path.clone());
        // The removed entry's own line ends at the newline after its content: everything after it up to the
        // promoted key — a blank line, a comment — is a person's, and `promote_first_key` keeps it.
        let after_key = end_of_last_line(text, content_end(text, entry, &walk));
        let Some(next) = next_entry_of(text, &walk, &item_path, after_key) else {
            // An item with one key has nothing to promote, and its `-` cannot stay on a line by itself.
            return Err(at_node_named(
                text,
                entry,
                &entry_path,
                &format!("`{entry_path}` is the only key of `{item_path}`"),
                "removing it would leave the item with no key on its `-` line",
                "remove the whole item instead, or give the item another key first",
            ));
        };
        return verified(promote_first_key(text, dash, line_start, next, after_key), path);
    }
    let from_key = entry.key.as_ref().map_or(entry.span.0, |k| k.span.0);
    let start = text[..from_key].rfind('\n').map_or(from_key, |at| at + 1);
    let end = end_of_last_line(text, content_end(text, entry, &walk));
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..start]);
    out.push_str(&text[end..]);
    verified(out, path)
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
        // `placements: []` becomes `placements:` and the block under it. **A comment after the `[]` stays
        // where it was**: it is the key's, not the empty list's, and the list is the only thing that
        // changes.
        // The line the empty collection stands on, split either side of it: what comes before is the key,
        // and whatever follows — a comment, when a person wrote one — is the KEY's, so it stays there.
        let line_start = text[..node.span.0].rfind('\n').map_or(0, |at| at + 1);
        let line_end = text[line_start..].find('\n').map_or(text.len(), |at| line_start + at + 1);
        let head = text[line_start..node.span.0].trim_end_matches(' ');
        // The collection's own end. Its span can stop a byte short of the bracket, so the bracket is
        // found rather than assumed, and everything after it on the line — a comment — stays with the
        // key.
        let close = if text[node.span.0..line_end].trim_start().starts_with('{') { '}' } else { ']' };
        let bracket = text[node.span.0..line_end].rfind(close).map_or(node.span.1, |at| node.span.0 + at + 1);
        let after = text[bracket.min(line_end)..line_end].trim_end_matches('\n');
        out.push_str(&text[..line_start]);
        out.push_str(head);
        out.push_str(after);
        out.push('\n');
        out.push_str(block.trim_end_matches('\n'));
        out.push('\n');
        out.push_str(&text[line_end..]);
        return out;
    }
    out.push_str(&text[..at]);
    // A file whose last line has no newline would otherwise have the block glued onto it.
    if at > 0 && !text[..at].ends_with('\n') {
        out.push('\n');
    }
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
    if node.kind == NodeKind::Scalar {
        // **A block scalar's span runs one line past its content.** `saphyr-parser` ends a `|` or `>`
        // scalar at the start of the next line, so an edit that took `span.1` would eat the field after
        // it. Its content's last line is the last non-blank line of the span, and that is where it ends.
        if node.scalar.as_ref().is_some_and(|leaf| leaf.style == Style::Block) {
            // The span ends on the next line's indentation, so the content's end is just past its last
            // non-blank character. The newline that ends that line stays: it is the line break the next
            // field is written after.
            let content = text[..node.span.1].trim_end();
            return content.len().max(node.span.0);
        }
        return node.span.1;
    }
    if is_flow(text, node) {
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
fn walk_or_refuse(text: &str) -> Result<Walk, Problem> {
    let walk = yaml::walk(text);
    // One question, one answer: `first_fault` is what the reader asks too, so an edit and a read cannot
    // disagree about which fault a file has. A file that does not read is refused before any path is
    // looked for, because a wrong path in a broken file must not be the answer.
    if let Some(site) = yaml::first_fault(text, &walk) {
        return Err(yaml::refusal_of(FILE, site));
    }
    Ok(walk)
}

/// The node at `path`, or a refusal naming it and its nearest neighbours.
#[allow(clippy::result_large_err)]
fn find<'a>(walk: &'a Walk, path: &str) -> Result<&'a Node, Problem> {
    // **The root's own path is the empty string**, which is how the walk spells it, and a caller wanting the
    // document's mapping says `""` rather than a name the file does not have.
    walk.nodes.iter().find(|n| n.path == path).ok_or_else(|| {
        let near = nearest(walk, path);
        // A path the file does not have has no line: no position is claimed rather than `line 1`.
        yaml::problem_nowhere(
            FILE,
            Some(path.to_owned()),
            &format!("`{path}` is not in this file"),
            &format!("the nearest paths are: {}", near.iter().map(|p| format!("`{p}`")).collect::<Vec<_>>().join(", ")),
            "check the path against the file's own keys",
        )
    })
}

/// **`path` is not what the call needed**, refused at the node that stands there.
fn not_a_at(text: &str, node: &Node, path: &str, wanted: &str) -> Problem {
    at_node_named(
        text,
        node,
        path,
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
