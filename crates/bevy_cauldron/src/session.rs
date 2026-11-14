//! **Document tabs: the things a workspace has open** (NV-04, #381; series getting around, #377).
//!
//! A workspace is a job and a layout is what is on screen (`docs/ui/vision.md` §4); *what you are
//! working on* is a third thing, and it is the tabs. Inside a workspace the things you open are tabs:
//!
//! | Workspace | Documents |
//! |---|---|
//! | Map, World | maps |
//! | Files | files |
//! | Kit | meshes, tiles, compositions |
//! | Animation | rigs, clips |
//! | Sound | cue tables, music scores |
//!
//! # One map is live at a time
//!
//! Choosing another map's tab opens that map, as opening it from the project page does ([`open_map_now`]
//! is the same `map_open` operation an agent calls). A map with unsaved edits asks first, through the
//! question the door switch already raises (`EditorState::map_request`). So the map tabs are *a list of
//! names* and the live map is [`crate::project::OpenMap`]: [`follow_the_open_map`] keeps the active tab
//! on it, and nothing else sets which map tab is active.
//!
//! # Session state is the person's, not the project's
//!
//! It is saved in `<data_dir>/Grimoire/projects/<hash of root>/session.yaml` ([`path_for`]), so
//! committing a project never commits somebody's open tabs. Only the binary names the data directory
//! ([`SessionStore`]), as with the recent list: a test's editor has none until it inserts one, so no
//! test writes the real data directory.
//!
//! # A tab whose thing no longer exists closes, with a notice naming it
//!
//! A map whose file has gone, a file deleted outside the editor: [`close_the_missing`] drops the tab
//! and says which, rather than keeping a tab that opens nothing.
//!
//! # Keys
//!
//! `Cmd`/`Ctrl`+`W` closes the active tab, `Cmd`/`Ctrl`+`Shift`+`T` reopens the last one closed, and
//! `Ctrl`+`Tab` (with `Shift`, backwards) cycles. They are `bevy_gooey::Shortcut`s read here, as the
//! terminal's chords are, and each yields to a text field or a filter box that holds the keyboard.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::picking::events::PointerClick;
use serde::{Deserialize, Serialize};

use crate::editor::EditorState;
use crate::project::{OpenMap, Project};
use crate::workspace::Workspace;

/// The version of `session.yaml` this editor reads and writes. A file of another version is refused by
/// name and the session starts empty, rather than being read as this one's.
pub const VERSION: u32 = 1;

/// The file's name, inside the project's own directory under the data directory.
pub const FILE: &str = "session.yaml";

/// **How many closed tabs "reopen" remembers** per workspace. Old enough to be forgotten is old enough
/// not to be wanted, and the file should not grow for ever.
pub const REOPEN_DEPTH: usize = 20;

/// The strip is drawn when a workspace has at least this many tabs open. One document is the title
/// the chrome already prints (`MAP · m`); a strip of one would be a second place saying the same
/// word, and it is not there until it has something to choose between.
pub const STRIP_FROM: usize = 2;

/// **What kind of thing a tab holds.**
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Map,
    File,
    Mesh,
    Tile,
    Composition,
    Rig,
    Clip,
    Cue,
    Score,
}

impl Kind {
    /// What a person calls it, for a notice and a refusal.
    pub const fn label(self) -> &'static str {
        match self {
            Kind::Map => "map",
            Kind::File => "file",
            Kind::Mesh => "mesh",
            Kind::Tile => "tile",
            Kind::Composition => "composition",
            Kind::Rig => "rig",
            Kind::Clip => "clip",
            Kind::Cue => "cue table",
            Kind::Score => "music score",
        }
    }
}

/// **The kinds of document a workspace holds** — the table above, as data. A workspace that holds
/// none (Project, Game, Agent) has no tabs.
pub const fn kinds_of(workspace: Workspace) -> &'static [Kind] {
    match workspace {
        Workspace::Map | Workspace::World => &[Kind::Map],
        Workspace::Files => &[Kind::File],
        Workspace::Kit => &[Kind::Mesh, Kind::Tile, Kind::Composition],
        Workspace::Animation => &[Kind::Rig, Kind::Clip],
        Workspace::Sound => &[Kind::Cue, Kind::Score],
        Workspace::Project | Workspace::Game | Workspace::Agent | Workspace::Design => &[],
    }
}

/// **One tab's document**: its kind and its name (a map's name, a file's path under the project, a
/// piece's id). The name is what [`Tabs`] compares, so two tabs never hold one thing.
#[derive(Clone, PartialEq, Eq, Debug, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Doc {
    pub kind: Kind,
    pub name: String,
}

impl Doc {
    pub fn new(kind: Kind, name: impl Into<String>) -> Doc {
        Doc { kind, name: name.into() }
    }

    pub fn map(name: impl Into<String>) -> Doc {
        Doc::new(Kind::Map, name)
    }
}

/// **One workspace's tabs**: what is open, which one is active, and what was closed last.
#[derive(Clone, Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tabs {
    #[serde(default)]
    pub open: Vec<Doc>,
    /// The index into [`Self::open`] of the active tab; `None` only when nothing is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<usize>,
    /// Closed tabs, the most recent last: what `Cmd+Shift+T` reopens.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub closed: Vec<Doc>,
}

impl Tabs {
    /// The active tab's document.
    pub fn active_doc(&self) -> Option<&Doc> {
        self.active.and_then(|i| self.open.get(i))
    }

    /// Where a document's tab is, if it is open.
    pub fn position(&self, doc: &Doc) -> Option<usize> {
        self.open.iter().position(|d| d == doc)
    }

    /// **Open a document, or switch to its tab if it is open already.** Returns whether anything
    /// changed. A document that was closed and is opened again leaves the reopen stack.
    pub fn open_doc(&mut self, doc: Doc) -> bool {
        self.closed.retain(|d| *d != doc);
        match self.position(&doc) {
            Some(i) if self.active == Some(i) => false,
            Some(i) => {
                self.active = Some(i);
                true
            }
            None => {
                self.open.push(doc);
                self.active = Some(self.open.len() - 1);
                true
            }
        }
    }

    /// **Close the tab at `index`**, remembering it for [`Self::reopen`]. The tab that takes its place
    /// is the one that slid into its index, or the last one if it was last; closing a tab left of the
    /// active one keeps the same document active.
    pub fn close(&mut self, index: usize) -> Option<Doc> {
        if index >= self.open.len() {
            return None;
        }
        let doc = self.open.remove(index);
        self.closed.retain(|d| *d != doc);
        self.closed.push(doc.clone());
        if self.closed.len() > REOPEN_DEPTH {
            let extra = self.closed.len() - REOPEN_DEPTH;
            self.closed.drain(..extra);
        }
        self.active = match self.active {
            _ if self.open.is_empty() => None,
            Some(a) if a > index => Some(a - 1),
            Some(a) if a == index => Some(index.min(self.open.len() - 1)),
            other => other,
        };
        Some(doc)
    }

    /// **Reopen the last closed tab**, as the active one. `None` when nothing was closed.
    pub fn reopen(&mut self) -> Option<Doc> {
        let doc = self.closed.pop()?;
        self.open_doc(doc.clone());
        Some(doc)
    }

    /// **Move to the next tab, or the previous one**, wrapping. False when there is nowhere to go.
    pub fn step(&mut self, back: bool) -> bool {
        let n = self.open.len();
        if n < 2 {
            return false;
        }
        let at = self.active.unwrap_or(0);
        self.active = Some(if back { (at + n - 1) % n } else { (at + 1) % n });
        true
    }

    /// Put a file's own claims in order: an `active` past the end is the last tab, and a workspace with
    /// tabs and no active one has the first.
    fn sanitized(mut self) -> Tabs {
        let n = self.open.len();
        self.active = match self.active {
            _ if n == 0 => None,
            Some(a) if a < n => Some(a),
            Some(_) => Some(n - 1),
            None => Some(0),
        };
        self
    }
}

/// **Every workspace's tabs.** A `Resource` because it is what the person has open, in this editor,
/// for this project: it is read from the session file when the project's own is first seen and
/// written back when it changes.
#[derive(Resource, Clone, Default, PartialEq, Eq, Debug)]
pub struct Session {
    workspaces: BTreeMap<String, Tabs>,
}

impl Session {
    /// A workspace's tabs, empty when it has none.
    pub fn tabs(&self, workspace: Workspace) -> Tabs {
        self.workspaces.get(workspace.id()).cloned().unwrap_or_default()
    }

    fn tabs_mut(&mut self, workspace: Workspace) -> &mut Tabs {
        self.workspaces.entry(workspace.id().to_owned()).or_default()
    }

    /// **Open a document in a workspace** that holds its kind. A workspace that does not hold it
    /// refuses with the reason, so a tab never lands where nothing could draw it.
    pub fn open(&mut self, workspace: Workspace, doc: Doc) -> Result<bool, String> {
        if !kinds_of(workspace).contains(&doc.kind) {
            return Err(format!(
                "the {} workspace does not hold {} documents",
                workspace.title(),
                doc.kind.label()
            ));
        }
        Ok(self.tabs_mut(workspace).open_doc(doc))
    }

    /// Close the tab for `doc` in `workspace`. Whether there was one.
    pub fn close(&mut self, workspace: Workspace, doc: &Doc) -> bool {
        let tabs = self.tabs_mut(workspace);
        match tabs.position(doc) {
            Some(i) => tabs.close(i).is_some(),
            None => false,
        }
    }

    /// Reopen the last tab closed in `workspace`.
    pub fn reopen(&mut self, workspace: Workspace) -> Option<Doc> {
        self.tabs_mut(workspace).reopen()
    }

    /// Step the active tab in `workspace`.
    pub fn step(&mut self, workspace: Workspace, back: bool) -> bool {
        self.tabs_mut(workspace).step(back)
    }

    /// **The session a file holds**, or why it does not read. A version this editor does not know is a
    /// refusal by name.
    pub fn read(path: &Path) -> Result<Session, String> {
        let saved: Saved = bevy_grimoire_core::yaml::read(path).map_err(|p| p.to_string())?;
        if saved.version != VERSION {
            return Err(format!(
                "{} is session version {}; this editor reads version {VERSION}",
                path.display(),
                saved.version
            ));
        }
        let workspaces = saved.workspaces.into_iter().map(|(id, tabs)| (id, tabs.sanitized())).collect();
        Ok(Session { workspaces })
    }

    /// **Write the session**, making the directory if it is the first time.
    pub fn write(&self, path: &Path) -> Result<(), String> {
        let saved = Saved {
            version: VERSION,
            workspaces: self.workspaces.iter().filter(|(_, t)| !t.open.is_empty() || !t.closed.is_empty()).map(|(k, v)| (k.clone(), v.clone())).collect(),
        };
        let text = bevy_grimoire_core::yaml::to_string(&saved).map_err(|p| p.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        bevy_grimoire_core::atomic_file::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    version: u32,
    #[serde(default)]
    workspaces: BTreeMap<String, Tabs>,
}

/// **The data directory the session files live under**: `<data_dir>/Grimoire`. Inserted by `main`;
/// absent in a test unless the test inserts one of its own.
#[derive(Resource, Clone, Debug)]
pub struct SessionStore(pub PathBuf);

/// **Where one project's session lives**: `<store>/projects/<hash of root>/session.yaml`.
///
/// The hash is FNV-1a over the root's text, so the same project is the same directory on every run and
/// two projects never share one, and the file's name says nothing about where the project is.
pub fn path_for(store: &Path, root: &Path) -> PathBuf {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in root.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    store.join("projects").join(format!("{hash:016x}")).join(FILE)
}

/// **The file the running session was read from and is written to**, once it is known. Absent until
/// [`restore`] has seen a project and a store.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct SessionFile {
    pub path: PathBuf,
    /// What the file holds, so a frame where nothing changed writes nothing.
    written: Session,
    /// **This project has no file yet**: the editor following its own open map into the tabs is not a
    /// change the person made, so the first session is written on their first real change, not on the
    /// frame the project opens.
    fresh: bool,
    /// **The file exists and would not read**: it is left byte for byte as it is, and nothing is saved
    /// over it for as long as this project is open.
    unreadable: bool,
}

/// Names a notice's first words, so a test can tell it from any other status line.
pub const RESTORED: &str = "restored your open tabs";

/// **Read the project's session the first time its root is seen**, and say so when the file would not
/// read. Replaces the session whole: what is on disk is what the person left.
pub fn restore(
    mut commands: Commands,
    mut session: ResMut<Session>,
    store: Option<Res<SessionStore>>,
    project: Option<Res<Project>>,
    file: Option<Res<SessionFile>>,
    mut state: Option<ResMut<EditorState>>,
) {
    let (Some(store), Some(project)) = (store, project) else {
        return;
    };
    let path = path_for(&store.0, &project.root);
    if file.is_some_and(|f| f.path == path) {
        return;
    }
    // **Whatever the last project had open is not this one's** (`Session` is `Ownership::Project`): a
    // project with no file, or one whose file will not read, starts empty rather than inheriting tabs.
    let mut fresh = false;
    let mut unreadable = false;
    let written = if path.is_file() {
        match Session::read(&path) {
            Ok(read) => {
                *session = read.clone();
                read
            }
            Err(e) => {
                // **The file is not read, so it is never written over**: it may be a newer editor's, and
                // its tabs must not be lost to this one's empty session. `written` is the session as it
                // now stands *after* the reset, and the flag holds [`save`] off for good.
                *session = Session::default();
                unreadable = true;
                if let Some(state) = state.as_deref_mut() {
                    state.status.problem(format!("NOT RESTORED: {e}; this project's tabs are not saved while its session file will not read"));
                }
                session.clone()
            }
        }
    } else {
        // **Nothing on disk, nothing carried over**: the new project starts with no tabs, and its file
        // is written on the person's first change rather than by the editor opening its own map.
        *session = Session::default();
        fresh = true;
        Session::default()
    };
    commands.insert_resource(SessionFile { path, written, fresh, unreadable });
}

/// **The active map tab is the open map.** Every map the editor opens — from the project page, an
/// agent's `map_open`, a tab — gets a tab in each workspace that holds maps, and it is the active one.
/// Nothing else chooses which map tab is active, so a restored session whose active tab names a map the
/// editor did not open follows the editor, not the file.
pub fn follow_the_open_map(open: Option<Res<OpenMap>>, mut session: ResMut<Session>, file: Option<ResMut<SessionFile>>) {
    // **Not before the file has been read**: the tabs would be written over by the first map.
    let (Some(open), Some(mut file)) = (open, file) else {
        return;
    };
    let doc = Doc::map(open.map.name.clone());
    for workspace in Workspace::ALL.into_iter().filter(|w| kinds_of(*w).contains(&Kind::Map)) {
        let wants = {
            let tabs = session.tabs(workspace);
            tabs.active_doc() != Some(&doc)
        };
        if wants {
            // A workspace that holds maps always accepts one.
            let _ = session.open(workspace, doc.clone());
        }
    }
    if file.fresh {
        // **A project that has never been saved is not given a file by its own opening map.** `restore`
        // reset the session and this ran in the same frame, so every tab here is the editor's own
        // doing, not the person's: the file is written on their first change instead.
        file.written = session.clone();
        file.fresh = false;
    }
}

/// **A tab whose document is gone closes, and the notice names it.** A map whose file is not on disk
/// any more (the open one is always judged by being open): the tab would open nothing.
pub fn close_the_missing(
    mut session: ResMut<Session>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    mut state: Option<ResMut<EditorState>>,
    mut tick: Local<u32>,
) {
    // Every 30th frame is plenty for a file deleted by hand, and costs nothing when settled.
    *tick = tick.wrapping_add(1);
    if !(*tick).is_multiple_of(30) {
        return;
    }
    let Some(project) = project else {
        return;
    };
    let live = open.as_ref().map(|o| o.map.name.clone());
    let gone: Vec<(Workspace, Doc)> = Workspace::ALL
        .into_iter()
        .flat_map(|w| session.tabs(w).open.into_iter().map(move |d| (w, d)))
        .filter(|(_, doc)| match doc.kind {
            Kind::Map => live.as_deref() != Some(doc.name.as_str()) && !project.maps_dir.join(bevy_grimoire_core::naming::map_file_name(&doc.name)).is_file(),
            Kind::File => !project.root.join(&doc.name).exists(),
            // A piece, a rig or a cue has no file of its own to look for yet; its owner closes it.
            _ => false,
        })
        .collect();
    for (workspace, doc) in gone {
        session.close(workspace, &doc);
        if let Some(state) = state.as_deref_mut() {
            state.status.note(format!("closed the tab for {}: it is gone from the project", doc.name));
        }
    }
}

/// **Write the session when it changed.** Not before the file is known, and not on a frame where it
/// holds what the file already does.
pub fn save(session: Res<Session>, file: Option<ResMut<SessionFile>>, mut state: Option<ResMut<EditorState>>) {
    let Some(mut file) = file else {
        return;
    };
    if file.unreadable || file.written == *session {
        return;
    }
    match session.write(&file.path) {
        Ok(()) => file.written = session.clone(),
        Err(e) => {
            // Said once: `written` stays behind, so the next frame would say it again — hold it.
            file.written = session.clone();
            if let Some(state) = state.as_deref_mut() {
                state.status.problem(format!("NOT SAVED: your open tabs: {e}"));
            }
        }
    }
}

/// **Open a map the way the project page does**, through the `map_open` operation: the same read, the
/// same refusal for a file that will not open. The screen is redrawn the next frame (`watch::Due`), and
/// the project remembers which map it is on.
pub fn open_map_now(world: &mut World, name: &str) -> Result<(), String> {
    let project = world.get_resource::<Project>().cloned().ok_or("there is no project open")?;
    let mut ctx = crate::ops::Ctx {
        view: None,
        trust_file: None,
        project: Some(project),
        open: None,
        source: crate::ops::Source::Human,
        root: None,
        session: crate::ops::session_id(),
        proposal: None,
    };
    match crate::ops::dispatch(&mut ctx, "map_open", &serde_json::json!({ "name": name })) {
        Ok(crate::ops::Applied::Done { .. }) => {}
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => return Err(e.0),
    }
    let fresh = ctx.open.take().ok_or_else(|| format!("`{name}` could not be opened"))?;
    if let Some(mut project) = world.get_resource_mut::<Project>() {
        project.open_map_name = Some(name.to_owned());
    }
    world.insert_resource(fresh);
    if let Some(mut due) = world.get_resource_mut::<crate::watch::Due>() {
        due.map_replaced = true;
    }
    Ok(())
}

/// **Make a map's tab the live map.** A map with unsaved edits asks first, with the question the door
/// switch raises, and the answer opens it (`EditorState::map_request`).
pub fn choose_map(world: &mut World, name: &str) {
    let Some(open) = world.get_resource::<OpenMap>() else {
        return;
    };
    if open.map.name == name {
        return;
    }
    if open.dirty() {
        let Some(mut state) = world.get_resource_mut::<EditorState>() else {
            return;
        };
        if state.leaving {
            return;
        }
        state.map_request = Some(name.to_owned());
        state.leaving = true;
        if let Some(mut confirm) = world.get_resource_mut::<crate::confirm::Confirm>() {
            confirm.ask(
                crate::confirm::Asked::SwitchDoor,
                "Leave this map?",
                "It has unsaved edits. Leaving saves them first.",
                "Save and go",
                "Stay",
            );
        }
        return;
    }
    if let Err(e) = open_map_now(world, name)
        && let Some(mut state) = world.get_resource_mut::<EditorState>()
    {
        state.status.problem(format!("NOT OPENED: {e}"));
    }
}

/// **Switch to a tab**: a map's tab opens that map, any other kind's tab becomes the active one.
pub fn activate(world: &mut World, workspace: Workspace, doc: &Doc) {
    if doc.kind == Kind::Map {
        choose_map(world, &doc.name);
        return;
    }
    if let Some(mut session) = world.get_resource_mut::<Session>() {
        let _ = session.open(workspace, doc.clone());
    }
}

/// **Close a tab.** The open map's tab goes to the map beside it, and a map with unsaved edits is not
/// closed under the person: they save it (or choose another tab, which asks).
pub fn close_tab(world: &mut World, workspace: Workspace, doc: &Doc) {
    let tabs = world.get_resource::<Session>().map(|s| s.tabs(workspace)).unwrap_or_default();
    let Some(index) = tabs.position(doc) else {
        return;
    };
    // **A file with unsaved edits is not closed under the person** (GD-11, #373): the tab is where the text lives.
    if doc.kind == Kind::File && world.get_resource::<crate::files::Files>().is_some_and(|f| f.is_dirty(&doc.name)) {
        if let Some(mut state) = world.get_resource_mut::<EditorState>() {
            state.status.problem(format!("{} has unsaved edits: save it before closing its tab", doc.name));
        }
        return;
    }
    if doc.kind == Kind::Map {
        let live = world.get_resource::<OpenMap>().map(|o| (o.map.name.clone(), o.dirty()));
        if live.as_ref().is_some_and(|(name, _)| *name == doc.name) {
            if tabs.open.len() < 2 {
                if let Some(mut state) = world.get_resource_mut::<EditorState>() {
                    state.status.problem(format!("{} is the only map open; open another first", doc.name));
                }
                return;
            }
            if live.is_some_and(|(_, dirty)| dirty) {
                if let Some(mut state) = world.get_resource_mut::<EditorState>() {
                    state.status.problem(format!("{} has unsaved edits: save it before closing its tab", doc.name));
                }
                return;
            }
            // The neighbour the close would make active.
            let mut after = tabs.clone();
            after.close(index);
            let Some(next) = after.active_doc().cloned() else {
                return;
            };
            if let Err(e) = open_map_now(world, &next.name) {
                if let Some(mut state) = world.get_resource_mut::<EditorState>() {
                    state.status.problem(format!("NOT OPENED: {e}"));
                }
                return;
            }
        }
    }
    if let Some(mut session) = world.get_resource_mut::<Session>() {
        session.close(workspace, doc);
    }
}

/// **Reopen the last closed tab** of the open workspace.
pub fn reopen_tab(world: &mut World, workspace: Workspace) {
    let doc = world.get_resource::<Session>().and_then(|s| s.tabs(workspace).closed.last().cloned());
    let Some(doc) = doc else {
        return;
    };
    // Dropped from the stack by `activate` → `open`; a map is opened for real first.
    if doc.kind == Kind::Map {
        choose_map(world, &doc.name);
    }
    if let Some(mut session) = world.get_resource_mut::<Session>() {
        let _ = session.open(workspace, doc);
    }
}

/// The chords, built where they are read because `Modifiers` has no const `|`.
fn chords() -> [bevy_gooey::Shortcut; 4] {
    use bevy_gooey::{Modifiers, Shortcut};
    [
        Shortcut::new(Modifiers::PRIMARY, KeyCode::KeyW),
        Shortcut::new(Modifiers::PRIMARY | Modifiers::SHIFT, KeyCode::KeyT),
        Shortcut::new(Modifiers::CTRL, KeyCode::Tab),
        Shortcut::new(Modifiers::CTRL | Modifiers::SHIFT, KeyCode::Tab),
    ]
}

/// **The three tab keys**, once a frame. They yield to a text field, a filter box and the journal, the
/// way every census key does.
pub fn tab_keys(world: &mut World) {
    let Some(live) = world.get_resource::<crate::keys::Live>().copied() else {
        return;
    };
    if live.0 == crate::keys::Context::Typing || live.2 != crate::keys::Holder::Tab {
        return;
    }
    let Some(workspace) = world.get_resource::<Workspace>().copied() else {
        return;
    };
    let pressed = {
        let keys = world.resource::<ButtonInput<KeyCode>>();
        chords().map(|c| c.just_pressed(keys))
    };
    let [close, reopen, next, back] = pressed;
    if close {
        let doc = world.get_resource::<Session>().and_then(|s| s.tabs(workspace).active_doc().cloned());
        if let Some(doc) = doc {
            close_tab(world, workspace, &doc);
        }
    } else if reopen {
        reopen_tab(world, workspace);
    } else if next || back {
        let to = world.get_resource::<Session>().and_then(|s| {
            let mut tabs = s.tabs(workspace);
            tabs.step(back).then(|| tabs.active_doc().cloned()).flatten()
        });
        if let Some(doc) = to {
            activate(world, workspace, &doc);
        }
    }
}

// ── The strip ────────────────────────────────────────────────────────────────

/// The row of tabs, a child of the workspace strip's band.
#[derive(Component)]
pub struct DocumentStrip;

/// One tab: the document it holds, so a click reaches it by identity and never by index.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct DocumentTab(pub Doc);

/// A tab's name.
#[derive(Component)]
pub struct DocumentName;

/// The dot a tab with unsaved edits wears.
#[derive(Component)]
pub struct DirtyDot;

/// A tab's close button.
#[derive(Component)]
pub struct DocumentClose(pub Doc);

/// What the strip last drew, so a settled editor rebuilds nothing.
#[derive(Default, PartialEq)]
pub struct Drawn {
    workspace: Option<Workspace>,
    tabs: Vec<(Doc, bool)>,
    active: Option<usize>,
}

/// **Draw the tab strip when the workspace has tabs to choose between** ([`STRIP_FROM`]), and take it
/// away when it has not. Rebuilt only when what it shows changed.
pub fn draw_the_tabs(
    mut commands: Commands,
    workspace: Option<Res<Workspace>>,
    session: Res<Session>,
    open: Option<Res<OpenMap>>,
    frame: Option<Res<crate::chrome::Frame>>,
    existing: Query<Entity, With<DocumentStrip>>,
    mut drawn: Local<Drawn>,
) {
    let (Some(workspace), Some(frame)) = (workspace, frame) else {
        return;
    };
    let tabs = session.tabs(*workspace);
    let dirty = |doc: &Doc| doc.kind == Kind::Map && open.as_ref().is_some_and(|o| o.map.name == doc.name && o.dirty());
    let want = Drawn {
        workspace: Some(*workspace),
        tabs: if tabs.open.len() >= STRIP_FROM { tabs.open.iter().map(|d| (d.clone(), dirty(d))).collect() } else { Vec::new() },
        active: tabs.active,
    };
    // A strip entity that is gone (the frame was rebuilt by a switch) is drawn again.
    let present = !existing.is_empty();
    if *drawn == want && (present || want.tabs.is_empty()) {
        return;
    }
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    if want.tabs.is_empty() {
        *drawn = want;
        return;
    }
    let ink = crate::chrome::ink();
    let strip = commands
        .spawn((
            DocumentStrip,
            bevy_gooey::row_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { align: Some(AlignItems::Stretch), ..default() }),
            bevy_gooey::Spacing { margin: crate::chrome::margin_left(crate::chrome::STEP_GROUP), ..default() },
            bevy::a11y::AccessibilityNode::from(accesskit::Node::new(accesskit::Role::TabList)),
        ))
        .id();
    commands.entity(frame.door_strip).add_child(strip);
    for (i, (doc, is_dirty)) in want.tabs.iter().enumerate() {
        let active = want.active == Some(i);
        let chip = commands
            .spawn((
                DocumentTab(doc.clone()),
                // A document tab: its words a row step apart, the same inset as the workspace strip's tabs (12 px across, 8 px down: not a step of the scale, so a literal)
                // and the active-tab marker's 3 px edge along the bottom (exactly 3 px).
                bevy_gooey::row_with(crate::chrome::GAP_ROW_STEPS, bevy_gooey::RowLayout { align: Some(AlignItems::Center), ..default() }),
                bevy_gooey::Spacing {
                    padding: bevy_gooey::Sides { top: Some(bevy_gooey::Space::Px(8.0)), right: Some(crate::chrome::STEP_PAD), bottom: Some(bevy_gooey::Space::Px(8.0)), left: Some(crate::chrome::STEP_PAD) },
                    ..default()
                },
                bevy_gooey::BorderPx(bevy_gooey::Sides { bottom: Some(3.0), ..default() }),
                bevy_gooey::InkOverride { edge: Some(if active { ink.accent } else { Color::NONE }), ..default() },
                BorderColor::all(if active { ink.accent } else { Color::NONE }),
                BackgroundColor(if active { ink.panel_bg } else { ink.row_bg }),
                bevy::a11y::AccessibilityNode::from(accesskit::Node::new(accesskit::Role::Tab)),
                bevy::prelude::AccessibleLabel::new(doc.name.clone()),
            ))
            .observe(click_tab)
            .id();
        commands.entity(strip).add_child(chip);
        commands.entity(chip).with_children(|c| {
            c.spawn((
                DocumentName,
                Text::new(doc.name.clone()),
                TextColor(ink.label),
                crate::chrome::font(crate::chrome::text::TAB),
                TextLayout::new(Justify::Left, LineBreak::NoWrap),
            ));
            if *is_dirty {
                c.spawn((
                    DirtyDot,
                    Text::new("\u{25CF}"),
                    TextColor(ink.accent),
                    crate::chrome::font(crate::chrome::text::LABEL),
                ));
            }
            c.spawn((
                DocumentClose(doc.clone()),
                Text::new("\u{00D7}"),
                TextColor(ink.muted),
                crate::chrome::font(crate::chrome::text::TAB),
            ))
            .observe(click_close);
        });
    }
    *drawn = want;
}

fn click_tab(click: On<PointerClick>, tabs: Query<&DocumentTab>, mut commands: Commands) {
    let Ok(DocumentTab(doc)) = tabs.get(click.entity) else {
        return;
    };
    let doc = doc.clone();
    commands.queue(move |world: &mut World| {
        let Some(workspace) = world.get_resource::<Workspace>().copied() else {
            return;
        };
        activate(world, workspace, &doc);
    });
}

fn click_close(mut click: On<PointerClick>, closes: Query<&DocumentClose>, mut commands: Commands) {
    // The close button is inside the tab: the tab must not also be chosen by the same click.
    click.propagate(false);
    let Ok(DocumentClose(doc)) = closes.get(click.entity) else {
        return;
    };
    let doc = doc.clone();
    commands.queue(move |world: &mut World| {
        let Some(workspace) = world.get_resource::<Workspace>().copied() else {
            return;
        };
        close_tab(world, workspace, &doc);
    });
}

/// **The session's plugin.** `Session` is a project-owned resource, so it is not reset by a switch.
pub struct SessionPlugin;

impl Plugin for SessionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Session>().add_systems(
            Update,
            (
                (restore, follow_the_open_map, close_the_missing, save, draw_the_tabs).chain(),
                tab_keys,
            )
                .run_if(in_state(crate::screen::Screen::Editor)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(n: &str) -> Doc {
        Doc::map(n)
    }

    /// **Opening a tab twice is one tab**, and opening makes it the active one.
    #[test]
    fn a_document_has_one_tab_and_opening_it_activates_it() {
        let mut tabs = Tabs::default();
        assert!(tabs.open_doc(doc("a")));
        assert!(tabs.open_doc(doc("b")));
        assert_eq!(tabs.open.len(), 2);
        assert_eq!(tabs.active, Some(1));
        assert!(tabs.open_doc(doc("a")), "opening an open tab switches to it");
        assert_eq!(tabs.open.len(), 2);
        assert_eq!(tabs.active, Some(0));
        assert!(!tabs.open_doc(doc("a")), "and opening the active one changes nothing");
    }

    /// **The active tab stays on its document through a close left of it**, and a closed active tab
    /// gives way to the one that slid into its place.
    #[test]
    fn closing_keeps_the_active_document() {
        let mut tabs = Tabs::default();
        for n in ["a", "b", "c"] {
            tabs.open_doc(doc(n));
        }
        assert_eq!(tabs.active_doc(), Some(&doc("c")));
        tabs.close(0);
        assert_eq!(tabs.active_doc(), Some(&doc("c")), "a close left of the active tab moves nothing");
        tabs.open_doc(doc("b"));
        tabs.close(0);
        assert_eq!(tabs.active_doc(), Some(&doc("c")), "closing the active tab lands on the tab in its place, or the last");
    }

    /// **Reopen restores the last tab closed**, most recent first, and stops when there is none.
    #[test]
    fn reopen_restores_the_last_closed_tab() {
        let mut tabs = Tabs::default();
        for n in ["a", "b", "c"] {
            tabs.open_doc(doc(n));
        }
        tabs.close(0);
        tabs.close(0);
        assert_eq!(tabs.reopen(), Some(doc("b")), "b was the last one closed");
        assert_eq!(tabs.active_doc(), Some(&doc("b")));
        assert_eq!(tabs.reopen(), Some(doc("a")));
        assert_eq!(tabs.reopen(), None);
    }

    /// **Stepping wraps**, and a lone tab has nowhere to go.
    #[test]
    fn stepping_wraps_both_ways() {
        let mut tabs = Tabs::default();
        tabs.open_doc(doc("a"));
        assert!(!tabs.step(false));
        tabs.open_doc(doc("b"));
        tabs.open_doc(doc("c"));
        assert!(tabs.step(false));
        assert_eq!(tabs.active, Some(0), "forward from the last wraps to the first");
        assert!(tabs.step(true));
        assert_eq!(tabs.active, Some(2), "back from the first wraps to the last");
    }

    /// **A workspace refuses a kind it does not hold**, naming both.
    #[test]
    fn a_workspace_refuses_a_document_it_does_not_hold() {
        let mut session = Session::default();
        let why = session.open(Workspace::Kit, doc("m")).expect_err("a map is not a kit document");
        assert!(why.contains("Kit") && why.contains("map"), "{why}");
        assert!(session.open(Workspace::Map, doc("m")).is_ok());
    }

    /// **The same project is the same directory, and another is not.**
    #[test]
    fn a_projects_session_is_in_its_own_directory() {
        let store = Path::new("/data/Grimoire");
        let a = path_for(store, Path::new("/work/a"));
        assert_eq!(a, path_for(store, Path::new("/work/a")));
        assert_ne!(a, path_for(store, Path::new("/work/b")));
        assert!(a.starts_with("/data/Grimoire/projects") && a.ends_with(FILE), "{}", a.display());
    }
}
