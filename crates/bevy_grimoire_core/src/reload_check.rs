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
    // ron raises a missing or an unknown field at the struct, so the path ends at the struct.
    let named = match err.inner() {
        ron::Error::MissingStructField { field, .. } => Some(field.to_string()),
        ron::Error::NoSuchStructField { found, .. } => Some(found.to_string()),
        _ => None,
    };
    match (path.as_str(), named) {
        (".", None) => None,
        (".", Some(field)) => Some(field),
        (_, None) => Some(path),
        (_, Some(field)) => Some(format!("{path}.{field}")),
    }
}

/// **Parse `text` as `kind`.** `file` is only what a refusal names.
pub fn check(kind: &FileKind, file: &str, text: &str) -> Check {
    use crate::{
        composition::Compositions, data_files::DataFiles, family::Family, heightfield::Heightfield,
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
        "rules" => run(text, Rules::parse),
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
