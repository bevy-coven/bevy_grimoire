//! **Moving a project's files from RON to YAML, losslessly** (GD-03, #365).
//!
//! A project used to be RON (`kits.ron`, `maps/town.map.ron`, `editor.ron`, …). This is the one
//! command that turns a project into YAML: `cauldron migrate <root>`, and the editor offers it when it
//! opens a RON project. Nothing else writes the old spelling again.
//!
//! # Two rules, and neither is a suggestion
//!
//! - **Lossless.** Each file is read with the type's own RON reader, written with the one YAML writer
//!   ([`crate::yaml::to_string`]), and **read back with that same type's YAML reader and compared with
//!   the value it came from** ([`crate::yaml::from_str`]). A single value that differs stops the whole
//!   migration, names the file, and **writes nothing at all** — not that file, not the ones before it.
//!   A half-migrated project is the one outcome a person cannot repair by hand.
//! - **Loud.** A RON comment is carried over as a YAML comment above the same key wherever its place
//!   can be found. One that cannot be placed is listed in the [`Report`], never dropped quietly. A file
//!   whose kind this crate has no type for (a mapper's forest, a game's own data file) stops the
//!   migration and is named: guessing at a value it cannot read is how a file loses a field.
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

use std::fmt::Debug;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::problem::{Detail, Location, Problem};
use crate::reload::{Base, FileKind, KINDS};
use crate::{composition::Compositions, destructible::{Bake, Marks}, family::Family, heightfield::Heightfield,
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

/// **The command a person runs**, which every refusal that meets a leftover RON file names.
pub const COMMAND: &str = "cauldron migrate <root>";

/// The page of the guide that explains the move: "Moving a project to YAML" in it.
const DOCS: &str = "docs/guide/your-game.md";

/// **Where a project's own files live**, under the game's root: what a [`Base::Grimoire`] pattern is
/// relative to. The same folder `project_data::migrate_legacy_data_dir` settled on.
pub const GRIMOIRE_DIR: &str = "assets/grimoire";

/// **The folders never walked**: the archive this module writes, and the ones no project file lives
/// under. A walk that descended into `target/` would migrate a build.
const SKIP_DIRS: [&str; 4] = [".git", "target", ".live", ARCHIVE_DIR];

/// **One file the migration converted.**
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Converted {
    /// The file as the project named it before: `assets/grimoire/kits.ron`, with `/`.
    pub rom: String,
    /// The file as it is named now: `assets/grimoire/kits.yaml`.
    pub yaml: String,
    /// The [`FileKind::id`] it is, so a person can see which type vouched for it.
    pub kind: &'static str,
    /// How many RON comments were carried into the YAML file.
    pub comments_carried: usize,
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

/// **The path a project file is written at now**, from the path it had: `kits.ron` → `kits.yaml`.
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
    walk(root, root, &mut out, &mut Vec::new())?;
    out.sort();
    Ok(out
        .into_iter()
        .filter_map(|rel| {
            if !rel.ends_with(&format!(".{RON_EXT}")) {
                return None;
            }
            let kind = kind_of_rel(&rel)?;
            Some((root.join(&rel), rel, kind))
        })
        .collect())
}

/// The kind of a project file named relative to the project root, which is where both bases hang from.
fn kind_of_rel(rel: &str) -> Option<&'static FileKind> {
    // `Base::Grimoire` paths are relative to `assets/grimoire`; a root file to the root itself.
    KINDS.iter().find(|k| match k.base {
        Base::Grimoire => rel
            .strip_prefix(GRIMOIRE_DIR)
            .and_then(|rest| rest.strip_prefix('/'))
            .is_some_and(|rest| k.matches(rest)),
        Base::Root => k.matches(rel),
    })
}

/// **Every file under `dir`, as a path relative to `root` with `/`**, depth-first, skipping [`SKIP_DIRS`].
#[allow(clippy::result_large_err)]
fn walk(root: &Path, dir: &Path, out: &mut Vec<String>, seen: &mut Vec<PathBuf>) -> Result<(), Problem> {
    // A symlinked directory that points at one of its own parents would walk forever.
    let real = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    if seen.contains(&real) {
        return Ok(());
    }
    seen.push(real);
    let entries = std::fs::read_dir(dir).map_err(|e| io_problem(dir, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| io_problem(dir, e))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            walk(root, &path, out, seen)?;
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

/// **The YAML text `text` becomes, or a refusal that says the value would change.**
///
/// The one conversion: read with the type's RON reader, write with the one YAML writer, read back with
/// the type's YAML reader, and answer the text only when the value came back equal. `file` is only what
/// a refusal names.
#[allow(clippy::result_large_err)]
fn convert<T>(text: &str, file: &str) -> Result<String, Problem>
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let value: T = ron::from_str(text).map_err(|e| {
        Problem::with(
            format!("{file} did not read as RON, so nothing was migrated"),
            Detail::default()
                .at(Location::file(file.to_owned()))
                .because(e.to_string())
                .fix(format!("a file this old is read by the build that wrote it; `{COMMAND}` refuses rather than guessing"))
                .docs(DOCS),
        )
    })?;
    let written = yaml::to_string(&value).map_err(|p| {
        Problem::with(
            format!("{file} did not write as YAML, so nothing was migrated"),
            p.detail(),
        )
    })?;
    let back: T = yaml::from_str(&written, file).map_err(|p| {
        Problem::with(
            format!("{file} wrote as YAML and did not read back, so nothing was migrated"),
            p.detail(),
        )
    })?;
    if back != value {
        return Err(Problem::with(
            format!("{file} would change value if it were written as YAML, so nothing was migrated"),
            Detail::default()
                .at(Location::file(file.to_owned()))
                .because(difference(&value, &back))
                .fix("a file this build cannot round-trip is left as it is; report it, do not hand-edit it".to_owned())
                .docs(DOCS),
        ));
    }
    Ok(written)
}

/// **Where two values of one type first disagree**, as a sentence: the debug forms, shortened to the
/// first place they part, so a refusal names the field and not the whole document.
fn difference<T: Debug>(before: &T, after: &T) -> String {
    let a = format!("{before:?}");
    let b = format!("{after:?}");
    let at = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
    let window = |s: &str| {
        let start = s[..at].char_indices().next_back().map_or(0, |(i, _)| i);
        let end = (at + DIFFERENCE_WINDOW).min(s.len());
        s[start..end].to_owned()
    };
    format!("the RON holds `{}` and the YAML reads `{}`", window(&a), window(&b))
}

/// **How many bytes of a value a [`difference`] sentence shows** on either side of where they part.
const DIFFERENCE_WINDOW: usize = 80;

/// **The YAML text a project file of `kind` becomes**, or a refusal naming the file.
///
/// One arm per kind this crate owns a type for. A kind whose type lives above this crate ([`UNOWNED`])
/// is refused here rather than guessed at: a mapper's forest and a game's own data file are read by the
/// crate that declares them, and only that crate can say whether the YAML holds the same value.
#[allow(clippy::result_large_err)]
fn write_kind(kind: &FileKind, text: &str, file: &str) -> Result<String, Problem> {
    match kind.id {
        "kits" => convert::<Kits>(text, file),
        "vocab" => convert::<Vocabularies>(text, file),
        "library" => convert::<Library>(text, file),
        "policy" => convert::<Policy>(text, file),
        "rules" => convert::<Rules>(text, file),
        "compositions" => convert::<Compositions>(text, file),
        "standards" => convert::<Standards>(text, file),
        "repetition" => convert::<RepetitionRules>(text, file),
        "rigs" => convert::<Rigs>(text, file),
        "notes" => convert::<Notes>(text, file),
        "plan" => convert::<Plan>(text, file),
        "references" => convert::<References>(text, file),
        "relations" => convert::<Relations>(text, file),
        "map" => convert::<Map>(text, file),
        "terrain" => convert::<Heightfield>(text, file),
        "water_plan" => convert::<WaterPlan>(text, file),
        "water_field" => convert::<WaterField>(text, file),
        "volume" => convert::<Volume>(text, file),
        "data_registry" => convert::<DataFiles>(text, file),
        "family_list" => convert::<Family>(text, file),
        "agent" => convert::<AgentSettings>(text, file),
        "play" => convert::<PlayConfig>(text, file),
        "destructible_marks" => convert::<Marks>(text, file),
        "destructible_bake" => convert::<Bake>(text, file),
        other => Err(Problem::with(
            format!("{file} is a `{other}` file, whose type this crate does not declare, so nothing was migrated"),
            Detail::default()
                .at(Location::file(file.to_owned()))
                .because(format!(
                    "`{other}` is read by the crate that declares its type ({})",
                    unowned_by(other).unwrap_or("a layer above this one")
                ))
                .fix(format!("run `{COMMAND}` from that layer, which can compare the value it reads"))
                .docs(DOCS),
        )),
    }
}

/// **Which crate declares the type a kind this crate cannot read is**, for a refusal that says who can.
fn unowned_by(kind: &str) -> Option<&'static str> {
    match kind {
        "forest" | "people" | "scripts" | "trees" => Some("`bevy_cauldron`, the editor"),
        "checks" => Some("`bevy_oracle`"),
        "family" => Some("`bevy_cauldron`'s family host"),
        "editor_settings" => Some("`bevy_cauldron`'s settings"),
        "game_data" => Some("the game that registers it in `data.yaml`"),
        _ => None,
    }
}

/// **Convert `root`'s project files to YAML**, or refuse and change nothing.
///
/// Every conversion is computed in memory first. Only when all of them are lossless is a single file
/// written; then the RON originals are moved into [`ARCHIVE_DIR`]. So a refusal leaves the project
/// byte for byte as it was, and a success leaves no `.ron` project file behind.
#[allow(clippy::result_large_err)]
pub fn migrate(root: &Path) -> Result<Report, Problem> {
    let plan = plan(root)?;
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
        let written = write_kind(kind, &text, rel)?;
        let (yaml_text, carried, not_placed) = carry_comments(&text, &written, rel);
        unplaced.extend(not_placed);
        converted.push(Converted {
            rom: rel.clone(),
            yaml: target_rel,
            kind: kind.id,
            comments_carried: carried,
        });
        pending.push((path.clone(), yaml_text, target));
    }

    // **Nothing is written until every file has converted.** Past this line a conversion that failed is
    // impossible, so the writes cannot leave a project half in one spelling.
    for (_, yaml_text, target) in &pending {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_problem(parent, e))?;
        }
        crate::atomic_file::write(target, yaml_text).map_err(|e| io_problem(target, e))?;
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
        let into = unique_path(&into);
        std::fs::rename(path, &into).map_err(|e| io_problem(path, e))?;
    }
    Ok(Report { converted, unplaced, archived_at: ARCHIVE_DIR.to_owned() })
}

/// `path`, or `path.1`, `path.2`, … — the first name beside it that is not taken.
fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    for n in 1.. {
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => format!("{name}.{n}"),
            None => continue,
        };
        let candidate = path.with_file_name(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("the loop above answers the first free name")
}

/// **`written`, with each RON comment `text` holds moved above the same field**; and the comments that
/// had no place, with why.
///
/// A comment belongs to the field it sits above in the RON (`// the lattice` above `patches:`), so it is
/// placed above the YAML line that opens that field, indented the same way. The field is found by
/// **walking both texts side by side in field order**, not by searching for the key's text: `note:` may
/// appear inside a string, and `patches:` may be absent from the YAML the writer emits. So a comment is
/// carried only when the field it names is one the writer actually wrote, and every other comment is
/// answered as unplaced rather than moved somewhere it might be wrong.
fn carry_comments(text: &str, written: &str, file: &str) -> (String, usize, Vec<Unplaced>) {
    let mut lines: Vec<String> = written.lines().map(str::to_owned).collect();
    // **Insert from the bottom up**, so an earlier insertion cannot move the line a later one names.
    let mut placements: Vec<(usize, String)> = Vec::new();
    let mut unplaced = Vec::new();
    for comment in ron_comments(text) {
        let Some(key) = comment.key else {
            unplaced.push(Unplaced {
                file: file.to_owned(),
                comment: comment.text,
                why: "nothing follows it but the end of the file, so there is no field to hang it above"
                    .to_owned(),
            });
            continue;
        };
        match opening_line(&lines, &key) {
            Some(at) => {
                let indent: String = lines[at].chars().take_while(|c| *c == ' ').collect();
                placements.push((at, format!("{indent}# {}", comment.text)));
            }
            None => unplaced.push(Unplaced {
                file: file.to_owned(),
                comment: comment.text,
                why: format!("the YAML the writer emits has no `{key}` field to put it above"),
            }),
        }
    }
    let carried = placements.len();
    placements.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    let mut lines: Vec<String> = std::mem::take(&mut lines);
    for (at, line) in placements {
        lines.insert(at, line);
    }
    let mut out = lines.join("\n");
    out.push('\n');
    (out, carried, unplaced)
}

/// **The YAML line that opens the field `key`**, or `None` when the writer emitted no such field.
///
/// A field is opened by a line whose text before its first `:` is the key — `- name: x` (a sequence
/// item's field) and `  name: x` (a mapping's) alike — so a `name:` occurring inside a scalar is never
/// mistaken for the field. The first such line wins: a type names its field once at each level, and a
/// comment above a RON field means the nearest one.
fn opening_line(lines: &[String], key: &str) -> Option<usize> {
    let opening = format!("{key}:");
    lines.iter().position(|line| {
        let trimmed = line.trim_start();
        let trimmed = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        trimmed.starts_with(&opening)
    })
}

/// **One RON comment, and the field it sits above**, when a field follows before the file ends.
pub struct Comment {
    /// What stood after the `//` on each line of one run, joined by a space.
    pub text: String,
    /// The field name the next line opens (`patches`), or `None` when no field follows.
    pub key: Option<String>,
}

/// **Every comment in `text`, in file order.** A run of `//` lines is one comment; a blank `//` line
/// inside a run starts a new one, so a paragraph break survives. A `/* */` comment is not carried (it
/// has no place above a key by the same arithmetic) and is answered as unplaced.
fn ron_comments(text: &str) -> Vec<Comment> {
    let mut out: Vec<Comment> = Vec::new();
    let mut run: Option<String> = None;
    let mut pending_block = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("/*") {
            pending_block = true;
            continue;
        }
        if pending_block {
            if trimmed.contains("*/") {
                pending_block = false;
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("//") {
            let rest = rest.trim().to_owned();
            match run.take() {
                Some(mut had) if !rest.is_empty() => {
                    had.push(' ');
                    had.push_str(&rest);
                    run = Some(had);
                }
                Some(had) => {
                    out.push(Comment { text: had, key: None });
                    run = Some(rest);
                }
                None => run = Some(rest),
            }
            continue;
        }
        let Some(had) = run.take() else { continue };
        if had.is_empty() {
            continue;
        }
        let key = field_name(trimmed);
        out.push(Comment { text: had, key });
    }
    if let Some(had) = run
        && !had.is_empty()
    {
        out.push(Comment { text: had, key: None });
    }
    out
}

/// **The field name a RON line opens**, or `None` for a line that opens no field.
///
/// `patches: [` opens `patches`. A constructor's argument list (`Some("x")`) and a tuple field
/// (`of: "chair@1"`) both read as `name:`, which is why only a name of letters, digits and underscores
/// counts: `Some("x")` opens nothing, `of: "chair@1"` opens `of`.
fn field_name(trimmed: &str) -> Option<String> {
    let (name, _) = trimmed.split_once(':')?;
    let name = name.trim();
    let ok = !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !matches!(name, "true" | "false");
    ok.then(|| name.to_owned())
}

/// **The refusal an open raises when `root` still holds a RON project file**: the file, and the command
/// that fixes it. `None` for a project that is already YAML.
///
/// This is the one decision, so the editor, the command line and an agent cannot disagree about what an
/// old project is.
#[allow(clippy::result_large_err)]
pub fn leftover(root: &Path) -> Result<Option<Problem>, Problem> {
    let plan = plan(root)?;
    let Some((_, rel, _)) = plan.first() else { return Ok(None) };
    Ok(Some(Problem::with(
        format!("{rel} is still RON, so this project has not been migrated"),
        Detail::default()
            .at(Location::file(rel.clone()))
            .because(format!(
                "the editor reads and writes YAML now, and a project holding `.{RON_EXT}` files is the spelling it no longer reads"
            ))
            .fix(format!("run `{COMMAND}`, then open it again"))
            .docs(DOCS),
    )))
}
