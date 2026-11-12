//! **Find and replace across the project** (GD-13, #375): the engine, with no window in it.
//!
//! `Cmd`/`Ctrl`+`Shift`+`F` in the Files workspace searches every text file of the project
//! ([`crate::files::listed_files`], so `target/` and what `.gitignore` names are not searched), with the
//! options `bevy_gooey`'s own find bar has (case, whole word, regular expression: **the same engine**,
//! [`bevy_gooey::find_all`] and [`bevy_gooey::replace_edits`], so a pattern means the same thing in a file's
//! bar and across the project) and a filter on the path.
//!
//! # A replace is planned, checked, then written, and it is one operation
//!
//! [`plan`] reads every file the query matches and says what each would become, **writing nothing**. It
//! refuses, naming the file and line, when the replace would leave a file that read **not reading**
//! ([`crate::files_lang::diagnose`]: a YAML file by its kind's own reader or the event walk, a RON file by
//! `ron`), and then nothing is written for any file: a project-wide rename is all of it or none of it.
//! [`apply`] writes the planned files atomically and, if one write fails, puts back the ones already written.
//! The operation `files_replace` ([`crate::ops_files`]) is [`plan`] then [`apply`], logged with an inverse
//! that puts every file back byte for byte, so one undo takes the whole replace back.
//!
//! # What is not replaced
//!
//! The files in `trust::COMMAND_FILES` and the guide ([`crate::files_lang::is_command_file`]) are never
//! written here: they are changed on their own pages, so a replace is not a way round a trust check. A
//! replace that matched one says so ([`Plan::skipped`]) rather than skipping it quietly.
//!
//! **Text replace is for text.** Renaming a reference (a piece id used by every map) is a typed operation
//! of its own, not a regular expression over files.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy_gooey::{CodeBuffer, CodeEdit, FindError, FindOptions, find_all, replace_edits};
use serde::{Deserialize, Serialize};

use crate::files::MAX_FILE_BYTES;
use crate::files_lang;

/// How many lines of context a result shows on each side of its line.
pub const CONTEXT_LINES: usize = 1;

/// **How many hits one file shows**: a file with a hundred thousand matches of a one-letter query is one
/// file's worth of rows, not the whole panel.
pub const HITS_PER_FILE: usize = 500;

/// How many files a search reads before it reports progress again.
pub const PROGRESS_EVERY: usize = 16;

/// **What to look for**, and where.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    /// The text, or the regular expression.
    pub text: String,
    /// Letters match only the same case.
    #[serde(default)]
    pub case_sensitive: bool,
    /// A match is a whole word.
    #[serde(default)]
    pub whole_word: bool,
    /// The text is a regular expression (the find bar's: no look-around, no back-references; the replacement may
    /// use `$1`).
    #[serde(default)]
    pub regex: bool,
    /// **Which paths are searched**: patterns separated by commas or spaces. A pattern with a `*` or `?` matches
    /// the whole path (`*` runs across folders: `assets/*.yaml`); one ending in `/` is a folder prefix
    /// (`assets/grimoire/`); any other is a piece of the path (`maps`). Empty is every file.
    #[serde(default)]
    pub paths: String,
}

impl Query {
    pub fn options(&self) -> FindOptions {
        FindOptions { case_sensitive: self.case_sensitive, whole_word: self.whole_word, regex: self.regex }
    }

    /// Whether the path filter admits `rel`.
    pub fn admits(&self, rel: &str) -> bool {
        let patterns: Vec<&str> = self.paths.split(|c: char| c == ',' || c.is_whitespace()).filter(|p| !p.is_empty()).collect();
        patterns.is_empty() || patterns.iter().any(|p| path_matches(p, rel))
    }
}

fn path_matches(pattern: &str, rel: &str) -> bool {
    if pattern.contains('*') || pattern.contains('?') {
        // A pattern without a `/` is about the file's name; one with it is about the path.
        let subject = if pattern.contains('/') { rel } else { rel.rsplit('/').next().unwrap_or(rel) };
        crate::files::wildcard(pattern, subject, true)
    } else if pattern.ends_with('/') {
        rel.starts_with(pattern) || rel.contains(&format!("/{pattern}"))
    } else {
        rel.contains(pattern)
    }
}

/// **One match, with its line and the lines around it.**
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    /// The line, from 0.
    pub line: usize,
    /// The byte offset into the line where the match starts, and how long it is on that line.
    pub col: usize,
    pub len: usize,
    /// The line's text.
    pub text: String,
    /// Up to [`CONTEXT_LINES`] lines before and after it.
    pub before: Vec<String>,
    pub after: Vec<String>,
}

/// **One file's matches.**
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileHits {
    pub rel: String,
    pub hits: Vec<Hit>,
    /// Whether the file had more than [`HITS_PER_FILE`].
    pub more: bool,
}

/// **Search one text**, the way the find bar does.
pub fn search_text(rel: &str, text: &str, query: &Query) -> Result<FileHits, FindError> {
    let buffer = CodeBuffer::new(text);
    let found = find_all(&buffer, &query.text, query.options())?;
    let more = found.len() > HITS_PER_FILE;
    let hits = found
        .into_iter()
        .take(HITS_PER_FILE)
        .map(|(start, end)| {
            let line_text = buffer.line(start.line).to_owned();
            let len = if end.line == start.line { end.col - start.col } else { line_text.len() - start.col };
            let near = |from: usize, to: usize| (from..to).map(|l| buffer.line(l).to_owned()).collect::<Vec<_>>();
            Hit {
                line: start.line,
                col: start.col,
                len,
                before: near(start.line.saturating_sub(CONTEXT_LINES), start.line),
                after: near((start.line + 1).min(buffer.len_lines()), (start.line + 1 + CONTEXT_LINES).min(buffer.len_lines())),
                text: line_text,
            }
        })
        .collect();
    Ok(FileHits { rel: rel.to_owned(), hits, more })
}

/// **Read a file the search may look in**: text, small enough, and not one of the files that are never
/// touched. `None` for a file that is none of those.
pub fn read_searchable(root: &Path, rel: &str) -> Option<String> {
    let full = root.join(rel);
    let meta = std::fs::metadata(&full).ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return None;
    }
    String::from_utf8(std::fs::read(full).ok()?).ok()
}

/// **Search the project**, one file at a time, calling `on_file` with each file that has a match and
/// `on_progress(read, of)` as it goes. Stops, with what it found, when `cancel` is set. A regular
/// expression that does not compile is the one error, and it is found on the first file.
///
/// Runs on whatever thread calls it: the panel runs it on one of its own (a background job, UI-22), and a
/// test runs it where it stands.
pub fn search(
    root: &Path,
    query: &Query,
    cancel: &AtomicBool,
    mut on_file: impl FnMut(FileHits),
    mut on_progress: impl FnMut(usize, usize),
) -> Result<(), FindError> {
    if query.text.is_empty() {
        return Ok(());
    }
    let candidates: Vec<String> = crate::files::listed_files(root).into_iter().filter(|rel| query.admits(rel)).collect();
    let total = candidates.len();
    for (n, rel) in candidates.into_iter().enumerate() {
        if cancel.load(Ordering::Acquire) {
            return Ok(());
        }
        if let Some(text) = read_searchable(root, &rel) {
            let found = search_text(&rel, &text, query)?;
            if !found.hits.is_empty() {
                on_file(found);
            }
        }
        if n % PROGRESS_EVERY == 0 || n + 1 == total {
            on_progress(n + 1, total);
        }
    }
    Ok(())
}

// ── Replace ──────────────────────────────────────────────────────────────────

/// **One line a replace changes**, as the preview's diff shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// The line, from 0, in the file as it is now.
    pub line: usize,
    pub old: String,
    pub new: String,
}

/// **One file a replace would write**: what it holds and what it would hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedFile {
    pub rel: String,
    pub before: String,
    pub after: String,
    /// How many matches were replaced.
    pub replacements: usize,
    pub changes: Vec<Change>,
}

/// **Everything a replace would do**, written nowhere.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub files: Vec<PlannedFile>,
    /// Files that matched and are never written here (the command files and the guide).
    pub skipped: Vec<String>,
}

impl Plan {
    /// How many matches would be replaced, in all.
    pub fn replacements(&self) -> usize {
        self.files.iter().map(|f| f.replacements).sum()
    }
}

/// **Why a replace did not happen.** Every one names what to do, and none writes anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The query is empty, or matches nothing.
    NothingToReplace,
    /// The pattern does not compile or took too much work.
    Pattern(String),
    /// The replace would leave a file that reads not reading: the file, the line, and the reader's sentence.
    Breaks { rel: String, line: usize, message: String },
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::NothingToReplace => write!(f, "nothing matches, so nothing was replaced"),
            Refused::Pattern(why) => write!(f, "the pattern is not usable ({why}), so nothing was replaced"),
            Refused::Breaks { rel, line, message } => write!(
                f,
                "this replace would leave {rel} unreadable (line {}: {message}), so nothing was written to any file; narrow the paths or the pattern",
                line + 1
            ),
        }
    }
}

/// **Apply edits over byte offsets of a whole text**: from the end, so each range still means what it did.
pub fn splice(text: &str, edits: &[CodeEdit]) -> String {
    let mut sorted: Vec<&CodeEdit> = edits.iter().collect();
    sorted.sort_by_key(|e| std::cmp::Reverse(e.range.start));
    let mut out = text.to_owned();
    for edit in sorted {
        out.replace_range(edit.range.clone(), &edit.text);
    }
    out
}

fn line_of(text: &str, offset: usize) -> usize {
    text[..offset].matches('\n').count()
}

/// **What a replace would do to every file the query reaches**, writing nothing.
///
/// `only`, when given, restricts it to those paths (a person who ticked three files of the preview). A
/// refusal writes nothing and names the file and line that would no longer read.
pub fn plan(root: &Path, query: &Query, replacement: &str, only: Option<&[String]>) -> Result<Plan, Refused> {
    if query.text.is_empty() {
        return Err(Refused::NothingToReplace);
    }
    let mut out = Plan::default();
    let mut candidates: Vec<String> = crate::files::listed_files(root).into_iter().filter(|rel| query.admits(rel)).collect();
    if let Some(only) = only {
        candidates.retain(|rel| only.contains(rel));
    }
    for rel in candidates {
        let Some(before) = read_searchable(root, &rel) else { continue };
        let buffer = CodeBuffer::new(&before);
        let edits = replace_edits(&buffer, &query.text, replacement, query.options()).map_err(|e| Refused::Pattern(e.to_string()))?;
        if edits.is_empty() {
            continue;
        }
        if files_lang::is_command_file(&rel) {
            out.skipped.push(rel);
            continue;
        }
        let after = splice(&before, &edits);
        if after == before {
            continue;
        }
        // **A file that read must still read.** One that was already broken is not made the reason a rename
        // is refused: the findings are compared, so only what this replace breaks stops it.
        let was = files_lang::diagnose(&rel, &before);
        let now = files_lang::diagnose(&rel, &after);
        if was.is_empty()
            && let Some(first) = now.first()
        {
            return Err(Refused::Breaks { rel, line: line_of(&after, first.range.start), message: first.message.clone() });
        }
        let mut changes: Vec<Change> = Vec::new();
        let old_lines: Vec<&str> = before.split('\n').collect();
        let mut groups: Vec<(usize, Vec<&CodeEdit>)> = Vec::new();
        for edit in &edits {
            let line = line_of(&before, edit.range.start);
            match groups.last_mut() {
                Some((l, group)) if *l == line => group.push(edit),
                _ => groups.push((line, vec![edit])),
            }
        }
        for (line, group) in groups {
            let start = before[..before.len().min(group[0].range.start)].rfind('\n').map_or(0, |at| at + 1);
            let base = start;
            let owned: Vec<CodeEdit> = group.iter().map(|e| CodeEdit::new(e.range.start - base..e.range.end - base, e.text.clone())).collect();
            let old = old_lines.get(line).copied().unwrap_or("").trim_end_matches('\r');
            let new = splice(old, &owned);
            changes.push(Change { line, old: old.to_owned(), new });
        }
        out.files.push(PlannedFile { rel, replacements: edits.len(), before, after, changes });
    }
    if out.files.is_empty() {
        return Err(Refused::NothingToReplace);
    }
    Ok(out)
}

pub fn apply(root: &Path, plan: &Plan) -> Result<(), String> {
    let mut written: Vec<&PlannedFile> = Vec::new();
    for file in &plan.files {
        if let Err(e) = bevy_grimoire_core::atomic_file::write(&root.join(&file.rel), &file.after) {
            // The original text is in hand: put back each file already written, and name any that could not be.
            let stuck: Vec<String> = written
                .iter()
                .filter(|done| bevy_grimoire_core::atomic_file::write(&root.join(&done.rel), &done.before).is_err())
                .map(|done| done.rel.clone())
                .collect();
            return Err(if stuck.is_empty() {
                format!("{} was not written ({e}); the files before it were put back", file.rel)
            } else {
                format!("{} was not written ({e}); these could NOT be put back and hold the replaced text: {}", file.rel, stuck.join(", "))
            });
        }
        written.push(file);
    }
    Ok(())
}