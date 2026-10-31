//! **Game data files the editor may tune: a registry and type-keeping leaf edits.**
//!
//! A game keeps tuning numbers in its own RON files (Sanctuary: `assets/data/tuning/<domain>.ron`).
//! They are not part of the grimoire project's schema, so the project lists them in
//! `<grimoire_dir>/data.ron`, and the editor edits one scalar at a time through [`set_leaf`], which
//! substitutes only that token's bytes: every comment, alignment and neighbouring literal survives.
//!
//! An edit never changes what *kind* of value a leaf is. A number stays a number (an integer token
//! takes only whole values), a string a string, `None` stays `None`; a value outside the registered
//! [`Bounds`] and a result that no longer parses are refused. Every refusal names the leaf path.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ron_surgery::{fmt_f32, scan_ron_leaves};

/// File name of the registry inside the grimoire directory.
pub const REGISTRY_FILE: &str = "data.ron";
/// The registry's schema version.
pub const REGISTRY_VERSION: u32 = 1;

/// An inclusive numeric range for one leaf; either end may be open.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Bounds {
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
}

/// One registered data file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataFile {
    /// Stable name the operations address the file by (`tuning/mob`).
    pub id: String,
    /// Path relative to the grimoire directory.
    pub path: String,
    #[serde(default)]
    pub note: Option<String>,
    /// Bounds by dotted leaf path.
    #[serde(default)]
    pub bounds: BTreeMap<String, Bounds>,
}

/// The parsed `data.ron`, with the directory its paths are relative to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataFiles {
    pub version: u32,
    pub files: Vec<DataFile>,
    #[serde(skip)]
    grimoire_dir: PathBuf,
}

impl DataFiles {
    /// **Read a registry from its file's text**, refusing one that is malformed, whose version this
    /// build does not know, or that names an id twice. It does not look at the disk: whether each
    /// entry's file exists is [`Self::load`]'s question, because it depends on where the registry is.
    /// Messages carry no path; [`Self::load`] prefixes the registry's.
    pub fn parse(text: &str) -> Result<DataFiles, String> {
        let d: DataFiles = ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
            .from_str(text)
            .map_err(|e| e.to_string())?;
        if d.version != REGISTRY_VERSION {
            return Err(format!("version {} is not {REGISTRY_VERSION}", d.version));
        }
        for (i, f) in d.files.iter().enumerate() {
            if d.files[..i].iter().any(|g| g.id == f.id) {
                return Err(format!("id `{}` is registered twice", f.id));
            }
        }
        Ok(d)
    }

    /// Read `<grimoire_dir>/data.ron`. An absent registry is `Ok(None)`; a malformed one, a version
    /// this build does not know, a repeated id, or an entry whose file does not exist is an error
    /// naming the entry.
    pub fn load(grimoire_dir: &Path) -> Result<Option<DataFiles>, String> {
        let reg = grimoire_dir.join(REGISTRY_FILE);
        let text = match std::fs::read_to_string(&reg) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{}: {e}", reg.display())),
        };
        let mut d = Self::parse(&text).map_err(|e| format!("{}: {e}", reg.display()))?;
        d.grimoire_dir = grimoire_dir.to_path_buf();
        for f in &d.files {
            let p = d.resolve(f);
            if !p.is_file() {
                return Err(format!(
                    "{}: entry `{}`: file `{}` does not exist ({})",
                    reg.display(),
                    f.id,
                    f.path,
                    p.display()
                ));
            }
        }
        Ok(Some(d))
    }

    /// The registered file with this id.
    pub fn get(&self, id: &str) -> Option<&DataFile> {
        self.files.iter().find(|f| f.id == id)
    }

    /// Where a registered file lives on disk.
    pub fn resolve(&self, f: &DataFile) -> PathBuf {
        self.grimoire_dir.join(&f.path)
    }
}

/// Every scalar leaf's dotted path and its token text, in source order.
pub fn leaves(text: &str) -> Result<Vec<(String, String)>, String> {
    Ok(scan_ron_leaves(text)?
        .into_iter()
        .map(|l| (l.path, l.text))
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Int,
    Float,
    Str,
    Bool,
    None,
    Variant,
}

impl Kind {
    fn of(token: &str) -> Kind {
        match token {
            "true" | "false" => Kind::Bool,
            "None" => Kind::None,
            _ if token.starts_with('"') => Kind::Str,
            _ if is_int(token) => Kind::Int,
            _ if token.parse::<f64>().is_ok() => Kind::Float,
            _ => Kind::Variant,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Kind::Int => "an integer",
            Kind::Float => "a float",
            Kind::Str => "a string",
            Kind::Bool => "a bool",
            Kind::None => "`None`",
            Kind::Variant => "an enum variant",
        }
    }
}

fn is_int(t: &str) -> bool {
    let u = t.strip_prefix(['-', '+']).unwrap_or(t);
    let digits = |s: &str, radix| {
        !s.is_empty()
            && s.chars().all(|c| c == '_' || c.is_digit(radix))
            && s.starts_with(|c: char| c.is_digit(radix))
    };
    match u.get(..2) {
        Some("0x") => digits(&u[2..], 16),
        Some("0o") => digits(&u[2..], 8),
        Some("0b") => digits(&u[2..], 2),
        _ => digits(u, 10),
    }
}

fn json_kind(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "a bool",
        serde_json::Value::Number(_) => "a number",
        serde_json::Value::String(_) => "a string",
        serde_json::Value::Array(_) => "an array",
        serde_json::Value::Object(_) => "an object",
    }
}

/// Up to five known paths sharing the longest prefix with `path`.
fn nearest(path: &str, known: &[String]) -> Vec<String> {
    let common = |k: &str| {
        path.chars()
            .zip(k.chars())
            .take_while(|(a, b)| a == b)
            .count()
    };
    let best = known.iter().map(|k| common(k)).max().unwrap_or(0);
    if best == 0 {
        return known.iter().take(5).cloned().collect();
    }
    known
        .iter()
        .filter(|k| common(k) == best)
        .take(5)
        .cloned()
        .collect()
}

/// Replace the scalar at `path` with `value`, keeping its kind, and return the new text. Only that
/// token's bytes change. See the module docs for what is refused.
pub fn set_leaf(
    text: &str,
    path: &str,
    value: &serde_json::Value,
    bounds: Option<&Bounds>,
) -> Result<String, String> {
    let all = scan_ron_leaves(text)?;
    let Some(leaf) = all.iter().find(|l| l.path == path) else {
        let known: Vec<String> = all.iter().map(|l| l.path.clone()).collect();
        let near = nearest(path, &known);
        return Err(format!(
            "`{path}` is not a leaf of this file; nearest: {}",
            near.iter()
                .map(|p| format!("`{p}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    };
    let kind = Kind::of(&leaf.text);
    let mismatch = |why: &str| {
        format!(
            "`{path}` holds {} (`{}`); {} is {why}",
            kind.name(),
            leaf.text,
            json_kind(value)
        )
    };

    let new = match (kind, value) {
        (Kind::Int | Kind::Float, serde_json::Value::Number(n)) => {
            let x = n
                .as_f64()
                .filter(|x| x.is_finite())
                .ok_or_else(|| format!("`{path}`: {n} is not a finite number"))?;
            if let Some(b) = bounds
                && (b.min.is_some_and(|m| x < m) || b.max.is_some_and(|m| x > m))
            {
                return Err(format!(
                    "`{path}`: {n} is outside its bounds [{}, {}]",
                    b.min.map_or("-inf".into(), |m| m.to_string()),
                    b.max.map_or("inf".into(), |m| m.to_string())
                ));
            }
            if kind == Kind::Int {
                if let Some(i) = n.as_i64() {
                    i.to_string()
                } else if let Some(u) = n.as_u64() {
                    u.to_string()
                } else if x.fract() == 0.0 && x.abs() < 9.0e15 {
                    (x as i64).to_string()
                } else {
                    return Err(format!(
                        "`{path}` holds an integer (`{}`); {n} is not a whole number",
                        leaf.text
                    ));
                }
            } else {
                let f = x as f32;
                if !f.is_finite() {
                    return Err(format!("`{path}`: {n} does not fit an f32"));
                }
                fmt_f32(f)
            }
        }
        (Kind::Str, serde_json::Value::String(s)) => {
            ron::to_string(s).map_err(|e| format!("`{path}`: {e}"))?
        }
        (Kind::Bool, serde_json::Value::Bool(b)) => b.to_string(),
        (Kind::None, _) => return Err(mismatch("a change from `None` to a value; refused")),
        (Kind::Variant, _) => {
            return Err(format!(
                "`{path}` holds an enum variant (`{}`); variants are not editable",
                leaf.text
            ));
        }
        _ => return Err(mismatch("a different kind of value")),
    };

    let mut out = String::with_capacity(text.len() + new.len());
    out.push_str(&text[..leaf.span.start]);
    out.push_str(&new);
    out.push_str(&text[leaf.span.end..]);
    ron::from_str::<ron::Value>(&out)
        .map_err(|e| format!("`{path}`: the edited file would not parse: {e}"))?;
    Ok(out)
}

/// The scalar at `path` as JSON, from its token text: what [`set_leaf`] would accept back. `None` for
/// a token that is not editable (`None`, an enum variant).
pub fn leaf_json(token: &str) -> Option<serde_json::Value> {
    match Kind::of(token) {
        Kind::Bool => Some(serde_json::Value::Bool(token == "true")),
        Kind::Str => ron::from_str::<String>(token).ok().map(serde_json::Value::String),
        Kind::Int => ron::from_str::<i64>(token).ok().map(Into::into),
        Kind::Float => token
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number),
        Kind::None | Kind::Variant => None,
    }
}

/// Put the exact token text `token` back at `path`, byte for byte: the undo of [`set_leaf`], which
/// may have rewritten `1.50` as `1.5`. Refused unless the leaf exists, and unless `token` is of the kind the leaf holds now
/// and the result parses.
pub fn restore_leaf(text: &str, path: &str, token: &str) -> Result<String, String> {
    let all = scan_ron_leaves(text)?;
    let Some(leaf) = all.iter().find(|l| l.path == path) else {
        return Err(format!("`{path}` is not a leaf of this file"));
    };
    if Kind::of(token) != Kind::of(&leaf.text) {
        return Err(format!("`{token}` is not a value `{path}` can hold"));
    }
    let mut out = String::with_capacity(text.len() + token.len());
    out.push_str(&text[..leaf.span.start]);
    out.push_str(token);
    out.push_str(&text[leaf.span.end..]);
    ron::from_str::<ron::Value>(&out).map_err(|e| format!("`{path}`: the restored file would not parse: {e}"))?;
    Ok(out)
}
