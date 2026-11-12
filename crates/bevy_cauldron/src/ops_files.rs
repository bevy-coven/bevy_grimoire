//! **A replace across the project's files, as one registered operation** (GD-13, #375): `files_replace`.
//!
//! `Cmd`/`Ctrl`+`Shift`+`F`'s Replace all (the project search panel, [`crate::files_panel`]) and an agent's
//! `grimoire/operation_apply` are this one operation, so a project-wide rename has **one log line and one
//! undo**, whoever made it. The work is [`crate::files_search`]: the query is planned over every file it
//! reaches, a plan that would leave a file unreadable is refused **before anything is written** (naming the
//! file and the line), and the files are then written atomically, all or none.
//!
//! # The inverse is every file's old text
//!
//! The change records each file's text from before as its inverse (`restore`), so one undo puts every file
//! back **byte for byte**, including a file whose own spelling (a `1.50` a writer would have shortened) the
//! replace never touched. Like `data_set`'s put-back it is the operation's own parameters, and only an undo
//! may send it ([`Operation::only_as_undo`]). An undo is refused when a file has changed since the replace
//! (`state_of`): a newer change is never overwritten.
//!
//! `restore` carries whole texts, so the log line of a replace over many large files is large; the replace is
//! limited to files of at most [`crate::files::MAX_FILE_BYTES`] for that reason too.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::files_search::{self, Query};
use crate::ops::{Ctx, Operation, Outcome, Refusal, Shown, Undo};

/// One file's text as it was, for the undo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Restore {
    /// Relative to the project's root.
    pub path: String,
    /// The file's whole text.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplaceParams {
    /// What to find: the text, or the regular expression.
    #[serde(default)]
    pub query: String,
    /// What each match becomes (a regular expression's may use `$1`).
    #[serde(default)]
    pub replacement: String,
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default)]
    pub whole_word: bool,
    #[serde(default)]
    pub regex: bool,
    /// Which paths: patterns separated by commas or spaces (`*.yaml`, `assets/grimoire/`, `maps`). Empty is every
    /// file the file tree lists.
    #[serde(default)]
    pub paths: String,
    /// Only these files (their paths under the project), when the preview was narrowed by hand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<String>>,
    /// **The undo's own form**: every file's old text, put back verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore: Option<Vec<Restore>>,
}

impl ReplaceParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["query", "replacement"],
            "properties": {
                "query": {"type": "string", "description": "What to find in the project's text files: the text, or a regular expression when `regex` is true."},
                "replacement": {"type": "string", "description": "What each match becomes. A regular expression's replacement may use $1."},
                "case_sensitive": {"type": "boolean", "description": "Letters match only the same case. Default false."},
                "whole_word": {"type": "boolean", "description": "A match is a whole word. Default false."},
                "regex": {"type": "boolean", "description": "The query is a regular expression (no look-around, no back-references). Default false."},
                "paths": {"type": "string", "description": "Which files, as patterns separated by commas or spaces: `*.yaml` (a name), `assets/*.yaml` (a path), `assets/grimoire/` (a folder), `maps` (a piece of the path). Empty is every text file of the project except target/ and what .gitignore names."},
                "only": {"type": "array", "items": {"type": "string"}, "description": "Only these files, by path under the project, when the preview was narrowed."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<ReplaceParams, Refusal> {
        let p: ReplaceParams = serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!(
                "these are not a project replace: {e}. The fields are `query`, `replacement`, `case_sensitive`, `whole_word`, `regex`, `paths` and `only`."
            ))
        })?;
        if p.restore.is_none() && p.query.is_empty() {
            return Err(Refusal::new("`query` is empty, so there is nothing to find and nothing was replaced."));
        }
        Ok(p)
    }

    fn query(&self) -> Query {
        Query { text: self.query.clone(), case_sensitive: self.case_sensitive, whole_word: self.whole_word, regex: self.regex, paths: self.paths.clone() }
    }
}

fn root_of(ctx: &Ctx) -> Result<std::path::PathBuf, Refusal> {
    ctx.root_or_project().ok_or_else(|| Refusal::new("no project is open, so there are no files to replace in."))
}

/// The text of `rel` now, or `None` when the file is not there or is not text.
fn current(root: &Path, rel: &str) -> Option<String> {
    std::fs::read_to_string(root.join(rel)).ok()
}

/// **Replace across the project's files.**
pub struct FilesReplace;

impl FilesReplace {
    /// The plan for a query, as a refusal when it cannot be one.
    fn planned(ctx: &Ctx, p: &ReplaceParams) -> Result<(std::path::PathBuf, files_search::Plan), Refusal> {
        let root = root_of(ctx)?;
        // **One plan for the plan, the place and the preview of one call**: dispatch asks all three, and each would read every
        // file again. The last plan is kept for a few seconds against the same root and parameters; a commit plans afresh.
        let key = (root.clone(), serde_json::to_string(p).unwrap_or_default());
        if let Some(plan) = cached_plan(&key) {
            return Ok((root, plan));
        }
        let plan = Self::fresh(&root, p)?;
        keep_plan(key, &plan);
        Ok((root, plan))
    }

    /// A plan read from the files now: refused when it would break a file, or reaches one a person is typing in.
    fn fresh(root: &std::path::Path, p: &ReplaceParams) -> Result<files_search::Plan, Refusal> {
        let plan = files_search::plan(root, &p.query(), &p.replacement, p.only.as_deref()).map_err(|e| Refusal::new(e.to_string()))?;
        // **An agent never merges into a person's unsaved text**, and a replace is an agent's reach too.
        if let Some(file) = plan.files.iter().find(|f| crate::files::person_is_editing(root, &f.rel)) {
            return Err(Refusal::new(format!("the person is editing {}; wait or ask. Nothing was changed.", file.rel)));
        }
        Ok(plan)
    }

    fn put_back(ctx: &Ctx, restore: &[Restore], write: bool) -> Result<Outcome, Refusal> {
        let root = root_of(ctx)?;
        let mut undone = Vec::new();
        let mut now = Vec::new();
        for r in restore {
            crate::files::project_path(&root, &r.path).map_err(Refusal::new)?;
            undone.push(Restore { path: r.path.clone(), text: current(&root, &r.path).unwrap_or_default() });
            now.push((r.path.clone(), r.text.clone()));
        }
        if write {
            for (written, (path, text)) in now.iter().enumerate() {
                if let Err(e) = bevy_grimoire_core::atomic_file::write(&root.join(path), text) {
                    let stuck: Vec<&str> = undone[..written]
                        .iter()
                        .filter(|back| bevy_grimoire_core::atomic_file::write(&root.join(&back.path), &back.text).is_err())
                        .map(|back| back.path.as_str())
                        .collect();
                    return Err(Refusal::new(if stuck.is_empty() {
                        format!("{path} was not put back ({e}); the files before it were left as they were.")
                    } else {
                        format!("{path} was not put back ({e}); these could NOT be returned to what they held and are half put back: {}", stuck.join(", "))
                    }));
                }
            }
        }
        let after: serde_json::Map<String, serde_json::Value> = now.iter().map(|(p, t)| (p.clone(), json!(t))).collect();
        Ok(Outcome {
            summary: format!("put back {} file(s)", restore.len()),
            affected: restore.len(),
            saved: write,
            inverse: Some(json!({"restore": undone})),
            after: Some(json!({"files": after})),
        })
    }
}

impl Operation for FilesReplace {
    fn name(&self) -> &'static str {
        "files_replace"
    }

    fn undo(&self) -> Undo {
        Undo::Inverse
    }

    fn schema(&self) -> serde_json::Value {
        ReplaceParams::schema()
    }

    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p = ReplaceParams::parse(params)?;
        if let Some(restore) = &p.restore {
            Self::put_back(ctx, restore, false)?;
            return Ok(json!({"summary": format!("put back {} file(s)", restore.len())}));
        }
        let (_, plan) = Self::planned(ctx, &p)?;
        let files: Vec<serde_json::Value> = plan
            .files
            .iter()
            .map(|f| {
                json!({
                    "path": f.rel,
                    "replacements": f.replacements,
                    "changes": f.changes.iter().map(|c| json!({"line": c.line + 1, "old": c.old, "new": c.new})).collect::<Vec<_>>(),
                })
            })
            .collect();
        let skipped = plan.skipped.clone();
        Ok(json!({
            "summary": format!("replace {} match(es) in {} file(s)", plan.replacements(), plan.files.len()),
            "files": files,
            "skipped": skipped,
        }))
    }

    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let p = ReplaceParams::parse(params)?;
        if let Some(restore) = &p.restore {
            return Self::put_back(ctx, restore, true);
        }
        let root = root_of(ctx)?;
        let plan = Self::fresh(&root, &p)?;
        let applied = files_search::apply(&root, &plan);
        // The files changed: what was planned from them is stale.
        *LAST_PLAN.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        applied.map_err(|e| Refusal::new(format!("{e}. Nothing was changed.")))?;
        let before: Vec<Restore> = plan.files.iter().map(|f| Restore { path: f.rel.clone(), text: f.before.clone() }).collect();
        let after: serde_json::Map<String, serde_json::Value> = plan.files.iter().map(|f| (f.rel.clone(), json!(f.after))).collect();
        let skipped = if plan.skipped.is_empty() { String::new() } else { format!("; left {} read-only file(s) alone", plan.skipped.len()) };
        Ok(Outcome {
            summary: format!("replaced {} match(es) in {} file(s){skipped}", plan.replacements(), plan.files.len()),
            affected: plan.replacements(),
            saved: true,
            inverse: Some(json!({"restore": before})),
            after: Some(json!({"files": after})),
        })
    }

    /// **The files, as they are on disk now**: an undo is refused when one is not what the change left.
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let root = ctx.root_or_project()?;
        let files = after.get("files")?.as_object()?;
        let now: serde_json::Map<String, serde_json::Value> =
            files.keys().map(|rel| (rel.clone(), current(&root, rel).map_or(serde_json::Value::Null, serde_json::Value::from))).collect();
        Some(json!({"files": now}))
    }

    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        ReplaceParams::parse(params).map(|p| p.restore.is_some()).unwrap_or(false)
    }

    fn writes(&self) -> &'static [&'static str] {
        // A project-wide replace writes the project's own text files, whatever they are: the kind that says "the
        // game's own files", which a running game follows live.
        &["game_data"]
    }

    fn shows(&self, ctx: &Ctx, params: &serde_json::Value) -> Shown {
        // The first file the replace would change, with its first changed line.
        let Ok(p) = ReplaceParams::parse(params) else { return Shown::File { path: PROJECT_FOLDER.to_owned(), lines: None } };
        let first = match &p.restore {
            Some(restore) => restore.first().map(|r| (r.path.clone(), None)),
            None => Self::planned(ctx, &p).ok().and_then(|(_, plan)| {
                plan.files.first().map(|f| (f.rel.clone(), f.changes.first().map(|c| [c.line as u32 + 1, c.line as u32 + 1])))
            }),
        };
        match first {
            Some((path, lines)) => Shown::File { path, lines },
            None => Shown::File { path: PROJECT_FOLDER.to_owned(), lines: None },
        }
    }
}

/// How long a plan is kept for the other questions of the same call, in seconds.
const PLAN_KEPT_S: u64 = 5;

type PlanKey = (std::path::PathBuf, String);

static LAST_PLAN: std::sync::LazyLock<std::sync::Mutex<Option<(PlanKey, std::time::Instant, files_search::Plan)>>> = std::sync::LazyLock::new(Default::default);

fn cached_plan(key: &PlanKey) -> Option<files_search::Plan> {
    let last = LAST_PLAN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let (kept, at, plan) = last.as_ref()?;
    (kept == key && at.elapsed().as_secs() < PLAN_KEPT_S).then(|| plan.clone())
}

fn keep_plan(key: PlanKey, plan: &files_search::Plan) {
    *LAST_PLAN.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((key, std::time::Instant::now(), plan.clone()));
}

/// **What a replace that has no file to point at names**: the project's own folder, the tree the Files workspace shows. A
/// replace always has somewhere a person can look (its files), so it never answers `Nowhere`.
const PROJECT_FOLDER: &str = ".";

// ── file_edit ────────────────────────────────────────────────────────────────

/// **Where an edit lands.** A field path (`placements[0].yaw`) names one YAML scalar; `{"lines": [3, 5]}` names
/// lines 3 to 5 (1-based, inclusive) of any text file, and `{"line": 3}` one line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum At {
    Field(String),
    Lines { lines: [usize; 2] },
    Line { line: usize },
}

/// **One edit**: where, and the new value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    pub at: At,
    /// For a field: the new scalar, of the kind the leaf already holds. For lines: the new text of those lines.
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditParams {
    /// The file, relative to the project's root.
    pub path: String,
    /// The edits, in order: each sees the text the one before it left.
    #[serde(default)]
    pub edits: Vec<Edit>,
    /// **The undo's own form**: the file's whole old text, put back verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore: Option<String>,
}

impl EditParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["path", "edits"],
            "properties": {
                "path": {"type": "string", "description": "The file to edit, relative to the project's root. The command files (agent, play) and the guide are refused: they are changed on their own pages."},
                "edits": {
                    "type": "array",
                    "minItems": 1,
                    "description": "The edits, in order; each sees the text the one before it left. A YAML field edit keeps every comment, and the file must still read (and validate against its kind) afterwards, or nothing is written.",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["at", "value"],
                        "properties": {
                            "at": {
                                "description": "Where: a YAML field path such as `placements[0].yaw`, or `{\"lines\": [3, 5]}` (1-based, inclusive) or `{\"line\": 3}` for any text file.",
                                "oneOf": [
                                    {"type": "string"},
                                    {"type": "object", "additionalProperties": false, "required": ["lines"], "properties": {"lines": {"type": "array", "items": {"type": "integer", "minimum": 1}, "minItems": 2, "maxItems": 2}}},
                                    {"type": "object", "additionalProperties": false, "required": ["line"], "properties": {"line": {"type": "integer", "minimum": 1}}}
                                ]
                            },
                            "value": {"description": "A field's new scalar (a number, string or bool of the kind the leaf already holds), or the new text of the lines."}
                        }
                    }
                }
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<EditParams, Refusal> {
        let p: EditParams = serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("these are not a file edit: {e}. The fields are `path` and `edits` (each `{{at, value}}`), and nothing else."))
        })?;
        if p.restore.is_none() && p.edits.is_empty() {
            return Err(Refusal::new("`edits` is empty, so there is nothing to change."));
        }
        Ok(p)
    }
}

/// A scalar a field edit writes, from JSON.
fn scalar(value: &serde_json::Value) -> Result<bevy_grimoire_core::yaml_surgery::Value, String> {
    use bevy_grimoire_core::yaml_surgery::Value;
    match value {
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::String(s) => Ok(Value::Str(s.clone())),
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Number(n) => match n.as_i64() {
            Some(i) => Ok(Value::Int(i)),
            None => n.as_f64().map(Value::Float).ok_or_else(|| format!("{n} is not a number a file can hold")),
        },
        other => Err(format!("a field edit sets one scalar (a number, a string or a bool), not {other}")),
    }
}

/// **Replace lines `from..=to` (1-based) of `text`** with `new`. `to == from - 1` inserts before `from`, which
/// is how a line is added at the end.
fn replace_lines(text: &str, from: usize, to: usize, new: &str) -> Result<String, String> {
    let count = text.split_inclusive('\n').count();
    if from == 0 || to + 1 < from || from > count + 1 || to > count {
        return Err(format!("lines {from} to {to} are not in a file of {count} line(s)"));
    }
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let head: String = lines[..from - 1].concat();
    let tail: String = lines[to..].concat();
    let replaced_end_had_break = to > 0 && lines.get(to - 1).is_some_and(|l| l.ends_with('\n'));
    let mut middle = new.to_owned();
    if !middle.is_empty() && !middle.ends_with('\n') && (replaced_end_had_break || !tail.is_empty() || to < from) {
        middle.push('\n');
    }
    Ok(format!("{head}{middle}{tail}"))
}

/// **Apply the edits to a text**, in order, and check what they leave reads.
fn edited(rel: &str, text: &str, edits: &[Edit]) -> Result<String, String> {
    let lang = crate::files_lang::Lang::of(rel);
    let mut now = text.to_owned();
    for (n, edit) in edits.iter().enumerate() {
        let which = format!("edit {}", n + 1);
        now = match &edit.at {
            At::Field(path) => {
                if lang != crate::files_lang::Lang::Yaml {
                    return Err(format!("{which}: `{path}` is a field path, and {rel} is not YAML; edit it by lines (`{{\"lines\": [from, to]}}`)"));
                }
                let value = scalar(&edit.value).map_err(|e| format!("{which}: {e}"))?;
                bevy_grimoire_core::yaml_surgery::set(&now, path, value, None).map_err(|p| format!("{which}: {}", p.line()))?
            }
            At::Lines { lines: [from, to] } => replace_lines(&now, *from, *to, text_of(&edit.value, &which)?).map_err(|e| format!("{which}: {e}"))?,
            At::Line { line } => replace_lines(&now, *line, *line, text_of(&edit.value, &which)?).map_err(|e| format!("{which}: {e}"))?,
        };
    }
    // **The file must still read** (and validate against its kind) or nothing is written.
    if let Some(finding) = crate::files_lang::diagnose(rel, &now).first() {
        let line = now[..finding.range.start].matches('\n').count() + 1;
        return Err(format!("this would leave {rel} unreadable (line {line}: {}), so nothing was written", finding.message));
    }
    Ok(now)
}

fn text_of<'a>(value: &'a serde_json::Value, which: &str) -> Result<&'a str, String> {
    value.as_str().ok_or_else(|| format!("{which}: the new text of lines is a string"))
}

/// **Edit a project file**: the agent's way to change a YAML or script file, with comments kept.
pub struct FileEdit;

impl FileEdit {
    /// What the file would hold, or why not: every refusal found before anything is written.
    fn resolve(ctx: &Ctx, p: &EditParams) -> Result<(std::path::PathBuf, String, String), Refusal> {
        let root = root_of(ctx)?;
        crate::files::project_path(&root, &p.path).map_err(Refusal::new)?;
        if crate::files_lang::is_command_file(&p.path) {
            return Err(Refusal::new(format!("{}: {} Nothing was changed.", p.path, crate::files_lang::READ_ONLY_NOTE)));
        }
        // **An agent never merges into a person's unsaved text.**
        if crate::files::person_is_editing(&root, &p.path) {
            return Err(Refusal::new(format!("the person is editing {}; wait or ask. Nothing was changed.", p.path)));
        }
        let old = current(&root, &p.path).ok_or_else(|| Refusal::new(format!("{} is not a text file in the project, so nothing was changed.", p.path)))?;
        let new = match &p.restore {
            Some(text) => text.clone(),
            None => edited(&p.path, &old, &p.edits).map_err(|e| Refusal::new(format!("{e}. Nothing was changed.")))?,
        };
        Ok((root, old, new))
    }
}

impl Operation for FileEdit {
    fn name(&self) -> &'static str {
        "file_edit"
    }

    fn undo(&self) -> Undo {
        Undo::Inverse
    }

    fn schema(&self) -> serde_json::Value {
        EditParams::schema()
    }

    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p = EditParams::parse(params)?;
        let (_, old, new) = Self::resolve(ctx, &p)?;
        Ok(json!({"summary": format!("edit {}", p.path), "path": p.path, "lines": crate::files::changed_lines(&old, &new)}))
    }

    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let p = EditParams::parse(params)?;
        let (root, old, new) = Self::resolve(ctx, &p)?;
        if new != old {
            bevy_grimoire_core::atomic_file::write(&root.join(&p.path), &new)
                .map_err(|e| Refusal::new(format!("{} was not written ({e}). Nothing was changed.", p.path)))?;
            if ctx.source == crate::ops::Source::Agent {
                crate::files::note_agent_edit(&root, &p.path);
            }
        }
        Ok(Outcome {
            summary: format!("edited {}", p.path),
            affected: usize::from(new != old),
            saved: new != old,
            inverse: Some(json!({"path": p.path, "restore": old})),
            after: Some(json!({"path": p.path, "text": new})),
        })
    }

    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let root = ctx.root_or_project()?;
        let path = after.get("path")?.as_str()?;
        Some(json!({"path": path, "text": current(&root, path)?}))
    }

    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        EditParams::parse(params).map(|p| p.restore.is_some()).unwrap_or(false)
    }

    fn writes(&self) -> &'static [&'static str] {
        &["game_data"]
    }

    fn shows(&self, ctx: &Ctx, params: &serde_json::Value) -> Shown {
        let Ok(p) = EditParams::parse(params) else { return Shown::File { path: PROJECT_FOLDER.to_owned(), lines: None } };
        let lines = Self::resolve(ctx, &p).ok().and_then(|(_, old, new)| crate::files::changed_lines(&old, &new));
        Shown::File { path: p.path, lines }
    }
}
