//! **Game data files the editor may tune: a registry and type-keeping leaf edits.**
//!
//! A game keeps tuning numbers in its own YAML files (Sanctuary: `assets/data/tuning/<domain>.yaml`).
//! They are not part of the grimoire project's schema, so the project lists them in
//! `<grimoire_dir>/data.yaml`, and the editor edits one scalar at a time through [`set_leaf`], which is
//! [`crate::yaml_surgery::set`] — the one YAML surgery, so an edit changes that leaf's bytes and nothing else,
//! and every comment, alignment and neighbouring literal survives.
//!
//! An edit never changes what *kind* of value a leaf is. A number stays a number (an integer token
//! takes only whole values), a string a string, `None` stays `None`; a value outside the registered
//! [`Bounds`] and a result that no longer parses are refused. Every refusal names the leaf path.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

use crate::reload_check::Refusal;
use crate::yaml_surgery::Value;

/// File name of the registry inside the grimoire directory.
pub const REGISTRY_FILE: &str = "data.yaml";
/// The registry's schema version.
pub const REGISTRY_VERSION: u32 = 1;

/// An inclusive numeric range for one leaf; either end may be open.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Bounds {
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
}

/// One registered data file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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

/// The parsed `data.yaml`, with the directory its paths are relative to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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
        let d: DataFiles = crate::yaml::from_str(text, REGISTRY_FILE).map_err(|p| p.line())?;
        if d.version != REGISTRY_VERSION {
            return Err(format!("version {} is not {REGISTRY_VERSION}", d.version));
        }
        for (i, f) in d.files.iter().enumerate() {
            if d.files[..i].iter().any(|g| g.id == f.id) {
                return Err(format!("id `{}` is registered twice", f.id));
            }
            // **Not a file that names a program.** A game's own data legitimately lives outside this folder
            // (`../data/mob.yaml`), so where a path points is not the question; what it is. `data_set`
            // edits a registered file's values, and `agent.yaml` or `play.yaml` registered here would let
            // it edit a command a person had not read (the review of 2026-11-02).
            if let Some(name) = std::path::Path::new(&f.path).file_name().and_then(|n| n.to_str())
                && crate::trust::COMMAND_FILES.contains(&name)
            {
                return Err(format!(
                    "entry `{}`: `{}` names a program the editor would start, so it cannot be a data file. Tuning values belong in the game's own data files",
                    f.id, f.path
                ));
            }
        }
        Ok(d)
    }

    /// Read `<grimoire_dir>/data.yaml`. An absent registry is `Ok(None)`; a malformed one, a version
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
    // **A tuning file is one mapping.** A bare scalar, a list or a cut-off document reads as *some* JSON
    // value, so the check is a mapping's shape and not "does it parse": `this is not a tuning file` is a
    // string, and a file that stopped mid-value is a mapping with a null where a number was meant. Both are
    // refused here rather than answered as a leaf list an edit could land in.
    let value: serde_json::Value = crate::yaml::from_str(text, "a data file").map_err(|p| p.line())?;
    if !value.is_object() {
        return Err(format!(
            "a data file holds one mapping of names to values, and this is {}",
            match value {
                serde_json::Value::Null => "`null`",
                serde_json::Value::Bool(_) => "a bool",
                serde_json::Value::Number(_) => "a number",
                serde_json::Value::String(_) => "a string",
                serde_json::Value::Array(_) => "a list",
                serde_json::Value::Object(_) => unreachable!(),
            }
        ));
    }
    Ok(crate::yaml::leaves(text))
}

/// The kind id a refusal of a registered data file carries.
const KIND: &str = "game_data";

/// **The field a scan stopped at**: its errors end "... at `mob.speed`" when they got that far.
fn stopped_at(why: &str) -> Option<String> {
    // **The YAML reader names the path it stopped at in parentheses**, `a data file (mob.name[1])`, because
    // the field is its own part of the `Problem` and the sentence does not repeat it. That path is what a
    // refusal carries, so it is read from there — and a refusal with no path (`()`) says nothing about where,
    // rather than claiming the root.
    let (_, rest) = why.rsplit_once(" (")?;
    let (path, _) = rest.split_once(')')?;
    let path = path.trim();
    (!path.is_empty()).then(|| path.to_owned())
}

/// **Every scalar leaf of `text`, or a refusal that names the file and the field** (#200, H3): the one
/// reader a data edit and the editor's look at a game's file both use, so the two cannot disagree about
/// what reads. `file` is only what the refusal names. A file cut off or not RON is refused at the leaf
/// where reading stopped (`mob.name`), when it got that far; a file the scan read and RON does not is
/// refused with RON's position and no field.
pub fn read(file: &str, text: &str) -> Result<Vec<(String, String)>, Refusal> {
    let refuse = |field, reason: String| Refusal { kind: KIND, file: file.to_owned(), field, reason };
    // **One reader, so it says where the fault is.** `leaves` refuses a file that does not read and answers
    // the field path its walk reached, which is what a person goes to.
    leaves(text).map_err(|why| refuse(stopped_at(&why), why))
}

/// `[0, 9]`, `[1, inf]`: a range as a refusal says it.
fn range(b: &Bounds) -> String {
    format!("[{}, {}]", b.min.map_or("-inf".into(), |m| m.to_string()), b.max.map_or("inf".into(), |m| m.to_string()))
}

/// **Check one registered file against its registry entry** (#200, H3): [`read`], then every leaf the
/// registry bounds must be there, a number, and inside its bounds. The bounded leaves are the fields
/// this crate knows a game's file must have, since the file's own type is the game's. A refusal names
/// the file and the leaf; a file that does not read is [`read`]'s refusal.
pub fn check(file: &str, entry: &DataFile, text: &str) -> Result<(), Refusal> {
    let all = read(file, text)?;
    for (path, b) in &entry.bounds {
        let refuse = |reason: String| Refusal { kind: KIND, file: file.to_owned(), field: Some(path.clone()), reason };
        let Some((_, token)) = all.iter().find(|(p, _)| p == path) else {
            let known: Vec<String> = all.iter().map(|(p, _)| p.clone()).collect();
            let near = nearest(path, &known).iter().map(|p| format!("`{p}`")).collect::<Vec<_>>().join(", ");
            return Err(refuse(format!("the registry bounds `{path}` and this file has no such leaf; nearest: {near}")));
        };
        // **The leaf is a number or it is refused**, and the refusal says what it holds instead: a bound on
        // a leaf somebody turned into a string is a registry that no longer describes its game, and reading
        // it as "not a number, carry on" is how a bound silently stops being checked.
        let Some(x) = leaf_json(token).and_then(|v| v.as_f64()) else {
            let held = leaf_json(token).map_or_else(|| "something no file holds".to_owned(), |v| {
                match v {
                    serde_json::Value::Bool(_) => "a bool".to_owned(),
                    serde_json::Value::String(_) => "a string".to_owned(),
                    other => format!("{other}"),
                }
            });
            return Err(refuse(format!(
                "`{path}` holds {held} (`{token}`) and the registry bounds it as a number"
            )));
        };
        if b.min.is_some_and(|m| x < m) || b.max.is_some_and(|m| x > m) {
            return Err(refuse(format!("`{path}` is {token}, outside its bounds {}", range(b))));
        }
    }
    Ok(())
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

/// **A JSON value as the surgery's**, for the one caller that holds JSON: an MCP request's arguments. A
/// value JSON cannot express as a project file is refused rather than turned into something else.
#[allow(clippy::result_large_err)]
fn to_value(value: &serde_json::Value) -> Result<Value, String> {
    match value {
        serde_json::Value::Null => Err("`null` is a value no data file holds".to_owned()),
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::String(s) => Ok(Value::Str(s.clone())),
        serde_json::Value::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
            (Some(i), _, _) => Ok(Value::Int(i)),
            (_, Some(u), _) => i64::try_from(u)
                .map(Value::Int)
                .map_err(|_| format!("{u} is too large for any integer a file holds")),
            (_, _, Some(x)) if x.is_finite() => Ok(Value::Float(x)),
            (_, _, Some(x)) => Err(format!("{x} is not a finite number")),
            _ => Err(format!("{n} is not a number a file holds")),
        },
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Err("a list or a mapping; a leaf takes a single value".to_owned())
        }
    }
}

/// Replace the scalar at `path` with `value`, keeping its kind, and return the new text. Only that
/// leaf's bytes change.
///
/// **This is [`crate::yaml_surgery::set`]**, the one edit a YAML project file has: it refuses a value of
/// another kind (a number into a string, a fraction into an integer), a number outside `bounds`, and a result
/// that no longer reads, and it names the field at the line and column it stands on. There is no second
/// implementation here — a data file is a project file with a registry entry, and the cutover gave it the
/// same reader, writer and surgery as every other kind.
#[allow(clippy::result_large_err)]
pub fn set_leaf(
    text: &str,
    path: &str,
    value: &serde_json::Value,
    bounds: Option<&Bounds>,
) -> Result<String, String> {
    let value = to_value(value).map_err(|why| format!("`{path}`: {why}"))?;
    // **A leaf that is not here is named with its neighbours**, which is the answer a person needs: a
    // mistyped `mob.speeed` is one edit away, and the surgery's own refusal says `not a value` without the
    // list of what is.
    let known: Vec<String> = crate::yaml::leaves(text).into_iter().map(|(p, _)| p).collect();
    if !known.iter().any(|p| p == path) {
        return Err(format!(
            "`{path}` is not a leaf of this file; nearest: {}",
            nearest(path, &known).iter().map(|p| format!("`{p}`")).collect::<Vec<_>>().join(", ")
        ));
    }
    crate::yaml_surgery::set(text, path, value, bounds).map_err(|p| p.line())
}

/// The scalar at `path` as JSON, from its token text: what [`set_leaf`] would accept back. `None` for a
/// token that is not a scalar a person edits — a mapping, a list, or a value JSON cannot hold.
pub fn leaf_json(token: &str) -> Option<serde_json::Value> {
    crate::yaml::to_json(token, "a leaf").ok().and_then(|value| match &value {
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => None,
        _ => Some(value),
    })
}

/// Put the exact token text `token` back at `path`: the undo of [`set_leaf`], which may have rewritten
/// `1.50` as `1.5`.
///
/// **The token is a YAML scalar**, so it is read back as the value it spells and written by the surgery —
/// which is what keeps a file reading. A token that does not spell a value of the leaf's kind is refused
/// before anything changes.
#[allow(clippy::result_large_err)]
pub fn restore_leaf(text: &str, path: &str, token: &str) -> Result<String, String> {
    let json = crate::yaml::to_json(token, "a restored value").map_err(|p| p.line())?;
    set_leaf(text, path, &json, None)
}
