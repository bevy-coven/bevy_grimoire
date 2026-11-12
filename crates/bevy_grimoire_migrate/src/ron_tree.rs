//! **RON read into a tree, for a file whose type this crate does not have** (GD-03, #365; M1/M2 of the review).
//!
//! A kind whose type lives in another crate (the editor's forests, people and scripts, the checks, a family crate's settings) or
//! none at all (a game's own data file) cannot be read "as its type" here. The conversion is by value instead: the RON is parsed
//! into a [`Tree`], written as YAML, and **the YAML is read back and compared with the tree** ([`Tree::to_json`] against the
//! one YAML reader's own answer). A difference stops the migration, naming the file.
//!
//! # The spelling rules, which are the rules a typed write has
//!
//! - `Some(x)` is `x` and `None` is null: what the YAML writer does for an `Option`.
//! - `Name(..)` is an enum variant and keeps its name as a YAML tag (`!Pine {height_m: 4.0}`): the editor's files are written
//!   without struct names, so a named parenthesis is a variant. A bare identifier is a unit variant, a plain string.
//! - `(a: 1)` is a mapping, `(1, 2)` a sequence, `()` null, `[..]` a sequence, `{k: v}` a mapping.
//!
//! A file this cannot express (an extension attribute such as `#![enable(implicit_some)]`) is refused naming the position.

use serde_json::Value;
use serde_yaml_ng::value::{Tag, TaggedValue};
use serde_yaml_ng::{Mapping, Value as Yaml};

/// **What a RON document holds.**
#[derive(Clone, Debug, PartialEq)]
pub enum Tree {
    /// `None`, `()` and an empty unit.
    Null,
    /// `true` / `false`.
    Bool(bool),
    /// An integer.
    Int(i128),
    /// A float.
    Float(f64),
    /// A string or a character.
    Text(String),
    /// `[..]` or a parenthesised tuple.
    List(Vec<Tree>),
    /// `{k: v}` or a parenthesised struct.
    Map(Vec<(Tree, Tree)>),
    /// `Name(..)`: a variant and its payload; a bare `Name` is `Text`.
    Variant(String, Box<Tree>),
}

impl Tree {
    /// **The tree as JSON**, the shape the project's YAML reader gives a document: a variant is a one-key object.
    pub fn to_json(&self) -> Value {
        match self {
            Tree::Null => Value::Null,
            Tree::Bool(b) => Value::Bool(*b),
            Tree::Int(i) => i64::try_from(*i).map(Value::from).or_else(|_| u64::try_from(*i).map(Value::from)).unwrap_or(Value::Null),
            Tree::Float(f) => Value::from(*f),
            Tree::Text(s) => Value::String(s.clone()),
            Tree::List(items) => Value::Array(items.iter().map(Tree::to_json).collect()),
            Tree::Map(entries) => Value::Object(entries.iter().map(|(k, v)| (key_text(k), v.to_json())).collect()),
            Tree::Variant(name, payload) => Value::Object([(name.clone(), payload.to_json())].into_iter().collect()),
        }
    }

    /// **The tree as a YAML value**, which the project's one writer prints.
    pub fn to_yaml(&self) -> Yaml {
        match self {
            Tree::Null => Yaml::Null,
            Tree::Bool(b) => Yaml::Bool(*b),
            Tree::Int(i) => i64::try_from(*i).map(Yaml::from).or_else(|_| u64::try_from(*i).map(Yaml::from)).unwrap_or(Yaml::Null),
            Tree::Float(f) => Yaml::from(*f),
            Tree::Text(s) => Yaml::String(s.clone()),
            Tree::List(items) => Yaml::Sequence(items.iter().map(Tree::to_yaml).collect()),
            Tree::Map(entries) => {
                let mut map = Mapping::new();
                for (k, v) in entries {
                    map.insert(k.to_yaml(), v.to_yaml());
                }
                Yaml::Mapping(map)
            }
            Tree::Variant(name, payload) => Yaml::Tagged(Box::new(TaggedValue { tag: Tag::new(name), value: payload.to_yaml() })),
        }
    }
}

/// A map key as the JSON object key the YAML reader gives it.
fn key_text(key: &Tree) -> String {
    match key {
        Tree::Text(s) => s.clone(),
        Tree::Int(i) => i.to_string(),
        Tree::Bool(b) => b.to_string(),
        other => other.to_json().to_string(),
    }
}

/// **The cap on how deep a document may nest**, so a hostile file is a refusal and not a stack overflow.
pub const MAX_DEPTH: usize = 64;

/// **Parse a RON document.** The error names the line and column.
pub fn parse(text: &str) -> Result<Tree, String> {
    let mut p = Parser { src: text.as_bytes(), text, at: 0 };
    p.skip();
    if p.rest().starts_with("#!") || p.rest().starts_with("#[") {
        return Err(p.err("a RON extension attribute (`#![enable(..)]`) is not read by this migration"));
    }
    let tree = p.value(0)?;
    p.skip();
    if p.at < p.src.len() {
        return Err(p.err("text after the document"));
    }
    Ok(tree)
}

struct Parser<'a> {
    src: &'a [u8],
    text: &'a str,
    at: usize,
}

impl Parser<'_> {
    fn rest(&self) -> &str {
        &self.text[self.at..]
    }

    fn err(&self, what: &str) -> String {
        let before = &self.text[..self.at.min(self.text.len())];
        let line = before.matches('\n').count() + 1;
        let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        format!("{line}:{col}: {what}")
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.at).copied()
    }

    /// Skip whitespace and comments (`//` to the end of the line, `/* */` nested).
    fn skip(&mut self) {
        loop {
            match self.peek() {
                Some(b) if b.is_ascii_whitespace() => self.at += 1,
                Some(b'/') if self.src.get(self.at + 1) == Some(&b'/') => {
                    while self.peek().is_some_and(|b| b != b'\n') {
                        self.at += 1;
                    }
                }
                Some(b'/') if self.src.get(self.at + 1) == Some(&b'*') => {
                    let mut depth = 0usize;
                    while self.at < self.src.len() {
                        if self.rest().starts_with("/*") {
                            depth += 1;
                            self.at += 2;
                        } else if self.rest().starts_with("*/") {
                            depth = depth.saturating_sub(1);
                            self.at += 2;
                            if depth == 0 {
                                break;
                            }
                        } else {
                            self.at += 1;
                        }
                    }
                }
                _ => return,
            }
        }
    }

    fn eat(&mut self, b: u8) -> bool {
        self.skip();
        if self.peek() == Some(b) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn value(&mut self, depth: usize) -> Result<Tree, String> {
        if depth > MAX_DEPTH {
            return Err(self.err(&format!("nested more than {MAX_DEPTH} deep")));
        }
        self.skip();
        match self.peek() {
            None => Err(self.err("the document ends where a value should be")),
            Some(b'"') => self.string().map(Tree::Text),
            Some(b'\'') => self.char_literal().map(Tree::Text),
            Some(b'[') => {
                self.at += 1;
                let items = self.items(b']', depth)?;
                Ok(Tree::List(items))
            }
            Some(b'{') => {
                self.at += 1;
                let mut entries = Vec::new();
                loop {
                    if self.eat(b'}') {
                        break;
                    }
                    let key = self.value(depth + 1)?;
                    if !self.eat(b':') {
                        return Err(self.err("a `:` after a map key"));
                    }
                    entries.push((key, self.value(depth + 1)?));
                    if !self.eat(b',') {
                        if !self.eat(b'}') {
                            return Err(self.err("a `,` or `}` in a map"));
                        }
                        break;
                    }
                }
                Ok(Tree::Map(entries))
            }
            Some(b'(') => {
                self.at += 1;
                self.paren(None, depth)
            }
            Some(b) if b == b'-' || b == b'+' || b.is_ascii_digit() => self.number(),
            Some(b) if b == b'_' || b.is_ascii_alphabetic() || b == b'r' => self.word(depth),
            Some(_) => Err(self.err("a value")),
        }
    }

    /// Values up to the closing byte, with trailing commas allowed.
    fn items(&mut self, close: u8, depth: usize) -> Result<Vec<Tree>, String> {
        let mut out = Vec::new();
        loop {
            if self.eat(close) {
                return Ok(out);
            }
            out.push(self.value(depth + 1)?);
            if !self.eat(b',') {
                if self.eat(close) {
                    return Ok(out);
                }
                return Err(self.err("a `,` or the closing bracket"));
            }
        }
    }

    /// The inside of `( .. )` after the open parenthesis: a struct (`field: value`), a tuple, or unit.
    fn paren(&mut self, name: Option<String>, depth: usize) -> Result<Tree, String> {
        self.skip();
        let inside = if self.eat(b')') {
            Tree::Null
        } else if self.looks_like_a_field() {
            let mut entries = Vec::new();
            loop {
                if self.eat(b')') {
                    break;
                }
                let field = self.identifier()?;
                if !self.eat(b':') {
                    return Err(self.err("a `:` after a field name"));
                }
                entries.push((Tree::Text(field), self.value(depth + 1)?));
                if !self.eat(b',') {
                    if !self.eat(b')') {
                        return Err(self.err("a `,` or `)` in a struct"));
                    }
                    break;
                }
            }
            Tree::Map(entries)
        } else {
            let mut items = Vec::new();
            let mut trailing = false;
            loop {
                if self.eat(b')') {
                    break;
                }
                items.push(self.value(depth + 1)?);
                trailing = false;
                if !self.eat(b',') {
                    if !self.eat(b')') {
                        return Err(self.err("a `,` or `)` in a tuple"));
                    }
                    break;
                }
                trailing = true;
            }
            if items.len() == 1 && !trailing { items.remove(0) } else { Tree::List(items) }
        };
        Ok(match name.as_deref() {
            None => inside,
            Some("Some") => inside,
            Some(n) => Tree::Variant(n.to_owned(), Box::new(inside)),
        })
    }

    /// Whether the next tokens are `identifier :` (and not `identifier ::`).
    fn looks_like_a_field(&mut self) -> bool {
        let save = self.at;
        self.skip();
        let ok = self.identifier().is_ok() && {
            self.skip();
            self.peek() == Some(b':')
        };
        self.at = save;
        ok
    }

    fn identifier(&mut self) -> Result<String, String> {
        self.skip();
        let start = self.at;
        while self.peek().is_some_and(|b| b == b'_' || b.is_ascii_alphanumeric()) {
            self.at += 1;
        }
        if start == self.at || self.src[start].is_ascii_digit() {
            self.at = start;
            return Err(self.err("a name"));
        }
        Ok(self.text[start..self.at].to_owned())
    }

    /// An identifier: `true`, `false`, `None`, `Some(..)`, `Name(..)`, a raw string, or a bare unit variant.
    fn word(&mut self, depth: usize) -> Result<Tree, String> {
        if self.rest().starts_with("r\"") || self.rest().starts_with("r#") {
            return self.raw_string().map(Tree::Text);
        }
        let name = self.identifier()?;
        match name.as_str() {
            "true" => return Ok(Tree::Bool(true)),
            "false" => return Ok(Tree::Bool(false)),
            "None" => return Ok(Tree::Null),
            "inf" => return Ok(Tree::Float(f64::INFINITY)),
            "NaN" => return Ok(Tree::Float(f64::NAN)),
            _ => {}
        }
        self.skip();
        if self.peek() == Some(b'(') {
            self.at += 1;
            return self.paren(Some(name), depth);
        }
        Ok(Tree::Text(name))
    }

    fn number(&mut self) -> Result<Tree, String> {
        let start = self.at;
        if matches!(self.peek(), Some(b'-' | b'+')) {
            self.at += 1;
        }
        if self.rest().starts_with("inf") || self.rest().starts_with("NaN") {
            let neg = self.src[start] == b'-';
            let word = self.identifier()?;
            return Ok(Tree::Float(if word == "NaN" { f64::NAN } else if neg { f64::NEG_INFINITY } else { f64::INFINITY }));
        }
        let hex = self.rest().starts_with("0x") || self.rest().starts_with("0X");
        while self.peek().is_some_and(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || ((b == b'-' || b == b'+') && !hex && matches!(self.src[self.at - 1], b'e' | b'E'))) {
            self.at += 1;
        }
        let raw: String = self.text[start..self.at].chars().filter(|c| *c != '_').collect();
        let float = !hex && raw.contains(['.', 'e', 'E']);
        if float {
            raw.parse::<f64>().map(Tree::Float).map_err(|_| self.err(&format!("`{raw}` is not a number")))
        } else if let Some(h) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
            i128::from_str_radix(h, 16).map(Tree::Int).map_err(|_| self.err(&format!("`{raw}` is not a number")))
        } else {
            raw.trim_start_matches('+').parse::<i128>().map(Tree::Int).map_err(|_| self.err(&format!("`{raw}` is not a number")))
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let Some(c) = self.rest().chars().next() else { return Err(self.err("a string that never ends")) };
            self.at += c.len_utf8();
            match c {
                '"' => return Ok(out),
                '\\' => out.push(self.escape()?),
                c => out.push(c),
            }
        }
    }

    fn escape(&mut self) -> Result<char, String> {
        let Some(c) = self.rest().chars().next() else { return Err(self.err("an escape that never ends")) };
        self.at += c.len_utf8();
        Ok(match c {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '0' => '\0',
            '\\' => '\\',
            '"' => '"',
            '\'' => '\'',
            'x' | 'u' => {
                let braced = c == 'u' && self.eat_raw(b'{');
                let len = if c == 'x' { 2 } else { self.rest().find('}').unwrap_or(0) };
                let digits = self.rest().get(..len).unwrap_or("").to_owned();
                self.at += len;
                if braced {
                    self.eat_raw(b'}');
                }
                u32::from_str_radix(&digits, 16).ok().and_then(char::from_u32).ok_or_else(|| self.err("an escape that is not a character"))?
            }
            _ => return Err(self.err(&format!("the escape `\\{c}` is not known"))),
        })
    }

    fn eat_raw(&mut self, b: u8) -> bool {
        if self.peek() == Some(b) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn raw_string(&mut self) -> Result<String, String> {
        self.at += 1;
        let hashes = self.rest().bytes().take_while(|b| *b == b'#').count();
        self.at += hashes;
        if !self.eat_raw(b'"') {
            return Err(self.err("a raw string opens with a quote"));
        }
        let close = format!("\"{}", "#".repeat(hashes));
        let Some(end) = self.rest().find(&close) else { return Err(self.err("a raw string that never ends")) };
        let out = self.rest()[..end].to_owned();
        self.at += end + close.len();
        Ok(out)
    }

    fn char_literal(&mut self) -> Result<String, String> {
        self.at += 1;
        let Some(c) = self.rest().chars().next() else { return Err(self.err("a character that never ends")) };
        self.at += c.len_utf8();
        let c = if c == '\\' { self.escape()? } else { c };
        if !self.eat_raw(b'\'') {
            return Err(self.err("a character literal closes with a quote"));
        }
        Ok(c.to_string())
    }
}
