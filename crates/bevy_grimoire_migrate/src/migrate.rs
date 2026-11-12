//! **Moving a project's files from RON to YAML, losslessly** (GD-03, #365).
//!
//! A project used to be RON (`kits.yaml`, `maps/town.map.yaml`, `editor.yaml`, …). [`migrate`] turns one
//! into YAML, and [`leftover`] is the refusal an open raises when a project has not been converted.
//!
//! # Every kind, one command
//!
//! Nothing reads RON now, so [`migrate_as`] converts every file a kind names: a kind whose type this crate has is read as
//! that type, any other (the editor's, the checks', a family crate's, a game's own data) is converted by value
//! ([`convert_value`]). [`leftover`] refuses an open on any of them.
//!
//! # Two rules, and neither is a suggestion
//!
//! - **Lossless.** Each file is read with the type's own RON reader, written with the one YAML writer
//!   ([`yaml::to_string`]), **its comments placed in that text**, and then **the commented text is
//!   read back with that same type's YAML reader** ([`yaml::from_str`]). A read that fails, or a
//!   value that writes differently than it read, stops the whole migration, names the file, and **writes
//!   nothing at all** — not that file, not the ones before it. A half-migrated project is the one outcome
//!   a person cannot repair by hand.
//! - **Loud.** A RON comment is carried over as a YAML comment above the same key wherever its place
//!   can be found. One that cannot be placed is listed in the [`Report`], never dropped quietly. A file
//!   whose kind this crate has no type for — `checks.ron` (the checks crate), `editor.yaml`, `family/*.ron`
//!   and a kit's `forest/people/scripts/trees` files (the editor) — **stops the whole migration**
//!   and is named: guessing at a value it cannot read is how a file loses a field.
//!
//! **A file no kind's pattern names is not walked at all.** `game_data` states no pattern (it is "the
//! files a game registers in `data.yaml`"), so a game's own tuning files are neither converted nor
//! refused: they stay RON, untouched, and this module has no opinion about them. Giving them a pattern
//! belongs to the crate that declares their type, in the slice that moves them.
//!
//! # The old files are moved, not deleted
//!
//! Every converted file is moved to [`ARCHIVE_DIR`] (`<root>/.dark_factory/migrated-ron/`), keeping its
//! path under the project, so a person can diff what the migration did and go back if they must. The
//! archive is inside the project, where the project's own `.gitignore` already keeps
//! `.dark_factory/schema/` — so it is one folder a person can find and one they can delete.
//!
//! # Why the check is typed, and why it cannot be a text comparison
//!
//! RON and YAML do not agree about how a value is spelled: `Some("x")` is a RON constructor and a YAML
//! plain scalar; a RON enum variant is `Near(to: "desk@1")` and a YAML tag (`!Near`). So comparing the
//! two texts, or even their scalar leaves, reports a difference for every file that holds an `Option`
//! or an enum — measured: `ron::Value` reads `(version: 1)` back as a *different* value once it has
//! been through YAML. The value, read by the type, is the only thing that can be compared, and that is
//! what [`convert`] does.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

use bevy_grimoire_core::problem::{Detail, Location, Problem};
use bevy_grimoire_core::reload::{Base, FileKind, KINDS};
use bevy_grimoire_core::{composition::Compositions, destructible::{Bake, Marks}, family::Family, heightfield::Heightfield,
    kits::Kits, library::Library, map::Map, note_agent::AgentSettings, notes::Notes, plan::Plan,
    play::PlayConfig, policy::Policy, references::References, relations::Relations,
    repetition::RepetitionRules, rigs::Rigs, rules::Rules, standards::Standards, vocab::Vocabularies,
    volume::Volume, water::{WaterField, WaterPlan}, data_files::DataFiles, yaml};

/// **The extension a project file carries now.** A project file's name ends in this, where it used to
/// end in [`RON_EXT`].
pub const YAML_EXT: &str = "yaml";

/// **The extension a project used to be written in**, and the one [`migrate`] converts away from.
pub const RON_EXT: &str = "ron";

/// **Where the old files go**, under the project root: `<root>/.dark_factory/migrated-ron/`.
pub const ARCHIVE_DIR: &str = ".dark_factory/migrated-ron";

/// **The shape of the line that fixes a project still holding RON files**, for a caller that has no
/// program name of its own — a test, or a library whose binary somebody else names.
///
/// **The program's own name is not this crate's to spell.** It is `bevy_cauldron`, and
/// `tests/engine_free.rs` refuses that identifier in this crate's sources; a refusal naming a command a
/// person cannot run is worse than one naming no command at all. So every entry point takes the line as
/// an argument ([`migrate`], [`migrate_as`], [`leftover`]), the editor passes `args::PROGRAM` — the
/// name of the binary it is built as — and this constant is only what an engine-free caller gets.
pub const COMMAND: &str = "<the editor> migrate <project folder>";

/// The page of the guide that explains the move: "Moving a project to YAML" in it.
const DOCS: &str = "docs/guide/your-game.md";

/// **Where a project's own files live**, under the game's root: what a [`Base::Grimoire`] pattern is
/// relative to. The same folder `project_data::migrate_legacy_data_dir` settled on.
pub const GRIMOIRE_DIR: &str = "assets/grimoire";

/// **The folders never walked**, by name: the ones no project file lives under. A walk that descended
/// into `target/` would migrate a build. [`ARCHIVE_DIR`] is not here — it is a path, and a walk compares
/// one folder name — it is skipped by [`is_archived`] instead.
const SKIP_DIRS: [&str; 3] = [".git", "target", ".live"];

/// **Whether a project-relative path is already inside the archive** this module writes, so a second run
/// never converts its own output back.
fn is_archived(rel: &str) -> bool {
    rel.starts_with(&format!("{ARCHIVE_DIR}/"))
}

/// **One file converted**: the text to write, and what became of its comments.
///
/// The text is the *checked* text — read back as its type and compared with the RON value by [`convert`] —
/// so the caller writes bytes that were verified rather than bytes assembled after the check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Converted {
    /// The file as the project named it before, with `/`: filled in by [`migrate`], empty in [`convert`].
    pub rom: String,
    /// The file as it is named now: filled in by [`migrate`], empty in [`convert`].
    pub yaml: String,
    /// The [`FileKind::id`] it is: filled in by [`migrate`]'s caller path, empty in [`convert`].
    pub kind: &'static str,
    /// How many RON comments were carried into the YAML file.
    pub comments_carried: usize,
    /// The RON comments that had no place in it, listed so none is dropped quietly.
    pub unplaced: Vec<Unplaced>,
    /// **The YAML text, comments and all**, as it will be written and as it was read back.
    pub text: String,
}

/// **A RON comment the migration could not put above its key**, listed so it is never dropped quietly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unplaced {
    /// The file the comment was in, as the project named it.
    pub file: String,
    /// The comment's text, exactly as the RON held it, without the `//`.
    pub comment: String,
    /// Why it has no place in the YAML file: the key it preceded is not one the writer emits, or it
    /// stood outside any key (a file header), which YAML has no node to hang it from.
    pub why: String,
}

/// **What a migration did**, which is also what `cauldron migrate` prints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// Every file converted, in the order it was walked.
    pub converted: Vec<Converted>,
    /// Every RON comment that had no place in its YAML file.
    pub unplaced: Vec<Unplaced>,
    /// Where the old files were moved to, relative to the project root ([`ARCHIVE_DIR`]).
    pub archived_at: String,
}

impl Report {
    /// **The report as a person reads it**, one line per file, then the unplaced comments, then where
    /// the old ones went.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for c in &self.converted {
            out.push(format!(
                "{} -> {} ({}), {} comment(s) carried",
                c.rom, c.yaml, c.kind, c.comments_carried
            ));
        }
        for u in &self.unplaced {
            out.push(format!("{}: comment not placed ({}): {}", u.file, u.why, u.comment));
        }
        if !self.converted.is_empty() {
            out.push(format!("{} file(s); the RON originals are in {}", self.converted.len(), self.archived_at));
        }
        out
    }
}

/// **The path a project file is written at now**, from the path it had: `kits.yaml` → `kits.yaml`.
/// `None` for a path that does not end in [`RON_EXT`], which is not a project file this converts.
pub fn yaml_path(rom: &Path) -> Option<PathBuf> {
    let name = rom.file_name()?.to_str()?;
    let stem = name.strip_suffix(&format!(".{RON_EXT}"))?;
    Some(rom.with_file_name(format!("{stem}.{YAML_EXT}")))
}

/// **Every RON file under `root` that a [`FileKind`] names**, as `(path on disk, path relative to the
/// root with `/`, the kind)`. Sorted, so a report and a refusal are the same on every run.
///
/// A `.ron` file no kind names (a game's own `oplog` neighbour, a stray note) is not a project file and
/// is left alone: this walks what the registry states, not every file that happens to be RON.
#[allow(clippy::result_large_err)]
pub fn plan(root: &Path) -> Result<Vec<(PathBuf, String, &'static FileKind)>, Problem> {
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    walk(root, root, &mut out, &mut Vec::new(), &mut skipped)?;
    // **A folder that cannot be listed is reported, never fatal.** [`leftover`] is what an *open* calls, and
    // a project whose one unreadable subfolder (a key-only backup, a folder from another account) refused
    // the open would be a project nobody can open again. The list still answers what it could read, and the
    // folders it could not are said out loud on stderr rather than swallowed.
    for (dir, why) in &skipped {
        eprintln!("migrate: cannot read {} ({why}), so its files are not part of this plan", dir.display());
    }
    out.sort();
    Ok(out
        .into_iter()
        .filter_map(|rel| {
            if !rel.ends_with(&format!(".{RON_EXT}")) || is_archived(&rel) {
                return None;
            }
            let kind = kind_of_rel(&rel)?;
            Some((root.join(&rel), rel, kind))
        })
        .collect())
}

/// The kind of a project file named relative to the project root, which is where both bases hang from.
///
/// **A kind matches its old spelling too** (GD-03, #365). The table states a kind's path as it is today
/// (`play.yaml`), so a file still written the way it was before the cutover (`play.ron`) matches nothing
/// by its pattern alone — and the refusal an open raises, and the migration that converts it, both have
/// to find exactly those files. So a second spelling is tried: the pattern with its `.yaml` ending put
/// back to `.ron`, which is the only extension the cutover moved.
fn kind_of_rel(rel: &str) -> Option<&'static FileKind> {
    // `Base::Grimoire` paths are relative to `assets/grimoire`; a root file to the root itself.
    KINDS.iter().find(|k| {
        let under = match k.base {
            Base::Grimoire => rel.strip_prefix(GRIMOIRE_DIR).and_then(|rest| rest.strip_prefix('/')),
            Base::Root => Some(rel),
        };
        let before = ron_spelling_of(k.pattern);
        under.is_some_and(|rest| k.matches(rest) || bevy_grimoire_core::reload::matches_pattern(&before, rest))
    })
}

/// **A kind's path pattern as the cutover left it**: `play.yaml` → `play.ron`, `maps/<name>.map.yaml` →
/// `maps/<name>.map.yaml`.
///
/// A file the cutover has not converted yet is named the way it always was, so a pattern that only says
/// `.yaml` matches nothing on that project. A pattern holding no `.yaml` (an `oplog.jsonl`, a
/// `references/<file>`) is answered unchanged, so it still matches only what it says.
fn ron_spelling_of(pattern: &str) -> String {
    match pattern.rsplit_once('.') {
        Some((stem, YAML_EXT)) => format!("{stem}.{RON_EXT}"),
        _ => pattern.to_owned(),
    }
}

/// **Every file under `dir`, as a path relative to `root` with `/`**, depth-first, skipping [`SKIP_DIRS`].
///
/// **The walk never leaves the project.** A symlinked folder — or a symlinked file — is skipped outright:
/// following one would convert a RON file *outside* the project, write its YAML out there and move its
/// original out of that folder into the archive, which is a migration of somebody else's directory. A
/// project that really wants a file from elsewhere copies it in.
#[allow(clippy::result_large_err)]
fn walk(
    root: &Path,
    dir: &Path,
    out: &mut Vec<String>,
    seen: &mut Vec<PathBuf>,
    skipped: &mut Vec<(PathBuf, std::io::Error)>,
) -> Result<(), Problem> {
    // A symlinked directory that points at one of its own parents would walk forever.
    let real = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    if seen.contains(&real) {
        return Ok(());
    }
    seen.push(real);
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            // The project's own root failing is the caller's business; a subfolder is not.
            if dir == root {
                return Err(io_problem(dir, e));
            }
            skipped.push((dir.to_path_buf(), e));
            return Ok(());
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                // One unreadable name does not make the folder unreadable.
                skipped.push((dir.to_path_buf(), e));
                continue;
            }
        };
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        // **`symlink_metadata` reads the link, not what it points at**, so a link is recognised before it
        // is followed — and it is never followed.
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            walk(root, &path, out, seen, skipped)?;
        } else if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

/// One filesystem refusal, naming the folder and why.
fn io_problem(path: &Path, e: std::io::Error) -> Problem {
    Problem::with(
        "a project folder did not read".to_owned(),
        Detail::default()
            .at(Location::file(path.display().to_string()))
            .because(e.to_string())
            .fix("check that the folder is in the project and can be read".to_owned())
            .docs(DOCS),
    )
}

/// **The YAML text `text` becomes, its comments placed, or a refusal that says the value would change.**
///
/// The one conversion, and the one place a comment is allowed to touch a file:
///
/// 1. read `text` with the type's RON reader;
/// 2. write it with the one YAML writer;
/// 3. **place the RON comments in that text** ([`carry_comments`]);
/// 4. **read the commented text back with the type's YAML reader and compare it with the RON value** — so
///    the text that goes to disk, comments and all, is the text that was verified, and a comment that
///    landed inside a value is caught here rather than by a person months later.
///
/// A difference at either step refuses, and the caller writes nothing. `file` is only what a refusal
/// names.
#[allow(clippy::result_large_err)]
pub fn convert<T>(text: &str, file: &str, command: &str) -> Result<Converted, Problem>
where
    T: Serialize + DeserializeOwned,
{
    convert_with::<T>(text, file, command, |_| {})
}

/// **[`convert`], with a change made to the value between the read and the write**: the registry's `path: mob.ron` entries become
/// `mob.yaml` here, so the YAML the registry is written as names the YAML its files are written as. The faithfulness check
/// compares what was written with what was read *after* the change.
#[allow(clippy::result_large_err)]
fn convert_with<T>(text: &str, file: &str, command: &str, fix: impl Fn(&mut T)) -> Result<Converted, Problem>
where
    T: Serialize + DeserializeOwned,
{
    let mut value: T = ron::from_str(text).map_err(|e| {
        Problem::with(
            format!("{file} did not read as RON, so nothing was migrated"),
            Detail::default()
                .at(Location::file(file.to_owned()))
                .because(e.to_string())
                .fix(format!("a file this old is read by the build that wrote it; `{command}` refuses rather than guessing"))
                .docs(DOCS),
        )
    })?;
    fix(&mut value);
    let plain = yaml::to_string(&value).map_err(|p| {
        Problem::with(format!("{file} did not write as YAML, so nothing was migrated"), p.detail())
    })?;
    // **The comments are placed before the check, not after it**, so the bytes that land on disk are the
    // bytes that were compared.
    let (written, comments_carried, unplaced) = carry_comments(text, &plain, file);
    // **Reading the written text is the check a comment cannot pass.** A comment put inside a `|-` block
    // scalar's body becomes part of the string, so this read either refuses the file or answers a value
    // the next step compares.
    let back: T = yaml::from_str(&written, file).map_err(|p| {
        Problem::with(
            format!("{file} wrote as YAML and did not read back, so nothing was migrated"),
            p.detail(),
        )
    })?;
    // **The faithfulness check is a second write, compared with the first.** `PartialEq` is the wrong
    // question for a float: a `NaN` writes out as `.nan` and reads back as the same not-a-number while
    // comparing unequal to itself, so a `!=` would refuse a file that loses nothing. The one writer's own
    // output is deterministic and in the type's field order, so two equal texts mean the same value —
    // and a field lost, a number re-spelled or a tag changed shows up as two different texts.
    let again = yaml::to_string(&back).map_err(|p| {
        Problem::with(format!("{file} did not write again as YAML, so nothing was migrated"), p.detail())
    })?;
    if again != plain {
        return Err(Problem::with(
            format!("{file} would change value if it were written as YAML, so nothing was migrated"),
            Detail::default()
                .at(Location::file(file.to_owned()))
                .because(first_difference(&plain, &again))
                .fix("a file this build cannot round-trip is left as it is; report it, do not hand-edit it".to_owned())
                .docs(DOCS),
        ));
    }
    Ok(Converted { rom: String::new(), yaml: String::new(), kind: "", comments_carried, unplaced, text: written })
}

/// **Where two written texts first disagree**, as a sentence naming the line.
///
/// Both texts are the one writer's output for the same type, so they agree line for line until the field
/// that differs — which means the first line that differs names that field, and nothing here ever slices
/// a string at a byte offset (a window inside a multi-byte character panics).
fn first_difference(before: &str, after: &str) -> String {
    for (n, (a, b)) in before.lines().zip(after.lines()).enumerate() {
        if a != b {
            return format!("line {}: the RON holds `{}` and the YAML reads `{}`", n + 1, a.trim(), b.trim());
        }
    }
    format!(
        "the two texts have {} and {} lines, so a value is missing",
        before.lines().count(),
        after.lines().count()
    )
}

/// **Convert a file by value**: RON into a [`crate::ron_tree::Tree`], written as YAML, the YAML read back and compared.
///
/// For a kind whose type is another crate's, and for a game's own data file (which has no type here at all). A difference
/// refuses, naming the file, and the caller writes nothing.
#[allow(clippy::result_large_err)]
pub fn convert_value(text: &str, file: &str, command: &str) -> Result<Converted, Problem> {
    let tree = crate::ron_tree::parse(text).map_err(|e| {
        Problem::with(
            format!("{file} did not read as RON, so nothing was migrated"),
            Detail::default()
                .at(Location::file(file.to_owned()))
                .because(e)
                .fix(format!("a file this old is read by the build that wrote it; `{command}` refuses rather than guessing"))
                .docs(DOCS),
        )
    })?;
    let plain = yaml::to_string(&tree.to_yaml())
        .map_err(|p| Problem::with(format!("{file} did not write as YAML, so nothing was migrated"), p.detail()))?;
    let (written, comments_carried, unplaced) = carry_comments(text, &plain, file);
    let back = yaml::to_json(&written, file)
        .map_err(|p| Problem::with(format!("{file} wrote as YAML and did not read back, so nothing was migrated"), p.detail()))?;
    if back != tree.to_json() {
        return Err(Problem::with(
            format!("{file} would change value if it were written as YAML, so nothing was migrated"),
            Detail::default()
                .at(Location::file(file.to_owned()))
                .because("the YAML reads to a different value than the RON held".to_owned())
                .fix("the file is not changed; report it with the file".to_owned())
                .docs(DOCS),
        ));
    }
    Ok(Converted { rom: String::new(), yaml: String::new(), kind: "", comments_carried, unplaced, text: written })
}

/// **The YAML text a project file of `kind` becomes**, or a refusal naming the file.
///
/// One arm per kind this crate owns a type for. A kind whose type lives above this crate is refused here
/// rather than guessed at: a mapper's forest, an editor settings file and a game's own data file are read
/// by the crate that declares them, and only that crate can say whether the YAML holds the same value.
#[allow(clippy::result_large_err)]
fn write_kind(kind: &FileKind, text: &str, file: &str, command: &str) -> Result<Converted, Problem> {
    match kind.id {
        "kits" => convert::<Kits>(text, file, command),
        "vocab" => convert::<Vocabularies>(text, file, command),
        "library" => convert::<Library>(text, file, command),
        "policy" => convert::<Policy>(text, file, command),
        "rules" => convert::<Rules>(text, file, command),
        "compositions" => convert::<Compositions>(text, file, command),
        "standards" => convert::<Standards>(text, file, command),
        "repetition" => convert::<RepetitionRules>(text, file, command),
        "rigs" => convert::<Rigs>(text, file, command),
        "notes" => convert::<Notes>(text, file, command),
        "plan" => convert::<Plan>(text, file, command),
        "references" => convert::<References>(text, file, command),
        "relations" => convert::<Relations>(text, file, command),
        "map" => convert::<Map>(text, file, command),
        "terrain" => convert::<Heightfield>(text, file, command),
        "water_plan" => convert::<WaterPlan>(text, file, command),
        "water_field" => convert::<WaterField>(text, file, command),
        "volume" => convert::<Volume>(text, file, command),
        "data_registry" => convert_with::<DataFiles>(text, file, command, |registry| {
            for entry in &mut registry.files {
                if let Some(stem) = entry.path.strip_suffix(&format!(".{RON_EXT}")) {
                    entry.path = format!("{stem}.{YAML_EXT}");
                }
            }
        }),
        "family_list" => convert::<Family>(text, file, command),
        "agent" => convert::<AgentSettings>(text, file, command),
        "play" => convert::<PlayConfig>(text, file, command),
        "destructible_marks" => convert::<Marks>(text, file, command),
        "destructible_bake" => convert::<Bake>(text, file, command),
        // **A kind whose type lives above this crate is converted by value** ([`crate::ron_tree`]): the YAML is read back and
        // compared with what the RON held, which is the check no type is needed for.
        _ => convert_value(text, file, command),
    }
}

/// **Convert `root`'s project files to YAML**, or refuse and change nothing.
///
/// Every conversion is computed in memory first. Only when all of them are lossless is a single file
/// written; then the RON originals are moved into [`ARCHIVE_DIR`]. So a refusal leaves the project
/// byte for byte as it was, and a success leaves no `.ron` project file behind.
///
/// **Every kind is converted**: one whose type this crate has as that type, any other by value ([`convert_value`]).
#[allow(clippy::result_large_err)]
pub fn migrate(root: &Path) -> Result<Report, Problem> {
    migrate_as(root, COMMAND)
}

/// **The same, naming `command` in what it says**: what `bevy_cauldron migrate <folder>` runs.
#[allow(clippy::result_large_err)]
pub fn migrate_as(root: &Path, command: &str) -> Result<Report, Problem> {
    migrate_where(root, command, |_| true)
}

/// **The game's own data files that the registries in `plan` register**, as plan entries of kind `game_data`.
///
/// A registry (`<kit>/data.ron`) lists tuning files by a path relative to itself. Each `.ron` one is converted by value with its
/// registry, so the registry and its files move together; a file that does not exist is skipped (the registry's own load names it),
/// and one that lies outside the project is refused rather than moved from somewhere this migration was not pointed at.
#[allow(clippy::result_large_err)]
fn registered_data_files(root: &Path, plan: &[(PathBuf, String, &'static FileKind)]) -> Result<Vec<(PathBuf, String, &'static FileKind)>, Problem> {
    let Some(kind) = KINDS.iter().find(|k| k.id == "game_data") else { return Ok(Vec::new()) };
    let mut out: Vec<(PathBuf, String, &'static FileKind)> = Vec::new();
    for (path, rel, registry) in plan {
        if registry.id != "data_registry" {
            continue;
        }
        let text = std::fs::read_to_string(path).map_err(|e| io_problem(path, e))?;
        let Ok(files) = ron::from_str::<DataFiles>(&text) else { continue };
        let dir = path.parent().unwrap_or(root);
        for entry in &files.files {
            if !entry.path.ends_with(&format!(".{RON_EXT}")) {
                continue;
            }
            let joined = dir.join(&entry.path);
            let Some(target) = normalised(&joined).filter(|t| t.starts_with(root)) else {
                return Err(Problem::with(
                    format!("{rel} registers `{}`, which is outside the project, so nothing was migrated", entry.path),
                    Detail::default()
                        .at(Location::file(rel.clone()))
                        .because("a migration moves only the files inside the project it was pointed at".to_owned())
                        .fix("convert that file by hand and point the registry at the YAML".to_owned())
                        .docs(DOCS),
                ));
            };
            if !target.is_file() || out.iter().any(|(p, _, _)| *p == target) {
                continue;
            }
            let shown = target.strip_prefix(root).unwrap_or(&target).to_string_lossy().replace('\\', "/");
            out.push((target, shown, kind));
        }
    }
    Ok(out)
}

/// **A path with its `.` and `..` resolved by name**, without touching the disk; `None` when it climbs above its start.
fn normalised(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    Some(out)
}

/// **The one migration body**: every file `wants` accepts, converted or nothing written at all.
#[allow(clippy::result_large_err)]
fn migrate_where(
    root: &Path,
    command: &str,
    wants: impl Fn(&FileKind) -> bool,
) -> Result<Report, Problem> {
    let mut plan: Vec<_> = plan(root)?.into_iter().filter(|(_, _, kind)| wants(kind)).collect();
    plan.extend(registered_data_files(root, &plan)?);
    let mut converted = Vec::new();
    let mut pending: Vec<(PathBuf, String, PathBuf)> = Vec::new();
    let mut unplaced = Vec::new();
    for (path, rel, kind) in &plan {
        let target = yaml_path(path).ok_or_else(|| {
            Problem::new(format!("{rel} ends in `.{RON_EXT}` and has no stub to build a `.{YAML_EXT}` name from"))
        })?;
        // The project's own spelling of the target, for the report and the refusal.
        let target_rel = rel
            .strip_suffix(&format!(".{RON_EXT}"))
            .map(|stem| format!("{stem}.{YAML_EXT}"))
            .unwrap_or_else(|| target.display().to_string());
        let text = std::fs::read_to_string(path).map_err(|e| io_problem(path, e))?;
        let mut done = write_kind(kind, &text, rel, command)?;
        // **Nothing is replaced.** A `kits.yaml` already beside its RON is a file a person made, and the
        // migration refuses rather than writing over it.
        if target.exists() {
            return Err(Problem::with(
                format!("{} already exists, so nothing was migrated", target_rel),
                Detail::default()
                    .at(Location::file(target_rel.clone()))
                    .because(format!("`{rel}` converts to a name a file is already at"))
                    .fix("move or delete the existing file, then run the migration again".to_owned())
                    .docs(DOCS),
            ));
        }
        unplaced.append(&mut done.unplaced);
        done.rom = rel.clone();
        done.yaml = target_rel;
        done.kind = kind.id;
        pending.push((path.clone(), done.text.clone(), target));
        converted.push(done);
    }

    // **Nothing is written until every file has converted.** Past this line a conversion that failed is
    // impossible, so the writes cannot leave a project half in one spelling.
    for (_, yaml_text, target) in &pending {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_problem(parent, e))?;
        }
        bevy_grimoire_core::atomic_file::write_new(target, yaml_text).map_err(|e| io_problem(target, e))?;
    }
    let archive = root.join(ARCHIVE_DIR);
    for (path, _, _) in &pending {
        let rel = path.strip_prefix(root).unwrap_or(path);
        let into = archive.join(rel);
        if let Some(parent) = into.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_problem(parent, e))?;
        }
        // A second migration of a project that already has an archive keeps the newer copy beside the
        // older one rather than replacing what a person may still want to diff.
        let into = unique_path(&into)?;
        // **Copied whole, then removed**, never renamed: the archive copy is a complete file (written beside
        // its final name and renamed into it by `atomic_file`) before the original is touched, so a failure
        // at any point leaves the original in place.
        let bytes = std::fs::read(path).map_err(|e| io_problem(path, e))?;
        bevy_grimoire_core::atomic_file::write_new(&into, bytes).map_err(|e| io_problem(&into, e))?;
        std::fs::remove_file(path).map_err(|e| io_problem(path, e))?;
    }
    Ok(Report { converted, unplaced, archived_at: ARCHIVE_DIR.to_owned() })
}

/// `path`, or `path.1`, `path.2`, … — the first name beside it that is not taken.
///
/// Answers a `Result` rather than panicking on an exhausted search: a folder with every one of
/// [`ARCHIVE_COPIES`] names taken is a refusal a person can act on, not a crash.
#[allow(clippy::result_large_err)]
fn unique_path(path: &Path) -> Result<PathBuf, Problem> {
    if !path.exists() {
        return Ok(path.to_path_buf());
    }
    for n in 1..=ARCHIVE_COPIES {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            return Err(Problem::new(format!("{} has no file name to archive under", path.display())));
        };
        let candidate = path.with_file_name(format!("{name}.{n}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(Problem::with(
        format!("{} already has {ARCHIVE_COPIES} archived copies", path.display()),
        Detail::default()
            .at(Location::file(path.display().to_string()))
            .because("every name the archive could take beside it is used")
            .fix("move the older copies out of the archive, then run the migration again".to_owned())
            .docs(DOCS),
    ))
}

/// **How many copies of one file the archive keeps beside each other.** A migration run more than this
/// many times over the same project is a mistake to fix rather than a name to invent.
const ARCHIVE_COPIES: usize = 99;

/// **`written`, with each RON comment `text` holds placed above the line it was written about**; and the
/// comments that had no place, with why.
///
/// **A comment can only go where a YAML parser will not read it as content**, so the lines a comment may
/// sit above are first narrowed to the ones that open a field or an item: a line inside a `|-` block
/// scalar is the *body of a string*, and inserting `# …` there changes the string's value. That is what
/// [`content_lines`] answers, and it is why the caller reads the commented text back as the type before
/// anything is written.
///
/// Both texts are walked **in field order**, with a cursor that only moves forward: a type names its
/// fields once at each level, so a comment above a RON field means the next YAML line that opens a field
/// of that name — never one further down that happens to repeat the word.
fn carry_comments(text: &str, written: &str, file: &str) -> (String, usize, Vec<Unplaced>) {
    let lines: Vec<String> = written.lines().map(str::to_owned).collect();
    let openable = content_lines(&lines);
    let mut placements: Vec<(usize, Vec<String>)> = Vec::new();
    // **Headers go above everything**, gathered separately: an insertion at index 0 is displaced by any
    // other insertion at 0.
    let mut headers: Vec<String> = Vec::new();
    let mut unplaced = Vec::new();
    // **The cursor**, so the walk is in field order and never looks back.
    let mut cursor = 0usize;
    for comment in ron_comments(text) {
        let Some(key) = comment.key else {
            if comment.header {
                headers.extend(comment.lines.iter().map(|l| format!("# {}", l.trim_end())));
            } else if let Some(name) = comment.item_name.as_deref().and_then(|n| item_line(&lines, &openable, n)) {
                let indent: String = lines[name].chars().take_while(|c| *c == ' ').collect();
                placements.push((name, comment.lines.iter().map(|l| format!("{indent}# {}", l.trim_end())).collect()));
                cursor = name;
            } else if comment.above_an_element {
                unplaced.push(Unplaced {
                    file: file.to_owned(),
                    comment: comment.lines.join(" "),
                    why: "it stands above a list item or a closing bracket rather than a field, and YAML \
                          has no field at that place to put it above"
                        .to_owned(),
                });
            } else {
                unplaced.push(Unplaced {
                    file: file.to_owned(),
                    comment: comment.lines.join(" "),
                    why: "it stands after the file's last field, so there is nothing to hang it above"
                        .to_owned(),
                });
            }
            continue;
        };
        match opening_line(&lines, &openable, &key, cursor) {
            Some(at) => {
                let indent: String = lines[at].chars().take_while(|c| *c == ' ').collect();
                placements.push((at, comment.lines.iter().map(|l| format!("{indent}# {}", l.trim_end())).collect()));
                cursor = at;
            }
            None => unplaced.push(Unplaced {
                file: file.to_owned(),
                comment: comment.lines.join(" "),
                why: format!("the YAML the writer emits has no `{key}` field to put it above"),
            }),
        }
    }
    // **Every comment that found a place**: a header is one, so it counts here and not only above a field.
    let carried = placements.len() + usize::from(!headers.is_empty());
    // Reversed first, so two comments above one line keep the order they were written in once both are inserted.
    placements.reverse();
    placements.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    let mut lines: Vec<String> = lines;
    for (at, block) in placements {
        for (n, line) in block.into_iter().enumerate() {
            lines.insert(at + n, line);
        }
    }
    // The header is the file's first block, in the order the RON wrote it.
    for (n, line) in headers.into_iter().enumerate() {
        lines.insert(n, line);
    }
    let mut out = lines.join("\n");
    out.push('\n');
    (out, carried, unplaced)
}

/// **The indices of the lines a comment may be inserted above**: every line that is not the body of a
/// block scalar.
///
/// The YAML writer puts a multi-line string in a `|-` (or `|`, `>`) block, whose body is the string's own
/// text — so a body line that reads `status: done` is content, and a `# …` inserted above it would become
/// part of the string. A block opens on a line whose value ends in a block indicator, and its body is the
/// run of following lines indented deeper than it; the first line that is not deeper ends the block.
fn content_lines(lines: &[String]) -> Vec<usize> {
    let mut openable = Vec::with_capacity(lines.len());
    let mut block: Option<usize> = None;
    for (at, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if let Some(owner) = block {
            // **A blank line inside a block is still content**, and a deeper one is too.
            if trimmed.is_empty() || indent > owner {
                continue;
            }
            block = None;
        }
        if opens_a_block(trimmed) {
            block = Some(indent);
        }
        openable.push(at);
    }
    openable
}

/// **Whether this line's value starts a block scalar**: `note: |-`, `note: >-`, `note:` `|`.
fn opens_a_block(trimmed: &str) -> bool {
    let Some((_, value)) = trimmed.split_once(':') else { return false };
    let value = value.trim();
    matches!(value, "|" | "|-" | "|+" | ">" | ">-" | ">+")
        || value.split_whitespace().next().is_some_and(|v| matches!(v, "|" | "|-" | "|+" | ">" | ">-" | ">+"))
}

/// **The index of the list item named `name`**: the first line in `openable` that opens an item with
/// exactly that name. **`- name: wall` is not `- name: wall_module`** — a prefix match confuses the two,
/// and the file still reads, so nothing else would catch it.
fn item_line(lines: &[String], openable: &[usize], name: &str) -> Option<usize> {
    let opening = format!("- name: {name}");
    openable.iter().copied().find(|at| {
        let trimmed = lines[*at].trim_start();
        trimmed == opening || trimmed.starts_with(&format!("{opening} "))
    })
}

/// **The index of the line that opens the field `key`**, or `None` when the writer emitted no such field.
///
/// Two cases, because a project file nests:
///
/// - **`key` occurs once** — `scp1048_ear` names one rig — so the line is that one, whatever the cursor
///   says. A single monotonic cursor would otherwise fail here: a comment above a `slots:` list moves the
///   cursor into that rig's body, and the next sibling rig opens *before* where the search had reached.
/// - **`key` occurs more than once** — `slots:` under every rig — so the walk is in field order: the first
///   occurrence at or after `from`, which is what makes a repeated field name land on the next one and
///   never on an earlier one.
///
/// Only a line [`content_lines`] kept is considered, so a `status:` inside a block scalar's body is never
/// mistaken for a field.
fn opening_line(lines: &[String], openable: &[usize], key: &str, from: usize) -> Option<usize> {
    let opening = format!("{key}:");
    let opens = |at: usize| {
        let trimmed = lines[at].trim_start();
        trimmed.strip_prefix("- ").unwrap_or(trimmed).starts_with(&opening)
    };
    let all: Vec<usize> = openable.iter().copied().filter(|at| opens(*at)).collect();
    match all.len() {
        0 => None,
        1 => all.first().copied(),
        _ => all.iter().copied().find(|at| *at >= from),
    }
}

/// **One RON comment, and where it stands relative to the file's fields.**
///
/// RON puts a comment in only four positions, and the YAML writer can carry the first two:
///
/// - **above a field** (`// the lattice` over `patches:`) — carried above that field;
/// - **before the file's first field** (a header describing the whole file) — carried at the top;
/// - **above a list item or a closer** — YAML has no field there, so it is listed as unplaced;
/// - **after the file's last field** — nothing to hang it from, so it is listed as unplaced.
pub struct Comment {
    /// The comment's lines, each already trimmed of its `//`. A blank `//` line stays as an empty
    /// string, so a paragraph break survives into the YAML exactly where the author put it.
    pub lines: Vec<String>,
    /// The field name the next line opens (`patches`), or `None` when the next line opens no field.
    pub key: Option<String>,
    /// **Whether it describes the whole file**: it stands before the file's first field, above the
    /// struct's `(`. That is a header, and it goes at the top of the YAML, which means the same thing.
    pub header: bool,
    /// **Whether it stands above a list item or a closer** (`( …` inside `patches: [`, or `]`). An item
    /// that names itself (`( name: "marking", … )`) can still be found in the YAML, by that name; a
    /// closer, and an item with no name, cannot, and is listed instead of being guessed at.
    pub above_an_element: bool,
    /// **The `name` of the list item it stands above**, when the item declares one: `marking` for
    /// `( name: "marking", note: … )`. This is what lets a comment above a list item find its item in
    /// the YAML, where the item is `- name: marking`.
    pub item_name: Option<String>,
}

fn ron_comments(text: &str) -> Vec<Comment> {
    let all: Vec<&str> = text.lines().collect();
    let mut out: Vec<Comment> = Vec::new();
    // **The comment run being read**, as its lines. A run ends at the first line that is not a comment.
    let mut run: Option<Vec<String>> = None;
    let mut pending_block = false;
    // **Whether a field has been opened yet**, which is what tells a header from a footer.
    let mut seen_field = false;

    // One run, standing above `all[at]` (or above nothing, at the file's end).
    let flush = |run: &mut Option<Vec<String>>, at: Option<usize>, seen_field: bool, out: &mut Vec<Comment>| {
        let Some(lines) = run.take() else { return };
        if lines.iter().all(|l| l.is_empty()) {
            return;
        }
        let (key, header, above_an_element, item_name) = match at {
            Some(at) => {
                let line = all[at].trim_start();
                match field_name(line) {
                    Some(key) => (Some(key), false, false, None),
                    None if !seen_field && line.starts_with('(') => (None, true, false, None),
                    None => (None, false, true, item_name_from(&all, at)),
                }
            }
            None => (None, !seen_field, false, None),
        };
        out.push(Comment { lines, key, header, above_an_element, item_name });
    };

    for (at, line) in all.iter().enumerate() {
        let mut trimmed = line.trim_start();
        // **A block comment is a comment** (review M4): its lines join the run above the next field, a one-line `/* x */` ends on
        // its own line, and whatever follows the `*/` on that line is read as code.
        if pending_block {
            let (inside, ended) = match trimmed.find("*/") {
                Some(i) => (&trimmed[..i], true),
                None => (trimmed, false),
            };
            run.get_or_insert_with(Vec::new).push(inside.trim_start_matches('*').trim().to_owned());
            if !ended {
                continue;
            }
            pending_block = false;
            trimmed = trimmed[trimmed.find("*/").map_or(0, |i| i + 2)..].trim_start();
            if trimmed.is_empty() {
                continue;
            }
        } else if let Some(body) = trimmed.strip_prefix("/*") {
            match body.find("*/") {
                Some(i) => {
                    run.get_or_insert_with(Vec::new).push(body[..i].trim().to_owned());
                    trimmed = body[i + 2..].trim_start();
                    if trimmed.is_empty() {
                        continue;
                    }
                }
                None => {
                    run.get_or_insert_with(Vec::new).push(body.trim_start_matches('*').trim().to_owned());
                    pending_block = true;
                    continue;
                }
            }
        }
        if let Some(rest) = trimmed.strip_prefix("//") {
            run.get_or_insert_with(Vec::new).push(rest.trim().to_owned());
            continue;
        }
        // **A blank line does not end a run.** A comment separated from its field by a blank line is an
        // ordinary style, and treating it as the end is what reported a field's own comment as a footer.
        if trimmed.is_empty() {
            continue;
        }
        let trailing = trailing_comment(trimmed);
        let code = trailing.map_or(trimmed, |(code, _)| code);
        let key = field_name(code);
        if key.is_some() {
            seen_field = true;
        }
        flush(&mut run, Some(at), seen_field, &mut out);
        // **A trailing same-line comment** (`scale: 2.0, // metres`) stands above the field it was written beside; one beside no
        // field (a closer, a list item) is listed rather than dropped.
        if let Some((_, comment)) = trailing {
            let lines = vec![comment.trim().to_owned()];
            out.push(match key {
                Some(key) => Comment { lines, key: Some(key), header: false, above_an_element: false, item_name: None },
                None => Comment { lines, key: None, header: false, above_an_element: true, item_name: item_name_from(&all, at) },
            });
        }
    }
    flush(&mut run, None, seen_field, &mut out);
    out
}

/// **Split a code line at a `//` that is not inside a string**: `(code, comment)`, or `None` when the line has no trailing comment.
fn trailing_comment(line: &str) -> Option<(&str, &str)> {
    let mut in_string = false;
    let mut escaped = false;
    let bytes = line.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if in_string {
            match (escaped, *b) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_string = false,
                _ => {}
            }
        } else if *b == b'"' {
            in_string = true;
        } else if *b == b'/' && bytes.get(i + 1) == Some(&b'/') {
            let code = line[..i].trim_end();
            return (!code.is_empty()).then(|| (code, &line[i + 2..]));
        }
    }
    None
}

/// **The field name a RON line opens**, or `None` for a line that opens no field.
///
/// `patches: [` opens `patches`. A constructor's argument list (`Some("x")`) and a tuple field
/// (`of: "chair@1"`) both read as `name:`, which is why only a name of letters, digits and underscores
/// counts: `Some("x")` opens nothing, `of: "chair@1"` opens `of`.
fn field_name(trimmed: &str) -> Option<String> {
    let (name, _) = trimmed.split_once(':')?;
    let name = name.trim();
    // **A map key is quoted in RON and plain in YAML**: `"researcher": (` becomes `researcher:`. Both
    // spellings name the same field, so both are read here or a rig's own comment is taken for a
    // comment above a list item.
    let name = match name.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) {
        Some(inner) => inner,
        None => name,
    };
    let ok = !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && !matches!(name, "true" | "false");
    ok.then(|| name.to_owned())
}

/// **The `name` the list item opening at `all[at]` declares**, or `None` for an item that names nothing.
///
/// A RON item's name may be on the line that opens it (`( name: "marking", … )`) or the next one
/// (`(`, then `name: "marking",`), which is how a long token is written. So the item's own lines —
/// those before its `)` — are searched for the first `name:`.
fn item_name_from(all: &[&str], at: usize) -> Option<String> {
    let opening = all.get(at)?.trim_start();
    if !opening.starts_with('(') {
        return None;
    }
    // The name is in the item's own lines: those before the line that closes it.
    for line in &all[at..] {
        let line = line.trim_start();
        if line != opening && line.starts_with(')') {
            break;
        }
        if let Some(value) = line.strip_prefix("( name:").or_else(|| line.strip_prefix("name:")) {
            let quoted = value.trim_start().strip_prefix('"')?;
            let (text, _) = quoted.split_once('"')?;
            return (!text.is_empty()).then(|| text.to_owned());
        }
    }
    None
}

/// **Read `text` as a `T` from the RON an earlier release wrote**, which is what a frozen fixture is.
///
/// The migration's own reader, exposed so a test that holds a release's file can say what value it held
/// before comparing it with the YAML the migration wrote. It is the only reader of the old spelling left
/// in the workspace, and it lives here for the reason this crate exists: nothing in a running program
/// reads RON any more, and a fixture is not a running program.
pub fn read_ron<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    ron::from_str(text).map_err(|e: ron::error::SpannedError| e.to_string())
}

/// **Whether `text` also reads as a RON document**, which after the cutover nothing should.
///
/// The cutover's whole claim is one spelling per file, and a reader that took both would be the second
/// path it exists to remove. So a test can write what this build writes and ask this whether the old
/// reader would have taken it too: a YAML document that also reads as RON means a kind kept its old
/// reader beside its new one.
pub fn reads_as_ron(text: &str) -> bool {
    ron::from_str::<ron::Value>(text).is_ok()
}

/// **One project file's text, as YAML, from the RON it was written in** — the conversion [`migrate`] runs,
/// for one file a caller already holds in memory.
///
/// Answers the text and the name it would be written at (`kits.yaml` → `kits.yaml`), or a refusal naming
/// the file. This exists for the two callers that are not a folder walk: the frozen fixtures a release
/// wrote (`tests/old_data.rs`), which are checked as the type reads them *today*, and a game's own data
/// file, whose type lives above this crate and which is therefore refused by name rather than guessed at.
///
/// **A file whose kind this crate cannot read is refused, not passed through.** The conversion is the
/// same one [`migrate`] runs, so a value that would change here is a value that would change there.
#[allow(clippy::result_large_err)]
pub fn written_as_yaml(rel: &str, text: &str, command: &str) -> Result<(String, String), Problem> {
    // **The name a frozen fixture carries is the project-relative one** (`kits.yaml`, `site/library.yaml`),
    // and a [`Base::Grimoire`] pattern hangs from `assets/grimoire`, so both spellings are tried. The root
    // itself is the other base, whose patterns are already project-relative.
    let kind = kind_of_rel(rel)
        .or_else(|| kind_of_rel(&format!("{GRIMOIRE_DIR}/{rel}")))
        .or_else(|| kind_of_rel(rel.trim_start_matches(&format!("{GRIMOIRE_DIR}/"))))
        .ok_or_else(|| {
        Problem::with(
            format!("{rel} is not a project file this build names"),
            Detail::default()
                .at(Location::file(rel.to_owned()))
                .because("no kind's path pattern matches it, so its type is unknown".to_owned())
                .fix(format!("run `{command}`, which converts what it knows and names what it does not"))
                .docs(DOCS),
        )
    })?;
    let done = write_kind(kind, text, rel, command)?;
    let yaml = rel
        .strip_suffix(&format!(".{RON_EXT}"))
        .map(|stem| format!("{stem}.{YAML_EXT}"))
        .unwrap_or_else(|| rel.to_owned());
    Ok((yaml, done.text))
}

/// **The refusal an open raises when `root` still holds a RON project file**: the file, and the command
/// that fixes it. `None` for a project that is already YAML.
///
/// This is the one decision, so the editor, the command line and an agent cannot disagree about what an
/// old project is.
///
/// **`command` is the caller's**, because the layer that reads the refusal is the layer that knows what
/// binary a person types: this crate must not name an engine crate (`tests/engine_free.rs`), and a
/// refusal naming a command that does not exist is a worse answer than one naming none.
///
/// **A folder that cannot be read does not hide the project.** The walk answers a refusal for a folder it
/// cannot list, and an unmigrated project's whole worth is that it still opens: so an unreadable
/// subfolder is *skipped* here and the other files still decide. A project with nothing readable is no
/// project, and the open that follows says why with its own words.
#[allow(clippy::result_large_err)]
pub fn leftover(root: &Path, command: &str) -> Result<Option<Problem>, Problem> {
    let plan = plan(root)?;
    // **Every kind refuses an open**: nothing reads RON any more, so a `.ron` file a kind names is a file no reader will take.
    let Some((_, rel, _)) = plan.first() else { return Ok(None) };
    let rel = rel.clone();
    Ok(Some(Problem::with(
        format!("{rel} is still RON, so this project has not been migrated"),
        Detail::default()
            .at(Location::file(rel.clone()))
            .because(format!(
                "the editor reads and writes YAML now, and a project holding `.{RON_EXT}` files is the spelling it no longer reads"
            ))
            .fix(format!("run `{command}`, then open it again"))
            .docs(DOCS),
    )))
}
