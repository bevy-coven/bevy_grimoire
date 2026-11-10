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

/// **The same, for a kind whose reader parses YAML now** (GD-03, #365): the field a refusal names is found
/// by the YAML deserialiser, which knows the field path, rather than by the RON one, which cannot read the
/// text at all. One function per spelling and no branch inside either, so the two readers cannot come to
/// disagree about a file by accident.
fn run_yaml<T: DeserializeOwned>(
    text: &str,
    parse: fn(&str) -> Result<T, String>,
) -> Result<(), (Option<String>, String)> {
    match parse(text) {
        Ok(_) => Ok(()),
        Err(reason) => Err((field_of_yaml::<T>(text), reason)),
    }
}

/// **The path to the field a YAML document is wrong at as a `T`**, or `None` when it reads.
///
/// [`field_of`]'s counterpart for the kinds the cutover moved, and the same shape: the deserialiser is run
/// again with `serde_path_to_error` only to name the path, because the kind's own `parse` answers a
/// sentence and not a path. A missing field is named by itself (serde reports it at the document's root,
/// which is no field at all); an unknown or wrongly typed one already has it in the path.
fn field_of_yaml<T: DeserializeOwned>(text: &str) -> Option<String> {
    let deserializer = serde_yaml_ng::Deserializer::from_str(text);
    let err = serde_path_to_error::deserialize::<_, T>(deserializer).err()?;
    let path = err.path().to_string();
    let path = path.trim_start_matches('.');
    // **`serde_yaml_ng` prefixes the path to its own message** (`blocks[0]: missing field `cell` at line 3`),
    // where `serde_json` and `ron` do not, so the field is found anywhere in the first line rather than only at
    // its start. A refusal that dropped it would name the item and not the field the item is missing.
    let message = err.inner().to_string();
    let missing = message
        .find("missing field `")
        .map(|at| &message[at + "missing field `".len()..])
        .and_then(|rest| rest.split_once('`'))
        .map(|(field, _)| field.to_owned());
    match (path.is_empty(), missing) {
        (true, None) => None,
        (true, Some(field)) => Some(field),
        (false, None) => Some(path.to_owned()),
        (false, Some(field)) => Some(format!("{path}.{field}")),
    }
}

/// **Read `text` as the `T` that kind `kind` is**, for a type whose reader lives in a layer above this
/// crate (the editor's forests, people and triggers).
///
/// **The reading is the caller's**, and that is the point: a kind whose type another crate declares is read
/// by that crate's reader — this crate cannot name the type's spelling, and guessing at it would be the second
/// reader #365 exists to remove. So a caller hands in the reader its own crate uses, and this adds what every
/// reader here adds: the kind, the file, and the field the refusal is about.
pub fn read<T: DeserializeOwned>(
    kind: &'static str,
    file: &str,
    text: &str,
    parse: fn(&str) -> Result<T, String>,
) -> Result<T, Refusal> {
    parse(text).map_err(|reason| Refusal {
        kind,
        file: file.to_owned(),
        field: field_of_yaml::<T>(text),
        reason,
    })
}

/// **Read `text` as the `T` that kind `kind` is, through this crate's own RON reader**, for a type whose
/// crate still ships RON (`bevy_oracle`'s checks, the editor's own settings layer).
///
/// **It exists so the door above stays a door.** A caller whose crate has moved to YAML hands in its reader;
/// a caller whose crate has not says so with this, and the day that crate moves its kinds the call becomes the
/// plain [`read`] with one argument more. Two named entry points rather than one silent default, so a grep for
/// `read_ron` is the list of kinds still outside the cutover.
pub fn read_ron<T: DeserializeOwned>(kind: &'static str, file: &str, text: &str) -> Result<T, Refusal> {
    fn parse<T: DeserializeOwned>(text: &str) -> Result<T, String> {
        ron::from_str(text).map_err(|e: ron::error::SpannedError| e.to_string())
    }
    parse(text).map_err(|reason| Refusal {
        kind,
        file: file.to_owned(),
        field: field_of_yaml::<T>(text),
        reason,
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
        "kits" => run_yaml(text, Kits::parse),
        "vocab" => run_yaml(text, Vocabularies::parse),
        "library" => run_yaml(text, Library::parse),
        "policy" => run_yaml(text, Policy::parse),
        "rules" => run_yaml(text, |t| {
            // `Rules::parse` only reads RON. What the editor refuses to open (a version it does not read, a
            // number no block can use) is `validate_numbers`, so a reload that said `applied` for a file the
            // project open refuses was telling the author the edit had landed.
            let rules = Rules::parse(t)?;
            rules.validate_numbers().map(|()| rules)
        }),
        "compositions" => run_yaml(text, Compositions::parse),
        "standards" => run_yaml(text, Standards::parse),
        "repetition" => run_yaml(text, RepetitionRules::parse),
        "rigs" => run_yaml(text, Rigs::parse),
        "notes" => run_yaml(text, Notes::parse),
        "plan" => run_yaml(text, Plan::parse),
        "references" => run_yaml(text, References::parse),
        "relations" => run_yaml(text, Relations::parse),
        "map" => run_yaml(text, Map::parse),
        "terrain" => run_yaml(text, Heightfield::parse),
        "water_plan" => run_yaml(text, WaterPlan::parse),
        "water_field" => run_yaml(text, WaterField::parse),
        "volume" => run_yaml(text, Volume::parse),
        "data_registry" => run_yaml(text, DataFiles::parse),
        "family_list" => run_yaml(text, Family::parse),
        "agent" => run_yaml(text, AgentSettings::parse),
        "play" => run_yaml(text, PlayConfig::parse),
        "destructible_marks" => run_yaml(text, Marks::parse),
        "destructible_bake" => run_yaml(text, Bake::parse),
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
