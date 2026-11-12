//! **What the file editor knows about the languages a project holds** (GD-11, #373, and GD-12, #374).
//!
//! Engine-free: text in, byte ranges out. [`crate::files`] is what turns these answers into what
//! `bevy_gooey`'s `code_editor` draws (`Highlights`, `Diagnostics`, `FoldRanges`, `CodeCompletions`,
//! `CodeHover`), so a rule about a language is stated once, here, and a test reads it without a window.
//!
//! # Colours and errors cannot disagree, because they are read from the same place
//!
//! - **YAML** is scanned line by line in the one shape the project's writer emits (block style, a key a
//!   line, an item a line: `bevy_grimoire_core::yaml`'s "written style"), and *its errors are the
//!   loader's*: a file of a known kind is refused by [`bevy_grimoire_core::reload_check::check_yaml`], the
//!   reader the game loads it with, and any other file by the event walk
//!   ([`bevy_grimoire_core::yaml::to_json`]) that refuses a duplicate key, an anchor, a tab and a second
//!   document. A key colour is the key the walk would name.
//! - **A reference is a link.** A field the kind's JSON Schema marks `x-dark-factory-ref` (a placement's
//!   `descriptor` names a library piece) is drawn as `SyntaxRole::Reference`; the schema is the one
//!   [`bevy_grimoire_core::schema`] generates from the Rust type, so nothing here knows which fields
//!   name another file's value.
//! - **Completion and hover are the schema's**: the field names a mapping may hold, the values an enum
//!   may take, the ids a reference may point at, and each field's doc comment.
//!
//! # Not a parser, and why that is a decision and not a shortcut
//!
//! The colours of RON, Rhai and Markdown come from a small tokenizer each (no new dependency: the
//! owner's call of 2026-11-12). Their *errors* are not guessed by it: RON is read by `ron` (and by the
//! kind's own reader when the path is a project file), Rhai's are only what a scan can be sure of (a
//! bracket that never closes, a string that never ends: the engine's own compile is a crate the editor
//! does not link), and Markdown has none.

use std::ops::Range;

use bevy_gooey::{CodeSeverity, SyntaxRole};
use bevy_grimoire_core::reload_check::{self, Check, Format};
use serde_json::Value;

/// **The languages the editor colours**, by the file's extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    Yaml,
    Ron,
    Rhai,
    Markdown,
    /// Anything else the project holds as text: no colour, no folds, no findings.
    Plain,
}

impl Lang {
    /// The language of a path.
    pub fn of(path: &str) -> Lang {
        let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
        match ext.as_deref() {
            Some("yaml" | "yml") => Lang::Yaml,
            Some("ron") => Lang::Ron,
            Some("rhai") => Lang::Rhai,
            Some("md" | "markdown") => Lang::Markdown,
            _ => Lang::Plain,
        }
    }
}

/// **Whether a file is one the editor shows read-only**: the files in `trust::COMMAND_FILES` (`agent`
/// and `play`, which name a program to start; the guide, which tells an agent what to do) and the YAML
/// spelling of the first two. Changing them is a decision with its own page and its own trust check, so
/// an edit here must not be a way round it.
pub fn is_command_file(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    bevy_grimoire_core::trust::COMMAND_FILES.contains(&name)
        || rel == bevy_grimoire_core::agent_guide::PROJECT_PATH
        || matches!(name, "agent.yaml" | "play.yaml")
}

/// The sentence a read-only file's tab says, so nobody wonders why the keys do nothing.
pub const READ_ONLY_NOTE: &str = "Read-only here: this file is changed on its own page, so an edit cannot bypass its trust check";

// ── Lines ────────────────────────────────────────────────────────────────────

/// **A text's lines**, as byte ranges: a line's range excludes its `\n` and its `\r`.
pub struct Lines<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    pub fn new(text: &'a str) -> Lines<'a> {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(at, _)| at + 1));
        Lines { text, starts }
    }

    /// How many lines there are: a text that ends in `\n` has an empty last one, as the editor counts.
    pub fn count(&self) -> usize {
        self.starts.len()
    }

    /// The bytes of line `n` (0-based), without its line break.
    pub fn range(&self, n: usize) -> Range<usize> {
        let start = self.starts.get(n).copied().unwrap_or(self.text.len());
        let end = self.starts.get(n + 1).map_or(self.text.len(), |next| next - 1);
        let end = end.max(start);
        if self.text[start..end].ends_with('\r') { start..end - 1 } else { start..end }
    }

    /// The line a byte offset is on.
    pub fn line_of(&self, offset: usize) -> usize {
        self.starts.partition_point(|&s| s <= offset).saturating_sub(1)
    }

    pub fn text(&self, n: usize) -> &'a str {
        &self.text[self.range(n)]
    }
}

// ── YAML ─────────────────────────────────────────────────────────────────────

/// **One step of a path into a document**: a key, or the index of a list item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Seg {
    Key(String),
    Index(usize),
}

/// What one YAML line holds, by byte offset into the whole text.
#[derive(Clone, Debug, Default)]
struct YLine {
    start: usize,
    /// Columns of each `- ` marker on the line.
    dashes: Vec<usize>,
    /// The column of the first character after the markers.
    body_col: usize,
    /// The key's bytes (quotes included) and its name (quotes off).
    key: Option<(Range<usize>, String)>,
    value: Option<Range<usize>>,
    comment: Option<Range<usize>>,
    blank: bool,
    comment_only: bool,
    /// A line of a `|` or `>` block scalar's body.
    block_body: bool,
}

impl YLine {
    /// **The column this line's structure starts at**: where a nested line must be deeper than.
    fn col(&self) -> usize {
        self.dashes.first().copied().unwrap_or(self.body_col)
    }
}

fn is_block_indicator(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('|' | '>')) && chars.all(|c| matches!(c, '+' | '-') || c.is_ascii_digit())
}

/// The end of a quoted scalar starting at `from` (a quote), as an offset into `s` past the closing quote,
/// or `s.len()` when it never closes on this line.
fn quoted_end(s: &str, from: usize) -> usize {
    let bytes = s.as_bytes();
    let quote = bytes[from];
    let mut at = from + 1;
    while at < bytes.len() {
        match bytes[at] {
            b'\\' if quote == b'"' => at += 2,
            b'\'' if quote == b'\'' && bytes.get(at + 1) == Some(&b'\'') => at += 2,
            q if q == quote => return at + 1,
            _ => at += 1,
        }
    }
    s.len()
}

/// Where a trailing comment starts in a plain scalar or after a value: a `#` that follows a space.
fn comment_at(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'"' | b'\'' if at == 0 || bytes[at - 1] == b' ' => at = quoted_end(s, at),
            b'#' if at == 0 || bytes[at - 1] == b' ' => return Some(at),
            _ => at += 1,
        }
    }
    None
}

/// Where the key of `rest` ends (its last byte, exclusive) and where its colon is, if `rest` starts with
/// a key: `name:` followed by a space or the end of the line.
fn key_split(rest: &str) -> Option<(usize, usize)> {
    let first = rest.chars().next()?;
    if matches!(first, '[' | '{' | '!' | '&' | '*' | '|' | '>' | '#' | '%' | '@' | '`') {
        return None;
    }
    let end = if matches!(first, '"' | '\'') { quoted_end(rest, 0) } else { 0 };
    let bytes = rest.as_bytes();
    let mut at = end;
    while at < bytes.len() {
        match bytes[at] {
            b':' if bytes.get(at + 1).is_none_or(|b| *b == b' ') => {
                let key_end = rest[..at].trim_end().len();
                return Some((key_end, at));
            }
            b'#' if at > 0 && bytes[at - 1] == b' ' => return None,
            _ if end > 0 && bytes[at] != b' ' => return None,
            _ => at += 1,
        }
    }
    None
}

fn parse_line(line: &str, start: usize, block: &mut Option<usize>) -> YLine {
    let mut out = YLine { start, ..YLine::default() };
    if line.trim().is_empty() {
        out.blank = true;
        out.body_col = line.len();
        return out;
    }
    let indent = line.len() - line.trim_start_matches(' ').len();
    if let Some(header) = *block {
        if indent > header {
            out.block_body = true;
            out.body_col = indent;
            out.value = Some(start + indent..start + line.trim_end().len());
            return out;
        }
        *block = None;
    }
    let mut col = indent;
    loop {
        let rest = &line[col..];
        if rest == "-" || rest.starts_with("- ") {
            out.dashes.push(col);
            col += 1;
            col += line[col..].len() - line[col..].trim_start_matches(' ').len();
        } else {
            break;
        }
    }
    out.body_col = col;
    let rest = &line[col..];
    if rest.is_empty() {
        return out;
    }
    if rest.starts_with('#') {
        out.comment_only = true;
        out.comment = Some(start + col..start + line.trim_end().len());
        return out;
    }
    let (value_from, structural) = match key_split(rest) {
        Some((key_end, colon)) => {
            let name = rest[..key_end].trim_matches(|c| c == '"' || c == '\'').to_owned();
            out.key = Some((start + col..start + col + key_end, name));
            (col + colon + 1, col)
        }
        None => (col, col),
    };
    let after = &line[value_from..];
    let skipped = after.len() - after.trim_start_matches(' ').len();
    let value_start = value_from + skipped;
    let tail = &line[value_start..];
    if tail.is_empty() {
        return out;
    }
    let (value_end, comment) = match comment_at(tail) {
        Some(0) => (value_start, Some(value_start..line.trim_end().len())),
        Some(at) => (value_start + at, Some(value_start + at..line.trim_end().len())),
        None => (line.trim_end().len(), None),
    };
    out.comment = comment.map(|r| start + r.start..start + r.end);
    let value = line[value_start..value_end].trim_end();
    if !value.is_empty() {
        out.value = Some(start + value_start..start + value_start + value.len());
        if is_block_indicator(value) {
            *block = Some(structural);
        }
    }
    out
}

fn scan_yaml(text: &str) -> Vec<YLine> {
    let lines = Lines::new(text);
    let mut block = None;
    (0..lines.count()).map(|n| parse_line(lines.text(n), lines.range(n).start, &mut block)).collect()
}

/// One level of the path to a line: a key or an item, at the column its structure starts.
struct Frame {
    col: usize,
    is_item: bool,
    seg: Seg,
    items: usize,
}

/// **Where in the document each line stands**, as a path of keys and item indexes, kept the way a reader
/// keeps it: by the column a key or a `-` starts at. A sequence written at its key's own column (the
/// style `serde_yaml` writes) still belongs to that key.
#[derive(Default)]
struct PathTracker {
    frames: Vec<Frame>,
    root_items: usize,
}

impl PathTracker {
    fn pop_for_key(&mut self, col: usize) {
        while self.frames.last().is_some_and(|f| f.col >= col) {
            self.frames.pop();
        }
    }

    fn pop_for_dash(&mut self, col: usize) {
        while self.frames.last().is_some_and(|f| f.col > col || (f.is_item && f.col == col)) {
            self.frames.pop();
        }
    }

    fn push_item(&mut self, col: usize) {
        let index = match self.frames.last_mut() {
            Some(parent) => {
                parent.items += 1;
                parent.items - 1
            }
            None => {
                self.root_items += 1;
                self.root_items - 1
            }
        };
        self.frames.push(Frame { col, is_item: true, seg: Seg::Index(index), items: 0 });
    }

    fn feed_dashes(&mut self, line: &YLine) {
        for &col in &line.dashes {
            self.pop_for_dash(col);
            self.push_item(col);
        }
    }

    /// Take a line in; afterwards [`Self::path`] is the path of the line's last key (or its item).
    fn feed(&mut self, line: &YLine) {
        if line.blank || line.comment_only || line.block_body {
            return;
        }
        self.feed_dashes(line);
        if let Some((range, name)) = &line.key {
            let col = range.start - line.start;
            self.pop_for_key(col);
            self.frames.push(Frame { col, is_item: false, seg: Seg::Key(name.clone()), items: 0 });
        }
    }

    fn path(&self) -> Vec<Seg> {
        self.frames.iter().map(|f| f.seg.clone()).collect()
    }
}

// ── The schema ───────────────────────────────────────────────────────────────

/// **A kind's JSON Schema, navigated by a path.** The schema is `bevy_grimoire_core::schema`'s, generated
/// from the type the loader reads, so what is found here is what the loader accepts.
pub struct Schema<'a> {
    root: &'a Value,
}

/// How many `$ref`s and combinators one lookup follows before it gives up: a schema that refers to itself
/// (a tree of items) must end, and nothing real nests this deep.
const SCHEMA_DEPTH: usize = 12;

impl<'a> Schema<'a> {
    pub fn new(root: &'a Value) -> Schema<'a> {
        Schema { root }
    }

    fn resolve(&self, node: &'a Value) -> &'a Value {
        let mut node = node;
        for _ in 0..SCHEMA_DEPTH {
            let Some(target) = node.get("$ref").and_then(Value::as_str).and_then(|r| r.strip_prefix('#')) else {
                return node;
            };
            match self.root.pointer(target) {
                Some(next) => node = next,
                None => return node,
            }
        }
        node
    }

    /// The node and every variant under it (`anyOf`, `oneOf`, `allOf`), references followed.
    fn expand(&self, node: &'a Value, out: &mut Vec<&'a Value>, depth: usize) {
        if depth > SCHEMA_DEPTH {
            return;
        }
        let node = self.resolve(node);
        out.push(node);
        for key in ["anyOf", "oneOf", "allOf"] {
            for variant in node.get(key).and_then(Value::as_array).into_iter().flatten() {
                self.expand(variant, out, depth + 1);
            }
        }
    }

    fn child(&self, nodes: &[&'a Value], seg: &Seg) -> Vec<&'a Value> {
        let mut out = Vec::new();
        for node in nodes {
            match seg {
                Seg::Key(key) => {
                    if let Some(property) = node.get("properties").and_then(|p| p.get(key)) {
                        self.expand(property, &mut out, 0);
                    } else if let Some(extra) = node.get("additionalProperties").filter(|v| v.is_object()) {
                        self.expand(extra, &mut out, 0);
                    }
                }
                Seg::Index(at) => {
                    if let Some(item) = node.get("prefixItems").and_then(Value::as_array).and_then(|a| a.get(*at)) {
                        self.expand(item, &mut out, 0);
                    } else if let Some(items) = node.get("items").filter(|v| v.is_object()) {
                        self.expand(items, &mut out, 0);
                    }
                }
            }
        }
        out
    }

    /// **The schema nodes a path can be**: one for a plain field, several where a type is one of a few.
    pub fn nodes_at(&self, path: &[Seg]) -> Vec<&'a Value> {
        let mut nodes = Vec::new();
        self.expand(self.root, &mut nodes, 0);
        for seg in path {
            nodes = self.child(&nodes, seg);
            if nodes.is_empty() {
                break;
            }
        }
        nodes
    }

    /// The kind a field names, when it is a reference (`x-dark-factory-ref`).
    pub fn reference_of(&self, path: &[Seg]) -> Option<&'a str> {
        self.nodes_at(path).into_iter().find_map(|n| n.get("x-dark-factory-ref").and_then(Value::as_str))
    }

    /// The field's doc comment: the schema's `description`.
    pub fn description_of(&self, path: &[Seg]) -> Option<&'a str> {
        self.nodes_at(path).into_iter().find_map(|n| n.get("description").and_then(Value::as_str))
    }

    /// The names a mapping at `path` may hold, each with the first line of its doc comment.
    pub fn fields_at(&self, path: &[Seg]) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for node in self.nodes_at(path) {
            let Some(properties) = node.get("properties").and_then(Value::as_object) else { continue };
            for (name, property) in properties {
                if out.iter().any(|(n, _)| n == name) {
                    continue;
                }
                let mut nodes = Vec::new();
                self.expand(property, &mut nodes, 0);
                let doc = nodes.iter().find_map(|n| n.get("description").and_then(Value::as_str)).unwrap_or("");
                out.push((name.clone(), first_sentence(doc)));
            }
        }
        out
    }

    /// The values a field may take when the schema lists them: an `enum`, a `const`, a boolean.
    pub fn values_at(&self, path: &[Seg]) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        let mut push = |value: String, detail: String| {
            if !out.iter().any(|(v, _)| *v == value) {
                out.push((value, detail));
            }
        };
        for node in self.nodes_at(path) {
            for value in node.get("enum").and_then(Value::as_array).into_iter().flatten() {
                if let Some(s) = value.as_str() {
                    push(s.to_owned(), first_sentence(node.get("description").and_then(Value::as_str).unwrap_or("")));
                }
            }
            if let Some(s) = node.get("const").and_then(Value::as_str) {
                push(s.to_owned(), first_sentence(node.get("description").and_then(Value::as_str).unwrap_or("")));
            }
            let boolean = match node.get("type") {
                Some(Value::String(t)) => t == "boolean",
                Some(Value::Array(types)) => types.iter().any(|t| t == "boolean"),
                _ => false,
            };
            if boolean {
                push("true".to_owned(), "boolean".to_owned());
                push("false".to_owned(), "boolean".to_owned());
            }
        }
        out
    }
}

/// The first sentence of a doc comment, on one line: what a completion row's detail has room for.
fn first_sentence(doc: &str) -> String {
    let one_line = doc.split("\n\n").next().unwrap_or("").replace('\n', " ");
    let trimmed = one_line.trim().trim_matches('*').trim();
    match trimmed.find(". ") {
        Some(at) => trimmed[..=at].to_owned(),
        None => trimmed.to_owned(),
    }
}

// ── Colour ───────────────────────────────────────────────────────────────────

/// **The spans a file's text is coloured in**, by byte range. `schema` is the file's kind's JSON Schema
/// when it has one: YAML values it marks as references are links.
pub fn highlights(lang: Lang, text: &str, schema: Option<&Value>) -> Vec<(Range<usize>, SyntaxRole)> {
    match lang {
        Lang::Yaml => yaml_highlights(text, schema.map(Schema::new)),
        Lang::Ron => code_highlights(text, Code::Ron).0,
        Lang::Rhai => code_highlights(text, Code::Rhai).0,
        Lang::Markdown => markdown_highlights(text),
        Lang::Plain => Vec::new(),
    }
}

fn scalar_role(value: &str) -> Option<SyntaxRole> {
    let first = value.chars().next()?;
    if matches!(first, '"' | '\'') {
        return Some(SyntaxRole::String);
    }
    if matches!(first, '{' | '[') {
        return None;
    }
    if matches!(value, "true" | "false" | "null" | "~") {
        return Some(SyntaxRole::Keyword);
    }
    if value.parse::<f64>().is_ok() || matches!(value, ".inf" | "-.inf" | ".nan") || value.starts_with("0x") || value.starts_with("0o") {
        return Some(SyntaxRole::Number);
    }
    if is_block_indicator(value) {
        return Some(SyntaxRole::Operator);
    }
    Some(SyntaxRole::String)
}

fn yaml_highlights(text: &str, schema: Option<Schema<'_>>) -> Vec<(Range<usize>, SyntaxRole)> {
    let mut spans = Vec::new();
    let mut tracker = PathTracker::default();
    for line in scan_yaml(text) {
        if let Some(comment) = &line.comment {
            spans.push((comment.clone(), SyntaxRole::Comment));
        }
        if line.blank {
            continue;
        }
        if line.block_body {
            if let Some(value) = &line.value {
                spans.push((value.clone(), SyntaxRole::String));
            }
            continue;
        }
        tracker.feed(&line);
        for &col in &line.dashes {
            spans.push((line.start + col..line.start + col + 1, SyntaxRole::Operator));
        }
        if let Some((range, _)) = &line.key {
            spans.push((range.clone(), SyntaxRole::Key));
        }
        let Some(value) = &line.value else { continue };
        let value_text = &text[value.clone()];
        // A tag (`!Near`) is a type name; what follows it is the value.
        let (value_range, value_text) = if let Some(rest) = value_text.strip_prefix('!') {
            let tag_len = 1 + rest.find(' ').unwrap_or(rest.len());
            spans.push((value.start..value.start + tag_len, SyntaxRole::Type));
            let after = value_text[tag_len..].trim_start();
            if after.is_empty() {
                continue;
            }
            let from = value.end - after.len();
            (from..value.end, after)
        } else {
            (value.clone(), value_text)
        };
        if value_text.starts_with('&') || value_text.starts_with('*') {
            spans.push((value_range, SyntaxRole::Reference));
            continue;
        }
        let is_reference = schema.as_ref().is_some_and(|s| s.reference_of(&tracker.path()).is_some());
        match (is_reference, scalar_role(value_text)) {
            (true, Some(SyntaxRole::String)) => spans.push((value_range, SyntaxRole::Reference)),
            (_, Some(role)) => spans.push((value_range, role)),
            (_, None) => {}
        }
    }
    spans
}

// ── RON and Rhai ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Code {
    Ron,
    Rhai,
}

const RHAI_KEYWORDS: [&str; 26] = [
    "let", "const", "fn", "private", "if", "else", "while", "do", "until", "loop", "for", "in", "break", "continue", "return",
    "switch", "throw", "try", "catch", "true", "false", "import", "export", "as", "this", "global",
];

const RON_KEYWORDS: [&str; 4] = ["true", "false", "None", "Some"];

/// One bracket the scan met: the character, and the line it is on.
#[derive(Clone, Copy)]
struct Bracket {
    ch: u8,
    at: usize,
    line: usize,
}

/// What a scan of RON or Rhai found: the spans, the brackets in order, and a string that never ended.
struct Scan {
    brackets: Vec<Bracket>,
    unterminated: Option<usize>,
}

fn code_highlights(text: &str, code: Code) -> (Vec<(Range<usize>, SyntaxRole)>, Scan) {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut scan = Scan { brackets: Vec::new(), unterminated: None };
    let mut line = 0;
    let mut at = 0;
    while at < bytes.len() {
        let b = bytes[at];
        match b {
            b'\n' => {
                line += 1;
                at += 1;
            }
            b'/' if bytes.get(at + 1) == Some(&b'/') => {
                let end = text[at..].find('\n').map_or(text.len(), |n| at + n);
                spans.push((at..end, SyntaxRole::Comment));
                at = end;
            }
            b'/' if bytes.get(at + 1) == Some(&b'*') => {
                let mut depth = 0;
                let mut end = at;
                while end < bytes.len() {
                    if bytes[end..].starts_with(b"/*") {
                        depth += 1;
                        end += 2;
                    } else if bytes[end..].starts_with(b"*/") {
                        depth -= 1;
                        end += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        if bytes[end] == b'\n' {
                            line += 1;
                        }
                        end += 1;
                    }
                }
                spans.push((at..end.min(text.len()), SyntaxRole::Comment));
                at = end.min(text.len());
            }
            b'"' | b'`' => {
                let quote = b;
                let begin = at;
                at += 1;
                let mut closed = false;
                while at < bytes.len() {
                    match bytes[at] {
                        b'\\' if quote == b'"' => at += 2,
                        c if c == quote => {
                            at += 1;
                            closed = true;
                            break;
                        }
                        b'\n' if quote == b'"' => break,
                        b'\n' => {
                            line += 1;
                            at += 1;
                        }
                        _ => at += 1,
                    }
                }
                let at_end = at.min(text.len());
                if !closed && scan.unterminated.is_none() {
                    scan.unterminated = Some(begin);
                }
                spans.push((begin..at_end, SyntaxRole::String));
                at = at_end;
            }
            b'(' | b')' | b'[' | b']' | b'{' | b'}' => {
                scan.brackets.push(Bracket { ch: b, at, line });
                at += 1;
            }
            b'0'..=b'9' => {
                let begin = at;
                at += 1;
                while at < bytes.len() {
                    match bytes[at] {
                        b'0'..=b'9' | b'_' | b'.' | b'a'..=b'z' | b'A'..=b'Z' => at += 1,
                        b'+' | b'-' if matches!(bytes[at - 1], b'e' | b'E') => at += 1,
                        _ => break,
                    }
                }
                spans.push((begin..at, SyntaxRole::Number));
            }
            b'-' if bytes.get(at + 1).is_some_and(u8::is_ascii_digit) => {
                let begin = at;
                at += 1;
                while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || matches!(bytes[at], b'_' | b'.')) {
                    at += 1;
                }
                spans.push((begin..at, SyntaxRole::Number));
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let begin = at;
                while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                    at += 1;
                }
                let word = &text[begin..at];
                let next = text[at..].trim_start_matches([' ', '\t']);
                let role = if code == Code::Ron && RON_KEYWORDS.contains(&word) || code == Code::Rhai && RHAI_KEYWORDS.contains(&word) {
                    Some(SyntaxRole::Keyword)
                } else if next.starts_with(':') && !next.starts_with("::") {
                    Some(SyntaxRole::Key)
                } else if code == Code::Rhai && next.starts_with('(') {
                    Some(SyntaxRole::Function)
                } else if code == Code::Ron && word.starts_with(|c: char| c.is_ascii_uppercase()) {
                    Some(SyntaxRole::Type)
                } else {
                    None
                };
                if let Some(role) = role {
                    spans.push((begin..at, role));
                }
            }
            _ => at += 1,
        }
    }
    (spans, scan)
}

/// The fold ranges of the brackets that open on one line and close on a later one: the header is the line
/// with the opener, and the closing line stays drawn.
fn bracket_folds(scan: &Scan) -> Vec<Range<usize>> {
    let mut stack: Vec<Bracket> = Vec::new();
    let mut out: Vec<Range<usize>> = Vec::new();
    for bracket in &scan.brackets {
        match bracket.ch {
            b'(' | b'[' | b'{' => stack.push(*bracket),
            _ => {
                if let Some(open) = stack.pop()
                    && open.line < bracket.line
                    && !out.iter().any(|r| r.start == open.line && r.end == bracket.line)
                {
                    out.push(open.line..bracket.line);
                }
            }
        }
    }
    out.sort_by_key(|r| (r.start, std::cmp::Reverse(r.end)));
    out
}

// ── Markdown ─────────────────────────────────────────────────────────────────

fn fence_of(line: &str) -> Option<&str> {
    let t = line.trim_start();
    ["```", "~~~"].into_iter().find(|f| t.starts_with(f))
}

fn heading_level(line: &str) -> Option<usize> {
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    (1..=6).contains(&hashes).then_some(()).and_then(|()| (line[hashes..].starts_with(' ')).then_some(hashes))
}

fn markdown_highlights(text: &str) -> Vec<(Range<usize>, SyntaxRole)> {
    let lines = Lines::new(text);
    let mut spans = Vec::new();
    let mut fenced = false;
    for n in 0..lines.count() {
        let range = lines.range(n);
        let line = &text[range.clone()];
        if fence_of(line).is_some() {
            spans.push((range, SyntaxRole::Operator));
            fenced = !fenced;
            continue;
        }
        if fenced {
            if !line.is_empty() {
                spans.push((range, SyntaxRole::String));
            }
            continue;
        }
        if heading_level(line).is_some() {
            spans.push((range, SyntaxRole::Keyword));
            continue;
        }
        if line.starts_with('>') {
            spans.push((range, SyntaxRole::Comment));
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let body = &line[indent..];
        let marker = if body.starts_with("- ") || body.starts_with("* ") || body.starts_with("+ ") {
            1
        } else {
            let digits = body.bytes().take_while(u8::is_ascii_digit).count();
            if digits > 0 && body[digits..].starts_with(". ") { digits + 1 } else { 0 }
        };
        if marker > 0 {
            spans.push((range.start + indent..range.start + indent + marker, SyntaxRole::Operator));
        }
        // Inline: `code` is a string, `[text](target)` a link.
        let mut at = 0;
        while at < line.len() {
            let rest = &line[at..];
            if rest.starts_with('`') {
                match rest[1..].find('`') {
                    Some(end) => {
                        spans.push((range.start + at..range.start + at + end + 2, SyntaxRole::String));
                        at += end + 2;
                    }
                    None => at += 1,
                }
            } else if rest.starts_with('[') {
                let link = rest.find("](").and_then(|mid| rest[mid..].find(')').map(|close| mid + close + 1));
                match link {
                    Some(end) => {
                        spans.push((range.start + at..range.start + at + end, SyntaxRole::Reference));
                        at += end;
                    }
                    None => at += 1,
                }
            } else {
                at += rest.chars().next().map_or(1, char::len_utf8);
            }
        }
    }
    spans
}

// ── Folds ────────────────────────────────────────────────────────────────────

/// **Where a file can fold**, as `start..end` over lines (the header stays drawn, `start + 1..end` hides):
/// every YAML mapping and sequence, every Rhai or RON block, every Markdown section and code fence.
pub fn folds(lang: Lang, text: &str) -> Vec<Range<usize>> {
    match lang {
        Lang::Yaml => yaml_folds(text),
        Lang::Ron => bracket_folds(&code_highlights(text, Code::Ron).1),
        Lang::Rhai => bracket_folds(&code_highlights(text, Code::Rhai).1),
        Lang::Markdown => markdown_folds(text),
        Lang::Plain => Vec::new(),
    }
}

fn yaml_folds(text: &str) -> Vec<Range<usize>> {
    let lines = scan_yaml(text);
    let mut out = Vec::new();
    for (at, header) in lines.iter().enumerate() {
        if header.blank || header.comment_only || header.block_body {
            continue;
        }
        // A line opens a block when it holds a key with no value, or a bare `-`, or an item whose own key has
        // none: what follows it is its contents.
        let opens = header.value.is_none() && (header.key.is_some() || !header.dashes.is_empty())
            || header.block_body_start(&lines, at);
        if !opens {
            continue;
        }
        let col = header.col();
        let key_col = header.key.as_ref().map(|(r, _)| r.start - header.start);
        let is_child = |l: &YLine| {
            l.blank
                || l.comment_only && l.body_col > col
                || l.block_body
                || l.col() > col
                || (key_col.is_some() && header.dashes.is_empty() && l.col() == col && !l.dashes.is_empty())
        };
        let mut end = at + 1;
        while end < lines.len() && is_child(&lines[end]) {
            end += 1;
        }
        // Trailing blank lines belong to nothing.
        while end > at + 1 && lines[end - 1].blank {
            end -= 1;
        }
        if end > at + 1 {
            out.push(at..end);
        }
    }
    out
}

impl YLine {
    /// Whether this line starts a block scalar (`key: |`) with body lines under it.
    fn block_body_start(&self, lines: &[YLine], at: usize) -> bool {
        self.value.is_some() && lines.get(at + 1).is_some_and(|next| next.block_body)
    }
}

fn markdown_folds(text: &str) -> Vec<Range<usize>> {
    let lines = Lines::new(text);
    let mut out = Vec::new();
    let mut headings: Vec<(usize, usize)> = Vec::new();
    let mut fence_start: Option<usize> = None;
    let count = lines.count();
    let last_content = (0..count).rev().find(|&n| !lines.text(n).trim().is_empty()).map_or(0, |n| n + 1);
    for n in 0..count {
        let line = lines.text(n);
        if fence_of(line).is_some() {
            match fence_start.take() {
                Some(start) => out.push(start..n),
                None => fence_start = Some(n),
            }
            continue;
        }
        if fence_start.is_some() {
            continue;
        }
        if let Some(level) = heading_level(line) {
            while let Some(&(start, open_level)) = headings.last() {
                if open_level >= level {
                    out.push(start..n);
                    headings.pop();
                } else {
                    break;
                }
            }
            headings.push((n, level));
        }
    }
    for (start, _) in headings {
        out.push(start..last_content.max(start + 1));
    }
    out.sort_by_key(|r| (r.start, std::cmp::Reverse(r.end)));
    out
}

// ── Findings ─────────────────────────────────────────────────────────────────

/// **One thing wrong in a file**, by byte range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub range: Range<usize>,
    pub severity: CodeSeverity,
    pub message: String,
}


/// **A finding on line `line` (1-based)**: the line's text, or the word the message names in backticks
/// when the line holds it (an unknown key points at the key).
fn finding_on(lines: &Lines<'_>, text: &str, line: usize, message: String, severity: CodeSeverity) -> Finding {
    let n = line.saturating_sub(1).min(lines.count().saturating_sub(1));
    let range = lines.range(n);
    let line_text = &text[range.clone()];
    let indent = line_text.len() - line_text.trim_start().len();
    let named = message.split('`').nth(1).filter(|w| !w.is_empty() && !w.contains(' ')).and_then(|w| line_text.find(w).map(|at| at..at + w.len()));
    let range = match named {
        Some(w) => range.start + w.start..range.start + w.end,
        None => range.start + indent..range.end.max(range.start + indent),
    };
    // An empty line (a missing field at the end of a file) still needs a range the editor can draw.
    let range = if range.is_empty() { range.start..(range.start + 1).min(text.len()).max(range.start) } else { range };
    Finding { range, severity, message }
}

/// **What is wrong with a file**, read the way the game reads it (see the module note): a project file by
/// its kind's own reader, any other YAML by the event walk, RON by `ron`, Rhai by a bracket scan.
///
/// `path` is the file's path under the project, with `/`.
pub fn diagnose(path: &str, text: &str) -> Vec<Finding> {
    let lines = Lines::new(text);
    let error = |line: usize, message: String| finding_on(&lines, text, line, message, CodeSeverity::Error);
    match Lang::of(path) {
        Lang::Yaml => {
            let refused = match reload_check::kind_of_path(path) {
                Some((kind, Format::Yaml)) => match reload_check::check_yaml(kind, path, text) {
                    Check::Refused(r) => Some((r.reason, r.field)),
                    Check::Valid => None,
                    Check::Unchecked => walk_refusal(path, text),
                },
                _ => walk_refusal(path, text),
            };
            let Some((reason, field)) = refused else { return Vec::new() };
            let (line, message) = split_position(&reason);
            let line = line
                .or_else(|| field.as_deref().and_then(|f| bevy_grimoire_core::yaml::lines(text).get(f).copied()))
                .unwrap_or(1);
            vec![error(line, message)]
        }
        Lang::Ron => {
            let refused = match reload_check::kind_of_path(path) {
                Some((kind, Format::Ron)) => match reload_check::check(kind, path, text) {
                    Check::Refused(r) => Some(r.reason),
                    Check::Valid => None,
                    Check::Unchecked => ron_refusal(text),
                },
                _ => ron_refusal(text),
            };
            let Some(reason) = refused else { return Vec::new() };
            let (line, message) = split_position(&reason);
            vec![error(line.unwrap_or(1), message)]
        }
        Lang::Rhai => rhai_findings(text, &lines),
        Lang::Markdown | Lang::Plain => Vec::new(),
    }
}

/// The event walk's refusal of a YAML file with no kind: `(sentence with its position, field)`.
fn walk_refusal(path: &str, text: &str) -> Option<(String, Option<String>)> {
    let problem = bevy_grimoire_core::yaml::to_json(text, path).err()?;
    let line = problem.where_.as_ref().and_then(|w| w.line);
    let field = problem.where_.as_ref().and_then(|w| w.field.clone());
    let why = problem.why.clone().unwrap_or_else(|| problem.what.clone());
    let reason = match line {
        Some(n) if !why.starts_with("line ") => format!("line {n}: {why}"),
        _ => why,
    };
    Some((reason, field))
}

fn ron_refusal(text: &str) -> Option<String> {
    ron::Options::default().from_str::<ron::Value>(text).err().map(|e| e.to_string())
}

/// **The line a reason names and the sentence without its position**: `line 5, column 3: …` from the YAML
/// reader, `5:3: …` from `ron`, and `map: 12:13-12:18: …` from a kind's reader (a word, then a span), each
/// 1-based. The finding is already *on* that line, so the sentence says only what is wrong.
fn split_position(reason: &str) -> (Option<usize>, String) {
    let digits = |s: &str| s.bytes().take_while(u8::is_ascii_digit).count();
    let mut rest = reason;
    // `map: 12:13-12:18: …`: a kind's word in front of the span.
    if let Some((head, tail)) = rest.split_once(": ")
        && !head.is_empty()
        && head.chars().all(|c| c.is_alphanumeric() || c == '_')
        && tail.starts_with(|c: char| c.is_ascii_digit())
    {
        rest = tail;
    }
    if let Some(after) = rest.strip_prefix("line ") {
        let n = digits(after);
        if let (Ok(line), Some((_, tail))) = (after[..n].parse::<usize>(), after.split_once(": ")) {
            return (Some(line), tail.to_owned());
        }
        return (after[..n].parse().ok(), rest.to_owned());
    }
    let n = digits(rest);
    if n > 0
        && rest[n..].starts_with(':')
        && let Ok(line) = rest[..n].parse::<usize>()
    {
        // The span's end (`-12:18`) and the colon after it, when there is one.
        let tail = rest.split_once(": ").map_or(rest, |(_, tail)| tail);
        return (Some(line), tail.trim().to_owned());
    }
    (None, reason.to_owned())
}

fn rhai_findings(text: &str, lines: &Lines<'_>) -> Vec<Finding> {
    let (_, scan) = code_highlights(text, Code::Rhai);
    let mut out = Vec::new();
    let mut stack: Vec<Bracket> = Vec::new();
    let error = |at: usize, message: String| {
        let line = lines.line_of(at);
        let range = at..at + 1;
        let _ = line;
        Finding { range, severity: CodeSeverity::Error, message }
    };
    for bracket in &scan.brackets {
        match bracket.ch {
            b'(' | b'[' | b'{' => stack.push(*bracket),
            close => {
                let want = match close {
                    b')' => b'(',
                    b']' => b'[',
                    _ => b'{',
                };
                match stack.pop() {
                    Some(open) if open.ch == want => {}
                    Some(open) => {
                        out.push(error(bracket.at, format!("`{}` does not close `{}` from line {}", close as char, open.ch as char, open.line + 1)));
                    }
                    None => out.push(error(bracket.at, format!("`{}` closes nothing", close as char))),
                }
            }
        }
    }
    for open in stack {
        out.push(error(open.at, format!("`{}` is never closed", open.ch as char)));
    }
    if let Some(at) = scan.unterminated {
        out.push(error(at, "this string never ends".to_owned()));
    }
    out.sort_by_key(|f| f.range.start);
    out
}

// ── Completion and hover ─────────────────────────────────────────────────────

/// **What the editor may offer at the caret**: the start of the word being completed, and the rows.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Completion {
    pub anchor: usize,
    /// `(label, detail, text to insert instead of the label)`.
    pub items: Vec<(String, String, Option<String>)>,
}

fn word_before(text: &str, offset: usize) -> usize {
    let head = &text[..offset];
    let start = head.rfind(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | '@'))).map_or(0, |at| at + head[at..].chars().next().map_or(1, char::len_utf8));
    start
}

/// **Complete a YAML file at `offset`**, from the kind's schema: at a key, the fields the mapping may hold;
/// at a value, the enum's values, a boolean's two, or the ids a reference may point at (`refs(kind)`).
/// Nothing for a file with no schema, or in any language but YAML.
pub fn complete(lang: Lang, text: &str, offset: usize, schema: Option<&Value>, refs: &dyn Fn(&str) -> Vec<String>) -> Completion {
    let (Lang::Yaml, Some(schema), true) = (lang, schema, text.is_char_boundary(offset)) else { return Completion::default() };
    let schema = Schema::new(schema);
    let lines = Lines::new(text);
    let at = lines.line_of(offset);
    let mut tracker = PathTracker::default();
    let mut block = None;
    for n in 0..at {
        tracker.feed(&parse_line(lines.text(n), lines.range(n).start, &mut block));
    }
    let start = lines.range(at).start;
    let before = parse_line(&text[start..offset], start, &mut block);
    if before.block_body || before.comment_only {
        return Completion::default();
    }
    let in_value = before.key.is_some() && text[start..offset].trim_end().len() > before.key.as_ref().map_or(0, |(r, _)| r.end - start) && {
        let after_key = &text[before.key.as_ref().map_or(start, |(r, _)| r.end)..offset];
        after_key.trim_start().starts_with(':')
    };
    if in_value {
        tracker.feed(&before);
        let path = tracker.path();
        let anchor = word_before(text, offset);
        let mut items: Vec<(String, String, Option<String>)> = Vec::new();
        if let Some(kind) = schema.reference_of(&path) {
            for id in refs(kind) {
                items.push((id, kind.to_owned(), None));
            }
        }
        for (value, detail) in schema.values_at(&path) {
            if !items.iter().any(|(label, _, _)| *label == value) {
                items.push((value, detail, None));
            }
        }
        return Completion { anchor, items };
    }
    tracker.feed_dashes(&before);
    tracker.pop_for_key(before.body_col);
    let path = tracker.path();
    let anchor = word_before(text, offset);
    let items = schema.fields_at(&path).into_iter().map(|(name, doc)| {
        let insert = format!("{name}: ");
        (name, doc, Some(insert))
    });
    Completion { anchor, items: items.collect() }
}

/// **What to say about the text at `offset`**: the doc comment of the field whose key is there (and that it
/// is a reference, to what), by the schema. The range is the key's bytes.
pub fn hover(lang: Lang, text: &str, offset: usize, schema: Option<&Value>) -> Option<(Range<usize>, String)> {
    let (Lang::Yaml, Some(schema)) = (lang, schema) else { return None };
    let schema = Schema::new(schema);
    let lines = Lines::new(text);
    let at = lines.line_of(offset);
    let mut tracker = PathTracker::default();
    let mut block = None;
    for n in 0..=at {
        let line = parse_line(lines.text(n), lines.range(n).start, &mut block);
        if n == at {
            let (range, _) = line.key.as_ref().filter(|(r, _)| r.contains(&offset))?;
            tracker.feed(&line);
            let path = tracker.path();
            let mut say = schema.description_of(&path).unwrap_or("").trim().to_owned();
            if let Some(kind) = schema.reference_of(&path) {
                if !say.is_empty() {
                    say.push_str("\n\n");
                }
                say.push_str(&format!("Names a value of `{kind}`."));
            }
            return (!say.is_empty()).then(|| (range.clone(), say));
        }
        tracker.feed(&line);
    }
    None
}

/// **The path of the key a line holds**, for a test and for the reference check: `placements[0].descriptor`.
pub fn path_at_line(text: &str, line: usize) -> String {
    let lines = Lines::new(text);
    let mut tracker = PathTracker::default();
    let mut block = None;
    for n in 0..=line.min(lines.count().saturating_sub(1)) {
        tracker.feed(&parse_line(lines.text(n), lines.range(n).start, &mut block));
    }
    let mut out = String::new();
    for seg in tracker.path() {
        match seg {
            Seg::Key(k) if out.is_empty() => out.push_str(&k),
            Seg::Key(k) => {
                out.push('.');
                out.push_str(&k);
            }
            Seg::Index(i) => out.push_str(&format!("[{i}]")),
        }
    }
    out
}
