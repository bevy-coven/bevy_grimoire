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
//! - **Block style**, never flow: one field a line.
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
//! `saphyr-parser` is also what tells a leaf its own bytes, which is what `yaml_surgery` edits.
//!
//! Engine-free: nothing here knows what a window, a map view or an agent is.

use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;
use saphyr_parser::{Event, ScalarStyle};

use crate::problem::{Detail, Location, Problem};

/// **How many spaces one nesting level of a mapping is indented by**: the writer's, and the step
/// `yaml_surgery` copies when it adds an item beside siblings that are indented.
pub const INDENT_SPACES: usize = 2;

/// The page that explains a YAML refusal to a person.
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
    // The tab first: `saphyr-parser` complains about a tab-indented line two ways — sometimes at the
    // tab, sometimes at the line after it — and the tab is the truth in both cases.
    let (blocks, site) = walk(text);
    if let Some(site) = indentation_tab(text, &blocks) {
        return Err(refusal(path, site));
    }
    if let Some(site) = site {
        return Err(refusal(path, site));
    }
    let deserializer = serde_yaml_ng::Deserializer::from_str(text);
    match serde_path_to_error::deserialize::<_, T>(deserializer) {
        Ok(value) => Ok(value),
        Err(error) => {
            let field = path_of(&error.path().to_string());
            let inner = error.inner();
            let (line, column) = inner.location().map_or((1, 1), |l| (l.line(), l.column()));
            Err(Problem::with(
                format!("{path} {WHAT}"),
                Detail::default()
                    .at(Location::file(path.to_owned()).field(field.clone()))
                    .because(format!("line {line}, column {column}: {}", sentence(&inner.to_string())))
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

/// **The field path a refusal names**, from `serde_path_to_error`'s path: `.` is the file's root and is
/// no field at all; `placements[0].turn_deg` is kept as it is.
fn path_of(path: &str) -> Option<String> {
    let path = path.trim_start_matches('.');
    (!path.is_empty()).then(|| path.to_owned())
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
struct Site {
    what: String,
    field: Option<String>,
    line: usize,
    column: usize,
    why: String,
    fix: String,
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

/// **Walk `text`'s events once**: the line ranges of its block scalars, and the first construct the file
/// may not say.
///
/// It answers both rather than stopping at the refusal, because a tab in a block scalar's content is
/// content and the walk is what says which lines those are: a file refused for a tab must not be
/// refused for one that is a character in a note.
fn walk(text: &str) -> (Blocks, Option<Site>) {
    let mut blocks: Blocks = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    let mut documents = 0usize;
    let parser = saphyr_parser::Parser::new_from_str(text);

    for event in parser {
        let (event, span) = match event {
            Ok(event) => event,
            Err(error) => {
                let marker = error.marker();
                let info = error.info().to_owned();
                let tab = info.to_ascii_lowercase().contains("tab");
                let site = Site {
                    what: if tab { "a tab is used for indentation".to_owned() } else { "the text is not YAML".to_owned() },
                    field: path_of(&child_path(&stack)),
                    line: marker.line(),
                    column: marker.col() + 1,
                    why: info,
                    fix: if tab { indent_fix() } else { "write the line as YAML 1.2".to_owned() },
                };
                return (blocks, Some(site));
            }
        };
        let line = span.start.line();
        let column = span.start.col() + 1;

        match event {
            Event::StreamStart | Event::StreamEnd | Event::Nothing => {}
            Event::DocumentStart(_) => {
                documents += 1;
                if documents > 1 {
                    return (blocks, Some(Site {
                        what: "a second document starts here".to_owned(),
                        field: None,
                        line,
                        column,
                        why: "a project file is one document, so a second one is a file that was not made".to_owned(),
                        fix: "keep one document here and put the other in a file of its own".to_owned(),
                    }));
                }
            }
            Event::DocumentEnd => {}
            Event::Alias(_) => {
                return (blocks, Some(Site {
                    what: "an alias (`*name`) is not read".to_owned(),
                    field: path_of(&child_path(&stack)),
                    line,
                    column,
                    why: "an alias points at an anchor elsewhere, so this value does not stand where it is read".to_owned(),
                    fix: "write the value out where it is used, and drop the anchor and its alias".to_owned(),
                }));
            }
            Event::Scalar(value, style, anchor, _tag) => {
                if anchor != 0 {
                    return (blocks, Some(anchor_refusal(&stack, line, column)));
                }
                if matches!(stack.last(), Some(Frame::Map { key: None, .. })) {
                    let key = value.to_string();
                    if style == ScalarStyle::Plain && key == "<<" {
                        return (blocks, Some(Site {
                            what: "a merge key (`<<`) is not read".to_owned(),
                            field: path_of(stack.last().map_or("", Frame::path)),
                            line,
                            column,
                            why: "a merge copies another mapping's keys in, so this mapping does not say what it holds".to_owned(),
                            fix: "write the keys out here, and drop the merge".to_owned(),
                        }));
                    }
                    if let Some(Frame::Map { path, key: slot, seen }) = stack.last_mut() {
                        if let Some(first) = seen.get(&key) {
                            return (blocks, Some(Site {
                                what: format!("`{key}` is given twice"),
                                field: path_of(&join_key(path, &key)),
                                line,
                                column,
                                why: format!("it is first given on line {first}, so a reader would take one of the two by accident of order"),
                                fix: format!("keep one `{key}`"),
                            }));
                        }
                        seen.insert(key.clone(), line);
                        *slot = Some(key);
                    }
                    continue;
                }
                if matches!(style, ScalarStyle::Literal | ScalarStyle::Folded) {
                    blocks.push((span.start.line(), span.end.line()));
                }
                finish_value(&mut stack);
            }
            Event::SequenceStart(anchor, _) => {
                if anchor != 0 {
                    return (blocks, Some(anchor_refusal(&stack, line, column)));
                }
                stack.push(Frame::Seq { path: child_path(&stack), next: 0 });
            }
            Event::MappingStart(anchor, _) => {
                if anchor != 0 {
                    return (blocks, Some(anchor_refusal(&stack, line, column)));
                }
                stack.push(Frame::Map { path: child_path(&stack), key: None, seen: Default::default() });
            }
            Event::SequenceEnd | Event::MappingEnd => {
                stack.pop();
                finish_value(&mut stack);
            }
        }
    }
    (blocks, None)
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
    }
}

fn indent_fix() -> String {
    format!("indent with spaces, {INDENT_SPACES} per level")
}

/// One frame of the walk: the node being read, and where the next child of it goes.
enum Frame {
    Seq { path: String, next: usize },
    Map { path: String, key: Option<String>, seen: std::collections::BTreeMap<String, usize> },
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
        Some(Frame::Map { path, key, .. }) => join_key(path, key.as_deref().unwrap_or("")),
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

/// The value at a path is finished, so its parent's next item or key is what comes after it.
fn finish_value(stack: &mut [Frame]) {
    match stack.last_mut() {
        Some(Frame::Seq { next, .. }) => *next += 1,
        Some(Frame::Map { key, .. }) => *key = None,
        None => {}
    }
}

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
            let indent = line[..leading].chars().map(|c| if c == '\t' { INDENT_SPACES } else { 1 }).sum();
            return Some(Site {
                what: "a tab is used for indentation".to_owned(),
                field: owning_path(text, number, indent),
                line: number,
                column: at + 1,
                why: format!("line {number} is indented with a tab, and a reader that took it would nest the line somewhere nobody wrote"),
                fix: indent_fix(),
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
                '\t' => width += INDENT_SPACES,
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
