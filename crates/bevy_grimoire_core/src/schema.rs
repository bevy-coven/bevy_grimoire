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
            // The kinds whose type genuinely lives in another crate. A kind of core's own that has no
            // schema would be a kind added to the reload table and not to `KINDS` above, which is a
            // different fault and is not counted as "another crate's".
            let elsewhere: Vec<&str> = crate::reload::KINDS
                .iter()
                .map(|k| k.id)
                .filter(|id| !KINDS.contains(id))
                .filter(|id| !crate::reload_check::unchecked().any(|k| k.id == *id))
                .collect();
            return Err(Problem::with(
                format!("there is no schema for `{kind}`"),
                Detail::default()
                    .because(format!(
                        "a schema is generated from a type, and this crate has none for it; the kinds it generates are {KINDS:?}{}",
                        if elsewhere.is_empty() { String::new() } else { format!(", and {} kinds' types live in another crate: {elsewhere:?}", elsewhere.len()) }
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
///
/// [`for_kind`] can only refuse a kind `KINDS` does not list, and this iterates `KINDS`, so every one
/// answers: a kind added to the list without its `for_kind` arm is a test failure, not a missing entry
/// here.
pub fn all() -> Vec<(&'static str, Value)> {
    KINDS
        .iter()
        .map(|kind| (*kind, for_kind(kind).unwrap_or_else(|e| panic!("`{kind}` is in schema::KINDS and for_kind has no arm for it: {}", e.line()))))
        .collect()
}

/// **The generated root schema of a type**, through `schemars`: what an editor reads, as the type
/// declares it.
///
/// A schema is `schemars`'s own `Schema`, which is a tree of `serde_json` values and strings, so
/// `to_value` cannot fail on it: the only `Err` it has is a map key that is not a string, and a schema's
/// keys are all `&'static str`. The `expect` states that, so a future schema shape that broke it would
/// panic here, in a test, rather than quietly returning `null` as a schema an editor then ignores.
fn root<T: schemars::JsonSchema>() -> Value {
    let generated = schemars::SchemaGenerator::default().into_root_schema_for::<T>();
    serde_json::to_value(generated).expect("a schemars schema is JSON values and string keys")
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

/// **The line a YAML file of `kind` starts with**, so an outside editor finds its schema.
///
/// **The path is relative to the file that carries it**, not to the project root: a YAML language server
/// resolves `$schema` from the YAML file's own folder, so `assets/grimoire/kits.yaml` says
/// `../../../.dark_factory/schema/kits.schema.json` and a file at the root says
/// `.dark_factory/schema/family_list.schema.json`. `file` is the file's path **relative to the project
/// root**, `/`-separated, which is the only thing the answer depends on besides the kind.
pub fn header(kind: &str, file: &str) -> String {
    let depth = file.split('/').count().saturating_sub(1);
    let up = if depth == 0 { String::new() } else { "../".repeat(depth) };
    format!("# {SCHEMA_HINT}{up}{SCHEMA_DIR}/{kind}{SCHEMA_EXT}")
}

/// **`text` with `kind`'s schema hint as its first line**, unless it already starts with one. A file the
/// editor writes begins with it; a file with a person's comments keeps them below it, and a file with no
/// hint gains one without any other line moving.
pub fn with_header(kind: &str, file: &str, text: &str) -> String {
    if text.starts_with(&format!("# {SCHEMA_HINT}")) {
        return text.to_owned();
    }
    format!("{}\n{text}", header(kind, file))
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

/// **The field path a kind's reader refuses `text` at**, or `None` when it reads — the reader's half of
/// "fails at the same path the schema names", so a test compares two readers rather than each against an
/// expectation of its own.
pub fn field_of(kind: &str, text: &str) -> Option<String> {
    fn at<T: serde::de::DeserializeOwned>(text: &str) -> Option<String> {
        crate::yaml::from_str::<T>(text, "a project file").err().and_then(|p| p.where_.and_then(|w| w.field))
    }
    match kind {
        "kits" => at::<crate::kits::Kits>(text),
        "vocab" => at::<crate::vocab::Vocabularies>(text),
        "library" => at::<crate::library::Library>(text),
        "policy" => at::<crate::policy::Policy>(text),
        "rules" => at::<crate::rules::Rules>(text),
        "compositions" => at::<crate::composition::Compositions>(text),
        "standards" => at::<crate::standards::Standards>(text),
        "repetition" => at::<crate::repetition::RepetitionRules>(text),
        "data_registry" => at::<crate::data_files::DataFiles>(text),
        "rigs" => at::<crate::rigs::Rigs>(text),
        "map" => at::<crate::map::Map>(text),
        "notes" => at::<crate::notes::Notes>(text),
        "plan" => at::<crate::plan::Plan>(text),
        "references" => at::<crate::references::References>(text),
        "relations" => at::<crate::relations::Relations>(text),
        "terrain" => at::<crate::heightfield::Heightfield>(text),
        "water_plan" => at::<crate::water::WaterPlan>(text),
        "water_field" => at::<crate::water::WaterField>(text),
        "volume" => at::<crate::volume::Volume>(text),
        "family_list" => at::<crate::family::Family>(text),
        "agent" => at::<crate::note_agent::AgentSettings>(text),
        "play" => at::<crate::play::PlayConfig>(text),
        "destructible_marks" => at::<crate::destructible::Marks>(text),
        "destructible_bake" => at::<crate::destructible::Bake>(text),
        _ => None,
    }
}

