//! `family.ron`: which of the owner's family crates a project uses.
//!
//! The editor hosts each family crate behind a cargo feature named after it. A project names the crates
//! it uses here, and the editor shows only those panels. Listing a crate the running build was not
//! compiled with is a problem the author is told about ([`Family::problems`]), never a silent no-op.
//! Engine-free: the build's feature set is passed in by the caller.

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::path::{Path, PathBuf};

pub const FAMILY_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Family {
    pub version: u32,
    pub crates: Vec<String>,
}

impl Default for Family {
    fn default() -> Self {
        Family { version: FAMILY_VERSION, crates: Vec::new() }
    }
}

/// `<project>/family.ron`.
pub fn path_for_project(root: &Path) -> PathBuf {
    root.join("family.ron")
}

impl Family {
    pub fn from_ron(text: &str) -> Result<Family, String> {
        let f: Family = ron::from_str(text).map_err(|e| format!("family: parse: {e}"))?;
        if f.version != FAMILY_VERSION {
            return Err(format!("family: version {} is not the {FAMILY_VERSION} this build reads", f.version));
        }
        let mut seen = std::collections::BTreeSet::new();
        for c in &f.crates {
            if !seen.insert(c.as_str()) {
                return Err(format!("family: {c} is listed twice"));
            }
        }
        Ok(f)
    }

    /// Reads the project's `family.ron`; a project without one uses no family crate.
    pub fn load(root: &Path) -> Result<Family, String> {
        let path = path_for_project(root);
        match std::fs::read_to_string(&path) {
            Ok(text) => Family::from_ron(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Family::default()),
            Err(e) => Err(format!("family: {}: {e}", path.display())),
        }
    }

    /// One message per listed crate the editor cannot host: an unknown name, or a known crate whose
    /// feature this build lacks. `known` is every crate the host can host, `built` the subset compiled in. Both come from the
    /// editor: this crate is engine-free and names no engine crate.
    pub fn problems(&self, known: &[&str], built: &[&str]) -> Vec<String> {
        self.crates
            .iter()
            .filter_map(|c| {
                if !known.contains(&c.as_str()) {
                    Some(format!("family.ron lists {c}, which is not a family crate the editor knows"))
                } else if !built.contains(&c.as_str()) {
                    Some(format!(
                        "family.ron lists {c}, but this editor was built without the `{c}` feature; rebuild with `--features {c}`"
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    /// The listed crates this build can host, in file order: the panels to show.
    pub fn active<'a>(&'a self, built: &[&str]) -> Vec<&'a str> {
        self.crates.iter().map(String::as_str).filter(|c| built.contains(c)).collect()
    }
}

/// Rewrites one field of a family settings file (RON text) and returns the new text with the field's
/// previous value, so the caller's undo is `set_field(new, path, old)`. `path` is dot-separated field
/// names (`sun.hour`). Only the value's bytes change: comments, order and layout elsewhere stay. The
/// settings are the family crate's own serde types, so the editor never parses them into a schema of
/// its own; `value` is checked only for being one balanced RON value.
pub fn set_field(text: &str, path: &str, value: &str) -> Result<(String, String), String> {
    let value = value.trim();
    if value.is_empty() || value_end(value, 0, &[]) != Some(value.len()) {
        return Err(format!("family: `{value}` is not one RON value"));
    }
    let (mut start, mut end) = (0, text.len());
    // The root may be a bare `Name(...)`, `(...)` or `{...}`: descend into its first bracket.
    for (depth, name) in path.split('.').enumerate() {
        let open = skip_ws(text, start, end);
        let open = if depth == 0 { open } else { start };
        let body = enter(text, open, end).ok_or_else(|| format!("family: `{name}` is not inside a struct"))?;
        let (vs, ve) = find_field(text, body, name)?;
        start = vs;
        end = ve;
    }
    let old = text[start..end].to_string();
    Ok((format!("{}{}{}", &text[..start], value, &text[end..]), old))
}

fn skip_ws(t: &str, mut i: usize, end: usize) -> usize {
    let b = t.as_bytes();
    while i < end {
        if b[i].is_ascii_whitespace() {
            i += 1;
        } else if t[i..].starts_with("//") {
            while i < end && b[i] != b'\n' {
                i += 1;
            }
        } else if t[i..].starts_with("/*") {
            i = t[i..].find("*/").map_or(end, |n| i + n + 2);
        } else {
            break;
        }
    }
    i
}

/// Given a value starting at `i` (after any `Name`), the byte range inside its brackets.
fn enter(t: &str, i: usize, end: usize) -> Option<(usize, usize)> {
    let b = t.as_bytes();
    let mut j = i;
    while j < end && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
        j += 1;
    }
    let j = skip_ws(t, j, end);
    if j >= end || !matches!(b[j], b'(' | b'{') {
        return None;
    }
    let close = value_end(t, j, &[])?;
    Some((j + 1, close - 1))
}

/// End (exclusive) of the RON value starting at `i`: a string/char literal, a bracketed group, or a run
/// up to a top-level `,` or closer. `stop` is unused room for callers; kept empty here.
fn value_end(t: &str, i: usize, _stop: &[u8]) -> Option<usize> {
    let b = t.as_bytes();
    let mut depth = 0i32;
    let mut j = i;
    while j < b.len() {
        match b[j] {
            b'"' => {
                j += 1;
                while j < b.len() && b[j] != b'"' {
                    j += if b[j] == b'\\' { 2 } else { 1 };
                }
                if j >= b.len() {
                    return None;
                }
            }
            b'/' if t[j..].starts_with("//") => {
                while j < b.len() && b[j] != b'\n' {
                    j += 1;
                }
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                if depth == 0 {
                    return Some(j);
                }
                depth -= 1;
                if depth == 0 {
                    // A group that opened this value ends it, unless a name-less run follows (never in RON).
                    return Some(j + 1);
                }
            }
            b',' if depth == 0 => return Some(j),
            _ => {}
        }
        j += 1;
    }
    if depth == 0 { Some(b.len()) } else { None }
}

/// The byte range of field `name`'s value among the fields in `body`, trailing whitespace excluded.
fn find_field(t: &str, body: (usize, usize), name: &str) -> Result<(usize, usize), String> {
    let b = t.as_bytes();
    let mut i = body.0;
    while i < body.1 {
        i = skip_ws(t, i, body.1);
        if i >= body.1 {
            break;
        }
        let ks = i;
        while i < body.1 && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
            i += 1;
        }
        let key = &t[ks..i];
        i = skip_ws(t, i, body.1);
        if key.is_empty() || i >= body.1 || b[i] != b':' {
            return Err(format!("family: `{name}` not found (a field list was expected)"));
        }
        i = skip_ws(t, i + 1, body.1);
        let vs = i;
        // A value may be `Name(..)`: scan the name, then the group; otherwise a scalar run.
        let ve = value_extent(t, vs, body.1);
        if key == name {
            let mut e = ve;
            while e > vs && b[e - 1].is_ascii_whitespace() {
                e -= 1;
            }
            return Ok((vs, e));
        }
        i = ve;
        if i < body.1 && b[i] == b',' {
            i += 1;
        }
    }
    Err(format!("family: no field `{name}`"))
}

/// Extent of a field value that may be `Name(...)`, a group, a string or a scalar, ending at the field's `,`.
fn value_extent(t: &str, i: usize, end: usize) -> usize {
    let b = t.as_bytes();
    let mut j = i;
    while j < end && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'-' || b[j] == b'.') {
        j += 1;
    }
    if j < end && matches!(b[j], b'(' | b'[' | b'{') {
        return value_end(t, j, &[]).unwrap_or(end);
    }
    if j > i {
        return j;
    }
    value_end(t, i, &[]).map_or(end, |e| e.min(end))
}

/// `<project>/family/`: one `<crate>.ron` of that crate's own settings each.
pub fn settings_dir(root: &Path) -> PathBuf {
    root.join("family")
}

/// Settings files are a few small files: look every tenth of a second and take a change as settled
/// after another, so a reload lands well inside the second the editor promises, even on a busy machine.
const SETTINGS_POLL_SECS: f64 = 0.1;
const SETTINGS_SETTLE_SECS: f64 = 0.1;

/// **Notices a family settings file changing on disk, and says which crates it touched** (#96's hot
/// reload). The editor's `family_set` and a person editing the file reach the same place: the
/// file. A settled change is reported once, by crate name, with the new text, so the preview and a
/// running game's own copy of the plugin read the same bytes. A file that no longer parses as RON is
/// reported as a problem, never applied: the previous good settings stay in force.
pub struct FamilyWatch {
    files: crate::live::FileWatch,
    dir: PathBuf,
    good: std::collections::BTreeMap<String, String>,
}

/// One settled change to a crate's settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsChange {
    /// The file now holds these settings.
    Reloaded { krate: String, text: String },
    /// The file is not valid RON; the last good settings stay in force.
    Invalid { krate: String, error: String },
    /// The file was deleted; the crate falls back to its own defaults.
    Removed { krate: String },
}

impl FamilyWatch {
    /// Stats and reads the project's settings now. Call before first use of them.
    pub fn new(root: &Path) -> FamilyWatch {
        let dir = settings_dir(root);
        let mut watch = FamilyWatch { files: crate::live::FileWatch::new(&dir, &["ron"]).with_timing(SETTINGS_POLL_SECS, SETTINGS_SETTLE_SECS), dir, good: Default::default() };
        for (krate, text) in watch.read() {
            if ron::from_str::<ron::Value>(&text).is_ok() {
                watch.good.insert(krate, text);
            }
        }
        watch
    }

    /// The settings that parsed when the watch was made, and every change since: what is in force now.
    pub fn current(&self) -> &std::collections::BTreeMap<String, String> {
        &self.good
    }

    fn read(&self) -> Vec<(String, String)> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return Vec::new() };
        let mut out: Vec<(String, String)> = entries
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension()? != "ron" || p.file_name()?.to_string_lossy().starts_with('.') {
                    return None;
                }
                Some((p.file_stem()?.to_string_lossy().into_owned(), std::fs::read_to_string(&p).ok()?))
            })
            .collect();
        out.sort();
        out
    }

    /// At most one look per [`crate::live::POLL_SECS`]; the changes once the files have settled.
    pub fn poll(&mut self, now_secs: f64) -> Vec<SettingsChange> {
        if !self.files.poll(now_secs) {
            return Vec::new();
        }
        let now = self.read();
        let mut out = Vec::new();
        for (krate, text) in &now {
            if self.good.get(krate) == Some(text) {
                continue;
            }
            match ron::from_str::<ron::Value>(text) {
                Ok(_) => {
                    self.good.insert(krate.clone(), text.clone());
                    out.push(SettingsChange::Reloaded { krate: krate.clone(), text: text.clone() });
                }
                Err(e) => out.push(SettingsChange::Invalid { krate: krate.clone(), error: e.to_string() }),
            }
        }
        let gone: Vec<String> = self.good.keys().filter(|k| !now.iter().any(|(n, _)| n == *k)).cloned().collect();
        for krate in gone {
            self.good.remove(&krate);
            out.push(SettingsChange::Removed { krate });
        }
        out
    }
}
