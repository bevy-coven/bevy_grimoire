//! **Check a file's text against its kind before anything applies it** (#200, H3).
//!
//! A reload validates first and applies second, so a bad edit costs the author a sentence and the
//! running program nothing. [`check`] is the first half for every kind of [`crate::reload::KINDS`]
//! whose type this crate owns: it parses with the type's own reader (the one a load uses, so the two
//! cannot disagree about what is valid), and on a refusal says *which file*, *which field* and *why*.
//!
//! # The field
//!
//! ron names a field itself only for one that is missing. For a value of the wrong type it names a
//! position, and the position says nothing about which field a nested struct was reading. So a
//! refusal is read again with `serde_path_to_error` watching, which yields the path
//! (`placements[3].at[1]`). That second read happens **only on a refusal**, to say where; it never
//! decides whether a file is valid. A refusal the type's own validation raises after the file parsed
//! (an unknown token, a heightfield whose size and data disagree) has no field here and says why in
//! its reason.
//!
//! # What is not checked here
//!
//! A kind whose type lives in another crate (a family crate's settings, the mapper's people, scripts
//! and forests, the decision crate's checks) or that has no text to parse is [`Check::Unchecked`]. The
//! list is [`unchecked`], and a test keeps it from growing unseen.

use serde::de::DeserializeOwned;

use crate::reload::{FileKind, KINDS};

/// **A file refused, and where.**
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The [`FileKind::id`].
    pub kind: &'static str,
    /// The path the caller gave, the one the author edited.
    pub file: String,
    /// The field the file is wrong at, as a path: `version`, `placements[3].at[1]`. For a file cut off
    /// or not RON it is the path where reading stopped (`placements[0]`), when it got that far. `None`
    /// for a file whose shape read and whose own validation refused it: the reason says why.
    pub field: Option<String>,
    /// The type's own error, with its position in the file.
    pub reason: String,
}

impl Refusal {
    /// `site/forest/yard.ron (trees[2].yaw): 1:47: Expected float`: the file, the field when the file
    /// got that far, and why. The same sentence `KindVerdict::sentence` says.
    pub fn sentence(&self) -> String {
        match &self.field {
            Some(field) => format!("{} ({field}): {}", self.file, self.reason),
            None => format!("{}: {}", self.file, self.reason),
        }
    }
}

/// **What [`check`] makes of a file.**
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Check {
    Valid,
    Refused(Refusal),
    /// This crate has no parser for the kind; see the module docs.
    Unchecked,
}

/// The type's own reader on `text`, and on a refusal the field it is wrong at.
fn run<T: DeserializeOwned>(
    text: &str,
    parse: fn(&str) -> Result<T, String>,
) -> Result<(), (Option<String>, String)> {
    match parse(text) {
        Ok(_) => Ok(()),
        Err(reason) => Err((field_of::<T>(text), reason)),
    }
}

/// **The path to the field `text` is wrong at as a `T`**, or `None` when it reads as one.
fn field_of<T: DeserializeOwned>(text: &str) -> Option<String> {
    let mut de = ron::de::Deserializer::from_str(text).ok()?;
    let err = serde_path_to_error::deserialize::<_, T>(&mut de).err()?;
    // A path serde_path_to_error cannot name (text that ended mid-value) prints `?` for the segment.
    let path = err.path().to_string().replace(".?", "");
    // ron raises a missing or an unknown field at the struct, so the path ends at the struct. An unknown
    // one was refused while its key was being read, so the path already ends with it.
    let (named, key_in_path) = match err.inner() {
        ron::Error::MissingStructField { field, .. } => (Some(field.to_string()), false),
        ron::Error::NoSuchStructField { found, .. } => (Some(found.to_string()), true),
        _ => (None, false),
    };
    match (path.as_str(), named) {
        (".", None) => None,
        (".", Some(field)) => Some(field),
        (_, None) => Some(path),
        (_, Some(field)) if key_in_path && (path == field || path.ends_with(&format!(".{field}"))) => Some(path),
        (_, Some(field)) => Some(format!("{path}.{field}")),
    }
}

/// **Read `text` as the `T` that kind `kind` is**, for a type whose reader is plain `ron::from_str` and
/// which lives in a layer above this crate (the editor's forests, people and triggers). A file that
/// does not read is refused with the file and the field, exactly as [`check`] does for the kinds this
/// crate owns, so the one reader decides both whether to apply a file and what to tell its author.
pub fn read<T: DeserializeOwned>(kind: &'static str, file: &str, text: &str) -> Result<T, Refusal> {
    ron::from_str(text).map_err(|e: ron::error::SpannedError| Refusal {
        kind,
        file: file.to_owned(),
        field: field_of::<T>(text),
        reason: e.to_string(),
    })
}

/// **Read every line of a JSON-lines file as a `T`**, for a log the editor appends to and a reader walks
/// (`oplog.jsonl`): a line that does not read as a `T` refuses the file, naming the line and the field.
///
/// The line is its number in the file, counted from 1 as an editor shows it (a blank line counts and is
/// skipped), and the field follows it: `line 3.seq` for a `seq` of the wrong type, `line 3.op` for one
/// that is missing, `line 3` for text that is not JSON at all or stopped before it named a field. Only
/// the first damaged line is named; the reason says how many more there are, so a repaired line moves
/// the verdict. A line is read the way `serde_json::from_str` reads it, trailing text included (two
/// entries glued together by a write cut short is a line that does not read).
pub fn read_lines<T: DeserializeOwned>(kind: &'static str, file: &str, text: &str) -> Result<Vec<T>, Refusal> {
    let mut read = Vec::new();
    let mut first: Option<(usize, Option<String>, String)> = None;
    let mut damaged = 0usize;
    for (n, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        match line_as::<T>(line) {
            Ok(value) => read.push(value),
            Err((field, reason)) => {
                damaged += 1;
                first.get_or_insert((n + 1, field, reason));
            }
        }
    }
    let Some((line, field, reason)) = first else { return Ok(read) };
    let more = match damaged - 1 {
        0 => String::new(),
        1 => "; 1 more line does not read".to_owned(),
        k => format!("; {k} more lines do not read"),
    };
    let field = match field {
        Some(field) => format!("line {line}.{field}"),
        None => format!("line {line}"),
    };
    Err(Refusal { kind, file: file.to_owned(), field: Some(field), reason: format!("{reason}{more}") })
}

/// One line as a `T`, or the field it is wrong at (`None` for text that is not JSON or stopped before it
/// named one) and why. serde_json names a missing field only in its message, so that is read from it.
fn line_as<T: DeserializeOwned>(line: &str) -> Result<T, (Option<String>, String)> {
    fn said(e: &serde_json::Error) -> String {
        // `... at line 1 column 40` says line 1 of the line; the column is all that helps.
        match e.to_string().rsplit_once(" at line ") {
            Some((what, _)) => format!("{what} (column {})", e.column()),
            None => e.to_string(),
        }
    }
    let mut de = serde_json::Deserializer::from_str(line);
    match serde_path_to_error::deserialize::<_, T>(&mut de) {
        Ok(value) => de.end().map(|()| value).map_err(|e| (None, said(&e))),
        Err(err) => {
            let path = err.path().to_string();
            let message = err.inner().to_string();
            let missing = message.strip_prefix("missing field `").and_then(|rest| rest.split_once('`')).map(|(f, _)| f.to_owned());
            let field = match (path.as_str(), missing) {
                (".", None) => None,
                (".", Some(field)) => Some(field),
                (_, None) => Some(path),
                (_, Some(field)) => Some(format!("{path}.{field}")),
            };
            Err((field, said(err.inner())))
        }
    }
}

/// **Parse `text` as `kind`.** `file` is only what a refusal names.
pub fn check(kind: &FileKind, file: &str, text: &str) -> Check {
    use crate::{
        composition::Compositions, data_files::DataFiles, destructible::{Bake, Marks}, family::Family,
        heightfield::Heightfield,
        kits::Kits, library::Library, map::Map, note_agent::AgentSettings, notes::Notes, plan::Plan,
        play::PlayConfig, policy::Policy, references::References, relations::Relations,
        repetition::RepetitionRules, rigs::Rigs, rules::Rules, standards::Standards,
        vocab::Vocabularies, volume::Volume, water::{WaterField, WaterPlan},
    };
    let result = match kind.id {
        "kits" => run(text, Kits::parse),
        "vocab" => run(text, Vocabularies::parse),
        "library" => run(text, Library::parse),
        "policy" => run(text, Policy::parse),
        "rules" => run(text, |t| {
            // `Rules::parse` only reads RON. What the editor refuses to open (a version it does not read, a
            // number no block can use) is `validate_numbers`, so a reload that said `applied` for a file the
            // project open refuses was telling the author the edit had landed.
            let rules = Rules::parse(t)?;
            rules.validate_numbers().map(|()| rules)
        }),
        "compositions" => run(text, Compositions::parse),
        "standards" => run(text, Standards::parse),
        "repetition" => run(text, RepetitionRules::parse),
        "rigs" => run(text, Rigs::parse),
        "notes" => run(text, Notes::from_ron),
        "plan" => run(text, Plan::from_ron),
        "references" => run(text, References::from_ron),
        "relations" => run(text, Relations::parse),
        "map" => run(text, Map::parse),
        "terrain" => run(text, Heightfield::parse),
        "water_plan" => run(text, WaterPlan::parse),
        "water_field" => run(text, WaterField::parse),
        "volume" => run(text, Volume::parse),
        "data_registry" => run(text, DataFiles::parse),
        "family_list" => run(text, Family::from_ron),
        "agent" => run(text, AgentSettings::from_ron),
        "play" => run(text, PlayConfig::from_ron),
        "destructible_marks" => run(text, Marks::parse),
        "destructible_bake" => run(text, Bake::parse),
        _ => return Check::Unchecked,
    };
    match result {
        Ok(()) => Check::Valid,
        Err((field, reason)) => Check::Refused(Refusal { kind: kind.id, file: file.to_owned(), field, reason }),
    }
}

/// **The kinds [`check`] cannot read**, so a reader knows which refusals it has to find another way.
pub fn unchecked() -> impl Iterator<Item = &'static FileKind> {
    KINDS.iter().filter(|k| matches!(check(k, "", ""), Check::Unchecked))
}
