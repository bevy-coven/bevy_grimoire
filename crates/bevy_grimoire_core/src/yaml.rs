//! **The one place a project file is read or written as YAML** (GD-01, #363).
//!
//! A project file is data a person and an agent both edit, so the rules it is read by are stated once
//! here and every kind that moves to YAML goes through them:
//!
//! | call | reads |
//! |---|---|
//! | [`read`] | a file on disk |
//! | [`from_str`] | text a caller already holds (an editor buffer, a test) |
//! | [`to_string`] | the one writer, whose style is below |
//!
//! # What a file may not say, and why
//!
//! YAML can express a value in ways that make the file stop saying what it means where it stands, and
//! a file a person reads and an agent edits has to say it. Each of these is **refused, never
//! guessed**:
//!
//! - **an unknown field** — the types deny them (`#[serde(deny_unknown_fields)]`), so a key nothing
//!   reads is a typo or a field from a newer build, and either way its value would be silently lost;
//! - **a key given twice** — which of the two a reader took would be an accident of order;
//! - **an anchor, an alias and a `<<` merge key** — `&b` … `*b` makes one value stand in two places,
//!   so the second place cannot be read without the first. The anchor is refused where it stands,
//!   because that is where the indirection starts;
//! - **more than one document** — a project file is one value; a second document is a second file
//!   that was not made;
//! - **a tab used for indentation** — YAML forbids it, and a reader that shrugs at it turns a mixed
//!   indentation into a nesting nobody wrote. A tab *inside* a string, a quoted scalar or a block
//!   scalar's content is content, and reads.
//!
//! # A refusal is one [`Problem`] (P7, `docs/ui/vision.md`)
//!
//! It names the file, the line and the column, the field's path (`placements[3].turn_deg`), what was
//! expected and why, and the fix. Every surface — a toast, a journal row, an agent's answer — is a
//! view of that one value, so none of them can drop a part the others have.
//!
//! # The written style, which is what a diff shows
//!
//! - **Block style**, never flow: one field a line, and an item of a list a line of its own. The one
//!   exception is an **empty** collection, which is `[]` or `{}` because a block has nothing to write.
//! - **Two spaces per nesting level** of mappings ([`INDENT_SPACES`]).
//! - **The type's field order**, because the file then reads the way the type is declared, so an edit
//!   of one field is a diff of one field.
//! - **A string is quoted only when YAML needs it**: `yard` is plain, `'a: b'` and `''` are quoted.
//!   `no`, `on` and `yes` stay **plain strings** — this reader follows YAML 1.2, where only `true`
//!   and `false` are booleans — so a file does not grow quotes to defend words YAML 1.1 would have
//!   read as something else.
//! - **A trailing newline, and no blank line after it.**
//!
//! `to_string` then [`from_str`] round-trips every type a project holds
//! (`tests/yaml_round_trips_every_project_type.rs`).
//!
//! # Why `serde_yaml_ng`, and why it is scanned first
//!
//! `serde_yaml_ng` reads YAML 1.2's core schema, so only `true` and `false` are booleans, and its
//! refusal carries a line and a column. Its reader alone is not enough: it *resolves* an anchor
//! silently, it reports a duplicate key with no position, and it accepts a leading tab. So the text is
//! walked once by `saphyr-parser`'s event parser, which reports every node's path and position — that
//! walk is where the refusals above come from, each at the line and field path it would have had — and
//! `serde_yaml_ng` then reads the value, its refusal reported in the same shape.
//!
//! Engine-free: nothing here knows what a window, a map view or an agent is.

use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;
use saphyr_parser::{Event, ScalarStyle};

use crate::problem::{Detail, Location, Problem};

/// **How many spaces one nesting level of a mapping is indented by**: the writer's, and the step
/// [`crate::yaml_surgery`] gives a new item's fields under its `-`.
pub const INDENT_SPACES: usize = 2;

/// **How many columns one tab is worth** when a refusal says where a tab-indented line would have sat.
/// YAML gives a tab no width at all (it is forbidden as indentation), so this is a reading convenience
/// for a person's editor, which is what the field path in a refusal is derived from. Eight is the tab
/// stop every editor's default starts at, and it is deliberately not [`INDENT_SPACES`]: a mapping's
/// step is the writer's style, while this is only how far a tab was thought to reach.
const TAB_WIDTH: usize = 8;

/// The page that explains a YAML refusal to a person: "The rules a project file is read by" in it.
const DOCS: &str = "docs/guide/your-game.md";

/// The sentence every refusal opens with, after the file's name.
const WHAT: &str = "did not read";

/// **Read the file at `path` as a `T`.** The path is what a refusal names, as the caller spells it.
///
/// [`Problem`] is the workspace's one refusal shape (`problem.rs`), which every surface renders, so a
/// reader here answers it and not a boxed copy of it: the size is the shape's, and boxing it would make
/// this reader's refusals a different type from the operation dispatcher's.
#[allow(clippy::result_large_err)]
pub fn read<T: DeserializeOwned>(path: &Path) -> Result<T, Problem> {
    let name = path.display().to_string();
    let text = std::fs::read_to_string(path).map_err(|e| {
        Problem::with(
            format!("{name} {WHAT}"),
            Detail::default()
                .at(Location::file(name.clone()))
                .because(e.to_string())
                .fix(format!("check that {name} is in the project and can be read"))
                .docs(DOCS),
        )
    })?;
    from_str(&text, &name)
}

/// **Read `text` as a `T`.** `path` is only what a refusal names: the file the text came from, or the
/// one it is about to be written to.
#[allow(clippy::result_large_err)]
pub fn from_str<T: DeserializeOwned>(text: &str, path: &str) -> Result<T, Problem> {
    let walk = walk(text);
    if let Some(site) = first_fault(text, &walk) {
        return Err(refusal(path, site));
    }
    let deserializer = serde_yaml_ng::Deserializer::from_str(text);
    match serde_path_to_error::deserialize::<_, T>(deserializer) {
        Ok(value) => Ok(value),
        Err(error) => {
            let field = path_of(&error.path().to_string());
            let inner = error.inner();
            let why = positioned(inner.location(), &inner.to_string());
            Err(Problem::with(
                format!("{path} {WHAT}"),
                Detail::default()
                    .at(Location::file(path.to_owned()).field(field.clone()))
                    .because(why)
                    .fix(fix_for(&field))
                    .docs(DOCS),
            ))
        }
    }
}

/// **Write `value` as a project file**, in the one style the module docs describe.
///
/// `serde_yaml_ng`'s writer is already block style, in the type's field order, with a trailing newline
/// and a string quoted only where YAML needs one, so this is that writer and nothing else: a second
/// style would be a second thing to keep in step, and a post-pass over its output would be a place
/// where a value and its text could disagree.
#[allow(clippy::result_large_err)]
pub fn to_string<T>(value: &T) -> Result<String, Problem>
where
    T: Serialize + ?Sized,
{
    serde_yaml_ng::to_string(value).map_err(|e| {
        Problem::with("a value did not write as YAML", Detail::default().because(e.to_string()).docs(DOCS))
    })
}

/// **Read `text` as the plain JSON a value holds**, keeping nothing but names, numbers and strings: a YAML tag
/// names an enum variant, and a JSON Schema and a reflected type both spell a named variant as the key of a
/// one-key object, so `!Near {to: …}` becomes `{"Near": {"to": …}}`.
///
/// **It is the same walk every refusal comes from**: the file is first checked by [`walk`] — no duplicate key,
/// no anchor, no alias, no second document — and only then converted, so a document a person could not see the
/// fault in is refused with a line rather than quietly reshaped. This is what a game's entity files are read
/// through, so they meet the rules every project file meets (#367).
#[allow(clippy::result_large_err)]
pub fn to_json(text: &str, path: &str) -> Result<serde_json::Value, Problem> {
    let walk = walk(text);
    if let Some(site) = first_fault(text, &walk) {
        return Err(refusal(path, site));
    }
    let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(text).map_err(|e| {
        Problem::with(
            format!("{path} {WHAT}"),
            Detail::default().because(positioned(e.location(), &e.to_string())).docs(DOCS),
        )
    })?;
    Ok(json_of(value))
}

/// A `serde_yaml_ng` value as JSON, a tag as the key of a one-key object.
fn json_of(value: serde_yaml_ng::Value) -> serde_json::Value {
    use serde_yaml_ng::Value as Y;
    match value {
        Y::Tagged(tagged) => {
            let mut map = serde_json::Map::new();
            let name = tagged.tag.to_string();
            map.insert(name.trim_start_matches('!').to_owned(), json_of(tagged.value));
            serde_json::Value::Object(map)
        }
        Y::Null => serde_json::Value::Null,
        Y::Bool(b) => serde_json::Value::Bool(b),
        Y::Number(n) => serde_json::to_value(n).unwrap_or(serde_json::Value::Null),
        Y::String(s) => serde_json::Value::String(s),
        Y::Sequence(items) => serde_json::Value::Array(items.into_iter().map(json_of).collect()),
        Y::Mapping(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(k, v)| (k.as_str().unwrap_or_default().to_owned(), json_of(v)))
                .collect(),
        ),
    }
}

/// **The field path a refusal names**, from `serde_path_to_error`'s path: `.` is the file's root and is
/// no field at all; `placements[0].turn_deg` is kept as it is.
fn path_of(path: &str) -> Option<String> {
    let path = path.trim_start_matches('.');
    (!path.is_empty()).then(|| path.to_owned())
}

/// **A serde message, with its line and column when it has one and with none when it has not.**
///
/// `serde_yaml_ng` answers no location for a few refusals — a stream it could not shape into one
/// document, among them — and writing `line 1, column 1` for those would send a reader to a place the
/// fault is not, which is worse than saying nothing about where.
fn positioned(location: Option<serde_yaml_ng::Location>, message: &str) -> String {
    match location {
        Some(at) => format!("line {}, column {}: {}", at.line(), at.column(), sentence(message)),
        None => sentence(message),
    }
}

/// A serde message without the path it repeats (the field is in [`Location`]) and without the
/// ` at line N column M` at the end (the line and column are their own part of the refusal).
fn sentence(message: &str) -> String {
    let message = match message.rfind(" at line ") {
        Some(at) => &message[..at],
        None => message,
    };
    // A refusal about a nested value repeats its own path first: `placements[0]: unknown field `x``.
    match message.find(": ") {
        Some(at) if looks_like_path(&message[..at]) => message[at + 2..].to_owned(),
        _ => message.to_owned(),
    }
}

/// Whether `text` reads as a field path (`placements[0].turn_deg`) rather than as prose.
fn looks_like_path(text: &str) -> bool {
    !text.is_empty()
        && !text.contains(' ')
        && text.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '[' | ']' | '-'))
}

/// What to do about a file wrong at `field`.
fn fix_for(field: &Option<String>) -> String {
    match field {
        Some(field) => format!("write `{field}` as the type declares it, or remove it"),
        None => "write the file as one YAML document of the type it holds".to_owned(),
    }
}

/// **One thing the scan found the file may not say**, with where it stands.
#[derive(Clone)]
pub(crate) struct Site {
    what: String,
    field: Option<String>,
    line: usize,
    column: usize,
    why: String,
    fix: String,
    /// **Whether this is `saphyr-parser`'s own fault report**, whose line may be one past the truth.
    /// See [`earlier_fault`].
    tab_report: bool,
}

/// **The first fault in a file**, whichever reader met it first: the tab that indents a line, or the
/// walk's own first refusal. One function, so the reader and an editor ask the same question and cannot
/// answer it two ways.
pub(crate) fn first_fault(text: &str, walk: &Walk) -> Option<Site> {
    let tab = indentation_tab(text, &walk.blocks);
    earlier_fault(tab, walk.site.clone())
}

/// **A refusal at a byte of the text**, with its own line and column: what a surgery says when it
/// refuses an edit, so no refusal carries a made-up position.
#[allow(clippy::result_large_err)]
pub(crate) fn problem_at_node(path: &str, field: Option<String>, text: &str, byte: usize, what: &str, why: &str, fix: &str) -> Problem {
    let site = Site {
        what: what.to_owned(),
        field,
        line: line_of(text, byte),
        column: column_of(text, byte),
        why: why.to_owned(),
        fix: fix.to_owned(),
        tab_report: false,
    };
    refusal(path, site)
}

/// **The line a byte is on**, counted from 1 as an editor shows it.
fn line_of(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].bytes().filter(|b| *b == b'\n').count() + 1
}

/// **The column a byte is at**, counted in characters from its line's start, from 1.
fn column_of(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    let start = text[..byte].rfind('\n').map_or(0, |at| at + 1);
    text[start..byte].chars().count() + 1
}

/// **A refusal about a place the file has no line for** — a path the file does not contain — says where
/// the file is and nothing about where in it, rather than inventing `line 1, column 1`.
#[allow(clippy::result_large_err)]
pub(crate) fn problem_nowhere(path: &str, field: Option<String>, what: &str, why: &str, fix: &str) -> Problem {
    Problem::with(
        format!("{path} — {what}"),
        Detail::default().at(Location::file(path.to_owned()).field(field)).because(why.to_owned()).fix(fix.to_owned()).docs(DOCS),
    )
}

/// **A refusal that adopts a fault the reader found**, naming the field an edit asked about: the edit's
/// own path, with the walk's own line and column and words.
pub(crate) fn problem_naming(path: &str, field: Option<String>, site: Site) -> Problem {
    refusal(path, Site { field, ..site })
}

/// A [`Site`] as the one [`Problem`] it is.
pub(crate) fn refusal_of(path: &str, site: Site) -> Problem {
    refusal(path, site)
}

/// `path[3]`, the way a refusal and an edit both spell a sequence item.
pub(crate) fn join_path_index(parent: &str, index: usize) -> String {
    join_index(parent, index)
}

/// `path.key`, the way a refusal and an edit both spell a mapping entry.
pub(crate) fn join_path_key(parent: &str, key: &str) -> String {
    join_key(parent, key)
}

/// A [`Site`] as the one [`Problem`] a refusal is: the file, the field, the line and column, why, and
/// the fix.
fn refusal(path: &str, site: Site) -> Problem {
    Problem::with(
        format!("{path} {WHAT}"),
        Detail::default()
            .at(Location::file(path.to_owned()).field(site.field))
            .because(format!("line {}, column {}: {} — {}", site.line, site.column, site.what, site.why))
            .fix(site.fix)
            .docs(DOCS),
    )
}

// -------------------------------------------------------------------------------------------------
// The scan: every node's path, kind and bytes, and the constructs a file may not use.
// -------------------------------------------------------------------------------------------------

/// The line range of each block scalar's **content** (its first content line, and the line after its
/// last), so a tab inside that content is not mistaken for indentation.
type Blocks = Vec<(usize, usize)>;

/// **What a node is.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NodeKind {
    Scalar,
    Sequence,
    Mapping,
}

/// **A node's key**, for a node that is a mapping's value: the key's text and its bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Key {
    pub(crate) name: String,
    pub(crate) span: (usize, usize),
}

/// **One node of a file**, with the path it stands at and the bytes it occupies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Node {
    /// `placements[3].turn_deg`; empty at the file's root.
    pub(crate) path: String,
    /// What stands at the path.
    pub(crate) kind: NodeKind,
    /// The node's own bytes: its first character through its last.
    pub(crate) span: (usize, usize),
    /// The key it is a value under, when it is one.
    pub(crate) key: Option<Key>,
    /// For an item of a block sequence: the byte of its `-`.
    pub(crate) dash: Option<usize>,
    /// For a scalar: its style and its text.
    pub(crate) scalar: Option<Leaf>,
}

/// **One scalar of a file as it stands**: its text without quotes, and the style that says what kind of
/// value it is. `yaml_surgery` decides whether a value may replace it from exactly this pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Leaf {
    pub(crate) text: String,
    pub(crate) style: Style,
}

/// **How a scalar was written**, which decides what kind of value it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Style {
    /// `90.0`: plain, so the reader decides whether it is a number, a bool or a word.
    Plain,
    /// `'a'` or `"a"`: always a string.
    Quoted,
    /// `|` or `>`: always a string, and it may span lines.
    Block,
}

impl Style {
    /// **Whether a scalar written this way holds a number**: a plain scalar whose text is one. A quoted
    /// scalar that spells `9` is the string `"9"`, which is the one thing a write must not turn a number
    /// into.
    pub(crate) fn accepts_number(self, text: &str) -> bool {
        self == Style::Plain && is_number(text)
    }

    /// **Whether a scalar written this way holds a string**, whatever its text spells: a quoted scalar
    /// and a block scalar always do, and a plain one does only when its text is not a number, a bool or
    /// a null.
    pub(crate) fn is_text(self, text: &str) -> bool {
        match self {
            Style::Quoted | Style::Block => true,
            Style::Plain => !is_number(text) && !matches!(text, "true" | "false" | "null" | "Null" | "NULL" | "~" | ""),
        }
    }
}

/// **Whether `text` reads as a number** to this reader: a YAML 1.2 integer or float, and its infinities
/// and NaN.
pub(crate) fn is_number(text: &str) -> bool {
    let unsigned = text.strip_prefix(['-', '+']).unwrap_or(text);
    if unsigned.is_empty() {
        return false;
    }
    if unsigned.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    if unsigned.parse::<f64>().is_ok() && (unsigned.contains('.') || unsigned.contains(['e', 'E'])) {
        return true;
    }
    if matches!(unsigned, ".inf" | ".Inf" | ".INF" | ".nan" | ".NaN" | ".NAN") {
        return true;
    }
    ["0x", "0o", "0b"].iter().any(|prefix| {
        unsigned.strip_prefix(prefix).is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric()))
    })
}

/// Everything the walk learned about a file.
#[derive(Clone)]
pub(crate) struct Walk {
    /// Every node, parents before children.
    pub(crate) nodes: Vec<Node>,
    /// The block scalars' content lines.
    blocks: Blocks,
    /// The first construct the file may not say, when it says one.
    pub(crate) site: Option<Site>,
}

/// **Walk `text`'s events once**: the line ranges of its block scalars, and the first construct the file
/// may not say.
///
/// It answers both rather than stopping at the refusal, because a tab in a block scalar's content is
/// content and the walk is what says which lines those are: a file refused for a tab must not be
/// refused for one that is a character in a note — and because the walk stops at the first fault (a
/// scan error ends `saphyr-parser`'s event stream), a tab found *after* that fault is not yet known to be
/// indentation: it may be the content of a block scalar further down. So [`from_str`] weighs the two:
/// see [`earlier_fault`].
pub(crate) fn walk(text: &str) -> Walk {
    let mut walk = Walk { nodes: Vec::new(), blocks: Vec::new(), site: None };
    let mut stack: Vec<Frame> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    let mut documents = 0usize;
    let parser = saphyr_parser::Parser::new_from_str(text);
    // `saphyr-parser` counts in characters and this module edits bytes, so the offsets are the byte of
    // each character, once.
    let offsets: Vec<usize> = text.char_indices().map(|(at, _)| at).collect();
    let byte = |index: usize| offsets.get(index).copied().unwrap_or(text.len());

    for event in parser {
        let (event, span) = match event {
            Ok(event) => event,
            Err(error) => {
                let marker = error.marker();
                let info = error.info().to_owned();
                let lower = info.to_ascii_lowercase();
                // **An alias is an alias however the parser met it.** A `*name` with nothing to point
                // at is a scan error (`found unknown anchor`) and never reaches `Event::Alias`, so the
                // file would otherwise be refused as "not YAML" — which says nothing about what to fix.
                // **A `*name` with nothing to point at** is the one anchor-flavoured scan error that is an
                // alias: `found unknown anchor`, at the alias. A broken `&` (`did not find expected
                // alphabetic or numeric character`) names no anchor at all, so it is refused as the
                // anchor syntax it is and not as an alias the file does not have.
                let site = if lower.contains("unknown anchor") {
                    alias_site(&stack, marker.line(), marker.col() + 1)
                } else if lower.contains("anchor") {
                    Site {
                        what: "a broken anchor (`&`) is not read".to_owned(),
                        field: path_of(&child_path(&stack)),
                        line: marker.line(),
                        column: marker.col() + 1,
                        why: info,
                        fix: "write the value out where it is used, and drop the anchor".to_owned(),
                        tab_report: false,
                    }
                } else if lower.contains("tab") {
                    Site {
                        what: "a tab is used for indentation".to_owned(),
                        field: path_of(&child_path(&stack)),
                        line: marker.line(),
                        column: marker.col() + 1,
                        why: info,
                        fix: indent_fix(),
                        tab_report: true,
                    }
                } else {
                    Site {
                        what: "the text is not YAML".to_owned(),
                        field: path_of(&child_path(&stack)),
                        line: marker.line(),
                        column: marker.col() + 1,
                        why: info,
                        fix: "write the line as YAML 1.2".to_owned(),
                        tab_report: true,
                    }
                };
                walk.site = Some(site);
                return walk;
            }
        };
        let start = byte(span.start.index());
        let end = byte(span.end.index());
        let line = span.start.line();
        let column = span.start.col() + 1;

        match event {
            Event::StreamStart | Event::StreamEnd | Event::Nothing => {}
            Event::DocumentStart(_) => {
                documents += 1;
                if documents > 1 {
                    walk.site = Some(Site {
                        what: "a second document starts here".to_owned(),
                        field: None,
                        line,
                        column,
                        why: "a project file is one document, so a second one is a file that was not made".to_owned(),
                        fix: "keep one document here and put the other in a file of its own".to_owned(),
                        tab_report: false,
                    });
                    return walk;
                }
            }
            Event::DocumentEnd => {}
            // Unreachable for `saphyr-parser` 0.3.3, which fails a `*name` before it emits this event
            // (see the scan-error arm above, which is what actually refuses one), and kept because the
            // event exists: a parser version that reports an alias instead must not read one through.
            Event::Alias(_) => {
                walk.site = Some(alias_site(&stack, line, column));
                return walk;
            }
            Event::Scalar(value, style, anchor, _tag) => {
                if anchor != 0 {
                    walk.site = Some(anchor_refusal(&stack, line, column));
                    return walk;
                }
                if matches!(stack.last(), Some(Frame::Map { key: None, .. })) {
                    let name = value.to_string();
                    if style == ScalarStyle::Plain && name == "<<" {
                        walk.site = Some(Site {
                            what: "a merge key (`<<`) is not read".to_owned(),
                            field: path_of(stack.last().map_or("", Frame::path)),
                            line,
                            column,
                            why: "a merge copies another mapping's keys in, so this mapping does not say what it holds".to_owned(),
                            fix: "write the keys out here, and drop the merge".to_owned(),
                            tab_report: false,
                        });
                        return walk;
                    }
                    if let Some(Frame::Map { path, key: slot, seen }) = stack.last_mut() {
                        if let Some(first) = seen.get(&name) {
                            walk.site = Some(Site {
                                what: format!("`{name}` is given twice"),
                                field: path_of(&join_key(path, &name)),
                                line,
                                column,
                                why: format!("it is first given on line {first}, so a reader would take one of the two by accident of order"),
                                fix: format!("keep one `{name}`"),
                                tab_report: false,
                            });
                            return walk;
                        }
                        seen.insert(name.clone(), line);
                        *slot = Some(Key { name, span: (start, end) });
                    }
                    continue;
                }
                let style = match style {
                    ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted => Style::Quoted,
                    ScalarStyle::Literal | ScalarStyle::Folded => Style::Block,
                    ScalarStyle::Plain => Style::Plain,
                };
                if style == Style::Block {
                    walk.blocks.push((span.start.line(), span.end.line()));
                }
                let path = child_path(&stack);
                let key = take_key(&mut stack);
                let dash = dash_before(text, start);
                walk.nodes.push(Node { path, kind: NodeKind::Scalar, span: (start, end), key, dash, scalar: Some(Leaf { text: value.to_string(), style }) });
                finish_value(&mut stack);
            }
            Event::SequenceStart(anchor, _) => {
                if anchor != 0 {
                    walk.site = Some(anchor_refusal(&stack, line, column));
                    return walk;
                }
                let path = child_path(&stack);
                let key = take_key(&mut stack);
                let dash = dash_before(text, start);
                open.push(walk.nodes.len());
                walk.nodes.push(Node { path: path.clone(), kind: NodeKind::Sequence, span: (start, end), key, dash, scalar: None });
                stack.push(Frame::Seq { path, next: 0 });
            }
            Event::MappingStart(anchor, _) => {
                if anchor != 0 {
                    walk.site = Some(anchor_refusal(&stack, line, column));
                    return walk;
                }
                let path = child_path(&stack);
                let key = take_key(&mut stack);
                let dash = dash_before(text, start);
                open.push(walk.nodes.len());
                walk.nodes.push(Node { path: path.clone(), kind: NodeKind::Mapping, span: (start, end), key, dash, scalar: None });
                stack.push(Frame::Map { path, key: None, seen: Default::default() });
            }
            Event::SequenceEnd | Event::MappingEnd => {
                // A collection's own bytes run to the end of its last child, which is where its end
                // event stands.
                if let Some(index) = open.pop() {
                    walk.nodes[index].span.1 = end;
                }
                stack.pop();
                finish_value(&mut stack);
            }
        }
    }
    walk
}

/// The refusal an alias earns, wherever it stands.
fn alias_site(stack: &[Frame], line: usize, column: usize) -> Site {
    Site {
        what: "an alias (`*name`) is not read".to_owned(),
        field: path_of(&child_path(stack)),
        line,
        column,
        why: "an alias points at an anchor elsewhere, so this value does not stand where it is read".to_owned(),
        fix: "write the value out where it is used, and drop the anchor and its alias".to_owned(),
        tab_report: false,
    }
}

/// The refusal an anchor earns, wherever it stands.
fn anchor_refusal(stack: &[Frame], line: usize, column: usize) -> Site {
    Site {
        what: "an anchor (`&name`) is not read".to_owned(),
        field: path_of(&child_path(stack)),
        line,
        column,
        why: "a value given a name so an alias elsewhere can repeat it does not say everything where it stands".to_owned(),
        fix: "write the value out where it is used, and drop the anchor and its alias".to_owned(),
        tab_report: false,
    }
}

fn indent_fix() -> String {
    format!("indent with spaces, {INDENT_SPACES} per level")
}

/// One frame of the walk: the node being read, and where the next child of it goes.
enum Frame {
    Seq { path: String, next: usize },
    Map { path: String, key: Option<Key>, seen: std::collections::BTreeMap<String, usize> },
}

impl Frame {
    fn path(&self) -> &str {
        match self {
            Frame::Seq { path, .. } | Frame::Map { path, .. } => path,
        }
    }
}

/// The path the node about to start would have.
fn child_path(stack: &[Frame]) -> String {
    match stack.last() {
        None => String::new(),
        Some(Frame::Seq { path, next }) => join_index(path, *next),
        Some(Frame::Map { path, key, .. }) => join_key(path, key.as_ref().map_or("", |k| k.name.as_str())),
    }
}

fn join_key(parent: &str, key: &str) -> String {
    match (parent.is_empty(), key.is_empty()) {
        (_, true) => parent.to_owned(),
        (true, false) => key.to_owned(),
        (false, false) => format!("{parent}.{key}"),
    }
}

fn join_index(parent: &str, index: usize) -> String {
    format!("{parent}[{index}]")
}

/// **The key a node is a value under**, taken off its parent so the next sibling does not inherit it.
fn take_key(stack: &mut [Frame]) -> Option<Key> {
    match stack.last_mut() {
        Some(Frame::Map { key, .. }) => key.take(),
        _ => None,
    }
}

/// **The `-` an item of a block sequence starts with**, when the item starts with one. A flow item has
/// none, and neither has a value written after its key on the same line.
fn dash_before(text: &str, start: usize) -> Option<usize> {
    let line_start = text[..start].rfind('\n').map_or(0, |at| at + 1);
    let prefix = &text[line_start..start];
    if !prefix.trim_start().starts_with('-') || prefix.contains('[') || prefix.contains('{') {
        return None;
    }
    Some(line_start + prefix.len() - prefix.trim_start().len())
}

/// The value at a path is finished, so its parent's next item or key is what comes after it.
fn finish_value(stack: &mut [Frame]) {
    match stack.last_mut() {
        Some(Frame::Seq { next, .. }) => *next += 1,
        Some(Frame::Map { key, .. }) => *key = None,
        None => {}
    }
}

/// **Whichever of the two faults a file has comes first**, or the only one it has.
///
/// The tab and the walk are two readers of one text, and they see different amounts of it: the tab check
/// reads every line, while the walk stops at the first fault. So a tab on line 9 with a duplicate key on
/// line 2 must lose — the duplicate key is what the file's author meets first, and the tab may even be
/// content of a block scalar the walk never reached.
///
/// `saphyr-parser` reports a tab it trips over at the tab's line or at the line after it (both measured
/// against 0.3.3), so a tab one line past the walk's fault is the same fault and is taken as the tab.
fn earlier_fault(tab: Option<Site>, walked: Option<Site>) -> Option<Site> {
    match (tab, walked) {
        // The slack is the *parser's* alone: it reports a tab it meets at the tab's line or at the line
        // after it. A duplicate key, an anchor, a merge or a second document is placed exactly, so a tab
        // below one of those is not yet known to be indentation — the walk stopped before the block
        // scalar it might be content of — and the exact fault wins.
        (Some(tab), Some(walked)) if walked.tab_report && tab.line <= walked.line + TAB_REPORT_SLACK_LINES => Some(tab),
        (_, Some(walked)) => Some(walked),
        (tab, None) => tab,
    }
}

/// **How far past the parser's own fault a tab may be and still be the same fault**, because
/// `saphyr-parser` reports a tab it meets at the tab's line and sometimes at the line after it. It applies
/// to a [`Site::tab_report`] only: every other fault is placed exactly.
const TAB_REPORT_SLACK_LINES: usize = 1;

/// **The first tab that indents a line**, outside every block scalar's content.
///
/// `serde_yaml_ng` accepts a leading tab, and `saphyr-parser` accepts one on the first line, so this is
/// the check that makes the rule hold for the whole file rather than most of it.
fn indentation_tab(text: &str, blocks: &Blocks) -> Option<Site> {
    for (n, line) in text.lines().enumerate() {
        let number = n + 1;
        if blocks.iter().any(|(first, after)| (*first..*after).contains(&number)) {
            continue;
        }
        let leading = line.len() - line.trim_start_matches([' ', '\t']).len();
        if let Some(at) = line[..leading].find('\t') {
            let indent = line[..leading].chars().map(|c| if c == '\t' { TAB_WIDTH } else { 1 }).sum();
            return Some(Site {
                what: "a tab is used for indentation".to_owned(),
                field: owning_path(text, number, indent),
                line: number,
                column: at + 1,
                why: format!("line {number} is indented with a tab, and a reader that took it would nest the line somewhere nobody wrote"),
                fix: indent_fix(),
                tab_report: false,
            });
        }
    }
    None
}

/// **The field a line at `indent` columns belongs to**: the keys above it whose block values contain
/// it, outermost first (`placements`), joined the way a refusal spells a path.
///
/// Only the keys, not a sequence item's index: a tab is refused before anything decides which item it
/// was in, and the key is what a person edits. `None` when the line is a root key's own line, since the
/// file's root is no field.
fn owning_path(text: &str, line: usize, indent: usize) -> Option<String> {
    let mut keys: Vec<&str> = Vec::new();
    let mut inside = indent;
    for above in text.lines().take(line - 1).collect::<Vec<_>>().into_iter().rev() {
        let trimmed = above.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut width = 0usize;
        for c in above.chars() {
            match c {
                ' ' => width += 1,
                '\t' => width += TAB_WIDTH,
                _ => break,
            }
        }
        if width >= inside {
            continue;
        }
        inside = width;
        if let Some(key) = trimmed.strip_suffix(':') {
            keys.push(key.trim().trim_matches(['\'', '"']));
        }
        if inside == 0 {
            break;
        }
    }
    keys.reverse();
    let path = keys.join(".");
    (!path.is_empty()).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_path_is_kept_and_a_root_is_no_field() {
        assert_eq!(path_of("."), None);
        assert_eq!(path_of(".placements[0].turn_deg"), Some("placements[0].turn_deg".to_owned()));
        assert_eq!(path_of("placements[0].turn_deg"), Some("placements[0].turn_deg".to_owned()));
    }

    #[test]
    fn a_refusal_with_no_position_invents_none() {
        assert_eq!(positioned(None, "invalid type: unit value, expected struct Yard"), "invalid type: unit value, expected struct Yard");
    }

    #[test]
    fn a_serde_message_loses_the_path_it_repeats_and_its_position() {
        assert_eq!(sentence("sealed: invalid type: string \"no\", expected a boolean at line 2 column 9"), "invalid type: string \"no\", expected a boolean");
        assert_eq!(sentence("placements[0]: unknown field `x`, expected `id` at line 3 column 5"), "unknown field `x`, expected `id`");
        assert_eq!(sentence("missing field `version`"), "missing field `version`");
        assert_eq!(sentence("duplicate field `name`"), "duplicate field `name`");
    }

    #[test]
    fn a_key_and_an_item_path_are_joined_the_way_a_refusal_spells_them() {
        assert_eq!(join_key("", "name"), "name");
        assert_eq!(join_key("placements[0]", "turn_deg"), "placements[0].turn_deg");
        assert_eq!(join_index("placements", 3), "placements[3]");
    }
}
