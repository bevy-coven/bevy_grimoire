//! **Bake for release: every YAML file a game loads, as postcard, checked against the YAML** (GD-08, #370).
//!
//! `cauldron bake <root> --out <dir>` (and the `game.bake` command) reads each project file the registry names
//! ([`crate::reload::KINDS`]), each game entity file (`game/<name>.entities.yaml`) and each data file
//! `data.yaml` registers, and writes one binary per file plus a [`MANIFEST_FILE`] naming each file's kind, its
//! format version and **the SHA-256 of its YAML source**. A release build loads only the binaries
//! ([`crate::load`]); a development build loads the YAML.
//!
//! # What the bytes are
//!
//! **A value tree, not the type.** postcard is not self-describing, and a type that skips a field when it is
//! empty (`#[serde(skip_serializing_if = ..)]`: forty-odd fields in this crate, among them the map's and the
//! library's) writes fewer fields than it reads, so postcard of the type itself does not read back. The tree is
//! what the YAML reader sees ([`crate::yaml::to_json`]: "a YAML value is JSON with tags"), and a type is built
//! from it the way the YAML reader builds it, so **there is no attribute a type may carry that the bake cannot
//! honour**: a rename, a default, an unknown-field refusal and a tagged enum all go through the same
//! deserialiser. What the tree costs against postcard of the type is measured by `tests/bake_is_faster_than_yaml.rs`.
//!
//! # Every bake proves itself
//!
//! Each file's bytes are decoded again and compared with the value its YAML parses to, for the file's tree and,
//! for a kind whose type this crate owns, for the typed value built from each. **Any difference fails the bake,
//! naming the file, and nothing is written**: a half-written bake is the one outcome a release cannot be built
//! from.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::problem::{Detail, Location, Problem};

/// **The page that explains a refused bake.**
const DOCS: &str = "docs/guide/your-game.md";

/// **The manifest's file name**, in the bake's folder.
pub const MANIFEST_FILE: &str = "manifest.json";

/// **The folder of the bake that holds the binaries.**
pub const BINARIES_DIR: &str = "files";

/// **A baked file's extension.**
pub const BAKED_EXT: &str = "bin";

/// **The digits a baked binary's name is numbered with** (`files/00007.bin`). The sweep of a re-bake removes only names of this
/// shape, so it can never take a file this scheme did not make.
pub const INDEX_DIGITS: usize = 5;

/// **The version of the container a binary is written in**, read before anything else in the binary: a build
/// that reads another one refuses the bake instead of reading it as something it is not.
pub const BAKE_FORMAT: u32 = 1;

/// **How deep a baked tree may nest.** postcard decodes a [`Tree`] with no bound of its own and a list frame is two bytes, so a
/// crafted ~1 MiB binary nests half a million deep: the recursive decode and [`Tree::into_value`] would overflow the stack, which
/// is an abort no `Result` and no hash can stop — `baked_sha256` sits in the same untrusted manifest. This is the reader's limit,
/// and it is [`bevy_grimoire_migrate`]'s `ron_tree::MAX_DEPTH` value because both read the same shapes with the same recursion.
/// No project file comes near it: the deepest is a schema's few levels.
pub const MAX_TREE_DEPTH: usize = 64;

/// **The folder a game's entity files live in**, under the project root. Equal to `bevy_grimoire::entities::ENTITIES_DIR`,
/// which a test of that crate holds to it.
pub const ENTITIES_DIR: &str = "game";

/// **The suffix an entity file has.**
pub const ENTITIES_EXT: &str = ".entities.yaml";

/// **The kind an entity file is.** It has no row in the registry: the game's own types make it up.
pub const ENTITIES_KIND: &str = "entities";

/// **The kind a file `data.yaml` registers is.** The game owns its shape, so the bake keeps it as a tree.
pub const DATA_KIND: &str = "game_data";

/// **How deep the walk of a project goes**, in folders. A project is a handful deep; a tree this deep is a loop
/// or a mistake, and the walk names it instead of running on.
pub const WALK_MAX_DEPTH: usize = 24;

/// **The folders the walk never enters**: build output, tools' own folders and a bake's own output (a bake must
/// not bake itself). A folder whose name begins with `.` is skipped too.
const SKIPPED_DIRS: &[&str] = &["target", "node_modules", "baked"];

/// **One thing a YAML file holds, as bytes can carry it.** The shape [`serde_json::Value`] has, as an enum
/// postcard can write (`Value` itself needs a self-describing format).
///
/// **Its reader refuses past [`MAX_TREE_DEPTH`], before anything recurses into it.** postcard has no bound of its own and a list
/// frame is two bytes, so a crafted binary nests half a million deep; the derived deserialiser would recurse until the stack was
/// gone, which is an abort no `Result` can catch. The bound is the guard around each variant's own read (below), so the recursion
/// the reader does is bounded by construction, and `decode` measures the result before `into_value` as a second check.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Tree {
    Null,
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    Text(String),
    List(Vec<Tree>),
    Map(Vec<(String, Tree)>),
}

/// **The depth a reader is at**, as a thread-local rather than a parameter: a `Deserialize` impl has one signature, and threading a
/// depth through it would mean a second deserialiser for every `Vec` and tuple postcard builds on the way.
///
/// It is raised before a variant's payload is read and lowered after, so a sibling starts where its parent did. A guard struct with
/// a `Drop` restores it even when the read fails, so a refusal on the way out leaves the next read at its own level.
mod depth {
    use std::cell::Cell;

    thread_local! {
        static HERE: Cell<usize> = const { Cell::new(0) };
    }

    /// The depth the reader is at.
    pub fn here() -> usize {
        HERE.with(Cell::get)
    }

    /// **One level deeper for as long as this lives.**
    pub struct Deeper(usize);

    impl Deeper {
        pub fn enter() -> Deeper {
            Deeper(HERE.with(|h| {
                let was = h.get();
                h.set(was + 1);
                was
            }))
        }
    }

    impl Drop for Deeper {
        fn drop(&mut self) {
            HERE.with(|h| h.set(self.0));
        }
    }
}

impl<'de> Deserialize<'de> for Tree {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Tree, D::Error> {
        use serde::de::{Error as _, Visitor};

        /// **The variants, in the order the enum declares them**, which is the index postcard writes: a varint variant tag, and then
        /// the payload with no name and no length for a newtype.
        const NAMES: &[&str] = &["Null", "Bool", "Int", "Uint", "Float", "Text", "List", "Map"];

        struct TreeVisitor;

        impl<'de> Visitor<'de> for TreeVisitor {
            type Value = Tree;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a baked value tree")
            }

            fn visit_enum<A: serde::de::EnumAccess<'de>>(self, data: A) -> Result<Tree, A::Error> {
                use serde::de::VariantAccess as _;
                // **postcard writes the variant as a `u32` varint**, handed to `DeserializeSeed` through a `u32` deserialiser:
                // asking the seed for a name here fails on every binary, which is what a hand-written visitor got wrong once.
                let (tag, variant) = data.variant::<u32>()?;
                // **The guard is around the payload**, which is the only place a tree can nest: `Bool` cannot, `List` can.
                let _deeper = depth::Deeper::enter();
                if depth::here() > MAX_TREE_DEPTH {
                    return Err(A::Error::custom(format!("nested more than {MAX_TREE_DEPTH} deep")));
                }
                Ok(match tag as usize {
                    0 => {
                        variant.unit_variant()?;
                        Tree::Null
                    }
                    1 => Tree::Bool(variant.newtype_variant()?),
                    2 => Tree::Int(variant.newtype_variant()?),
                    3 => Tree::Uint(variant.newtype_variant()?),
                    4 => Tree::Float(variant.newtype_variant()?),
                    5 => Tree::Text(variant.newtype_variant()?),
                    6 => Tree::List(variant.newtype_variant()?),
                    7 => Tree::Map(variant.newtype_variant()?),
                    other => return Err(A::Error::custom(format!("`{other}` is not a baked value (the variants are {})", NAMES.len()))),
                })
            }
        }

        d.deserialize_enum("Tree", NAMES, TreeVisitor)
    }
}

impl Tree {
    /// **The tree a value is.** A number that is neither an `i64` nor a `u64` is a float.
    pub fn from_value(value: &Value) -> Tree {
        match value {
            Value::Null => Tree::Null,
            Value::Bool(b) => Tree::Bool(*b),
            Value::Number(n) => match (n.as_i64(), n.as_u64()) {
                (Some(i), _) => Tree::Int(i),
                (None, Some(u)) => Tree::Uint(u),
                (None, None) => Tree::Float(n.as_f64().unwrap_or_default()),
            },
            Value::String(s) => Tree::Text(s.clone()),
            Value::Array(items) => Tree::List(items.iter().map(Tree::from_value).collect()),
            Value::Object(map) => Tree::Map(map.iter().map(|(k, v)| (k.clone(), Tree::from_value(v))).collect()),
        }
    }

    /// **The value a tree is.**
    pub fn into_value(self) -> Value {
        match self {
            Tree::Null => Value::Null,
            Tree::Bool(b) => Value::Bool(b),
            Tree::Int(i) => Value::from(i),
            Tree::Uint(u) => Value::from(u),
            Tree::Float(f) => Value::from(f),
            Tree::Text(s) => Value::String(s),
            Tree::List(items) => Value::Array(items.into_iter().map(Tree::into_value).collect()),
            Tree::Map(entries) => Value::Object(entries.into_iter().map(|(k, v)| (k, v.into_value())).collect()),
        }
    }
}

/// **What a binary holds**: the container's version, the file it was made from and the tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Binary {
    format: u32,
    kind: String,
    file: String,
    tree: Tree,
}

/// **One file of a bake**, as the manifest lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// The YAML file, relative to the project root and written with `/`.
    pub file: String,
    /// Its kind: a [`crate::reload::FileKind::id`], [`ENTITIES_KIND`] or [`DATA_KIND`].
    pub kind: String,
    /// The `version` the file says it is, or `None` for a kind that carries none (an entity file).
    pub format_version: Option<u32>,
    /// **The SHA-256 of the YAML source**, in lower-case hex: what says a release was built from these bytes.
    pub source_sha256: String,
    /// The binary, relative to the bake's folder.
    pub baked: String,
    /// The SHA-256 of the binary, so a binary changed by hand is refused before it is read.
    pub baked_sha256: String,
}

/// **The manifest: what a bake holds.**
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// [`BAKE_FORMAT`].
    pub bake_format: u32,
    /// Every file, in path order.
    pub files: Vec<Entry>,
}

/// **What a bake wrote.**
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// How many files were baked.
    pub files: usize,
    /// How many bytes the binaries hold, together.
    pub bytes: usize,
    /// The manifest's path.
    pub manifest: PathBuf,
}

/// **The SHA-256 of `bytes`, as lower-case hex.**
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn problem(file: &str, what: &str, why: impl Into<String>, fix: &str) -> Problem {
    Problem::with(what.to_owned(), Detail::default().at(Location::file(file.to_owned())).because(why).fix(fix.to_owned()).docs(DOCS))
}

/// **The files a bake holds for the project at `root`**, as `(path relative to root, kind)`, in path order.
///
/// A file is in it when the registry names it ([`crate::reload_check::kind_of_path`]), it is an entity file, or
/// `data.yaml` registers it. A symlink is never followed, so a project cannot bake a file from outside itself.
#[allow(clippy::result_large_err)]
pub fn project_files(root: &Path) -> Result<Vec<(String, String)>, Problem> {
    let mut found: BTreeMap<String, String> = BTreeMap::new();
    let mut registries: Vec<String> = Vec::new();
    let mut pending: Vec<(PathBuf, String, usize)> = vec![(root.to_path_buf(), String::new(), 0)];
    while let Some((dir, prefix, depth)) = pending.pop() {
        if depth > WALK_MAX_DEPTH {
            return Err(problem(
                &prefix,
                &format!("{prefix} is more than {WALK_MAX_DEPTH} folders deep"),
                "a project this deep is a loop or a mistake",
                "move the project's files up, or remove the link that makes a loop",
            ));
        }
        let read = std::fs::read_dir(&dir).map_err(|e| {
            problem(&dir.display().to_string(), &format!("{} could not be listed", dir.display()), e.to_string(), "check that the folder is in the project and can be read")
        })?;
        for entry in read.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_symlink() {
                continue;
            }
            let rel = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
            if kind.is_dir() {
                if !name.starts_with('.') && !SKIPPED_DIRS.contains(&name.as_str()) {
                    pending.push((entry.path(), rel, depth + 1));
                }
            } else if kind.is_file()
                && rel.ends_with(".yaml")
                && let Some(kind_id) = kind_of(&rel)
            {
                if kind_id == "data_registry" {
                    registries.push(rel.clone());
                }
                found.insert(rel, kind_id.to_owned());
            }
        }
    }
    for registry in registries {
        let text = std::fs::read_to_string(root.join(&registry)).map_err(|e| {
            problem(&registry, &format!("{registry} could not be read"), e.to_string(), "check that the file can be read")
        })?;
        let files = crate::data_files::DataFiles::parse(&text)
            .map_err(|why| problem(&registry, &format!("{registry} does not read"), why, "fix the registry, then bake again"))?;
        let dir = registry.rsplit_once('/').map_or("", |(dir, _)| dir);
        for file in files.files {
            if let Some(rel) = join_inside(dir, &file.path)
                && rel.ends_with(".yaml")
                && root.join(&rel).is_file()
            {
                found.entry(rel).or_insert_with(|| DATA_KIND.to_owned());
            }
        }
    }
    Ok(found.into_iter().collect())
}

/// **The kind a project-relative YAML path is**, or `None` when the bake holds no such file.
fn kind_of(rel: &str) -> Option<&'static str> {
    if let Some(kind) = crate::reload_check::kind_of_path(rel) {
        return Some(kind.id);
    }
    let in_game = rel.strip_prefix(ENTITIES_DIR).and_then(|r| r.strip_prefix('/'))?;
    (!in_game.contains('/') && in_game.ends_with(ENTITIES_EXT)).then_some(ENTITIES_KIND)
}

/// **`path` taken relative to `dir`, as a path under the project root**, or `None` when it climbs out of it.
fn join_inside(dir: &str, path: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?),
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(parts.join("/"))
}

/// **One file, baked and proved**: the bytes, the version it says and the YAML's value.
struct Made {
    bytes: Vec<u8>,
    format_version: Option<u32>,
}

/// **Bake `text`, the YAML of the file `rel` of kind `kind`, and prove the bytes against it.**
#[allow(clippy::result_large_err)]
fn bake_text(kind: &str, rel: &str, text: &str) -> Result<Made, Problem> {
    let value = crate::yaml::to_json(text, rel)?;
    // **The kind's own reader first**, so a file the loader refuses is refused here, with its own sentence.
    if let Some(known) = crate::reload::KINDS.iter().find(|k| k.id == kind)
        && let crate::reload_check::Check::Refused(refusal) = crate::reload_check::check(known, rel, text)
    {
        return Err(problem(rel, &format!("{rel} does not read, so it cannot be baked"), refusal.sentence(), "fix the file, then bake again"));
    }
    let bytes = encode(kind, rel, &value)?;
    // **The proof**: the bytes, decoded, are the YAML's value, and the type built from them is the type built from the YAML.
    let decoded = decode(rel, &bytes)?;
    if decoded.1 != value {
        return Err(problem(
            rel,
            &format!("{rel} did not survive the bake"),
            "the value decoded from the binary is not the value the YAML reads to",
            "this is a defect in the bake: report it with the file",
        ));
    }
    typed_check(kind, rel, text, &decoded.1)?;
    let format_version = value.get("version").and_then(Value::as_u64).and_then(|v| u32::try_from(v).ok());
    Ok(Made { bytes, format_version })
}

/// **The bytes of a file's value**, in this build's container.
#[allow(clippy::result_large_err)]
pub fn encode(kind: &str, file: &str, value: &Value) -> Result<Vec<u8>, Problem> {
    // **A value past the reader's depth is refused here**, so this build never writes a binary its own reader would reject and the
    // refusal names the file rather than arriving as a stack overflow at load time.
    let depth = depth_of(value);
    if depth > MAX_TREE_DEPTH {
        return Err(problem(
            file,
            &format!("{file} nests {depth} deep and a baked file holds at most {MAX_TREE_DEPTH}"),
            "a value this deep is not a project file's shape",
            "report it: this is a defect in the type, not in the project",
        ));
    }
    let binary = Binary { format: BAKE_FORMAT, kind: kind.to_owned(), file: file.to_owned(), tree: Tree::from_value(value) };
    postcard::to_allocvec(&binary)
        .map_err(|e| problem(file, &format!("{file} did not encode"), e.to_string(), "this is a defect in the bake: report it"))
}

/// **How deep a value nests**, counted without recursing on the value's own stack: a list or a mapping adds one to its deepest
/// member's depth, and everything else is a leaf.
///
/// Iterative, because the thing being measured is exactly the shape that overflows a recursive walk. A cyclic value cannot exist
/// in `serde_json`, so this terminates.
fn depth_of(value: &Value) -> usize {
    let mut deepest = 1;
    let mut stack: Vec<(&Value, usize)> = vec![(value, 1)];
    while let Some((value, depth)) = stack.pop() {
        deepest = deepest.max(depth);
        if depth > MAX_TREE_DEPTH {
            // Past the limit there is nothing more to learn, and pushing half a million frames to say so is the fault itself.
            return depth;
        }
        match value {
            Value::Array(items) => stack.extend(items.iter().map(|v| (v, depth + 1))),
            Value::Object(map) => stack.extend(map.values().map(|v| (v, depth + 1))),
            _ => {}
        }
    }
    deepest
}

/// **Decode a binary into the kind it holds and its value**, refusing one made by another format.
#[allow(clippy::result_large_err)]
pub fn decode(file: &str, bytes: &[u8]) -> Result<(String, Value), Problem> {
    // **A binary that does not decode may be one nested past what this build reads** (a crafted manifest's), and postcard reports
    // its own `Serde` error without the visitor's sentence, so the limit is named here: the reader refuses rather than walking such
    // a tree until the stack is gone, and this is the sentence a person sees.
    let binary: Binary = postcard::from_bytes(bytes).map_err(|e| problem(
        file,
        &format!("the baked {file} does not decode, so it is not a tree this build reads (a value may nest at most {MAX_TREE_DEPTH} deep)"),
        e.to_string(),
        "bake the project again",
    ))?;
    if binary.format != BAKE_FORMAT {
        return Err(problem(
            file,
            &format!("the baked {file} is format {}, and this build reads {BAKE_FORMAT}", binary.format),
            "the bake was made by another build",
            "bake the project again with this build",
        ));
    }
    if binary.file != file {
        return Err(problem(
            file,
            &format!("the baked {file} holds {}", binary.file),
            "a binary was moved or renamed",
            "bake the project again",
        ));
    }
    Ok((binary.kind, binary.tree.into_value()))
}

/// **A type built from the YAML and the same type built from the decoded value are one value.**
#[allow(clippy::result_large_err)]
fn typed_check(kind: &str, rel: &str, text: &str, decoded: &Value) -> Result<(), Problem> {
    use crate::{
        composition::Compositions, data_files::DataFiles, destructible::{Bake, Marks}, family::Family, heightfield::Heightfield,
        kits::Kits, library::Library, map::Map, note_agent::AgentSettings, notes::Notes, plan::Plan, play::PlayConfig,
        policy::Policy, references::References, relations::Relations, repetition::RepetitionRules, rigs::Rigs, rules::Rules,
        standards::Standards, vocab::Vocabularies, volume::Volume, water::{WaterField, WaterPlan},
    };
    match kind {
        "kits" => same::<Kits>(rel, text, decoded, Kits::parse),
        "vocab" => same::<Vocabularies>(rel, text, decoded, Vocabularies::parse),
        "library" => same::<Library>(rel, text, decoded, Library::parse),
        "policy" => same::<Policy>(rel, text, decoded, Policy::parse),
        "rules" => same::<Rules>(rel, text, decoded, Rules::parse),
        "compositions" => same::<Compositions>(rel, text, decoded, Compositions::parse),
        "standards" => same::<Standards>(rel, text, decoded, Standards::parse),
        "repetition" => same::<RepetitionRules>(rel, text, decoded, RepetitionRules::parse),
        "rigs" => same::<Rigs>(rel, text, decoded, Rigs::parse),
        "notes" => same::<Notes>(rel, text, decoded, Notes::parse),
        "plan" => same::<Plan>(rel, text, decoded, Plan::parse),
        "references" => same::<References>(rel, text, decoded, References::parse),
        "relations" => same::<Relations>(rel, text, decoded, Relations::parse),
        "map" => same::<Map>(rel, text, decoded, Map::parse),
        "terrain" => same::<Heightfield>(rel, text, decoded, Heightfield::parse),
        "water_plan" => same::<WaterPlan>(rel, text, decoded, WaterPlan::parse),
        "water_field" => same::<WaterField>(rel, text, decoded, WaterField::parse),
        "volume" => same::<Volume>(rel, text, decoded, Volume::parse),
        "data_registry" => same::<DataFiles>(rel, text, decoded, DataFiles::parse),
        "family_list" => same::<Family>(rel, text, decoded, Family::parse),
        "agent" => same::<AgentSettings>(rel, text, decoded, AgentSettings::parse),
        "play" => same::<PlayConfig>(rel, text, decoded, PlayConfig::parse),
        "destructible_marks" => same::<Marks>(rel, text, decoded, Marks::parse),
        "destructible_bake" => same::<crate::destructible::Bake>(rel, text, decoded, Bake::parse),
        // A kind another crate owns, the entity files and a game's own data: the tree is the whole value.
        _ => Ok(()),
    }
}

/// **`T` from the YAML (through the type's own `parse`) and `T` from the decoded value, as the same JSON.**
#[allow(clippy::result_large_err)]
fn same<T: Serialize + DeserializeOwned>(
    rel: &str,
    text: &str,
    decoded: &Value,
    parse: impl Fn(&str) -> Result<T, String>,
) -> Result<(), Problem> {
    let from_yaml = parse(text).map_err(|why| problem(rel, &format!("{rel} does not read, so it cannot be baked"), why, "fix the file, then bake again"))?;
    let from_bytes: T = serde_json::from_value(decoded.clone()).map_err(|e| {
        problem(rel, &format!("{rel} did not survive the bake"), format!("the decoded value does not read as the file's type: {e}"), "this is a defect in the bake: report it with the file")
    })?;
    let (a, b) = (serde_json::to_value(&from_yaml), serde_json::to_value(&from_bytes));
    match (a, b) {
        (Ok(a), Ok(b)) if a == b => Ok(()),
        _ => Err(problem(
            rel,
            &format!("{rel} did not survive the bake"),
            "the value built from the binary is not the value built from the YAML",
            "this is a defect in the bake: report it with the file",
        )),
    }
}

/// **Bake the project at `root` into `out`.** The whole bake is made and proved in memory first; nothing is written
/// unless every file survived, so a failed bake leaves `out` as it was.
#[allow(clippy::result_large_err)]
pub fn bake(root: &Path, out: &Path) -> Result<Report, Problem> {
    let mut made: Vec<(Entry, Vec<u8>)> = Vec::new();
    for (index, (rel, kind)) in project_files(root)?.into_iter().enumerate() {
        let source = std::fs::read(root.join(&rel))
            .map_err(|e| problem(&rel, &format!("{rel} could not be read"), e.to_string(), "check that the file can be read"))?;
        let text = String::from_utf8(source.clone())
            .map_err(|e| problem(&rel, &format!("{rel} is not UTF-8"), e.to_string(), "save the file as UTF-8"))?;
        let baked = bake_text(&kind, &rel, &text)?;
        let name = format!("{BINARIES_DIR}/{index:0width$}.{BAKED_EXT}", width = INDEX_DIGITS);
        made.push((
            Entry {
                file: rel,
                kind,
                format_version: baked.format_version,
                source_sha256: sha256_hex(&source),
                baked: name,
                baked_sha256: sha256_hex(&baked.bytes),
            },
            baked.bytes,
        ));
    }
    let io = |what: &str, path: &Path, e: std::io::Error| {
        problem(&path.display().to_string(), &format!("{what} {}", path.display()), e.to_string(), "check that the folder can be written")
    };
    std::fs::create_dir_all(out.join(BINARIES_DIR)).map_err(|e| io("could not make", &out.join(BINARIES_DIR), e))?;
    // **A file of an earlier bake that this one does not have is removed**, so the folder is this bake and nothing older.
    //
    // Only the names **this scheme produces** — `{n:05}.bin` — are swept. It used to remove every `*.bin` under `files/`, which
    // is the whole folder's contents: a person's own file beside the bake, or one another tool numbered differently, went with a
    // re-bake. A name that matches neither this scheme nor the previous manifest's list is not this bake's to delete.
    if let Ok(old) = std::fs::read_dir(out.join(BINARIES_DIR)) {
        for entry in old.filter_map(Result::ok) {
            let path = entry.path();
            let ours = path.file_stem().and_then(|s| s.to_str()).is_some_and(|stem| {
                stem.len() == INDEX_DIGITS && stem.bytes().all(|b| b.is_ascii_digit())
            }) && path.extension().is_some_and(|e| e == BAKED_EXT);
            if ours {
                std::fs::remove_file(&path).map_err(|e| io("could not remove", &path, e))?;
            }
        }
    }
    let mut bytes = 0;
    for (entry, data) in &made {
        bytes += data.len();
        let path = out.join(&entry.baked);
        crate::atomic_file::write_cache(&path, data).map_err(|e| io("could not write", &path, e))?;
    }
    let manifest = Manifest { bake_format: BAKE_FORMAT, files: made.into_iter().map(|(entry, _)| entry).collect() };
    let manifest_path = out.join(MANIFEST_FILE);
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|e| problem(MANIFEST_FILE, "the manifest did not encode", e.to_string(), "this is a defect in the bake: report it"))?;
    crate::atomic_file::write_cache(&manifest_path, format!("{text}\n")).map_err(|e| io("could not write", &manifest_path, e))?;
    Ok(Report { files: manifest.files.len(), bytes, manifest: manifest_path })
}

/// **Check a bake against the YAML it was made from**, file by file: the binary is the one the manifest hashed, it decodes,
/// and what it decodes to is what the YAML reads to now. The first file that is not so is named.
///
/// This is the comparison a bake runs on itself before it writes ([`bake`]), run again on what is on disk: a binary changed by
/// hand fails here, naming the file.
#[allow(clippy::result_large_err)]
pub fn verify(root: &Path, dir: &Path) -> Result<usize, Problem> {
    let baked = Baked::open(dir, root)?;
    let mut checked = 0;
    for entry in baked.files() {
        let decoded = baked.value(&entry.file)?;
        let text = std::fs::read_to_string(root.join(&entry.file)).map_err(|e| {
            problem(&entry.file, &format!("{} could not be read", entry.file), e.to_string(), "check that the file can be read")
        })?;
        let value = crate::yaml::to_json(&text, &entry.file)?;
        if decoded != value {
            return Err(problem(
                &entry.file,
                &format!("the baked {} is not the value its YAML reads to", entry.file),
                "the binary and the YAML disagree",
                "bake the project again",
            ));
        }
        checked += 1;
    }
    Ok(checked)
}

/// **A bake, opened for a release to load from.**
#[derive(Clone, Debug)]
pub struct Baked {
    dir: PathBuf,
    root: PathBuf,
    entries: BTreeMap<String, Entry>,
}

impl Baked {
    /// **Open the bake in `dir`, for the project at `root`**: the manifest must read and be of this build's format.
    #[allow(clippy::result_large_err)]
    pub fn open(dir: &Path, root: &Path) -> Result<Baked, Problem> {
        let path = dir.join(MANIFEST_FILE);
        let shown = path.display().to_string();
        let text = std::fs::read_to_string(&path)
            .map_err(|e| problem(&shown, &format!("{shown} could not be read"), e.to_string(), "bake the project (`cauldron bake <root> --out <dir>`) and ship the folder"))?;
        let manifest: Manifest = serde_json::from_str(&text)
            .map_err(|e| problem(&shown, &format!("{shown} does not read"), e.to_string(), "bake the project again"))?;
        if manifest.bake_format != BAKE_FORMAT {
            return Err(problem(
                &shown,
                &format!("{shown} is bake format {}, and this build reads {BAKE_FORMAT}", manifest.bake_format),
                "the bake was made by another build",
                "bake the project again with this build",
            ));
        }
        // **Every entry stays inside the bake and the project.** A manifest is not trusted: `entry.baked` is joined into the bake
        // folder and `entry.file` is joined into the project, so `../../../../etc/passwd` or `/etc/passwd` read a file on the
        // machine — and a matching `baked_sha256` in the same manifest let it decode. The write side always had `join_inside`;
        // the read side checks the same thing, once, here.
        for entry in &manifest.files {
            if join_inside("", &entry.file).as_deref() != Some(entry.file.as_str()) || join_inside(BINARIES_DIR, &entry.baked).is_none() {
                return Err(problem(
                    &entry.file,
                    &format!("{shown} names `{}`, which is outside the bake", entry.file),
                    format!("its binary is `{}`, and a bake reads only its own folder and the project it was made from", entry.baked),
                    "bake the project again (`cauldron bake <root> --out <dir>`) and ship that folder",
                ));
            }
        }
        Ok(Baked { dir: dir.to_path_buf(), root: root.to_path_buf(), entries: manifest.files.into_iter().map(|e| (e.file.clone(), e)).collect() })
    }

    /// **Refuse a bake whose YAML has changed since**, naming the first file: a release never quietly runs old data.
    ///
    /// For each file the manifest lists whose YAML is at the project's root, the YAML's SHA-256 must be the one the bake recorded; a YAML
    /// file the project holds that the bake does not list is refused too. A release that ships without its YAML has nothing
    /// to compare and passes: what it holds is what [`Self::tree`] checks the binaries against their own hashes for.
    #[allow(clippy::result_large_err)]
    pub fn refuse_if_stale(&self) -> Result<(), Problem> {
        let root = self.root.as_path();
        for (rel, entry) in &self.entries {
            // **A source the project no longer has is not "nothing to compare"**: `rm` would otherwise bypass the check the whole
            // function exists for. A release ships without its YAML and passes on purpose (see the doc above), so this only refuses
            // when the project *is* there and the file it lists is not — which is a file someone deleted after the bake.
            let Ok(source) = std::fs::read(root.join(rel)) else {
                if root.is_dir() {
                    return Err(problem(
                        rel,
                        &format!("{rel} is in the bake and is not in the project"),
                        "the file the bake was made from was removed, so there is nothing to compare it with",
                        "restore the file, or bake the project again without it",
                    ));
                }
                continue;
            };
            if sha256_hex(&source) != entry.source_sha256 {
                return Err(problem(
                    rel,
                    &format!("{rel} has changed since the game was baked"),
                    "the baked data was made from other bytes than the YAML there now",
                    "bake the project again (`cauldron bake <root> --out <dir>`) before this build starts",
                ));
            }
        }
        if root.is_dir() {
            for (rel, _) in project_files(root)? {
                if !self.entries.contains_key(&rel) {
                    return Err(problem(
                        &rel,
                        &format!("{rel} is not in the bake"),
                        "the file was added after the game was baked",
                        "bake the project again (`cauldron bake <root> --out <dir>`) before this build starts",
                    ));
                }
            }
        }
        Ok(())
    }

    /// **The value of a baked file** (`rel` relative to the project root), after the binary is checked against its own hash.
    #[allow(clippy::result_large_err)]
    pub fn value(&self, rel: &str) -> Result<Value, Problem> {
        let entry = self.entries.get(rel).ok_or_else(|| {
            problem(rel, &format!("{rel} is not in the bake"), "this build loads only baked files", "bake the project again with the file in it")
        })?;
        // **The binary is named relative to the bake's own folder**, and a name that climbs out is refused here as it is in
        // `open`: this is the read that would touch the filesystem.
        // **`entry.baked` already begins with `files/`**, which is how the writer names it, so it is checked as a path *inside*
        // the bake rather than joined into `files/` again: `join_inside(BINARIES_DIR, ..)` would make `files/files/00000.bin`.
        let inside = join_inside("", &entry.baked).filter(|p| p.starts_with(BINARIES_DIR)).ok_or_else(|| {
            problem(rel, &format!("the baked {rel} is named outside the bake"), format!("`{}` climbs out of {BINARIES_DIR}", entry.baked), "bake the project again")
        })?;
        let path = self.dir.join(&inside);
        let bytes = std::fs::read(&path).map_err(|e| {
            problem(rel, &format!("the baked {rel} could not be read"), format!("{}: {e}", path.display()), "bake the project again")
        })?;
        if sha256_hex(&bytes) != entry.baked_sha256 {
            return Err(problem(
                rel,
                &format!("the baked {rel} is not the file the bake made"),
                format!("{} has been changed since", path.display()),
                "bake the project again",
            ));
        }
        decode(rel, &bytes).map(|(_, value)| value)
    }

    /// **The path of a file relative to the project root**, written with `/`, when it is under it.
    pub fn rel_of(&self, path: &Path) -> Option<String> {
        let inside = if path.is_absolute() || self.root.as_os_str() != "." {
            path.strip_prefix(&self.root).ok()?
        } else {
            path.strip_prefix(".").unwrap_or(path)
        };
        let parts: Vec<String> = inside.components().filter_map(|c| c.as_os_str().to_str().map(str::to_owned)).collect();
        (!parts.is_empty()).then(|| parts.join("/"))
    }

    /// **Whether the bake holds `rel`.**
    pub fn has(&self, rel: &str) -> bool {
        self.entries.contains_key(rel)
    }

    /// **The files this bake lists**, in path order.
    pub fn files(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }
}
