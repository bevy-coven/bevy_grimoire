//! **YAML**: the project's own files, by the loader's own readers (see the module note of [`super`]).

use std::collections::HashMap;
use std::ops::Range;

use bevy_gooey::{CodeSeverity, SyntaxRole};
use bevy_grimoire_core::reload_check::{self, Check};
use bevy_grimoire_core::yaml::SpanKind;
use serde_json::Value;

use super::{Completion, Env, Finding, Lang, Language, Lines, finding_on, word_before};

/// The language of `.yaml` and `.yml` files.
pub struct Yaml;

impl Language for Yaml {
    fn lang(&self) -> Lang {
        Lang::Yaml
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["yaml", "yml"]
    }

    fn highlights(&self, text: &str, schema: Option<&Value>) -> Vec<(Range<usize>, SyntaxRole)> {
        yaml_highlights(text, schema.map(Schema::new))
    }

    fn folds(&self, text: &str) -> Vec<Range<usize>> {
        yaml_folds(text)
    }

    /// **What is wrong with a YAML file**: a project file by its kind's own reader, any other by the event walk.
    fn diagnose(&self, path: &str, text: &str, _env: &Env<'_>) -> Vec<Finding> {
        let lines = Lines::new(text);
        let refused = match reload_check::kind_of_path(path) {
            Some(kind) => match reload_check::check(kind, path, text) {
                Check::Refused(r) => Some((r.reason, r.field)),
                Check::Valid => None,
                Check::Unchecked => walk_refusal(path, text),
            },
            None => walk_refusal(path, text),
        };
        let Some((reason, field)) = refused else { return Vec::new() };
        let (line, message) = split_position(&reason);
        let line = line
            .or_else(|| field.as_deref().and_then(|f| bevy_grimoire_core::yaml::lines(text).get(f).copied()))
            .unwrap_or(1);
        vec![finding_on(&lines, text, line, message, CodeSeverity::Error)]
    }

    fn complete(&self, _path: &str, text: &str, offset: usize, schema: Option<&Value>, env: &Env<'_>) -> Completion {
        complete_yaml(text, offset, schema, env.refs)
    }

    fn hover(&self, _path: &str, text: &str, offset: usize, schema: Option<&Value>, _env: &Env<'_>) -> Option<(Range<usize>, String)> {
        hover_yaml(text, offset, schema)
    }
}

// ── YAML ─────────────────────────────────────────────────────────────────────

/// **One step of a path into a document**: a key, or the index of a list item.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
    // **The keys, dashes and scalars come from the loader's own walk** ([`bevy_grimoire_core::yaml::spans`]: the
    // event parser the reader refuses a file by), so a colour is a node the loader reads. The walk speaks for the
    // text before the construct it refused; from that line on, and for what its events do not carry (comments, tags),
    // the line scanner above colours as best it can.
    let (parsed, fault) = bevy_grimoire_core::yaml::spans(text);
    let lines = Lines::new(text);
    let cutoff = fault.map_or(text.len(), |line| lines.range(line.saturating_sub(1).min(lines.count().saturating_sub(1))).start);
    let mut links: HashMap<Vec<Seg>, bool> = HashMap::new();
    let mut is_link = |path: &[Seg]| -> bool {
        let shape: Vec<Seg> = path.iter().map(|s| if let Seg::Index(_) = s { Seg::Index(0) } else { s.clone() }).collect();
        *links.entry(shape.clone()).or_insert_with(|| reference_kind(schema.as_ref(), &shape).is_some())
    };
    let mut spans = Vec::new();
    for span in parsed.iter().filter(|s| s.range.end <= cutoff) {
        let role = match &span.kind {
            SpanKind::Key => Some(SyntaxRole::Key),
            SpanKind::Dash => Some(SyntaxRole::Operator),
            SpanKind::Quoted | SpanKind::Block => Some(SyntaxRole::String),
            SpanKind::Plain(value) => scalar_role(value),
        };
        let role = match role {
            Some(SyntaxRole::String) if !matches!(span.kind, SpanKind::Block) && is_link(&segs_of(&span.path)) => Some(SyntaxRole::Reference),
            other => other,
        };
        if let Some(role) = role {
            spans.push((span.range.clone(), role));
        }
    }
    let mut tracker = PathTracker::default();
    for line in scan_yaml(text) {
        if let Some(comment) = &line.comment {
            spans.push((comment.clone(), SyntaxRole::Comment));
        }
        if line.blank {
            continue;
        }
        let parsed_line = line.start < cutoff;
        if line.block_body {
            if let (false, Some(value)) = (parsed_line, &line.value) {
                spans.push((value.clone(), SyntaxRole::String));
            }
            continue;
        }
        tracker.feed(&line);
        if !parsed_line {
            for &col in &line.dashes {
                spans.push((line.start + col..line.start + col + 1, SyntaxRole::Operator));
            }
            if let Some((range, _)) = &line.key {
                spans.push((range.clone(), SyntaxRole::Key));
            }
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
        if parsed_line {
            continue;
        }
        if value_text.starts_with('&') || value_text.starts_with('*') {
            spans.push((value_range, SyntaxRole::Reference));
            continue;
        }
        match (is_link(&tracker.path()), scalar_role(value_text)) {
            (true, Some(SyntaxRole::String)) => spans.push((value_range, SyntaxRole::Reference)),
            (_, Some(role)) => spans.push((value_range, role)),
            (_, None) => {}
        }
    }
    spans
}

/// **A path as the loader spells it** (`placements[3].descriptor`), as steps.
fn segs_of(path: &str) -> Vec<Seg> {
    let mut out = Vec::new();
    for part in path.split('.').filter(|p| !p.is_empty()) {
        let (name, mut rest) = part.split_once('[').map_or((part, ""), |(n, r)| (n, r));
        if !name.is_empty() {
            out.push(Seg::Key(name.to_owned()));
        }
        while let Some((index, tail)) = rest.split_once(']') {
            if let Ok(i) = index.parse() {
                out.push(Seg::Index(i));
            }
            rest = tail.trim_start_matches('[');
        }
    }
    out
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

/// **The line a reason names and the sentence without its position**: `line 5, column 3: …` from the YAML
/// reader, and `map: 12:13-12:18: …` from a kind's reader (a word, then a span), each
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

/// **Complete a YAML file at `offset`**, from the kind's schema: at a key, the fields the mapping may hold;
/// at a value, the enum's values, a boolean's two, or the ids a reference may point at (`refs(kind)`).
/// Nothing for a file with no schema, or in any language but YAML.
fn complete_yaml(text: &str, offset: usize, schema: Option<&Value>, refs: &dyn Fn(&str) -> Vec<String>) -> Completion {
    let Some(schema) = schema else { return Completion::default() };
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
fn hover_yaml(text: &str, offset: usize, schema: Option<&Value>) -> Option<(Range<usize>, String)> {
    let schema = schema?;
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

/// **The key whose value names a sound cue** (NV-07, #384). No file kind of this repository has a field that names a cue
/// yet, so a cue reference is read by the field's name as well as by a schema's `x-dark-factory-ref: cue`: a game's own data
/// files say `cue: door.creak` and the editor follows it to the cue.
pub const CUE_FIELD: &str = "cue";

/// **The kind a cue reference has**, the word a schema's `x-dark-factory-ref` would carry.
pub const CUE_KIND: &str = "cue";

/// **The kind a field names, when it names another thing**: the schema's `x-dark-factory-ref`, else [`CUE_KIND`] for a field
/// called [`CUE_FIELD`]. One function, read by the colouring, by the scan, and by the click, so they cannot disagree about
/// which values are links.
fn reference_kind<'a>(schema: Option<&Schema<'a>>, path: &[Seg]) -> Option<&'a str> {
    let shape: Vec<Seg> = path.iter().map(|s| if let Seg::Index(_) = s { Seg::Index(0) } else { s.clone() }).collect();
    schema
        .and_then(|s| s.reference_of(&shape))
        .or_else(|| matches!(path.last(), Some(Seg::Key(key)) if key == CUE_FIELD).then_some(CUE_KIND))
}

/// **One value of a YAML file that names another thing**: its kind (`library`, `cue`), the name it holds, the bytes of the
/// value, and the line (0-based) it is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    pub kind: String,
    pub name: String,
    pub range: Range<usize>,
    pub line: usize,
}

/// **Every reference in a YAML text**, in file order, by the schema's marks and by [`CUE_FIELD`]. Nothing for any other
/// language. A value that is a tag, an anchor, a flow collection or a block scalar names nothing here.
pub fn references(lang: Lang, text: &str, schema: Option<&Value>) -> Vec<Reference> {
    if lang != Lang::Yaml {
        return Vec::new();
    }
    let schema = schema.map(Schema::new);
    let lines = Lines::new(text);
    let mut tracker = PathTracker::default();
    let mut block = None;
    let mut out = Vec::new();
    for n in 0..lines.count() {
        let line = parse_line(lines.text(n), lines.range(n).start, &mut block);
        if line.blank || line.comment_only || line.block_body {
            continue;
        }
        tracker.feed(&line);
        let (Some(_), Some(value)) = (&line.key, &line.value) else { continue };
        let raw = &text[value.clone()];
        if raw.starts_with(['!', '&', '*', '|', '>', '[', '{']) {
            continue;
        }
        let Some(kind) = reference_kind(schema.as_ref(), &tracker.path()) else { continue };
        let name = raw.trim_matches(['"', '\'']);
        if name.is_empty() {
            continue;
        }
        out.push(Reference { kind: kind.to_owned(), name: name.to_owned(), range: value.clone(), line: n });
    }
    out
}

/// **The reference under a caret** (a byte offset), when the caret is on a value that names another thing: what Cmd/Ctrl+click
/// and F12 follow (NV-07, #384).
pub fn reference_at(lang: Lang, text: &str, offset: usize, schema: Option<&Value>) -> Option<Reference> {
    references(lang, text, schema).into_iter().find(|r| r.range.start <= offset && offset <= r.range.end)
}
