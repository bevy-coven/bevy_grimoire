//! **A hand-back the person kept becomes a benchmark task draft** (BM-07, #417).
//!
//! The tasks an agent struggles with on a real project are the best benchmark tasks there are, because a
//! person asked for them and a person accepted the answer. In the Agent workspace a hand-back the person
//! kept gets **Make a benchmark task**, which writes
//! `.dark_factory/bench/drafts/<id>/task.yaml` in **the project's own folder**:
//!
//! | field | what it holds |
//! |---|---|
//! | `prompt` | what the person asked for: the hand-back's `specified` lines |
//! | `setup` | none — the scene is the draft's own `fixture/`, the maps **as they were before the agent's first change in that hand-back**, rebuilt by taking every logged change back **on a copy** of the project (the open map is never touched) |
//! | `solution` | the hand-back's operations, as the log recorded them: the result the person accepted |
//! | `checks` | drafted from the change: each placement the hand-back added or turned a `placement_at` with a tolerance, each one it removed an absent `placement_exists`, every other file of `maps/` an `unchanged` |
//!
//! # A draft is a draft
//!
//! It is **never run** until a person moves it into a suite: the folder is `drafts/`, which the suite's loader
//! skips, and the move is a file move the person makes. The file opens in the Files workspace so the person
//! reads it first, and BM-01's validity test then proves it like any other task (the do-nothing run must fail
//! a check, the `solution` must pass every one). A game's draft stays in the game: its maps are copies of the
//! game's own, written under the game's own `.dark_factory/`, and nothing is copied into this repository.
//!
//! # What cannot be drafted, and says why
//!
//! - **An operation with no recorded inverse** cannot be taken back on the copy, so the setup cannot be
//!   rebuilt. The button is disabled and its tooltip names the operation ([`lacks_inverse`]).
//! - **A hand-back with a newer change standing on it**: the undo walk refuses to take back a change under a
//!   newer one, and so does this, saying which. Draft it before the next change, or take that one back first.
//! - **A change that leaves nothing to ask about** — no placement added, removed or turned, and no other
//!   map file — has no check, and a task with no check measures nothing.

use std::path::Path;

use bevy_grimoire_core::map::{Map, MAP_SUFFIX};
use serde::Serialize;
use serde_json::{json, Value};

use crate::ops::{self, Applied, Ctx, Entry, Source};
use crate::present::HandBack;
use crate::project::{OpenMap, Project, GRIMOIRE_DIR};

/// **Where drafts are written**, relative to the project's root. `bench/` is where a game's suite lives
/// (`cauldron-bench --suite .dark_factory/bench`), and `drafts/` under it is where a task waits to be read.
pub const DRAFTS_DIR: &str = ".dark_factory/bench/drafts";

/// **How far a placement may be from where the hand-back put it**, metres, before a check says it moved.
/// A quarter of a metre: close enough that "the bed against the wall" still is, far enough that a different
/// but equally good answer is not failed for being a hand's width off.
pub const PLACEMENT_TOLERANCE_M: f32 = 0.25;

/// **How far a placement's facing may be from the hand-back's**, degrees.
pub const YAW_TOLERANCE_DEG: f32 = 15.0;

/// **How far a placement's lift may be from the hand-back's**, metres.
pub const LIFT_TOLERANCE_M: f32 = 0.02;

/// **The longest title slug in a draft's id**, in characters, so a folder name stays readable.
pub const SLUG_MAX_CHARS: usize = 40;

/// **A draft's budget**, which is the suite's default: ten minutes and sixty calls. The person edits it in
/// review, with the rest.
pub const BUDGET_WALL_S: u32 = 600;
/// See [`BUDGET_WALL_S`].
pub const BUDGET_TOOL_CALLS: u32 = 60;

/// **The directory of a project's maps, inside its grimoire directory.**
const MAPS_DIR: &str = "maps";

/// **The name of the file a task is read from**, which is the bench crate's own `TASK_FILE`.
const TASK_FILE: &str = "task.yaml";

/// **The hand-back the draft is made from, and where its files went.**
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Made {
    /// The draft's id, which is also its folder's name.
    pub id: String,
    /// **The task file, relative to the project's root**, which is what the Files workspace opens and what a
    /// kept wish names.
    pub rel: String,
    /// Whether the draft was written now (`false`: it was there already, and is left as it is).
    pub written: bool,
}

/// **The log lines a hand-back covers**, in the order the agent listed them, or why they cannot be read.
fn covered(entries: &[Entry], handback: &HandBack) -> Result<Vec<Entry>, String> {
    if handback.operation_ids.is_empty() {
        return Err("the hand-back lists no operations, so there is no change to make a task of".to_owned());
    }
    handback
        .operation_ids
        .iter()
        .map(|id| {
            entries
                .iter()
                .find(|e| &e.operation_id == id)
                .cloned()
                .ok_or_else(|| format!("`{id}` is not in the operation log"))
        })
        .collect()
}

/// **The first operation of a hand-back that recorded no inverse**, by name, or `None`.
///
/// A change with no inverse cannot be taken back, so the scene before it cannot be rebuilt, so the draft's
/// setup cannot be made. The button is disabled and its tooltip names this operation.
pub fn lacks_inverse(entries: &[Entry], handback: &HandBack) -> Option<String> {
    handback
        .operation_ids
        .iter()
        .filter_map(|id| entries.iter().find(|e| &e.operation_id == id))
        .find(|e| e.inverse.is_none())
        .map(|e| e.op.clone())
}

/// **What the button says when it is disabled**, or `None` when the hand-back can be made a task.
pub fn cannot(project: &Project, handback: &HandBack) -> Option<String> {
    let entries = ops::read_entries(&ops::oplog_path(project));
    if handback.operation_ids.is_empty() {
        return Some("this hand-back lists no operations, so there is no change to make a task of".to_owned());
    }
    lacks_inverse(&entries, handback).map(|op| {
        format!("`{op}` recorded no inverse, so the map before this hand-back cannot be rebuilt and no task can be made of it")
    })
}

/// **A title as a folder name**: lower case, letters and digits, single dashes, at most [`SLUG_MAX_CHARS`].
fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out: String = out.chars().take(SLUG_MAX_CHARS).collect();
    out.trim_matches('-').to_owned()
}

/// **A draft's id**: the hand-back's own id, its title and a stamp of the operations it covers,
/// `present-1-the-bed-against-the-wall-3f9a01bc`.
///
/// **The stamp is what makes the id one hand-back's alone.** `present-<n>` restarts with every editor session, so a later
/// session's `present-1` with the same title would otherwise open the earlier session's draft ("already drafted") and
/// draft nothing. The operation ids are unique within a project and a hand-back's list is its own, so the same hand-back
/// always gets the same id (a second press is "show me") and a different one never does.
fn draft_id(handback: &HandBack) -> String {
    let slug = slug(&handback.title);
    let stamp = bevy_grimoire_core::glb::fnv1a(handback.operation_ids.join("\n").as_bytes()) as u32;
    if slug.is_empty() { format!("{}-{stamp:08x}", handback.id) } else { format!("{}-{slug}-{stamp:08x}", handback.id) }
}

/// **A string an agent supplied, as one line of a comment**: every run of control characters, newlines and Unicode line breaks
/// becomes one space, so nothing the agent wrote can end the comment and begin a line of the file.
fn one_line(text: &str) -> String {
    text.split(|c: char| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}')).filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ")
}

/// **The comment a draft opens with.** Every line starts with `#`; the hand-back's title is the agent's own words and
/// is written through [`one_line`].
fn header(handback: &HandBack) -> String {
    format!(
        "# **A benchmark task drafted from hand-back {} ({}), for a person to read** (BM-07, #417).\n\
         #\n\
         # `fixture/` holds the maps as they were before the agent's first change; `solution` is what the agent did;\n\
         # `checks` were drafted from the difference. Edit anything, then move this folder into a suite\n\
         # (`.dark_factory/bench/`). It is never run from `drafts/`.\n",
        one_line(&handback.id),
        one_line(&handback.title)
    )
}

/// **Copy a directory tree's YAML files**, folders included, into a place that does not exist yet.
///
/// Meshes, pictures and sounds are skipped: taking a change back reads and writes the project's text files, and a
/// copy of a game's art would be a copy of a game's assets for no reason the undo walk has.
fn copy_yaml_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for entry in std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))?.flatten() {
        let (path, target) = (entry.path(), to.join(entry.file_name()));
        let kind = entry.file_type().map_err(|e| format!("{}: {e}", path.display()))?;
        if kind.is_dir() {
            copy_yaml_tree(&path, &target)?;
        } else if kind.is_file() && is_yaml(&path) {
            let held = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            bevy_grimoire_core::atomic_file::write(&target, held).map_err(|e| format!("{} -> {}: {e}", path.display(), target.display()))?;
        }
    }
    Ok(())
}

/// Whether a file is one of the project's YAML text files.
fn is_yaml(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e == "yaml" || e == "yml")
}

/// **Every YAML file under a project's grimoire directory**, by path relative to it (`maps/bedroom.map.yaml`,
/// `furniture/terrain/yard.yaml`), with its text, sorted.
fn yaml_files(grimoire: &Path) -> Vec<(String, String)> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, String)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let (path, name) = (entry.path(), entry.file_name().to_string_lossy().into_owned());
            let rel = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
            if path.is_dir() {
                walk(&path, &rel, out);
            } else if is_yaml(&path)
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                out.push((rel, text));
            }
        }
    }
    let mut out = Vec::new();
    walk(grimoire, "", &mut out);
    out.sort();
    out
}

/// **The checks the change asks of a run**, from the map before it and the map after.
///
/// An added placement and a turned or moved one get a `placement_at` with the tolerances above; a removed
/// one an absent `placement_exists`. The map is read through the core's own parser, and a map that does not
/// read says so rather than yielding no checks.
fn placement_checks(before: &str, after: &str, file: &str) -> Result<Vec<Value>, String> {
    let (before, after) = (
        Map::parse(before).map_err(|e| format!("`{file}` before the change does not read: {e}"))?,
        Map::parse(after).map_err(|e| format!("`{file}` after the change does not read: {e}"))?,
    );
    let mut checks = Vec::new();
    for row in &after.placements {
        let was = before.placements.iter().find(|p| p.id == row.id);
        let same = was.is_some_and(|p| {
            p.at == row.at && (p.yaw - row.yaw).abs() < f32::EPSILON && (p.lift - row.lift).abs() < f32::EPSILON && p.tip == row.tip
        });
        if same {
            continue;
        }
        let mut body = json!({
            "id": row.id,
            "at": [row.at.0, row.at.1],
            "tolerance_m": PLACEMENT_TOLERANCE_M,
            "yaw_deg": row.yaw,
            "yaw_tolerance_deg": YAW_TOLERANCE_DEG,
        });
        if row.lift.abs() > f32::EPSILON {
            body["lift_m"] = json!(row.lift);
            body["lift_tolerance_m"] = json!(LIFT_TOLERANCE_M);
        }
        checks.push(json!({ "placement_at": body }));
    }
    for row in &before.placements {
        if !after.placements.iter().any(|p| p.id == row.id) {
            checks.push(json!({ "placement_exists": { "id": row.id, "count": 0 } }));
        }
    }
    Ok(checks)
}

/// A step of a task, in the shape the task file writes it.
#[derive(Serialize)]
struct Step {
    op: String,
    params: Value,
}

/// A task's budget, in the shape the task file writes it.
#[derive(Serialize)]
struct Budget {
    wall_s: u32,
    tool_calls: u32,
}

/// **A task file, in the order a person reads it.** The bench crate reads this with its own `Task`, which
/// refuses any field written here that it does not have; the order is the file's, not a rule.
#[derive(Serialize)]
struct TaskFile {
    id: String,
    title: String,
    split: &'static str,
    tags: Vec<String>,
    map: String,
    prompt: String,
    setup: Vec<Step>,
    solution: Vec<Step>,
    checks: Vec<Value>,
    budget: Budget,
}

/// **The map's file stem**: `bedroom` for `…/bedroom.map.yaml`.
fn stem(map_path: &Path) -> Result<String, String> {
    map_path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(MAP_SUFFIX))
        .map(str::to_owned)
        .ok_or_else(|| format!("{} is not a map file", map_path.display()))
}

/// **Make a benchmark task draft of a hand-back the person kept.**
///
/// Reads the operation log, rebuilds the maps as they were on a **copy** of the project's data, writes the
/// draft under [`DRAFTS_DIR`], and says where. A draft already there is left as it is and named, because the
/// person may have edited it: a second press is "show me", not "write it again".
///
/// The open map is never touched, and nothing is written outside the draft's own folder and a scratch
/// directory this removes.
pub fn make(project: &Project, open: &OpenMap, handback: &HandBack) -> Result<Made, String> {
    let id = draft_id(handback);
    let folder = project.root.join(DRAFTS_DIR).join(&id);
    let rel = format!("{DRAFTS_DIR}/{id}/{TASK_FILE}");
    if folder.join(TASK_FILE).is_file() {
        return Ok(Made { id, rel, written: false });
    }
    let entries = ops::read_entries(&ops::oplog_path(project));
    let applied = covered(&entries, handback)?;
    if let Some(op) = lacks_inverse(&entries, handback) {
        return Err(format!(
            "`{op}` recorded no inverse, so the map before this hand-back cannot be rebuilt and no task can be made of it"
        ));
    }
    let map = stem(&open.map_path)?;

    // **The copy**: the project's data directory, in a scratch folder of its own, which is what the undo walk
    // is run on. The real project, its map and its log are read and never written.
    let scratch = std::env::temp_dir().join(format!("grimoire-bench-draft-{}-{id}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let grimoire = project.root.join(GRIMOIRE_DIR);
    // **The authoring kit the copy opens with**, the directory under the grimoire directory the project writes new work to.
    let kit = project
        .grimoire_dir
        .strip_prefix(&grimoire)
        .ok()
        .and_then(|p| p.to_str())
        .filter(|k| !k.is_empty())
        .map(str::to_owned);
    let made = (|| {
        let copied = scratch.join(GRIMOIRE_DIR);
        copy_yaml_tree(&grimoire, &copied)?;
        let after = yaml_files(&copied);
        let copy = Project::open(&scratch, kit.as_deref())?;
        // **The log goes with the copy**, because the undo walk reads the inverses from it; the walk's own undo lines are written to the
        // copy's log, and the project's is read and never written.
        let (log, copied_log) = (ops::oplog_path(project), ops::oplog_path(&copy));
        if let Some(parent) = copied_log.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let held = std::fs::read(&log).map_err(|e| format!("{}: {e}", log.display()))?;
        bevy_grimoire_core::atomic_file::write(&copied_log, held).map_err(|e| format!("{} -> {}: {e}", log.display(), copied_log.display()))?;
        let opened = OpenMap::open(&copy, &map)?;
        let mut ctx = Ctx {
            project: Some(copy),
            open: Some(opened),
            source: Source::Human,
            session: ops::session_id(),
            ..Ctx::default()
        };
        // **Newest first**, because the map's state can only be rewound from its newest change backwards.
        for entry in applied.iter().rev() {
            match ops::undo(&mut ctx, Some(&entry.operation_id)) {
                Ok((_, Applied::Done { .. })) => {}
                Ok((_, Applied::Refused(why))) | Err(why) => {
                    return Err(format!(
                        "taking `{}` back on a copy stopped: {}. A task is made of a hand-back with no newer change on it; take that change back first, or make the task before the next one.",
                        entry.op, why.0
                    ));
                }
            }
        }
        Ok((after, yaml_files(&copied)))
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    let (after, before) = made?;

    // **What changed**: the YAML files whose text differs. The placements inside a map are what its checks ask about, and every map
    // the change left alone is one the run must leave alone.
    let mut checks: Vec<Value> = Vec::new();
    let mut changed: Vec<(String, String)> = Vec::new();
    let mut quiet: Vec<String> = Vec::new();
    let in_maps = |rel: &str| rel.strip_prefix(MAPS_DIR).is_some_and(|r| r.starts_with('/') && r.ends_with(MAP_SUFFIX) && !r[1..].contains('/'));
    for (name, text_after) in &after {
        let text_before = before.iter().find(|(n, _)| n == name).map(|(_, t)| t);
        match text_before {
            Some(text_before) if text_before != text_after => {
                if in_maps(name) {
                    checks.extend(placement_checks(text_before, text_after, name)?);
                }
                changed.push((name.clone(), text_before.clone()));
            }
            Some(_) if in_maps(name) => quiet.push(name.clone()),
            Some(_) => {}
            // A file the undo walk removed: it did not exist before. There is no "before" to lay.
            None => {}
        }
    }
    // **Only the checks drawn from the change count.** The `unchanged` check on the maps the hand-back left alone asks nothing about
    // the hand-back: a run that does nothing passes it, so a draft whose only checks are those measures nothing.
    if checks.is_empty() {
        return Err("this hand-back changed no placement, so a task of it would have no check that asks for its change, and a task that asks for nothing measures nothing".to_owned());
    }
    if !quiet.is_empty() {
        checks.push(json!({ "unchanged": quiet }));
    }

    let prompt = if handback.specified.is_empty() {
        format!("{}\n\nWhen it is done, hand the result back with grimoire_present.", handback.title)
    } else {
        let asked: Vec<String> = handback.specified.iter().map(|s| format!("- {s}")).collect();
        format!("Do this in the project:\n{}\n\nWhen it is done, hand the result back with grimoire_present.", asked.join("\n"))
    };
    let mut solution: Vec<Step> =
        applied.iter().map(|e| Step { op: e.op.clone(), params: e.params.clone() }).collect();
    solution.push(Step {
        op: "grimoire_present".to_owned(),
        params: json!({
            "title": handback.title,
            "summary": format!("{} operation(s) were applied: {}.", applied.len(), applied.iter().map(|e| e.op.as_str()).collect::<Vec<_>>().join(", ")),
            "specified": handback.specified,
        }),
    });
    let file = TaskFile {
        id: id.clone(),
        title: handback.title.clone(),
        split: "dev",
        tags: vec!["draft".to_owned()],
        map,
        prompt,
        setup: Vec::new(),
        solution,
        checks,
        budget: Budget { wall_s: BUDGET_WALL_S, tool_calls: BUDGET_TOOL_CALLS },
    };
    let body = bevy_grimoire_core::yaml::to_string(&file).map_err(|p| format!("the draft could not be written: {}", p.line()))?;
    let text = format!("{}{body}", header(handback));

    // **The scene**: each map the change touched, as it was, in the task's own `fixture/`.
    let fixture = folder.join("fixture").join(GRIMOIRE_DIR);
    for (name, text_before) in &changed {
        write(&fixture.join(name), text_before)?;
    }
    write(&folder.join(TASK_FILE), &text)?;
    Ok(Made { id, rel, written: true })
}

/// **Write one file atomically**, making its folder.
fn write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    bevy_grimoire_core::atomic_file::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A title becomes a folder name a person can read, however it is punctuated.
    #[test]
    fn a_title_is_a_readable_folder_name() {
        assert_eq!(slug("The bed — against the north wall!"), "the-bed-against-the-north-wall");
        assert_eq!(slug("  ---  "), "");
        assert!(slug(&"word ".repeat(40)).chars().count() <= SLUG_MAX_CHARS);
        assert!(!slug(&"word ".repeat(40)).ends_with('-'));
    }

    /// An added, a turned and a removed placement each get the check that asks for it, and an untouched
    /// one gets none.
    #[test]
    fn each_kind_of_change_gets_its_check() {
        let before = "version: 9\nname: m\nnote: null\norigin:\n- 0.0\n- 0.0\n- 0.0\nbounds:\n- 6.0\n- 3.0\n- 5.0\nbash: null\nplacements:\n\
- id: keep\n  descriptor: chair\n  at:\n  - 1.0\n  - 1.0\n  yaw: 0.0\n\
- id: turn\n  descriptor: chair\n  at:\n  - 2.0\n  - 1.0\n  yaw: 0.0\n\
- id: gone\n  descriptor: chair\n  at:\n  - 3.0\n  - 1.0\n  yaw: 0.0\n";
        let after = "version: 9\nname: m\nnote: null\norigin:\n- 0.0\n- 0.0\n- 0.0\nbounds:\n- 6.0\n- 3.0\n- 5.0\nbash: null\nplacements:\n\
- id: keep\n  descriptor: chair\n  at:\n  - 1.0\n  - 1.0\n  yaw: 0.0\n\
- id: turn\n  descriptor: chair\n  at:\n  - 2.0\n  - 1.0\n  yaw: 90.0\n\
- id: new\n  descriptor: chair\n  at:\n  - 4.0\n  - 2.0\n  yaw: 0.0\n";
        let checks = placement_checks(before, after, "m.map.yaml").unwrap_or_else(|e| panic!("{e}"));
        let names: Vec<String> = checks
            .iter()
            .map(|c| {
                let (name, body) = c.as_object().and_then(|o| o.iter().next()).map(|(k, v)| (k.clone(), v.clone())).unwrap_or_default();
                format!("{name}:{}", body["id"].as_str().unwrap_or_default())
            })
            .collect();
        assert_eq!(names, ["placement_at:turn", "placement_at:new", "placement_exists:gone"]);
    }

    fn hand(id: &str, title: &str, operations: &[&str]) -> HandBack {
        HandBack {
            kind: crate::present::HandBackKind::Presentation,
            id: id.to_owned(),
            key: id.to_owned(),
            root: std::path::PathBuf::from("/p"),
            title: title.to_owned(),
            operation_ids: operations.iter().map(|s| (*s).to_owned()).collect(),
            specified: Vec::new(),
            accepted: true,
            draft: None,
        }
    }

    /// **Two sessions' `present-1` with one title are two drafts** (review): the number restarts with every editor session, so the id
    /// carries a stamp of the operations. The same hand-back is the same draft, so a second press still finds it.
    #[test]
    fn a_later_sessions_handback_with_the_same_title_is_a_different_draft() {
        let first = hand("present-1", "The bed", &["op-1", "op-2"]);
        let later = hand("present-1", "The bed", &["op-7", "op-8"]);
        assert_ne!(draft_id(&first), draft_id(&later), "two hand-backs share one draft folder");
        assert_eq!(draft_id(&first), draft_id(&first.clone()), "one hand-back has one draft");
        assert!(draft_id(&first).starts_with("present-1-the-bed-"), "{}", draft_id(&first));
        assert_ne!(draft_id(&hand("present-1", "!!!", &["a"])), draft_id(&hand("present-1", "???", &["b"])), "an empty slug still tells hand-backs apart");
    }

    /// **An agent's title cannot end the comment** and put a line of its own in the file: a newline, a carriage return or a Unicode line
    /// break in it is a space, and every line of the header stays a comment.
    #[test]
    fn a_title_with_a_newline_cannot_write_into_the_draft() {
        for title in ["bed\nkey: value", "bed\r\nkey: value", "bed\u{2028}key: value", "bed\u{85}key: value", "bed\0key: value"] {
            let text = header(&hand("present-1", title, &["op-1"]));
            for line in text.lines() {
                assert!(line.starts_with('#'), "`{title:?}` wrote a line that is not a comment: {line:?}");
            }
            assert!(text.contains("bed key: value"), "the title is kept, on one line: {text}");
        }
    }
}
