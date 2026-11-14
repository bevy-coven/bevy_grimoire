//! **The Files workspace: the project's files, and the editor for them** (GD-11, #373; series the game as
//! data, #362).
//!
//! A project is data a person and an agent both edit, and until now the person's way to change one by hand
//! was another program. This is the editor's own: a **file tree** on the left (every text file under the
//! project's root except `target/` and what `.gitignore` names) and, in the middle, `bevy_gooey`'s
//! `code_editor` on the file whose tab is active. Files are document tabs (NV-04, [`crate::session`]), so
//! the tabs come back with the session.
//!
//! # The file is the truth
//!
//! The editor holds no second copy of a file's *meaning*. What it holds is **text**: what the person has
//! typed ([`OpenFile::text`]) beside what the file said when it was last read or written
//! ([`OpenFile::disk`]), because a workspace switch rebuilds the frame and unsaved typing has to outlive the
//! widget that showed it. Saving writes the text through [`bevy_grimoire_core::atomic_file`] and nothing
//! else: **a save from here is a save from an outside editor**. The watchers that already reload the map, the
//! kits and the game's data read the change as they read any other, so there is no second path to keep in
//! step, and no operation: a person's own save is not an operation (an agent's edit is, GD-14).
//!
//! # A file that changes under unsaved edits is reported, never overwritten
//!
//! [`poll_the_disk`] looks at each open file every [`POLL_S`]. A file nobody has typed in simply reloads. A
//! file with unsaved edits keeps them, and the editor says so with two buttons, **Reload** (take the disk's)
//! and **Keep mine** (the person's text stays, and a save is then their own decision to overwrite). Saving
//! while the question is open is refused, so nothing is ever written over a change the person has not seen.
//!
//! # What the editor draws, and where each comes from
//!
//! [`crate::files_lang`]: colours from a tokenizer per language, errors from the loader's own readers
//! ([`DIAGNOSE_IDLE_S`] after the last key), completion and hover from the kit's JSON Schema, folds from the
//! structure. **Nothing here is drawn by hand**: the tree, the editor, the two buttons and the header are
//! `bevy_gooey`'s and the skin's (P10).
//!
//! # Read-only files
//!
//! The files in `trust::COMMAND_FILES` (`agent`, `play`) and the guide open read-only, with a note: they are
//! changed on their own pages, so an edit here must not be a way round a trust check
//! ([`crate::files_lang::is_command_file`]).
//!
//! Spawned on demand (the frame is rebuilt on every workspace switch), never kept hidden elsewhere: the
//! editor's protected tests count every button and take the first search field of the whole world.

use std::collections::{BTreeMap, HashMap};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, InteractionDisabled, ScrollPosition, UiGlobalTransform};
use bevy::ui_widgets::{Activate, ValueChange};
use bevy_gooey::{
    CodeAuthor, CodeCompletionRequested, CodeCompletions, CodeEdit, CodeEditor, CodeEditorValue, CodeHover, CodeHoverRequested, CodeIndent,
    CodeIndentation, CodeMinimap, CodePos, CodeSeverity, CompletionItem, Diagnostic, Diagnostics, FoldRanges, Highlights, ListView, RowSource, SelectMode,
    TableSelection, Tree, TreeNode, observe, tree,
};
use serde_json::Value;

use crate::chrome::{self, Frame, FrameSystems, Side};
use crate::editor::EditorState;
use crate::files_lang::{self, Lang};
use crate::project::Project;
use crate::screen::Screen;
use crate::session::{Doc, Kind, Session};
use crate::workspace::Workspace;

/// **How long the editor waits after the last key before it reads the text again** for errors, in seconds:
/// 150 ms (the issue's number). The colours and folds do not wait; a wave under a half-typed word would.
pub const DIAGNOSE_IDLE_S: f64 = 0.15;

/// How often the open files are looked at for a change made from outside, in seconds: the watcher's own
/// quarter of a second.
pub const POLL_S: f64 = 0.25;

/// How often the project's tree is read again for a file made or removed, in seconds.
pub const TREE_POLL_S: f64 = 1.5;

/// **The largest file the editor opens**, in bytes: two mebibytes. A text file a person edits by hand is
/// kilobytes; past this it is data a program wrote, and a 2 MB buffer in a UI thread is a stall, not an edit.
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// How many entries the tree holds before it says it stopped: a project with a hundred thousand files is
/// one nobody browses from a sidebar.
pub const TREE_LIMIT: usize = 20_000;

/// **How many entries one folder lists before the tree stops reading it**: a folder of a hundred thousand files is
/// read for its first ones and says so, rather than being collected, statted and sorted whole.
pub const TREE_DIR_LIMIT: usize = 5_000;

/// FNV-1a's offset basis and prime (64-bit): the hash a row's key and a project's session path are made with.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// How many spaces the Tab key puts in a YAML or Markdown file (the writer's own step) and in anything else, and
/// how many columns a tab character is drawn.
const YAML_INDENT_SPACES: u8 = 2;
const CODE_INDENT_SPACES: u8 = 4;
const TAB_COLUMNS: u8 = 4;

/// The width of the file tree's panel, in logical pixels.
pub const TREE_PANEL_W: f32 = 300.0;

/// The folders the tree never lists, by name, beside what `.gitignore` names.
const SKIPPED_DIRS: [&str; 4] = [".git", "target", ".live", "node_modules"];

/// The notice under the tree when it stopped at [`TREE_LIMIT`].
pub const TREE_TRUNCATED: &str = "The tree is not complete: it lists at most 20,000 entries, and 5,000 of a folder; narrow the project to see the rest";

// ── What is open ─────────────────────────────────────────────────────────────

/// What a file looked like the last time it was read or written: its size and its modification time. A
/// size and a time rather than a hash, as the project's own watcher does: noticing a change must not read
/// the file.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
}

impl Stamp {
    fn of(path: &Path) -> Option<Stamp> {
        let meta = std::fs::metadata(path).ok()?;
        Some(Stamp { len: meta.len(), modified: meta.modified().ok() })
    }
}

/// **One open file's text and what the disk held.**
#[derive(Clone, Debug)]
pub struct OpenFile {
    /// What the editor holds: the person's text, saved or not.
    pub text: String,
    /// What the file said when it was last read or written.
    pub disk: String,
    stamp: Option<Stamp>,
    /// What the disk holds now, when it changed under unsaved edits: waiting for the person's answer.
    pub external: Option<String>,
    /// Shown, and saved, as read-only: a command file or the guide.
    pub read_only: bool,
    /// **Why this tab cannot be edited or saved as a file**, when it cannot: the file is not text, is too big,
    /// or has been deleted from the project. Said in the header, in place of the text it never held.
    pub problem: Option<String>,
    /// When the text last changed by typing, on the real clock: the errors wait for [`DIAGNOSE_IDLE_S`] after it.
    edited_at: Option<f64>,
    /// Counts the times the text was replaced from outside the editor widget (a reload), so the widget knows
    /// to take it.
    pub pushed: u64,
    /// A match to bring into view and select when the editor next shows this file: line, column (bytes into the
    /// line) and length (a search result's, GD-13).
    pub reveal: Option<(usize, usize, usize)>,
    /// A change from outside that landed on a file the person had not typed in, as the one edit that makes it:
    /// the widget applies it without moving the caret or the person's selection (GD-14, #376).
    incoming: Option<(CodeEdit, CodeAuthor)>,
    /// The lines (1-based, inclusive) an agent changed and the person has not touched since: marked on the minimap.
    pub agent_lines: Vec<[u32; 2]>,
}

impl OpenFile {
    /// Whether the person has typed since the last read or write.
    pub fn dirty(&self) -> bool {
        self.text != self.disk
    }
}

/// **The files the Files workspace has open**, by path under the project. `Project`-owned: unsaved typing
/// survives a switch to the Map and back, and is dropped with the project.
#[derive(Resource, Default)]
pub struct Files {
    root: Option<PathBuf>,
    open: BTreeMap<String, OpenFile>,
    schemas: HashMap<&'static str, Option<Arc<Value>>>,
}

impl Files {
    /// The open file at `rel`.
    pub fn get(&self, rel: &str) -> Option<&OpenFile> {
        self.open.get(rel)
    }

    pub fn get_mut(&mut self, rel: &str) -> Option<&mut OpenFile> {
        self.open.get_mut(rel)
    }

    /// Whether `rel` is open with unsaved edits.
    pub fn is_dirty(&self, rel: &str) -> bool {
        self.open.get(rel).is_some_and(OpenFile::dirty)
    }

    /// **Save every file with unsaved edits**, for "save and leave" and "save and quit". The first refusal stops it, naming the
    /// file; what was saved before it stays saved.
    pub fn save_all(&mut self, root: &Path) -> Result<(), String> {
        let dirty: Vec<String> = self.dirty().into_iter().map(str::to_owned).collect();
        for rel in dirty {
            save_file(self, root, &rel)?;
        }
        publish_unsaved(root, Vec::new());
        Ok(())
    }

    /// **Let go of every unsaved edit**, for "discard changes": the files are as the disk has them.
    pub fn discard_all(&mut self, root: &Path) {
        for file in self.open.values_mut() {
            file.text = file.disk.clone();
            file.pushed += 1;
            file.incoming = None;
        }
        publish_unsaved(root, Vec::new());
    }

    /// The paths with unsaved edits, in order.
    pub fn dirty(&self) -> Vec<&str> {
        self.open.iter().filter(|(_, f)| f.dirty()).map(|(rel, _)| rel.as_str()).collect()
    }

    /// **The JSON Schema of the kind `rel` is**, generated from the type the loader reads
    /// ([`bevy_grimoire_core::schema`]) and kept: generating one is a `schemars` walk.
    pub fn schema_for(&mut self, rel: &str) -> Option<Arc<Value>> {
        let kind = bevy_grimoire_core::reload_check::kind_of_path(rel)?;
        self.schemas
            .entry(kind.id)
            .or_insert_with(|| bevy_grimoire_core::schema::for_kind(kind.id).ok().map(Arc::new))
            .clone()
    }
}

/// **Whether a path is one the Files workspace may open**: a relative path that stays inside the project.
/// `Err` says why not, for a notice.
pub(crate) fn inside(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let path = Path::new(rel);
    if rel.is_empty() || path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_) | Component::RootDir)) {
        return Err(format!("{rel} is not a path inside the project"));
    }
    let full = root.join(path);
    // A symlink is followed only while it stays in the project: the tree never lists one, and a path typed by
    // hand or sent by an agent must not reach what the project's folder does not hold. **A path that is not there yet is judged by the nearest
    // folder that is** (a new page under a folder that links out of the project is outside it), and **a link that leads nowhere is outside too**,
    // because a write through it would make the file where the link points.
    let Ok(base) = std::fs::canonicalize(root) else { return Ok(full) };
    let mut probe = full.as_path();
    let real = loop {
        match std::fs::canonicalize(probe) {
            Ok(real) => break Some(real),
            Err(_) if std::fs::symlink_metadata(probe).is_ok_and(|m| m.file_type().is_symlink()) => {
                return Err(format!("{rel} is outside the project"));
            }
            Err(_) => match probe.parent() {
                Some(parent) => probe = parent,
                None => break None,
            },
        }
    };
    if real.is_some_and(|real| !real.starts_with(&base)) {
        return Err(format!("{rel} is outside the project"));
    }
    Ok(full)
}

/// The operation log, which no operation may rewrite: it is the audit and the undo's source.
const OPLOG_PATH: &str = "assets/grimoire/oplog.jsonl";

/// **A project path an operation may write**: inside the project (links followed only while they stay in it, and a path
/// through a linked folder that leaves it is refused), not in a folder the tree never lists (`.git`, `target`, `.live`,
/// `node_modules`), not named by the project's `.gitignore`, and not the operation log. Writing `.git/hooks/*` or
/// `oplog.jsonl` would be a way round a trust check and round the undo.
pub(crate) fn project_path(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let full = inside(root, rel)?;
    let parts: Vec<String> = Path::new(rel).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    if parts.iter().any(|part| SKIPPED_DIRS.contains(&part.as_str())) || rel.replace('\\', "/") == OPLOG_PATH {
        return Err(format!("{rel} is not a file the project's tree lists, so it is not edited here"));
    }
    let ignore = std::fs::read_to_string(root.join(".gitignore")).map(|t| Ignore::parse(&t)).unwrap_or_default();
    let mut walked = String::new();
    for (n, part) in parts.iter().enumerate() {
        if !walked.is_empty() {
            walked.push('/');
        }
        walked.push_str(part);
        if ignore.matches(&walked, n + 1 < parts.len()) {
            return Err(format!("{rel} is ignored by the project's .gitignore, so it is not edited here"));
        }
    }
    Ok(full)
}

/// **Read a file as the editor holds it**: its text, or why it is not a text file the editor opens.
fn read_text(full: &Path, rel: &str) -> Result<(String, Option<Stamp>), String> {
    let meta = std::fs::metadata(full).map_err(|e| format!("{rel}: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{rel} is not a file"));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!("{rel} is {} KiB; the file editor opens files up to {} KiB", meta.len() / 1024, MAX_FILE_BYTES / 1024));
    }
    let bytes = std::fs::read(full).map_err(|e| format!("{rel}: {e}"))?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{rel} is not text (it is not UTF-8)"))?;
    Ok((text, Stamp::of(full)))
}

impl OpenFile {
    fn from_disk(root: &Path, rel: &str) -> Result<OpenFile, String> {
        let full = inside(root, rel)?;
        let (text, stamp) = read_text(&full, rel)?;
        Ok(OpenFile {
            disk: text.clone(),
            text,
            stamp,
            external: None,
            read_only: files_lang::is_command_file(rel),
            problem: None,
            edited_at: None,
            pushed: 0,
            reveal: None,
            incoming: None,
            agent_lines: Vec::new(),
        })
    }
}

/// **Open a file in the Files workspace**: read it, and give it a tab, active. The same call a click in the
/// tree, a search result and an agent's follow make. Refuses what is not a text file inside the project, and
/// says why.
pub fn open_file(world: &mut World, rel: &str) -> Result<(), String> {
    let root = world.get_resource::<Project>().map(|p| p.root.clone()).ok_or("there is no project open")?;
    {
        let mut files = world.get_resource_mut::<Files>().ok_or("the file editor is not running")?;
        if files.root.as_deref() != Some(root.as_path()) {
            files.root = Some(root.clone());
            files.open.clear();
        }
        if !files.open.contains_key(rel) {
            let file = OpenFile::from_disk(&root, rel)?;
            files.open.insert(rel.to_owned(), file);
        }
    }
    let mut session = world.get_resource_mut::<Session>().ok_or("there is no session to hold the tab")?;
    session.open(Workspace::Files, Doc::new(Kind::File, rel))?;
    Ok(())
}

/// **Hold a file's text for an editor that is not the Files workspace's** (SG-05, #404): the Design workspace's Edit. The file is read
/// into [`Files`] exactly as an open reads it and gets **no tab**, so the Files workspace's tabs are untouched; the buffer lives while it
/// has unsaved typing or the Design workspace is editing it (`keep_the_open_files`). A buffer already held is left as it is.
pub fn hold_for_editing(world: &mut World, rel: &str) -> Result<(), String> {
    let root = world.get_resource::<Project>().map(|p| p.root.clone()).ok_or("there is no project open")?;
    let mut files = world.get_resource_mut::<Files>().ok_or("the file editor is not running")?;
    if files.root.as_deref() != Some(root.as_path()) {
        files.root = Some(root.clone());
        files.open.clear();
    }
    if !files.open.contains_key(rel) {
        let file = OpenFile::from_disk(&root, rel)?;
        files.open.insert(rel.to_owned(), file);
    }
    Ok(())
}

pub fn save_file(files: &mut Files, root: &Path, rel: &str) -> Result<(), String> {
    let file = files.open.get_mut(rel).ok_or_else(|| format!("{rel} is not open"))?;
    if let Some(why) = &file.problem {
        return Err(why.clone());
    }
    if file.read_only {
        return Err(format!("{rel}: {}", files_lang::READ_ONLY_NOTE));
    }
    if file.external.is_some() {
        return Err(format!("{rel} changed on disk while you were editing it: choose Reload or Keep mine first"));
    }
    // **Nothing to write is nothing written**: a save of a clean file does not touch it (its modification time is
    // somebody's to watch).
    if !file.dirty() {
        return Ok(());
    }
    let full = inside(root, rel)?;
    // **A file that is gone is not recreated by a save** (and neither is its folder): the person is told, and the
    // text stays in the tab.
    if !full.exists() {
        file.problem = Some(format!("{rel} was deleted from the project; your text is still here, and a save does not bring the file back"));
        return Err(format!("{rel} no longer exists, so it was not saved"));
    }
    // **A change the poll has not seen yet is still a change**: compare with what the disk holds this instant,
    // so a save in the quarter second after an outside write cannot overwrite it.
    if let Ok(now) = std::fs::read_to_string(&full)
        && now != file.disk
        && now != file.text
    {
        file.external = Some(now);
        return Err(format!("{rel} changed on disk while you were editing it: choose Reload or Keep mine first"));
    }
    bevy_grimoire_core::atomic_file::write(&full, &file.text).map_err(|e| format!("{rel} was not saved: {e}"))?;
    file.disk = file.text.clone();
    file.stamp = Stamp::of(&full);
    Ok(())
}
// ── The tree ─────────────────────────────────────────────────────────────────

pub fn key_of(rel: &str) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in rel.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
/// **The project's `.gitignore`, as far as a file tree needs it**: a line is a name, a path from the root, or
/// a glob (`*` and `?`); a trailing `/` is a folder only; a leading `/` anchors at the root; `#` is a
/// comment. Negation (`!`) and `**` are not read: a tree that shows an ignored file is a smaller harm than
/// one that hides a project's.
#[derive(Default)]
pub struct Ignore {
    patterns: Vec<(String, bool, bool)>,
}

impl Ignore {
    pub fn parse(text: &str) -> Ignore {
        let patterns = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('!'))
            .map(|l| {
                let dir_only = l.ends_with('/');
                let l = l.trim_end_matches('/');
                let anchored = l.starts_with('/') || l.contains('/');
                (l.trim_start_matches('/').to_owned(), dir_only, anchored)
            })
            .collect();
        Ignore { patterns }
    }

    /// Whether `rel` (with `/`) is ignored.
    pub fn matches(&self, rel: &str, is_dir: bool) -> bool {
        let name = rel.rsplit('/').next().unwrap_or(rel);
        self.patterns.iter().any(|(pattern, dir_only, anchored)| {
            if *dir_only && !is_dir {
                return false;
            }
            if *anchored { wildcard(pattern, rel, false) } else { wildcard(pattern, name, false) }
        })
    }
}

/// **The one wildcard matcher**: `*` any run of characters, `?` one, over the whole of `text`. With `across_slash` false a
/// `*` and a `?` stop at a `/` (a `.gitignore` line); true lets them run across folders (a path filter). Iterative, with
/// one place to back up to, so a pattern of many stars cannot take exponential time.
pub(crate) fn wildcard(pattern: &str, text: &str, across_slash: bool) -> bool {
    let (p, t) = (pattern.as_bytes(), text.as_bytes());
    let (mut pi, mut ti) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        let literal = pi < p.len() && p[pi] != b'*' && (p[pi] == t[ti] || (p[pi] == b'?' && (across_slash || t[ti] != b'/')));
        if literal {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == b'*' {
            star = Some((pi, ti));
            pi += 1;
        } else if let Some((sp, st)) = star {
            if !across_slash && t[st] == b'/' {
                return false;
            }
            star = Some((sp, st + 1));
            pi = sp + 1;
            ti = st + 1;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == b'*' {
        pi += 1;
    }
    pi == p.len()
}
/// What the walk of a project's folders made: the tree's nodes and what each key names.
#[derive(Default)]
struct Walked {
    nodes: Vec<TreeNode>,
    names: HashMap<u64, (String, bool)>,
    count: usize,
    truncated: bool,
    /// An order-independent sum of what is listed, so a settled tree is told from a changed one without keeping a copy.
    fingerprint: u64,
    /// Read with no limit: what a search and a replace walk, so a project-wide rename never covers only part of a project.
    uncapped: bool,
}

fn walk_dir(root: &Path, dir: &Path, ignore: &Ignore, walked: &mut Walked) -> Vec<TreeNode> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    // **A folder is read for its first [`TREE_DIR_LIMIT`] entries only**: collecting, statting and sorting a folder of a
    // hundred thousand files is the stall the limit exists to prevent.
    let mut read = entries.filter_map(Result::ok);
    let mut listed: Vec<(String, bool)> = Vec::new();
    for entry in read.by_ref().take(if walked.uncapped { usize::MAX } else { TREE_DIR_LIMIT }) {
        let Ok(meta) = std::fs::symlink_metadata(entry.path()) else { continue };
        // A link is never followed (the tree stays inside the project) and never listed.
        if !meta.file_type().is_symlink() {
            listed.push((entry.file_name().to_string_lossy().into_owned(), meta.is_dir()));
        }
    }
    if read.next().is_some() {
        walked.truncated = true;
    }
    listed.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
    let mut out = Vec::new();
    for (name, is_dir) in listed {
        let rel_path = dir.join(&name);
        let Ok(rel) = rel_path.strip_prefix(root) else { continue };
        let rel = rel.to_string_lossy().replace('\\', "/");
        if (is_dir && SKIPPED_DIRS.contains(&name.as_str())) || name == ".DS_Store" || ignore.matches(&rel, is_dir) {
            continue;
        }
        if !walked.uncapped && walked.count >= TREE_LIMIT {
            walked.truncated = true;
            break;
        }
        walked.count += 1;
        let key = key_of(&rel);
        walked.names.insert(key, (rel.clone(), is_dir));
        walked.fingerprint = walked.fingerprint.wrapping_add(key ^ u64::from(is_dir));
        let mut node = TreeNode::new(key, name);
        if is_dir {
            node = node.children(walk_dir(root, &rel_path, ignore, walked));
        }
        out.push(node);
    }
    out
}
fn walk_project(root: &Path) -> Walked {
    walk_with(root, false)
}

/// **The files the tree lists**, with the tree's own caps: the finder's index reads this (NV-05, #382) on a timer, so it is bounded the way the
/// tree is. A search and a replace use [`listed_files`], which has no limit.
pub fn tree_files(root: &Path) -> Vec<String> {
    walk_project(root).names.into_values().filter(|(_, dir)| !dir).map(|(rel, _)| rel).collect()
}

fn walk_with(root: &Path, uncapped: bool) -> Walked {
    let ignore = std::fs::read_to_string(root.join(".gitignore")).map(|t| Ignore::parse(&t)).unwrap_or_default();
    let mut walked = Walked { uncapped, ..Walked::default() };
    let nodes = walk_dir(root, root, &ignore, &mut walked);
    walked.nodes = nodes;
    walked
}
/// **The paths the tree lists**, files only, in tree order: what a test and the search read.
pub fn listed_files(root: &Path) -> Vec<String> {
    // **No limit**: the tree stops at its caps and says so, but a search and a replace must see every file.
    let walked = walk_with(root, true);
    let mut files: Vec<String> = walked.names.into_values().filter(|(_, dir)| !dir).map(|(rel, _)| rel).collect();
    files.sort();
    files
}

// ── Components ───────────────────────────────────────────────────────────────

/// **The headings of the open page**, as of one revision of its text (SG-04, #403): what an outline lists. A language with no
/// headings has none. Kept on the code editor's own entity beside its folds, by the one system that decorates it.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct FileOutline {
    /// The revision of the text the headings are of.
    pub revision: u64,
    pub headings: Vec<files_lang::Heading>,
}

/// The tree panel in the left dock.
#[derive(Component)]
pub struct FilesPanel;

/// The sentence under the tree's title when the project has more than the tree lists.
#[derive(Component)]
pub struct TreeNotice;

/// The tree itself: a gooey `tree`.
#[derive(Component)]
pub struct FilesTree;

/// The column in the middle that holds the header, the question about a changed file and the editor.
#[derive(Component)]
pub struct FilesHost;

/// The line over the editor: the file's path, a dot when it has unsaved edits, and why it is read-only.
#[derive(Component)]
pub struct FileHeader;

/// The question a file that changed under unsaved edits asks, with its two buttons.
#[derive(Component)]
pub struct ConflictBar(pub String);

/// The two answers.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConflictAnswer {
    Reload,
    KeepMine,
}

/// The sentence over the two buttons.
pub fn conflict_sentence(rel: &str) -> String {
    format!("{rel} changed on disk while you have unsaved edits.")
}

/// The code editor, and what it is showing.
#[derive(Component, Default)]
pub struct FileEditor {
    /// The path of the file the widget holds.
    pub shown: Option<String>,
    /// [`OpenFile::pushed`] when the widget last took the text.
    pushed: u64,
    /// The value revision the colours and folds were last computed for.
    highlighted: u64,
    /// The value revision the errors were last computed for.
    analysed: u64,
}

impl FileEditor {
    /// **A fresh editor widget's bookkeeping for `rel`**: nothing coloured or analysed yet, so the first frame does both.
    pub fn showing(rel: &str, pushed: u64) -> FileEditor {
        FileEditor { shown: Some(rel.to_owned()), pushed, highlighted: u64::MAX, analysed: u64::MAX }
    }
}

fn in_files(workspace: Option<Res<Workspace>>) -> bool {
    workspace.is_some_and(|w| *w == Workspace::Files)
}

/// The path of the file whose tab is active in the Files workspace.
pub fn active_file(session: &Session) -> Option<String> {
    session.tabs(Workspace::Files).active_doc().filter(|d| d.kind == Kind::File).map(|d| d.name.clone())
}

fn indentation_of(lang: Lang) -> CodeIndentation {
    let spaces = match lang {
        Lang::Yaml | Lang::Markdown => YAML_INDENT_SPACES,
        Lang::Ron | Lang::Rhai | Lang::Plain => CODE_INDENT_SPACES,
    };
    CodeIndentation { indent: CodeIndent::Spaces(spaces), tab_width: TAB_COLUMNS }
}
// ── The plugin ───────────────────────────────────────────────────────────────

/// **The Files workspace**: the tree, the editor and what keeps them true.
pub struct FilesPlugin;

impl Plugin for FilesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PointerAt>();
        app.init_resource::<Files>()
            .add_systems(OnEnter(Screen::Editor), spawn_the_files_workspace.after(FrameSystems).run_if(in_files))
            .add_systems(
                Update,
                (
                    keep_the_open_files,
                    poll_the_disk,
                    keep_the_editor,
                    decorate_the_editor,
                    track_the_pointer,
                    follow_on_f12,
                    follow_on_primary_click,
                    keep_the_header,
                    keep_the_conflict_bar,
                    keep_the_tree,
                    reveal_the_active_row,
                )
                    .chain()
                    .run_if(in_state(Screen::Editor))
                    .run_if(in_files),
            )
            // **The Design workspace edits through the same machinery** (SG-05, #404): the buffer kept, the disk polled, the editor driven and
            // coloured, in the Design workspace's page instead of the Files workspace's.
            .add_systems(
                Update,
                (keep_the_open_files, poll_the_disk, keep_the_design_editor, decorate_the_editor)
                    .chain()
                    .run_if(in_state(Screen::Editor))
                    .run_if(Workspace::design_is_open),
            );
    }
}

/// **Spawn the tree and the editor's column** when the Files workspace is entered. The code editor itself is
/// spawned by [`keep_the_editor`] while a file is open, so a workspace with no file open holds no editor.
/// **The node a workspace's central page stands in**: one card of the editor's panel look (padding, margin, edge, rounded corners) that fills what the
/// docks leave. The Files workspace and the Design workspace both build their page host on it, so there is one such node and no second copy of it.
pub(crate) fn page_host() -> impl Bundle {
    (
        // A gooey fill column (it grows into what the docks leave, with no least width of its own), `GAP_ROW` px between its parts, `PAD` in all round, `MARGIN` out all round,
        // a 1 px edge and a panel radius. Its fill and edge are the editor's (`InkFollow`: `Ground`, `PanelEdge` lit by focus, and the theme move them).
        bevy_gooey::fill_column(0.0),
        bevy_gooey::GapPx(chrome::GAP_ROW),
        bevy_gooey::Sizing { min_height: Some(bevy_gooey::Extent::Px(0.0)), ..default() },
        bevy_gooey::Spacing { padding: bevy_gooey::Sides::all(chrome::STEP_PAD), margin: bevy_gooey::Sides::all(chrome::STEP_PAD) },
        bevy_gooey::BorderPx(bevy_gooey::Sides::all(chrome::EDGE_W)),
        bevy_gooey::Shape { radius: Some(bevy_gooey::Radius::Step(1)), ..default() },
        bevy_gooey::InkFollow,
    )
}

fn spawn_the_files_workspace(mut commands: Commands, frame: Res<Frame>) {
    let panel = chrome::panel_root(&mut commands, &frame, Side::Left, TREE_PANEL_W, true, false, "FILES").insert(FilesPanel).id();
    commands.entity(panel).with_children(|p| {
        chrome::title(p, "FILES");
        p.spawn((bevy_gooey::label(String::new()), TreeNotice));
        p.spawn(bevy_gooey::fill_region())
            .with_children(|column| {
                column.spawn((tree(Vec::<TreeNode>::new()), FilesTree, SelectMode::Single, observe(open_the_picked_file)));
            });
    });
    let ink = chrome::ink();
    let host = commands
        .spawn((
            FilesHost,
            page_host(),
            BackgroundColor(ink.panel_bg),
            chrome::Ground(ink.panel_bg),
            chrome::PanelGround,
            BorderColor::all(ink.panel_edge),
            chrome::PanelEdge,
            Hovered::default(),
            AccessibleLabel::new("File editor"),
        ))
        .with_children(|host| {
            host.spawn((
                FileHeader,
                Text::new(String::new()),
                TextColor(ink.text),
                chrome::font(chrome::text::LABEL),
                TextLayout::new(Justify::Left, LineBreak::NoWrap),
            ));
            host.spawn((bevy_gooey::text_input("Set a field: path value", ""), FieldLine, observe(on_field_line)));
        })
        .id();
    commands.entity(frame.viewport).add_child(host);
}

/// **The field line**: `<field path> <value>` sets one value of the open YAML file through the operation `file_edit`,
/// with every comment kept, as the DATA line sets a game's tuning value. The same write an agent's `apply` makes.
#[derive(Component)]
pub struct FieldLine;

fn on_field_line(change: On<ValueChange<String>>, mut commands: Commands) {
    if !change.is_final || change.value.trim().is_empty() {
        return;
    }
    let line = change.value.clone();
    commands.queue(move |world: &mut World| set_the_field(world, &line));
}

/// **Set one field of the open file** from a `<path> <value>` line, through the operation `file_edit`, and put it on
/// the undo stack. A refusal goes to the status line and writes nothing.
pub fn set_the_field(world: &mut World, line: &str) {
    let Some(rel) = world.get_resource::<Session>().and_then(active_file) else { return };
    let Some(project) = world.get_resource::<Project>().cloned() else { return };
    let (path, value_text) = line.trim().split_once(char::is_whitespace).unwrap_or((line.trim(), ""));
    let value = serde_json::from_str::<serde_json::Value>(value_text.trim()).unwrap_or_else(|_| serde_json::Value::String(value_text.trim().to_owned()));
    let mut ctx = crate::ops::Ctx { project: Some(project), source: crate::ops::Source::Human, session: crate::ops::session_id(), ..crate::ops::Ctx::default() };
    let params = serde_json::json!({"path": rel, "edits": [{"at": path, "value": value}]});
    let Some(mut state) = world.remove_resource::<EditorState>() else { return };
    match crate::ops::dispatch(&mut ctx, "file_edit", &params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record_logged(operation_id, format!("{}: {path}", outcome.summary), false);
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
    world.insert_resource(state);
}

/// A click in the tree opens the file it names; a folder's row opens nothing (its chevron opens it).
fn open_the_picked_file(picked: On<ValueChange<TableSelection>>, mut commands: Commands) {
    let Some(key) = picked.value.iter().next() else { return };
    commands.queue(move |world: &mut World| {
        let Some(rel) = tree_names(world).get(&key).filter(|(_, dir)| !dir).map(|(rel, _)| rel.clone()) else { return };
        if let Err(why) = open_file(world, &rel)
            && let Some(mut state) = world.get_resource_mut::<EditorState>()
        {
            state.status.problem(format!("NOT OPENED: {why}"));
        }
    });
}

/// What each row key of the tree names, as the last walk made it: kept on the tree's own entity.
#[derive(Component, Default)]
struct TreeNames {
    names: HashMap<u64, (String, bool)>,
    fingerprint: u64,
    next_poll: f64,
    selected: Option<u64>,
    /// The row key the tree has been opened and scrolled to (#359), so a person who scrolls away is not pulled back.
    revealed: Option<u64>,
    /// How many frames in a row the revealed row has been inside the viewport: layout is a frame behind the rows.
    steady: u8,
    truncated: bool,
    /// The walk in flight, on a thread of its own: the tree is read off the UI thread.
    scan: Option<std::sync::Mutex<std::sync::mpsc::Receiver<Walked>>>,
}
fn tree_names(world: &mut World) -> HashMap<u64, (String, bool)> {
    world.query_filtered::<&TreeNames, With<FilesTree>>().iter(world).next().map(|t| t.names.clone()).unwrap_or_default()
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// **Keep the open files in step with the tabs.** A tab the session restored has no text yet: read it. A
/// file whose tab was closed and which has nothing unsaved is let go. The project changing drops all of them.
fn keep_the_open_files(
    mut files: ResMut<Files>,
    session: Res<Session>,
    project: Option<Res<Project>>,
    mut state: Option<ResMut<EditorState>>,
    design: Option<Res<crate::design_view::DesignView>>,
) {
    let Some(project) = project else { return };
    if files.root.as_deref() != Some(project.root.as_path()) {
        files.root = Some(project.root.clone());
        files.open.clear();
    }
    let tabs = session.tabs(Workspace::Files);
    let wanted: Vec<&str> = tabs.open.iter().filter(|d| d.kind == Kind::File).map(|d| d.name.as_str()).collect();
    for rel in &wanted {
        if files.open.contains_key(*rel) {
            continue;
        }
        match OpenFile::from_disk(&project.root, rel) {
            Ok(file) => {
                files.open.insert((*rel).to_owned(), file);
            }
            Err(why) => {
                // A file the tab names and the editor cannot open is told once: the tab is left (the session
                // closes a tab whose file is gone), and the text it never held cannot be edited.
                if let Some(state) = state.as_deref_mut() {
                    state.status.problem(format!("NOT OPENED: {why}"));
                }
                files.open.insert(
                    (*rel).to_owned(),
                    OpenFile {
                        text: String::new(),
                        disk: String::new(),
                        stamp: None,
                        external: None,
                        read_only: true,
                        problem: Some(why.clone()),
                        edited_at: None,
                        pushed: 0,
                        reveal: None,
                        incoming: None,
                        agent_lines: Vec::new(),
                    },
                );
            }
        }
    }
    // **A file the Design workspace is editing is kept** even while clean (SG-05, #404): it has no tab here.
    let design_editing = design.as_deref().and_then(crate::design_view::DesignView::editing_file).map(str::to_owned);
    files.open.retain(|rel, file| wanted.contains(&rel.as_str()) || file.dirty() || design_editing.as_deref() == Some(rel.as_str()));
    let dirty: Vec<String> = files.open.iter().filter(|(_, f)| f.dirty()).map(|(rel, _)| rel.clone()).collect();
    publish_unsaved(&project.root, dirty);
}

fn poll_the_disk(mut files: ResMut<Files>, time: Res<Time<Real>>, mut last: Local<f64>) {
    let now = time.elapsed_secs_f64();
    if now - *last < POLL_S {
        return;
    }
    *last = now;
    let Some(root) = files.root.clone() else { return };
    for (rel, file) in files.open.iter_mut() {
        let Ok(full) = inside(&root, rel) else { continue };
        let stamp = Stamp::of(&full);
        if stamp == file.stamp {
            continue;
        }
        file.stamp = stamp.clone();
        // **A file that is gone is said to be gone**, and the text stays where it is.
        if stamp.is_none() {
            file.problem = Some(format!("{rel} was deleted from the project; your text is still here, and a save does not bring the file back"));
            continue;
        }
        // **Read the way an open reads** (a size and a UTF-8 check), so a file that grew past the limit or stopped being
        // text is refused here too, and the tab says why.
        let text = match read_text(&full, rel) {
            Ok((text, _)) => text,
            Err(why) => {
                file.problem = Some(why);
                continue;
            }
        };
        if file.problem.take().is_some() {
            // It reads again (or is back): a stub with no text takes the file whole.
            file.read_only = files_lang::is_command_file(rel);
        }
        if text == file.disk {
            file.external = None;
        } else if text == file.text {
            // The same text arrived by another road: nothing to ask.
            file.disk = text;
            file.external = None;
        } else if file.text == file.disk {
            // **Nobody has typed here, so the change simply lands** (GD-14, #376): as the one edit that makes it, from
            // the Familiar's hand when an agent's `file_edit` just wrote the file, so the widget keeps the person's
            // caret and selection and glides the changed lines in tinted in the author's colour.
            let agent = take_agent_edit(&root, rel);
            let ink = chrome::ink();
            let author = if agent { CodeAuthor::new("the Familiar", ink.suggest) } else { CodeAuthor::new("outside", ink.accent) };
            file.incoming = diff_edit(&file.text, &text).map(|edit| (edit, author));
            file.agent_lines = if agent { changed_lines(&file.text, &text).into_iter().collect() } else { Vec::new() };
            file.disk = text.clone();
            file.text = text;
            file.pushed += 1;
            file.external = None;
        } else {
            file.external = Some(text);
        }
    }
}
/// **Show the active file in the code editor**, spawning the editor when a file is open and taking it away
/// when none is; switch the text when the tab changes. A change that arrived from outside lands as one external
/// edit (the caret and the selection stay where the person left them); a search result's line is brought into view.
fn keep_the_editor(
    mut commands: Commands,
    mut files: ResMut<Files>,
    session: Res<Session>,
    hosts: Query<Entity, With<FilesHost>>,
    mut editors: Query<(Entity, &mut FileEditor, &mut CodeEditorValue, &mut CodeEditor, Option<&InteractionDisabled>)>,
) {
    let Ok(host) = hosts.single() else { return };
    drive_the_editor(&mut commands, &mut files, host, active_file(&session), &mut editors);
}

/// **The Design workspace's editor** (SG-05, #404): the same widget, the same buffer ([`Files`]) and the same driver as the Files
/// workspace's, in the page's own place, for the file Edit was pressed on.
pub(crate) fn keep_the_design_editor(
    mut commands: Commands,
    mut files: ResMut<Files>,
    design: Res<crate::design_view::DesignView>,
    hosts: Query<Entity, With<crate::design_view::DesignBody>>,
    mut editors: Query<(Entity, &mut FileEditor, &mut CodeEditorValue, &mut CodeEditor, Option<&InteractionDisabled>)>,
) {
    let Ok(host) = hosts.single() else { return };
    drive_the_editor(&mut commands, &mut files, host, design.editing_file().map(str::to_owned), &mut editors);
}

/// **Put `active` in the code editor under `host`**, spawning the editor when there is a file and taking it away when there is none.
fn drive_the_editor(
    commands: &mut Commands,
    files: &mut Files,
    host: Entity,
    active: Option<String>,
    editors: &mut Query<(Entity, &mut FileEditor, &mut CodeEditorValue, &mut CodeEditor, Option<&InteractionDisabled>)>,
) {
    match (active, editors.iter_mut().next()) {
        (None, Some((entity, ..))) => {
            commands.entity(entity).despawn();
        }
        (Some(rel), None) => {
            let Some(file) = files.open.get_mut(&rel) else { return };
            file.incoming = None;
            let editor = commands
                .spawn((
                    bevy_gooey::code_editor(rel.clone(), file.text.clone()),
                    FileEditor::showing(&rel, file.pushed),
                    indentation_of(Lang::of(&rel)),
                    // **The text small down the right edge** (GD-12, #374): the person's way round a long file, with the
                    // errors and the lines an agent changed marked on it.
                    CodeMinimap,
                    observe(on_edit),
                    observe(on_completion_request),
                    observe(on_hover_request),
                ))
                .id();
            if file.read_only {
                commands.entity(editor).insert(InteractionDisabled);
            }
            commands.entity(host).add_child(editor);
        }
        (Some(rel), Some((entity, mut editor, mut value, mut widget, disabled))) => {
            let Some(file) = files.open.get_mut(&rel) else { return };
            let switched = editor.shown.as_deref() != Some(rel.as_str());
            if switched {
                value.set(file.text.clone());
                file.incoming = None;
                commands.entity(entity).insert(indentation_of(Lang::of(&rel)));
                editor.shown = Some(rel.clone());
                editor.pushed = file.pushed;
            } else if editor.pushed != file.pushed {
                match file.incoming.take() {
                    Some((edit, author)) => {
                        if widget.apply_external_edit(&[edit], author).is_err() {
                            // A range the widget will not take (inside a line break): the whole text instead.
                            value.set(file.text.clone());
                        }
                    }
                    None => value.set(file.text.clone()),
                }
                editor.pushed = file.pushed;
            } else if let Some((line, col, len)) = file.reveal.take() {
                widget.select(CodePos::new(line, col), CodePos::new(line, col + len));
                widget.scroll_to_line(line.saturating_sub(REVEAL_LINES_ABOVE));
            }
            match (file.read_only, disabled.is_some()) {
                (true, false) => {
                    commands.entity(entity).insert(InteractionDisabled);
                }
                (false, true) => {
                    commands.entity(entity).remove::<InteractionDisabled>();
                }
                _ => {}
            }
        }
        (None, None) => {}
    }
}

/// **Put the caret on a heading's line in the open file** (SG-04, #403): `index` is the heading's place in the page's outline. The editor takes
/// it the way it takes a search result's place ([`OpenFile::reveal`]), so the line comes into view.
pub fn reveal_heading(world: &mut World, index: u64) {
    let Ok(index) = usize::try_from(index) else { return };
    let Some(line) = world.query::<&FileOutline>().iter(world).next().and_then(|o| o.headings.get(index)).map(|h| h.line) else { return };
    let Some(rel) = world.get_resource::<Session>().and_then(active_file) else { return };
    if let Some(mut files) = world.get_resource_mut::<Files>()
        && let Some(file) = files.open.get_mut(&rel)
    {
        file.reveal = Some((line, 0, 0));
    }
}

/// How many lines of context stay above a line brought into view by a search result.
pub const REVEAL_LINES_ABOVE: usize = 3;

/// **The person typed.** Keep the text, and (Cmd or Ctrl+S) save it.
fn on_edit(
    change: On<ValueChange<String>>,
    mut files: ResMut<Files>,
    editors: Query<&FileEditor>,
    time: Res<Time<Real>>,
    mut state: Option<ResMut<EditorState>>,
) {
    let Ok(editor) = editors.get(change.source) else { return };
    let Some(rel) = editor.shown.clone() else { return };
    let now = time.elapsed_secs_f64();
    if let Some(file) = files.open.get_mut(&rel)
        && file.text != change.value
    {
        // The person typed: what they changed is theirs now, so the agent's marks go.
        file.text = change.value.clone();
        file.agent_lines.clear();
        file.edited_at = Some(now);
    }
    if !change.is_final {
        return;
    }
    let Some(root) = files.root.clone() else { return };
    match save_file(&mut files, &root, &rel) {
        Ok(()) => {
            if let Some(state) = state.as_deref_mut() {
                state.status.note(format!("saved {rel}"));
            }
        }
        Err(why) => {
            if let Some(state) = state.as_deref_mut() {
                state.status.problem(format!("NOT SAVED: {why}"));
            }
        }
    }
}

/// **Colours and folds as the text changes; errors once the person has stopped** ([`DIAGNOSE_IDLE_S`]).
fn decorate_the_editor(
    mut commands: Commands,
    mut files: ResMut<Files>,
    time: Res<Time<Real>>,
    index: Option<Res<crate::goto::GotoIndex>>,
    mut editors: Query<(Entity, &mut FileEditor, &CodeEditorValue)>,
) {
    let now = time.elapsed_secs_f64();
    let root = files.root.clone();
    for (entity, mut editor, value) in &mut editors {
        let Some(rel) = editor.shown.clone() else { continue };
        let revision = value.revision();
        let lang = Lang::of(&rel);
        if editor.highlighted != revision {
            let schema = files.schema_for(&rel);
            commands.entity(entity).insert((
                Highlights::new(revision, files_lang::highlights(lang, value.text(), schema.as_deref())),
                FoldRanges::new(revision, files_lang::folds(lang, value.text())),
                FileOutline { revision, headings: files_lang::outline(lang, value.text()) },
            ));
            editor.highlighted = revision;
        }
        let waiting = files.open.get(&rel).and_then(|f| f.edited_at).is_some_and(|at| now - at < DIAGNOSE_IDLE_S);
        if editor.analysed != revision && !waiting {
            // **The project around the file**: a Markdown page's links and pictures are looked for under its root.
            let around = files_lang::Env { root: root.as_deref(), refs: &|_: &str| Vec::new(), files: &Vec::new };
            let mut items: Vec<Diagnostic> = files_lang::diagnose_in(&around, &rel, value.text()).into_iter().map(|f| Diagnostic::new(f.range, f.severity, f.message)).collect();
            if let Some(file) = files.open.get(&rel) {
                items.extend(agent_marks(value.text(), &file.agent_lines));
            }
            // **A cue the project does not hold is marked where it is written** (NV-07, #384): a reference to nothing is a diagnostic, never a link
            // that does nothing. Judged against the finder's index, once the index has been built.
            if let Some(index) = index.as_deref().filter(|i| i.is_built()) {
                let schema = files.schema_for(&rel);
                for found in files_lang::references(lang, value.text(), schema.as_deref()) {
                    if index.holds(&found.kind, &found.name) == Some(false) && found.kind == files_lang::CUE_KIND {
                        let message = format!("`{}` is not a cue of this project (the cues are in family/bevy_siren.ron)", found.name);
                        items.push(Diagnostic::new(found.range, CodeSeverity::Warning, message));
                    }
                }
            }
            commands.entity(entity).insert(Diagnostics::new(revision, items));
            editor.analysed = revision;
            if let Some(file) = files.open.get_mut(&rel) {
                file.edited_at = None;
            }
        }
    }
}

/// **Where the pointer last was, in the window's logical pixels** (NV-07, #384): the file editor's click needs the place a
/// click landed to tell a click in the editor from one on the tree. Bevy's own `Window::cursor_position` is one source; a
/// `PointerInput` message is the other, and it is the one a test's synthetic click and a touch both speak. A resource rather
/// than a read at the click, because a message lasts two frames and the release is a frame after the press.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct PointerAt(pub Option<Vec2>);

/// **Keep [`PointerAt`] true** from either channel: the pointer's own moves, and a pointer input's place.
fn track_the_pointer(mut at: ResMut<PointerAt>, mut inputs: MessageReader<bevy::picking::pointer::PointerInput>, windows: Query<&Window, With<bevy::window::PrimaryWindow>>) {
    // **The last pointer input of the frame wins**, so a synthetic click over an editor is read at its own place rather than at
    // wherever a `CursorMoved` left the pointer.
    for input in inputs.read() {
        at.0 = Some(input.location.position);
    }
    if at.0.is_none()
        && let Some(window) = windows.iter().next()
        && let Some(cursor) = window.cursor_position()
    {
        at.0 = Some(cursor);
    }
}

/// **The open code editors**, as one fact: the file's own state, the widget, its text and where it is drawn. Bundled because
/// three systems read the same four components and the one that also reads the pointer was over Bevy's parameter cap.
#[derive(bevy::ecs::system::SystemParam)]
pub struct OpenEditor<'w, 's> {
    editors: Query<'w, 's, (&'static FileEditor, &'static CodeEditor, &'static CodeEditorValue, &'static ComputedNode, &'static UiGlobalTransform)>,
}

/// **What a Cmd/Ctrl+click reads**: the open editors, the window the pointer is measured in, the pointer's last place, and the two
/// gates. Bundled because they answer one question — *is this click on a reference in the editor?* — and the system was over Bevy's
/// parameter cap.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ClickFacts<'w, 's> {
    open: OpenEditor<'w, 's>,
    windows: Query<'w, 's, &'static Window, With<bevy::window::PrimaryWindow>>,
    at: Res<'w, PointerAt>,
    live: Option<Res<'w, crate::keys::Live>>,
    confirm: Option<Res<'w, crate::confirm::Confirm>>,
}

/// **Whether a reference may be followed right now**: the gates `nav`'s walk and the other chords take — no text field holding the
/// keyboard, the tab holding it, and no question on screen. One function, so F12 and the click cannot disagree about when they are
/// allowed, and neither can drift from `nav`.
fn may_follow(live: Option<&crate::keys::Live>, confirm: Option<&crate::confirm::Confirm>) -> bool {
    let some_other_holder = live.is_some_and(|l| l.0 == crate::keys::Context::Typing || l.2 != crate::keys::Holder::Tab);
    !some_other_holder && !confirm.is_some_and(crate::confirm::Confirm::is_open)
}

/// **The reference under the caret of one editor**, when the caret is on a value that names another thing.
fn reference_under_caret(files: &mut Files, editor: &FileEditor, widget: &CodeEditor, value: &CodeEditorValue) -> Option<files_lang::Reference> {
    let rel = editor.shown.clone()?;
    let schema = files.schema_for(&rel);
    let offset = widget.buffer().offset_of(widget.buffer().caret());
    files_lang::reference_at(Lang::of(&rel), value.text(), offset, schema.as_deref())
}

/// **Open what a reference names**, or say on the status line what it names that the project does not hold.
fn follow_reference_later(commands: &mut Commands, found: files_lang::Reference) {
    commands.queue(move |world: &mut World| {
        if let Err(why) = crate::crosslinks::follow_reference(world, &found.kind, &found.name)
            && let Some(mut state) = world.get_resource_mut::<EditorState>()
        {
            state.status.problem(format!("NOT OPENED: {why}"));
        }
    });
}

/// **F12 follows the reference under the caret** (NV-07, #384), the editor's own key as the editor's other keys are: not an action of the keymap.
fn follow_on_f12(
    keys: Res<ButtonInput<KeyCode>>,
    mut files: ResMut<Files>,
    open: OpenEditor,
    mut state: Option<ResMut<EditorState>>,
    live: Option<Res<crate::keys::Live>>,
    confirm: Option<Res<crate::confirm::Confirm>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::F12) {
        return;
    }
    // **The gates every other chord respects** (`nav::walk_on_the_chords`): not while a text field has the keyboard, not while
    // another pane holds it, and not over a question — the finder's own box is a text field, so Cmd+P then F12 would otherwise
    // follow the reference behind the overlay.
    if !may_follow(live.as_deref(), confirm.as_deref()) {
        return;
    }
    let found = open.editors.iter().next().and_then(|(editor, widget, value, _, _)| reference_under_caret(&mut files, editor, widget, value));
    match found {
        Some(found) => follow_reference_later(&mut commands, found),
        None => {
            if let Some(state) = state.as_mut() {
                state.status.note("there is no reference under the caret to follow");
            }
        }
    }
}

/// **Cmd/Ctrl+click follows the reference under the click** (NV-07, #384): the press has already put the caret where the pointer is, so the
/// reference under the caret is the one under the click. Only while the pointer is over the editor; a plain click only edits.
fn follow_on_primary_click(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut files: ResMut<Files>,
    // **The editor's own place on screen**, so a click is a click *in the editor*: the tree, the header and the notice are in the same
    // workspace, and a Cmd+click on one of them must not follow whatever reference the caret was left on. `Hovered` is no use for this
    // (bevy sets it on the deepest node under the pointer, which inside the editor is the editor's own node but says nothing about where
    // in it the pointer is), so the node's bounds are measured against the pointer.
    facts: ClickFacts,
    mut commands: Commands,
) {
    // **On the release**, a frame after the press: the editor puts the caret where the pointer went down in its own system, and a read on the
    // press frame could be of the caret's old place.
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let primary = [KeyCode::SuperLeft, KeyCode::SuperRight, KeyCode::ControlLeft, KeyCode::ControlRight].into_iter().any(|k| keys.pressed(k));
    if !primary || !may_follow(facts.live.as_deref(), facts.confirm.as_deref()) {
        return;
    }
    // **Inside the editor, by the node's own bounds.** `ComputedNode` and `UiGlobalTransform` are in physical pixels and the pointer is
    // in the window's logical ones, so the pointer is scaled by the window's factor before the test. A click with no pointer place at
    // all follows nothing: there is no way to tell it from one on the tree.
    let (Some(pointer), scale) = (facts.at.0, facts.windows.iter().next().map_or(1.0, Window::scale_factor)) else {
        return;
    };
    let found = facts
        .open
        .editors
        .iter()
        .find(|(_, _, _, node, place)| {
            let offset = pointer * scale - place.translation;
            let half = node.size() * 0.5;
            offset.x.abs() <= half.x && offset.y.abs() <= half.y
        })
        .and_then(|(editor, widget, value, _, _)| reference_under_caret(&mut files, editor, widget, value));
    if let Some(found) = found {
        follow_reference_later(&mut commands, found);
    }
}

fn keep_the_header(files: Res<Files>, session: Res<Session>, mut headers: Query<&mut Text, With<FileHeader>>) {
    let Ok(mut header) = headers.single_mut() else { return };
    let want = match active_file(&session).and_then(|rel| files.open.get(&rel).map(|f| (rel, f))) {
        None => "Open a file from the tree".to_owned(),
        Some((rel, file)) => {
            let mut line = rel;
            // **Why it cannot be edited, in the words of the reason**: a file that is not text, too big or gone says so,
            // and a command file says it is changed on its own page.
            let note = file.problem.as_deref().or(file.read_only.then_some(files_lang::READ_ONLY_NOTE));
            if file.dirty() {
                line.push_str("  \u{25CF}");
            }
            if let Some(note) = note {
                line.push_str("  \u{2014}  ");
                line.push_str(note);
            }
            line
        }
    };
    if header.0 != want {
        header.0 = want;
    }
}
fn keep_the_conflict_bar(
    mut commands: Commands,
    files: Res<Files>,
    session: Res<Session>,
    hosts: Query<Entity, With<FilesHost>>,
    bars: Query<(Entity, &ConflictBar)>,
) {
    let Ok(host) = hosts.single() else { return };
    let asking = active_file(&session).filter(|rel| files.open.get(rel).is_some_and(|f| f.external.is_some()));
    let standing = bars.single().ok();
    // **The bar names the file it asks about**: switching to another file that also changed replaces it.
    match (asking, standing) {
        (Some(rel), Some((_, bar))) if bar.0 == rel => {}
        (Some(rel), standing) => {
            if let Some((old, _)) = standing {
                commands.entity(old).despawn();
            }
            let bar = commands
                .spawn((
                    ConflictBar(rel.clone()),
                    bevy_gooey::row(1.0),
                    children![
                        (bevy_gooey::label(conflict_sentence(&rel))),
                        (bevy_gooey::button("Reload"), ConflictAnswer::Reload, observe(answer_the_conflict)),
                        (bevy_gooey::button("Keep mine"), ConflictAnswer::KeepMine, observe(answer_the_conflict)),
                    ],
                ))
                .id();
            commands.entity(host).insert_children(CONFLICT_BAR_INDEX, &[bar]);
        }
        (None, Some((bar, _))) => {
            commands.entity(bar).despawn();
        }
        (None, None) => {}
    }
}

/// Where in the editor's column the question stands: under the header (child 0), over the field line and the editor.
const CONFLICT_BAR_INDEX: usize = 1;
/// **Reload** takes the disk's text and drops the person's; **Keep mine** keeps theirs and takes what the disk
/// holds as the new thing a save would replace.
fn answer_the_conflict(activate: On<Activate>, answers: Query<&ConflictAnswer>, mut files: ResMut<Files>, session: Res<Session>) {
    let Ok(answer) = answers.get(activate.entity) else { return };
    let Some(rel) = active_file(&session) else { return };
    let Some(file) = files.open.get_mut(&rel) else { return };
    let Some(theirs) = file.external.take() else { return };
    match answer {
        ConflictAnswer::Reload => {
            file.text = theirs.clone();
            file.disk = theirs;
            file.pushed += 1;
        }
        ConflictAnswer::KeepMine => file.disk = theirs,
    }
}

/// **Complete from the kit's schema**: the field names, an enum's values, the ids a reference points at.
fn on_completion_request(
    request: On<CodeCompletionRequested>,
    mut files: ResMut<Files>,
    editors: Query<(&FileEditor, &CodeEditorValue)>,
    trees: Query<&TreeNames, With<FilesTree>>,
    project: Option<Res<Project>>,
    mut commands: Commands,
) {
    let Ok((editor, value)) = editors.get(request.entity) else { return };
    let Some(rel) = editor.shown.clone() else { return };
    let schema = files.schema_for(&rel);
    let refs = |kind: &str| -> Vec<String> {
        let Some(project) = project.as_deref() else { return Vec::new() };
        match kind {
            "library" => project.library.descriptors.iter().map(|d| d.id.clone()).collect(),
            "compositions" => project.compositions.compositions.iter().map(|c| c.id.clone()).collect(),
            _ => Vec::new(),
        }
    };
    // **The files the tree lists** are what a Markdown path may name after the board's pictures.
    let listed = || -> Vec<String> { trees.iter().next().map(|t| t.names.values().filter(|(_, dir)| !dir).map(|(rel, _)| rel.clone()).collect()).unwrap_or_default() };
    let root = files.root.clone();
    let around = files_lang::Env { root: root.as_deref(), refs: &refs, files: &listed };
    let found = files_lang::complete(&rel, value.text(), request.offset, schema.as_deref(), &around);
    let items = found
        .items
        .into_iter()
        .map(|(label, detail, insert)| {
            let item = CompletionItem::new(label).detail(detail);
            match insert {
                Some(text) => item.insert(text),
                None => item,
            }
        })
        .collect();
    commands.entity(request.entity).insert(CodeCompletions { anchor: found.anchor, items });
}

/// **Say what a key is**, from its doc comment, and what a picture is, from its place's board.
fn on_hover_request(request: On<CodeHoverRequested>, mut files: ResMut<Files>, editors: Query<(&FileEditor, &CodeEditorValue)>, mut commands: Commands) {
    let Ok((editor, value)) = editors.get(request.entity) else { return };
    let Some(rel) = editor.shown.clone() else { return };
    let schema = files.schema_for(&rel);
    let root = files.root.clone();
    let around = files_lang::Env { root: root.as_deref(), refs: &|_: &str| Vec::new(), files: &Vec::new };
    if let Some((range, text)) = files_lang::hover(&rel, value.text(), request.offset, schema.as_deref(), &around) {
        commands.entity(request.entity).insert(CodeHover { revision: request.revision, range, text });
    }
}

fn keep_the_tree(
    mut commands: Commands,
    project: Option<Res<Project>>,
    session: Res<Session>,
    time: Res<Time<Real>>,
    mut trees: Query<(Entity, &mut bevy_gooey::Table, Option<&mut TreeNames>), With<FilesTree>>,
    mut notices: Query<&mut Text, With<TreeNotice>>,
) {
    let Some(project) = project else { return };
    let Ok((entity, mut table, names)) = trees.single_mut() else { return };
    let Some(mut names) = names else {
        commands.entity(entity).insert(TreeNames::default());
        return;
    };
    let now = time.elapsed_secs_f64();
    // **The walk is read off the UI thread**: started on a timer, taken when it is done, and only a changed listing
    // replaces the nodes.
    let finished = names.scan.as_ref().and_then(|scan| scan.lock().ok().map(|rx| rx.try_recv()));
    match finished {
        Some(Ok(walked)) => {
            names.scan = None;
            names.truncated = walked.truncated;
            if walked.fingerprint != names.fingerprint || names.names.is_empty() {
                if let Some(tree) = table.source_mut::<Tree>() {
                    let open = tree.open_keys();
                    tree.set_nodes(walked.nodes);
                    for key in open {
                        tree.set_open(key, true);
                    }
                }
                names.names = walked.names;
                names.fingerprint = walked.fingerprint;
            }
        }
        Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => names.scan = None,
        _ => {}
    }
    if names.scan.is_none() && now >= names.next_poll {
        names.next_poll = now + TREE_POLL_S;
        let root = project.root.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(walk_project(&root));
        });
        names.scan = Some(std::sync::Mutex::new(rx));
    }
    let want_notice = if names.truncated { TREE_TRUNCATED } else { "" };
    for mut notice in &mut notices {
        if notice.0 != want_notice {
            notice.0 = want_notice.to_owned();
        }
    }
    let want = active_file(&session).map(|rel| key_of(&rel));
    if want != names.selected {
        names.selected = want;
        match want {
            Some(key) if names.names.contains_key(&key) => {
                commands.entity(entity).insert(TableSelection::of(key));
            }
            _ => {
                commands.entity(entity).insert(TableSelection::default());
            }
        }
    }
}

/// **How many frames the active file's row must stay inside the tree's viewport before the reveal lets go of it.** Layout
/// reads the rows of the frame before, so a row that fits once may be moved by the scroll bounds the next.
const REVEAL_STEADY_FRAMES: u8 = 2;

/// **Open the folders above the active file and scroll its row into view** (#359, AF-03). A selection alone names a row
/// the tree may not have: a file in a shut folder has none, and a long tree keeps the one it has below its fold. gooey
/// scrolls a table to its cursor for a key or a click only, so this puts the viewport where the row is the way the
/// DATA table's reveal does (`reveal::table_row_geometry` and `chrome::scroll_to_reveal`, the editor's one pair of
/// scroll sums, whose own threshold says when a row is already where it should be), once per file made active, and
/// stops when the row has been inside for [`REVEAL_STEADY_FRAMES`]. The table is touched only when a folder has to open.
fn reveal_the_active_row(
    mut trees: Query<(Entity, &mut bevy_gooey::Table, &mut TreeNames), With<FilesTree>>,
    children: Query<&Children>,
    mut viewports: Query<(&ComputedNode, &UiGlobalTransform, &ListView, &mut ScrollPosition)>,
) {
    for (entity, mut table, mut names) in &mut trees {
        let Some(key) = names.selected else {
            if names.revealed.is_some() {
                names.revealed = None;
            }
            continue;
        };
        if names.revealed == Some(key) {
            continue;
        }
        let Some(rel) = names.names.get(&key).map(|(rel, _)| rel.clone()) else { continue };
        // The viewport is the nearest descendant of the tree that is a `ListView`; one with no height has nothing to show yet.
        let Some(viewport) = children.iter_descendants(entity).find(|e| viewports.contains(*e)) else { continue };
        let Ok((node, at, view, mut scroll)) = viewports.get_mut(viewport) else { continue };
        if view.rows <= 0.0 || node.size().y <= 0.0 {
            continue;
        }
        // Every folder above the file: the prefixes before each `/`. The table is written only for one that is shut.
        let shut: Vec<u64> = match table.source::<Tree>() {
            Some(tree) => rel.match_indices('/').map(|(at, _)| key_of(&rel[..at])).filter(|folder| tree.is_open(*folder) == Some(false)).collect(),
            None => continue,
        };
        if !shut.is_empty()
            && let Some(tree) = table.source_mut::<Tree>()
        {
            for folder in shut {
                tree.set_open(folder, true);
            }
        }
        let Some(tree) = table.source::<Tree>() else { continue };
        let Some(row) = (0..RowSource::len(tree)).find(|row| RowSource::key(tree, *row) == key) else { continue };
        let (scroll_y, max) = chrome::scroll_bounds(node);
        let inv = node.inverse_scale_factor();
        let Some((row_mid, pitch)) = crate::reveal::table_row_geometry((at.translation.y, node.size().y), view.rows, row, scroll_y, inv) else { continue };
        match chrome::scroll_to_reveal((row_mid, pitch / 2.0), (at.translation.y, node.size().y / 2.0), scroll_y, max, inv) {
            Some(want) => {
                names.steady = 0;
                scroll.0.y = want;
            }
            None => {
                names.steady += 1;
                if names.steady >= REVEAL_STEADY_FRAMES {
                    names.revealed = Some(key);
                    names.steady = 0;
                }
            }
        }
    }
}

// ── What an agent's operations need to know about the open files ─────────────

/// What the operations (`file_edit`, `files_replace`) may ask of this module without a window: which files a
/// person has unsaved edits in, and which file an agent just wrote.
///
/// **Process-wide, keyed by project root**, because an operation runs in a [`crate::ops::Ctx`], which owns
/// what it carries and has no world to ask. The editor publishes the first every frame it changes
/// ([`publish_unsaved`]); the second is noted by `file_edit` as it writes and taken by the poll that sees the
/// write ([`take_agent_edit`]).
#[derive(Default)]
struct Registry {
    /// The open project's root and the files the person has unsaved edits in: one slot, because one project is open.
    unsaved: Option<(PathBuf, Vec<String>)>,
    agent: Vec<(PathBuf, String)>,
}

static REGISTRY: std::sync::LazyLock<std::sync::Mutex<Registry>> = std::sync::LazyLock::new(Default::default);

fn registry() -> std::sync::MutexGuard<'static, Registry> {
    // A panic with the lock held cannot leave it half-written: both fields are replaced whole.
    REGISTRY.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn person_is_editing(root: &Path, rel: &str) -> bool {
    registry().unsaved.as_ref().is_some_and(|(r, files)| r == root && files.iter().any(|f| f == rel))
}

/// **Whether any file of the open project has unsaved edits**: what leaving the editor, and quitting it, ask about
/// beside the map.
pub fn any_unsaved() -> bool {
    registry().unsaved.as_ref().is_some_and(|(_, files)| !files.is_empty())
}
fn publish_unsaved(root: &Path, dirty: Vec<String>) {
    let mut registry = registry();
    let same = registry.unsaved.as_ref().is_some_and(|(r, files)| r == root && *files == dirty);
    if !same {
        registry.unsaved = (!dirty.is_empty()).then(|| (root.to_path_buf(), dirty));
    }
}
/// **Note that an agent just wrote `rel`**, so the editor tints the lines it changes in the Familiar's colour.
pub fn note_agent_edit(root: &Path, rel: &str) {
    let mut registry = registry();
    let entry = (root.to_path_buf(), rel.to_owned());
    if !registry.agent.contains(&entry) {
        registry.agent.push(entry);
    }
}

/// Whether an agent's write of `rel` was noted, and forget it.
fn take_agent_edit(root: &Path, rel: &str) -> bool {
    let mut registry = registry();
    let before = registry.agent.len();
    registry.agent.retain(|(r, f)| !(r == root && f == rel));
    registry.agent.len() != before
}

/// **The one edit that turns `old` into `new`**: everything between their common head and tail, on character
/// boundaries. `None` when they are the same text.
pub fn diff_edit(old: &str, new: &str) -> Option<CodeEdit> {
    if old == new {
        return None;
    }
    let mut start = old.bytes().zip(new.bytes()).take_while(|(a, b)| a == b).count();
    while !old.is_char_boundary(start) || !new.is_char_boundary(start) {
        start -= 1;
    }
    let (mut end_old, mut end_new) = (old.len(), new.len());
    while end_old > start && end_new > start && old.as_bytes()[end_old - 1] == new.as_bytes()[end_new - 1] {
        end_old -= 1;
        end_new -= 1;
    }
    while end_old < old.len() && end_new < new.len() && (!old.is_char_boundary(end_old) || !new.is_char_boundary(end_new)) {
        end_old += 1;
        end_new += 1;
    }
    Some(CodeEdit::new(start..end_old, new[start..end_new].to_owned()))
}

/// **The lines (1-based, inclusive) that differ**, from the first changed line to the last in the new text.
pub fn changed_lines(old: &str, new: &str) -> Option<[u32; 2]> {
    if old == new {
        return None;
    }
    let a: Vec<&str> = old.split('\n').collect();
    let b: Vec<&str> = new.split('\n').collect();
    let head = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let tail = a[head..].iter().rev().zip(b[head..].iter().rev()).take_while(|(x, y)| x == y).count();
    let last = (b.len() - tail).max(head + 1);
    Some([head as u32 + 1, last as u32])
}

/// **The agent's changed lines, as marks the minimap draws** (and the gutter): a hint on the first word of each
/// line, saying who changed it. The editor's own diagnostics are the one channel `bevy_gooey` marks a line
/// through, so the marks ride it with the least severity there is.
fn agent_marks(text: &str, lines: &[[u32; 2]]) -> Vec<Diagnostic> {
    let index = files_lang::Lines::new(text);
    let mut out = Vec::new();
    for [first, last] in lines {
        for line in (*first as usize)..=(*last as usize) {
            if line == 0 || line > index.count() {
                continue;
            }
            let range = index.range(line - 1);
            let line_text = &text[range.clone()];
            let indent = line_text.len() - line_text.trim_start().len();
            let from = range.start + indent;
            let to = (from + line_text[indent..].chars().next().map_or(0, char::len_utf8)).max(from);
            out.push(Diagnostic::new(from..to, CodeSeverity::Hint, AGENT_CHANGED));
        }
    }
    out
}

/// What the mark on an agent's line says.
pub const AGENT_CHANGED: &str = "Changed by the Familiar";

/// The marks [`agent_marks`] makes, for a test.
pub fn agent_marks_for_test(text: &str, lines: &[[u32; 2]]) -> Vec<Diagnostic> {
    agent_marks(text, lines)
}
