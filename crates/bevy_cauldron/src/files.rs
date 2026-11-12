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
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::{Activate, ValueChange};
use bevy_gooey::{
    CodeCompletionRequested, CodeCompletions, CodeEditorValue, CodeHover, CodeHoverRequested, CodeIndent, CodeIndentation, CompletionItem,
    Diagnostic, Diagnostics, FoldRanges, Highlights, SelectMode, TableSelection, Tree, TreeNode, observe, tree,
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

/// The width of the file tree's panel, in logical pixels.
pub const TREE_PANEL_W: f32 = 300.0;

/// The folders the tree never lists, by name, beside what `.gitignore` names.
const SKIPPED_DIRS: [&str; 4] = [".git", "target", ".live", "node_modules"];

/// The notice under the tree when it stopped at [`TREE_LIMIT`].
pub const TREE_TRUNCATED: &str = "The tree stops at 20,000 entries; narrow the project to see the rest";

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
    /// When the text last changed by typing, on the real clock: the errors wait for [`DIAGNOSE_IDLE_S`] after it.
    edited_at: Option<f64>,
    /// Counts the times the text was replaced from outside the editor widget (a reload), so the widget knows
    /// to take it.
    pub pushed: u64,
    /// A line to bring into view and put the caret on, when the editor next shows this file.
    pub reveal: Option<usize>,
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

    /// The paths with unsaved edits, in order.
    pub fn dirty(&self) -> Vec<&str> {
        self.open.iter().filter(|(_, f)| f.dirty()).map(|(rel, _)| rel.as_str()).collect()
    }

    /// **The JSON Schema of the kind `rel` is**, generated from the type the loader reads
    /// ([`bevy_grimoire_core::schema`]) and kept: generating one is a `schemars` walk.
    pub fn schema_for(&mut self, rel: &str) -> Option<Arc<Value>> {
        let (kind, _) = bevy_grimoire_core::reload_check::kind_of_path(rel)?;
        self.schemas
            .entry(kind.id)
            .or_insert_with(|| bevy_grimoire_core::schema::for_kind(kind.id).ok().map(Arc::new))
            .clone()
    }

    /// Replace an open file's text from outside the editor widget (an agent's edit that could not land line by
    /// line, a reload): the widget takes it on its next frame. Does not touch what the disk held.
    pub fn replace_text(&mut self, rel: &str, text: String) -> bool {
        let Some(file) = self.open.get_mut(rel) else { return false };
        file.text = text;
        file.pushed += 1;
        true
    }
}

/// **Whether a path is one the Files workspace may open**: a relative path that stays inside the project.
/// `Err` says why not, for a notice.
fn inside(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let path = Path::new(rel);
    if rel.is_empty() || path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_) | Component::RootDir)) {
        return Err(format!("{rel} is not a path inside the project"));
    }
    let full = root.join(path);
    // A symlink is followed only while it stays in the project: the tree never lists one, and a path typed by
    // hand or sent by an agent must not reach what the project's folder does not hold.
    if let (Ok(real), Ok(base)) = (std::fs::canonicalize(&full), std::fs::canonicalize(root))
        && !real.starts_with(&base)
    {
        return Err(format!("{rel} is outside the project"));
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
            edited_at: None,
            pushed: 0,
            reveal: None,
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

/// **Save one open file**: write its text, atomically, and take the write as what the disk holds. A refusal
/// names why and writes nothing: a read-only file, a file that changed under the person's edits (they have
/// not answered the question yet), a file that is no longer there to be replaced by a surprise.
pub fn save_file(files: &mut Files, root: &Path, rel: &str) -> Result<(), String> {
    let file = files.open.get_mut(rel).ok_or_else(|| format!("{rel} is not open"))?;
    if file.read_only {
        return Err(format!("{rel}: {}", files_lang::READ_ONLY_NOTE));
    }
    if file.external.is_some() {
        return Err(format!("{rel} changed on disk while you were editing it: choose Reload or Keep mine first"));
    }
    let full = inside(root, rel)?;
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

/// A tree row's key: FNV-1a over the path, so the same file is the same row on every rebuild and the table
/// keeps its selection and its open folders.
pub fn key_of(rel: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in rel.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
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
            if *anchored { glob(pattern, rel) } else { glob(pattern, name) }
        })
    }
}

/// `*` (any run of characters but `/`) and `?` (one), over the whole of `text`.
fn glob(pattern: &str, text: &str) -> bool {
    fn go(p: &[u8], t: &[u8]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some(b'*'), _) => go(&p[1..], t) || (!t.is_empty() && t[0] != b'/' && go(p, &t[1..])),
            (Some(b'?'), Some(c)) if *c != b'/' => go(&p[1..], &t[1..]),
            (Some(a), Some(b)) if a == b => go(&p[1..], &t[1..]),
            _ => false,
        }
    }
    go(pattern.as_bytes(), text.as_bytes())
}

/// What the walk of a project's folders made: the tree's nodes and what each key names.
#[derive(Default)]
struct Walked {
    nodes: Vec<TreeNode>,
    names: HashMap<u64, (String, bool)>,
    count: usize,
    truncated: bool,
}

fn walk_dir(root: &Path, dir: &Path, ignore: &Ignore, walked: &mut Walked, open: &dyn Fn(u64) -> Option<bool>) -> Vec<TreeNode> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut listed: Vec<(String, bool)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let meta = std::fs::symlink_metadata(entry.path()).ok()?;
            // A link is never followed (the tree stays inside the project) and never listed.
            (!meta.file_type().is_symlink()).then(|| (entry.file_name().to_string_lossy().into_owned(), meta.is_dir()))
        })
        .collect();
    listed.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
    let mut out = Vec::new();
    for (name, is_dir) in listed {
        let rel_path = dir.join(&name);
        let Ok(rel) = rel_path.strip_prefix(root) else { continue };
        let rel = rel.to_string_lossy().replace('\\', "/");
        if (is_dir && SKIPPED_DIRS.contains(&name.as_str())) || name == ".DS_Store" || ignore.matches(&rel, is_dir) {
            continue;
        }
        if walked.count >= TREE_LIMIT {
            walked.truncated = true;
            break;
        }
        walked.count += 1;
        let key = key_of(&rel);
        walked.names.insert(key, (rel.clone(), is_dir));
        let mut node = TreeNode::new(key, name);
        if is_dir {
            node = node.children(walk_dir(root, &rel_path, ignore, walked, open));
            if open(key).unwrap_or(false) {
                node = node.expanded();
            }
        }
        out.push(node);
    }
    out
}

/// **Every file and folder under the project, as tree nodes**, with a map from each row's key to what it
/// names. The first level opens; the rest stay as the person left them.
fn walk_project(root: &Path, open: &dyn Fn(u64) -> Option<bool>) -> Walked {
    let ignore = std::fs::read_to_string(root.join(".gitignore")).map(|t| Ignore::parse(&t)).unwrap_or_default();
    let mut walked = Walked::default();
    let nodes = walk_dir(root, root, &ignore, &mut walked, open);
    walked.nodes = nodes;
    walked
}

/// **The paths the tree lists**, files only, in tree order: what a test and the search read.
pub fn listed_files(root: &Path) -> Vec<String> {
    let walked = walk_project(root, &|_| None);
    let mut files: Vec<String> = walked.names.into_values().filter(|(_, dir)| !dir).map(|(rel, _)| rel).collect();
    files.sort();
    files
}

// ── Components ───────────────────────────────────────────────────────────────

/// The tree panel in the left dock.
#[derive(Component)]
pub struct FilesPanel;

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
pub struct ConflictBar;

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

fn in_files(workspace: Option<Res<Workspace>>) -> bool {
    workspace.is_some_and(|w| *w == Workspace::Files)
}

/// The path of the file whose tab is active in the Files workspace.
pub fn active_file(session: &Session) -> Option<String> {
    session.tabs(Workspace::Files).active_doc().filter(|d| d.kind == Kind::File).map(|d| d.name.clone())
}

fn indentation_of(lang: Lang) -> CodeIndentation {
    let spaces = match lang {
        Lang::Yaml | Lang::Markdown => 2,
        Lang::Ron | Lang::Rhai | Lang::Plain => 4,
    };
    CodeIndentation { indent: CodeIndent::Spaces(spaces), tab_width: 4 }
}

// ── The plugin ───────────────────────────────────────────────────────────────

/// **The Files workspace**: the tree, the editor and what keeps them true.
pub struct FilesPlugin;

impl Plugin for FilesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Files>()
            .add_systems(OnEnter(Screen::Editor), spawn_the_files_workspace.after(FrameSystems).run_if(in_files))
            .add_systems(
                Update,
                (
                    keep_the_open_files,
                    poll_the_disk,
                    keep_the_editor,
                    decorate_the_editor,
                    keep_the_header,
                    keep_the_conflict_bar,
                    keep_the_tree,
                )
                    .chain()
                    .run_if(in_state(Screen::Editor))
                    .run_if(in_files),
            );
    }
}

/// **Spawn the tree and the editor's column** when the Files workspace is entered. The code editor itself is
/// spawned by [`keep_the_editor`] while a file is open, so a workspace with no file open holds no editor.
fn spawn_the_files_workspace(mut commands: Commands, frame: Res<Frame>) {
    let panel = chrome::panel_root(&mut commands, &frame, Side::Left, TREE_PANEL_W, true, false, "FILES").insert(FilesPanel).id();
    commands.entity(panel).with_children(|p| {
        chrome::title(p, "FILES");
        p.spawn(Node { flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, ..default() })
            .with_children(|column| {
                column.spawn((tree(Vec::<TreeNode>::new()), FilesTree, SelectMode::Single, observe(open_the_picked_file)));
            });
    });
    let ink = chrome::ink();
    let host = commands
        .spawn((
            FilesHost,
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(chrome::GAP_ROW),
                padding: UiRect::all(Val::Px(chrome::PAD)),
                margin: UiRect::all(Val::Px(chrome::MARGIN)),
                border: UiRect::all(Val::Px(chrome::EDGE_W)),
                border_radius: BorderRadius::all(Val::Px(chrome::RADIUS_PANEL)),
                ..default()
            },
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
        })
        .id();
    commands.entity(frame.viewport).add_child(host);
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
    listed: Vec<(String, bool)>,
    next_poll: f64,
    selected: Option<u64>,
}

fn tree_names(world: &mut World) -> HashMap<u64, (String, bool)> {
    world.query_filtered::<&TreeNames, With<FilesTree>>().iter(world).next().map(|t| t.names.clone()).unwrap_or_default()
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// **Keep the open files in step with the tabs.** A tab the session restored has no text yet: read it. A
/// file whose tab was closed and which has nothing unsaved is let go. The project changing drops all of them.
fn keep_the_open_files(mut files: ResMut<Files>, session: Res<Session>, project: Option<Res<Project>>, mut state: Option<ResMut<EditorState>>) {
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
                    OpenFile { text: String::new(), disk: String::new(), stamp: None, external: None, read_only: true, edited_at: None, pushed: 0, reveal: None },
                );
            }
        }
    }
    files.open.retain(|rel, file| wanted.contains(&rel.as_str()) || file.dirty());
}

/// **Look at each open file for a change made from outside** (an editor, a build, an agent's tool).
fn poll_the_disk(mut files: ResMut<Files>, time: Res<Time<Real>>, mut last: Local<f64>) {
    let now = time.elapsed_secs_f64();
    if now - *last < POLL_S {
        return;
    }
    *last = now;
    let Some(root) = files.root.clone() else { return };
    for (rel, file) in files.open.iter_mut() {
        let full = root.join(rel);
        let stamp = Stamp::of(&full);
        if stamp == file.stamp {
            continue;
        }
        file.stamp = stamp;
        let Ok(text) = std::fs::read_to_string(&full) else { continue };
        if text == file.disk {
            file.external = None;
        } else if text == file.text {
            // The same text arrived by another road: nothing to ask.
            file.disk = text;
            file.external = None;
        } else if file.text == file.disk {
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
/// when none is; switch the text when the tab changes or the text was replaced from outside.
fn keep_the_editor(
    mut commands: Commands,
    files: Res<Files>,
    session: Res<Session>,
    hosts: Query<Entity, With<FilesHost>>,
    mut editors: Query<(Entity, &mut FileEditor, &mut CodeEditorValue, Option<&InteractionDisabled>)>,
) {
    let Ok(host) = hosts.single() else { return };
    let active = active_file(&session);
    let existing = editors.iter_mut().next();
    match (active.as_deref().and_then(|rel| files.open.get(rel).map(|f| (rel, f))), existing) {
        (None, Some((entity, ..))) => {
            commands.entity(entity).despawn();
        }
        (Some((rel, file)), None) => {
            let lang = Lang::of(rel);
            let editor = commands
                .spawn((
                    bevy_gooey::code_editor(rel.to_owned(), file.text.clone()),
                    FileEditor { shown: Some(rel.to_owned()), pushed: file.pushed, highlighted: u64::MAX, analysed: u64::MAX },
                    indentation_of(lang),
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
        (Some((rel, file)), Some((entity, mut editor, mut value, disabled))) => {
            let switched = editor.shown.as_deref() != Some(rel);
            if switched || editor.pushed != file.pushed {
                value.set(file.text.clone());
                if switched {
                    commands.entity(entity).insert(indentation_of(Lang::of(rel)));
                }
                editor.shown = Some(rel.to_owned());
                editor.pushed = file.pushed;
                editor.highlighted = u64::MAX;
                editor.analysed = u64::MAX;
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
    if let Some(file) = files.open.get_mut(&rel) {
        file.text = change.value.clone();
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
fn decorate_the_editor(mut commands: Commands, mut files: ResMut<Files>, time: Res<Time<Real>>, mut editors: Query<(Entity, &mut FileEditor, &CodeEditorValue)>) {
    let now = time.elapsed_secs_f64();
    for (entity, mut editor, value) in &mut editors {
        let Some(rel) = editor.shown.clone() else { continue };
        let revision = value.revision();
        let lang = Lang::of(&rel);
        if editor.highlighted != revision {
            let schema = files.schema_for(&rel);
            commands.entity(entity).insert((
                Highlights::new(revision, files_lang::highlights(lang, value.text(), schema.as_deref())),
                FoldRanges::new(revision, files_lang::folds(lang, value.text())),
            ));
            editor.highlighted = revision;
        }
        let waiting = files.open.get(&rel).and_then(|f| f.edited_at).is_some_and(|at| now - at < DIAGNOSE_IDLE_S);
        if editor.analysed != revision && !waiting {
            let findings = files_lang::diagnose(&rel, value.text());
            commands.entity(entity).insert(Diagnostics::new(
                revision,
                findings.into_iter().map(|f| Diagnostic::new(f.range, f.severity, f.message)),
            ));
            editor.analysed = revision;
            if let Some(file) = files.open.get_mut(&rel) {
                file.edited_at = None;
            }
        }
    }
}

/// **The line over the editor**: the path, a dot when there are unsaved edits, and why a file is read-only.
fn keep_the_header(files: Res<Files>, session: Res<Session>, mut headers: Query<&mut Text, With<FileHeader>>) {
    let Ok(mut header) = headers.single_mut() else { return };
    let want = match active_file(&session).and_then(|rel| files.open.get(&rel).map(|f| (rel, f))) {
        None => "Open a file from the tree".to_owned(),
        Some((rel, file)) => {
            let mut line = rel;
            if file.dirty() {
                line.push_str("  \u{25CF}");
            }
            if file.read_only {
                line.push_str("  \u{2014}  ");
                line.push_str(files_lang::READ_ONLY_NOTE);
            }
            line
        }
    };
    if header.0 != want {
        header.0 = want;
    }
}

/// **The question a changed file asks**: spawned while one is waiting, gone when it is answered.
fn keep_the_conflict_bar(
    mut commands: Commands,
    files: Res<Files>,
    session: Res<Session>,
    hosts: Query<Entity, With<FilesHost>>,
    bars: Query<(Entity, &Children), With<ConflictBar>>,
) {
    let Ok(host) = hosts.single() else { return };
    let asking = active_file(&session).filter(|rel| files.open.get(rel).is_some_and(|f| f.external.is_some()));
    match (asking, bars.single()) {
        (Some(rel), Err(_)) => {
            let bar = commands
                .spawn((
                    ConflictBar,
                    bevy_gooey::row(1.0),
                    children![
                        (bevy_gooey::label(conflict_sentence(&rel))),
                        (bevy_gooey::button("Reload"), ConflictAnswer::Reload, observe(answer_the_conflict)),
                        (bevy_gooey::button("Keep mine"), ConflictAnswer::KeepMine, observe(answer_the_conflict)),
                    ],
                ))
                .id();
            commands.entity(host).insert_children(1, &[bar]);
        }
        (None, Ok((bar, _))) => {
            commands.entity(bar).despawn();
        }
        _ => {}
    }
}

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
    let found = files_lang::complete(Lang::of(&rel), value.text(), request.offset, schema.as_deref(), &refs);
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

/// **Say what a key is**, from its doc comment.
fn on_hover_request(request: On<CodeHoverRequested>, mut files: ResMut<Files>, editors: Query<(&FileEditor, &CodeEditorValue)>, mut commands: Commands) {
    let Ok((editor, value)) = editors.get(request.entity) else { return };
    let Some(rel) = editor.shown.clone() else { return };
    let schema = files.schema_for(&rel);
    if let Some((range, text)) = files_lang::hover(Lang::of(&rel), value.text(), request.offset, schema.as_deref()) {
        commands.entity(request.entity).insert(CodeHover { revision: request.revision, range, text });
    }
}

/// **Keep the tree true to the folder**: read again every [`TREE_POLL_S`], replace the nodes only when what
/// is listed changed (a settled tree costs one walk and no rebuild), and keep the row of the active file
/// selected.
fn keep_the_tree(
    mut commands: Commands,
    project: Option<Res<Project>>,
    session: Res<Session>,
    time: Res<Time<Real>>,
    mut trees: Query<(Entity, &mut bevy_gooey::Table, Option<&mut TreeNames>), With<FilesTree>>,
) {
    let Some(project) = project else { return };
    let Ok((entity, mut table, names)) = trees.single_mut() else { return };
    let Some(mut names) = names else {
        commands.entity(entity).insert(TreeNames::default());
        return;
    };
    let now = time.elapsed_secs_f64();
    if now >= names.next_poll {
        names.next_poll = now + TREE_POLL_S;
        let Some(tree) = table.source_mut::<Tree>() else { return };
        let walked = walk_project(&project.root, &|key| tree.is_open(key));
        let mut listed: Vec<(String, bool)> = walked.names.values().cloned().collect();
        listed.sort();
        if listed != names.listed {
            tree.set_nodes(walked.nodes);
            names.names = walked.names;
            names.listed = listed;
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
