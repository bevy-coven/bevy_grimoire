//! **One JSON Schema per file kind, generated from the type** (GD-04, #366).
//!
//! An outside editor gets completion and hover from a schema, so each kind's schema is **generated from
//! the Rust type the reader reads** — a schema written by hand could describe a file no reader accepts,
//! and the two would drift with nothing failing. [`all`] is every kind's schema, [`for_kind`] is one,
//! [`write_all`] puts them in a project, and [`header`] is the line an editor's YAML file starts with so
//! its extension finds the schema.
//!
//! # What a generated schema carries
//!
//! - **The type's own shape**, field for field, through `schemars`, including the defaults the type
//!   states, so an editor completes an empty file with the fields that have one.
//! - **A field's doc comment as its `description`**, which is what hover shows a person and what an agent
//!   reads. There is nothing to maintain: the comment is the field's own.
//! - **`x-dark-factory-ref: <kind>`** on a field that names another file's value — a placement's
//!   `descriptor` names a library piece, a stamp names a composition — so a file editor and navigation
//!   can follow it. The field is marked once, in the type, with `#[schemars(extend("x-dark-factory-ref"
//!   = "library"))]`.
//! - **`x-dark-factory: <kind>`** and a `$comment` saying the file is generated, so a hand edit that is
//!   overwritten is explained by the file itself.
//!
//! # Where it lives
//!
//! `<root>/.dark_factory/schema/<kind>.schema.json`. The files are **derived**: the editor writes them on
//! every open, so a hand edit is overwritten, and the `$comment` says so. They are listed in
//! [`crate::reload::KINDS`] as [`crate::reload::ReloadClass::Live`], because nothing holds them in memory
//! and the editor writes them afresh.
//!
//! A game's own reflected components are `game.schema.json`, from the running game's registry, which
//! [`write_game`] writes when the game starts. This crate cannot make that file —
//! the registry is the game's — so it writes the one it can and the game's side hands it the rest.
//!
//! Engine-free: nothing here knows what a game's registry is; [`write_game`] takes the schema as text.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::problem::{Detail, Location, Problem};

/// **The folder, under a project's root, that holds the generated schema files.**
pub const SCHEMA_DIR: &str = ".dark_factory/schema";

/// **The file extension a kind's schema has**, so the editor's glob and an outside editor's discovery
/// agree on one name.
pub const SCHEMA_EXT: &str = ".schema.json";

/// **The comment line an editor's YAML file starts with**: `# yaml-language-server: $schema=<path>`. The
/// YAML language server reads it, and every editor built on it (VS Code, Neovim, Zed) follows.
pub const SCHEMA_HINT: &str = "yaml-language-server: $schema=";

/// **Every kind whose schema this crate generates**, each with the JSON of its type.
///
/// The list is [`crate::reload::KINDS`] filtered to the kinds whose type lives here: a kind whose type is
/// a game's, a family crate's or the editor's cannot have a schema generated from a type this crate does
/// not have.
pub const KINDS: &[&str] = &[
    "kits",
    "vocab",
    "library",
    "policy",
    "rules",
    "compositions",
    "standards",
    "repetition",
    "data_registry",
    "rigs",
    "map",
    "notes",
    "plan",
    "references",
    "relations",
    "terrain",
    "water_plan",
    "water_field",
    "volume",
    "family_list",
    "agent",
    "play",
    "destructible_marks",
    "destructible_bake",
];

/// **One kind's schema**, as the JSON an editor reads.
#[allow(clippy::result_large_err)]
pub fn for_kind(kind: &str) -> Result<Value, Problem> {
    let mut schema = match kind {
        "kits" => root::<crate::kits::Kits>(),
        "vocab" => root::<crate::vocab::Vocabularies>(),
        "library" => root::<crate::library::Library>(),
        "policy" => root::<crate::policy::Policy>(),
        "rules" => root::<crate::rules::Rules>(),
        "compositions" => root::<crate::composition::Compositions>(),
        "standards" => root::<crate::standards::Standards>(),
        "repetition" => root::<crate::repetition::RepetitionRules>(),
        "data_registry" => root::<crate::data_files::DataFiles>(),
        "rigs" => root::<crate::rigs::Rigs>(),
        "map" => root::<crate::map::Map>(),
        "notes" => root::<crate::notes::Notes>(),
        "plan" => root::<crate::plan::Plan>(),
        "references" => root::<crate::references::References>(),
        "relations" => root::<crate::relations::Relations>(),
        "terrain" => root::<crate::heightfield::Heightfield>(),
        "water_plan" => root::<crate::water::WaterPlan>(),
        "water_field" => root::<crate::water::WaterField>(),
        "volume" => root::<crate::volume::Volume>(),
        "family_list" => root::<crate::family::Family>(),
        "agent" => root::<crate::note_agent::AgentSettings>(),
        "play" => root::<crate::play::PlayConfig>(),
        "destructible_marks" => root::<crate::destructible::Marks>(),
        "destructible_bake" => root::<crate::destructible::Bake>(),
        _ => {
            let known: Vec<crate::reload::FileKind> = crate::reload::KINDS.iter().copied().filter(|k| !KINDS.contains(&k.id)).collect();
            return Err(Problem::with(
                format!("there is no schema for `{kind}`"),
                Detail::default()
                    .because(format!(
                        "a schema is generated from a type, and this crate has none for it; the kinds it generates are {KINDS:?}{}",
                        if known.is_empty() { String::new() } else { format!(", and {} kinds' types live in another crate", known.len()) }
                    ))
                    .fix("ask for a kind whose type `bevy_grimoire_core` owns")
                    .docs("docs/guide/your-game.md"),
            ));
        }
    };
    stamp(&mut schema, kind);
    Ok(schema)
}

/// **Every kind's schema**, as `(kind, schema)`.
#[allow(clippy::result_large_err)]
pub fn all() -> Vec<(&'static str, Value)> {
    KINDS.iter().filter_map(|kind| for_kind(kind).map(|schema| (*kind, schema)).ok()).collect()
}

/// **The generated root schema of a type**, through `schemars`: what an editor reads, as the type
/// declares it.
fn root<T: schemars::JsonSchema>() -> Value {
    let generated = schemars::SchemaGenerator::default().into_root_schema_for::<T>();
    serde_json::to_value(generated).unwrap_or(Value::Null)
}

/// **Mark a schema as this crate's and this kind's**, and say it is generated.
fn stamp(schema: &mut Value, kind: &str) {
    let Value::Object(map) = schema else { return };
    map.insert("x-dark-factory".to_owned(), Value::from(kind));
    map.insert(
        "$comment".to_owned(),
        Value::from("generated from the Rust type by bevy_grimoire_core::schema — a hand edit is overwritten the next time the editor opens this project"),
    );
}

/// **Write every kind's schema into the project at `root`**, and answer the paths written.
///
/// A file that cannot be written is one [`Problem`] naming it: a schema is what an outside editor's
/// completion depends on, so a silent failure would show up as an editor that offers nothing.
#[allow(clippy::result_large_err)]
pub fn write_all(root: &Path) -> Result<Vec<PathBuf>, Problem> {
    let dir = root.join(SCHEMA_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| write_problem(&dir, &e.to_string()))?;
    let mut written = Vec::new();
    for (kind, schema) in all() {
        let path = dir.join(format!("{kind}{SCHEMA_EXT}"));
        write_json(&path, &schema)?;
        written.push(path);
    }
    Ok(written)
}

/// **Write a game's own reflected schema** — the text the running game's remote registry answers — into
/// the project: the game's components are what its own files accept, so its schema is the game's to
/// produce and this only puts it where an editor looks.
#[allow(clippy::result_large_err)]
pub fn write_game(root: &Path, schema: &str) -> Result<PathBuf, Problem> {
    let dir = root.join(SCHEMA_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| write_problem(&dir, &e.to_string()))?;
    // A registry that answered text which is not JSON would be a game's bug, and saying so here is
    // better than writing a file an editor cannot read.
    let value: Value = serde_json::from_str(schema).map_err(|e| {
        Problem::with(
            "the game's schema is not JSON",
            Detail::default()
                .because(e.to_string())
                .fix("check what the game's registry answered for its schema")
                .docs("docs/guide/your-game.md"),
        )
    })?;
    let path = dir.join(format!("game{SCHEMA_EXT}"));
    write_json(&path, &value)?;
    Ok(path)
}

/// **The line a YAML file of `kind` starts with**, so an outside editor finds its schema. The path is
/// relative to the project root, which is where the schema files live.
pub fn header(kind: &str) -> String {
    format!("# {SCHEMA_HINT}{SCHEMA_DIR}/{kind}{SCHEMA_EXT}")
}

/// **`text` with `kind`'s schema hint as its first line**, unless it already starts with one. A file the
/// editor writes begins with it; a file with a person's comments keeps them below it, and a file with no
/// hint gains one without any other line moving.
pub fn with_header(kind: &str, text: &str) -> String {
    if text.starts_with(SCHEMA_HINT) || text.starts_with(&format!("# {SCHEMA_HINT}")) {
        return text.to_owned();
    }
    format!("{}\n{text}", header(kind))
}

#[allow(clippy::result_large_err)]
fn write_json(path: &Path, value: &Value) -> Result<(), Problem> {
    let mut text = serde_json::to_string_pretty(value).map_err(|e| write_problem(path, &e.to_string()))?;
    text.push('\n');
    crate::atomic_file::write(path, &text).map_err(|e| write_problem(path, &e.to_string()))
}

fn write_problem(path: &Path, why: &str) -> Problem {
    Problem::with(
        format!("{}: could not be written", path.display()),
        Detail::default()
            .at(Location::file(path.display().to_string()))
            .because(why.to_owned())
            .fix("check the project folder is writable")
            .docs("docs/guide/your-game.md"),
    )
}

/// **Whether a kind's type is one this crate owns**, so a caller knows if [`for_kind`] can answer.
pub fn owns(kind: &str) -> bool {
    KINDS.contains(&kind)
}

/// **A kind's fixture as the JSON an editor's extension shows**, which is the value the type holds: read
/// by this crate's own reader, then as JSON. A file whose YAML uses a tag for an enum (`!Near`) is a
/// JSON object to an editor, and this is the one place that says so.
#[allow(clippy::result_large_err)]
pub fn value_of(kind: &str, text: &str) -> Result<Value, Problem> {
    match kind {
        "kits" => json_of::<crate::kits::Kits>(text),
        "vocab" => json_of::<crate::vocab::Vocabularies>(text),
        "library" => json_of::<crate::library::Library>(text),
        "policy" => json_of::<crate::policy::Policy>(text),
        "rules" => json_of::<crate::rules::Rules>(text),
        "compositions" => json_of::<crate::composition::Compositions>(text),
        "standards" => json_of::<crate::standards::Standards>(text),
        "repetition" => json_of::<crate::repetition::RepetitionRules>(text),
        "data_registry" => json_of::<crate::data_files::DataFiles>(text),
        "rigs" => json_of::<crate::rigs::Rigs>(text),
        "map" => json_of::<crate::map::Map>(text),
        "notes" => json_of::<crate::notes::Notes>(text),
        "plan" => json_of::<crate::plan::Plan>(text),
        "references" => json_of::<crate::references::References>(text),
        "relations" => json_of::<crate::relations::Relations>(text),
        "terrain" => json_of::<crate::heightfield::Heightfield>(text),
        "water_plan" => json_of::<crate::water::WaterPlan>(text),
        "water_field" => json_of::<crate::water::WaterField>(text),
        "volume" => json_of::<crate::volume::Volume>(text),
        "family_list" => json_of::<crate::family::Family>(text),
        "agent" => json_of::<crate::note_agent::AgentSettings>(text),
        "play" => json_of::<crate::play::PlayConfig>(text),
        "destructible_marks" => json_of::<crate::destructible::Marks>(text),
        "destructible_bake" => json_of::<crate::destructible::Bake>(text),
        _ => for_kind(kind).map(|_| Value::Null),
    }
}

/// A file read by this crate's reader, then as JSON.
#[allow(clippy::result_large_err)]
fn json_of<T>(text: &str) -> Result<Value, Problem>
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let value: T = crate::yaml::from_str(text, "a fixture")?;
    serde_json::to_value(value).map_err(|e| {
        Problem::with("a fixture did not convert to JSON", Detail::default().because(e.to_string()))
    })
}

/// **Write one kind's schema into the project at `root`**, which is what an editor does for a kind it
/// just wrote a file of.
#[allow(clippy::result_large_err)]
pub fn write_kind(root: &Path, kind: &str) -> Result<PathBuf, Problem> {
    let schema = for_kind(kind)?;
    let dir = root.join(SCHEMA_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| write_problem(&dir, &e.to_string()))?;
    let path = dir.join(format!("{kind}{SCHEMA_EXT}"));
    write_json(&path, &schema)?;
    Ok(path)
}

/// The JSON text of a schema, for a tool that hands it to an agent.
#[allow(clippy::result_large_err)]
pub fn to_json(schema: &Value) -> Result<String, Problem> {
    serde_json::to_string_pretty(schema).map_err(|e| {
        Problem::with("a schema did not write as JSON", Detail::default().because(e.to_string()))
    })
}

