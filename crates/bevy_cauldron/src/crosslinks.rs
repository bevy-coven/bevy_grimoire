//! **Cross-links: a reference opens what it names** (NV-07, #384; series getting around, #377).
//!
//! Everything that names another thing opens it through the one reveal, so there is no second way to
//! arrive anywhere. This module holds what a reference needs and nothing the finder already has: the
//! scan of the project's files for the values that name something, the line a thing is written on, and
//! the one call that opens a file at a line.
//!
//! | reference | destination | where |
//! |---|---|---|
//! | a piece's **Used by** | the maps that place it | the Kit's Meshes pane and the inspector ([`used_by_line`], from `Project::maps_that_place`) |
//! | a placement's **Open file** | its map's file, at that row | the Map inspector ([`OpenFileButton`]), through [`open_in_files`] |
//! | a piece's **Open piece file** | `library.yaml`, at that entry | the Map inspector ([`OpenPieceFileButton`]) |
//! | a cue's **Used by** in Sound | the files that name it | [`crate::world_lists::cue_use_rows`], from [`crate::goto::GotoIndex::used_by`] |
//! | Cmd/Ctrl+click or F12 on a value that names something | what it names | [`crate::files`]'s editor, through [`follow_reference`] |
//!
//! # One index, and no second scan
//!
//! **A `Used by` list is read from the finder's own index** ([`crate::goto::GotoIndex`]): the scan in [`scan`] is what builds the
//! index's files and references, so a list on a panel and a row in the finder cannot disagree about what names what. A piece's list
//! is the exception only in where it reads: it asks `Project::maps_that_place`, the one reader of the project's `maps/` directory
//! that a removal's refusal asks too.
//!
//! # A reference to a thing that does not exist is never a dead link
//!
//! [`follow_reference`] answers `Err` with a sentence naming the value and the thing it should have been, and the caller says it on the
//! status line; the file editor also marks such a cue with a warning (`files::decorate_the_editor`), so the person is told where it is
//! before they try the click.
//!
//! # What names a cue
//!
//! No file kind of this repository has a field that names a cue yet, so a cue reference is a value whose key is
//! [`crate::files_lang::CUE_FIELD`] (or a field a schema marks `x-dark-factory-ref: cue`). That is an assumed decision, recorded on
//! #384: a game's own data says `cue: door.creak`, and the editor follows it.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;

use bevy::prelude::*;
use bevy_gooey::Activate;

use crate::chrome::{self, Severity};
use crate::editor::{DoorRequest, EditorState, TargetLock};
use crate::files_lang::{self, Lang};
use crate::inspector_view::PoseBlock;
use crate::project::{OpenMap, Project};
use crate::screen::Screen;
use crate::workspace::Workspace;

/// How often the project's files are looked at again for a reference made or removed, in seconds: the file tree's own rhythm.
pub const SCAN_POLL_S: f32 = 1.5;

/// **The largest YAML file the scan reads**, in bytes: the Files workspace's own limit, so the scan never reads what the editor will not open.
pub const SCAN_MAX_BYTES: u64 = crate::files::MAX_FILE_BYTES;

/// **The map file's list of placements**, whose rows a placement's line is found among: a stamp or a note can carry the same word.
pub const PLACEMENTS_KEY: &str = "placements";

/// **The library file's list of descriptors** (`library.yaml`'s `descriptors:`), for the same reason: a stamp or a note can
/// carry a piece's word too.
pub const PIECES_KEY: &str = "descriptors";

/// How often the inspector's `Used by` line reads the project's maps again, in seconds. A selection change is read at once.
pub const USED_BY_POLL_S: f32 = 1.0;

/// **One value in a project file that names another thing**: its kind (`cue`, `library`), the name it holds, the file (relative to the
/// project) and the line, counted from 1 as an editor shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    pub kind: String,
    pub name: String,
    pub file: String,
    /// **The line, counted from 0** as every line in this crate is (`component_line`, `FieldFileButton::line`,
    /// `nav::FILE_STOP`). A person reads 1-based numbers, so the one place that shows one — a `Used by` row — adds 1.
    pub line: usize,
}

/// **What a read of the project's files found**: the text files the tree lists, the references in the YAML ones, and the stamp they were
/// read at.
#[derive(Clone, Debug, Default)]
pub struct Scan {
    pub files: Vec<String>,
    pub refs: Vec<Named>,
    pub stamp: u64,
}

/// **Whether a file is one the editor opens and the finder lists**: a language the editor colours, or a plain text kind a project holds.
pub fn is_text_file(rel: &str) -> bool {
    Lang::of(rel) != Lang::Plain || matches!(rel.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).as_deref(), Some("toml" | "json" | "txt"))
}

/// **Read the project's text files and the references in them**, or `None` when nothing changed since `last` (a file added, removed,
/// resized or touched changes the stamp). The listing is the Files tree's own, so what the finder names is what the tree shows.
pub fn scan(root: &Path, last: Option<u64>) -> Option<Scan> {
    let mut files: Vec<String> = crate::files::tree_files(root).into_iter().filter(|rel| is_text_file(rel)).collect();
    files.sort();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut stats: Vec<Option<(u64, std::time::SystemTime)>> = Vec::with_capacity(files.len());
    for rel in &files {
        rel.hash(&mut hasher);
        let stat = std::fs::metadata(root.join(rel)).ok().map(|m| (m.len(), m.modified().unwrap_or(std::time::UNIX_EPOCH)));
        stat.hash(&mut hasher);
        stats.push(stat);
    }
    let stamp = hasher.finish();
    if last == Some(stamp) {
        return None;
    }
    let mut schemas: HashMap<&'static str, Option<serde_json::Value>> = HashMap::new();
    let mut refs = Vec::new();
    for (rel, stat) in files.iter().zip(&stats) {
        if Lang::of(rel) != Lang::Yaml || stat.is_none_or(|(len, _)| len > SCAN_MAX_BYTES) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(rel)) else { continue };
        let schema = bevy_grimoire_core::reload_check::kind_of_path(rel)
            .and_then(|kind| schemas.entry(kind.id).or_insert_with(|| bevy_grimoire_core::schema::for_kind(kind.id).ok()).as_ref());
        for found in files_lang::references(Lang::Yaml, &text, schema) {
            refs.push(Named { kind: found.kind, name: found.name, file: rel.clone(), line: found.line });
        }
    }
    Some(Scan { files, refs, stamp })
}

/// **The maps that place `id`**, by name, from `Project::maps_that_place` — the one reader of the
/// project's `maps/` directory, so a second scan cannot disagree with it.
pub fn used_by(project: &Project, id: &str) -> Result<Vec<String>, String> {
    project.maps_that_place(id)
}

/// **The list a panel's `Used by` shows**, and the sentence when there is nothing.
pub fn used_by_line(project: &Project, id: &str) -> Result<String, String> {
    let maps = used_by(project, id)?;
    Ok(match maps.len() {
        0 => format!("nothing places `{id}`"),
        1 => format!("placed in {}", maps[0]),
        n => format!("placed in {n} maps: {}", maps.join(", ")),
    })
}

/// **Whether `id` stands in `text` as a name of its own**: not part of a longer word, so `bed` is not found inside `bedroom`. A name is
/// letters, digits and `_ @ . -`.
fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '@' | '.' | '-')
}

/// **The first line (0-based) of `text` that names `id`** as a value or a key of its own, **inside `section`** when one is
/// given: the map's `placements:` list, the library's `pieces:` list. A stamp, a composition member or a note can carry the same
/// word, and the first one in the file is not the row the thing is written on.
pub fn line_naming_in(text: &str, id: &str, section: Option<&str>) -> Option<usize> {
    if id.is_empty() {
        return None;
    }
    let names_it = |line: &str| {
        line.match_indices(id).any(|(at, _)| {
            let before = line[..at].chars().next_back();
            let after = line[at + id.len()..].chars().next();
            !before.is_some_and(is_name_char) && !after.is_some_and(is_name_char)
        })
    };
    let Some(section) = section else {
        return text.lines().position(names_it);
    };
    // **From the section's own key to the next key at that indent or shallower**: the list's rows, and nothing else's.
    let mut lines = text.lines().enumerate();
    let mut indent_of_section = None;
    for (n, line) in lines.by_ref() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(section) && trimmed[section.len()..].trim_start().starts_with(':') {
            indent_of_section = Some(line.len() - trimmed.len());
            if names_it(line) {
                return Some(n);
            }
            break;
        }
    }
    let indent_of_section = indent_of_section?;
    for (n, line) in lines {
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        // A key back at the section's own indent is a sibling: the section has ended.
        if indent <= indent_of_section && !line.trim_start().starts_with('-') {
            break;
        }
        if names_it(line) {
            return Some(n);
        }
    }
    None
}

/// **The first line (0-based) of `text` that names `id`** anywhere in it, as a value or a key of its own.
pub fn line_naming(text: &str, id: &str) -> Option<usize> {
    line_naming_in(text, id, None)
}

/// **The file and the line a placement is written on**, in the open map's own file: `(relative path, 0-based line)`.
pub fn placement_line(project: &Project, open: &OpenMap, id: &str) -> Result<(String, usize), String> {
    let rel = relative_to(project, &open.map_path)?;
    let text = std::fs::read_to_string(&open.map_path).map_err(|e| format!("{rel} cannot be read: {e}"))?;
    let line = line_naming_in(&text, id, Some(PLACEMENTS_KEY)).ok_or_else(|| format!("`{id}` is not written in {rel}"))?;
    Ok((rel, line))
}

/// **The file and the line a library piece is written on**: `(relative path, 0-based line)`.
pub fn piece_line(project: &Project, id: &str) -> Result<(String, usize), String> {
    let path = project.grimoire_dir.join(crate::project::LIBRARY_FILE);
    let rel = relative_to(project, &path)?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{rel} cannot be read: {e}"))?;
    let line = line_naming_in(&text, id, Some(PIECES_KEY)).ok_or_else(|| format!("`{id}` is not written in {rel}"))?;
    Ok((rel, line))
}

/// **The file and the line a game's entity file sets one component of one entity on** (NV-09, #386): `(path relative to the project, 0-based
/// line)`, from the project's `game/*.entities.yaml`, by the loader's own path map ([`bevy_grimoire_core::yaml::lines`]) so the line is the one
/// the loader reads. `component` is the short type name the state view shows (`Health`); the entity is the name the game gave it, and a child
/// is found by its own name. `None` when no entity file sets it: the game made it some other way, and the state view says nothing about a
/// file that is not there.
pub fn component_line(root: &Path, entity: &str, component: &str) -> Option<(String, usize)> {
    let dir = root.join(bevy_grimoire::entities::ENTITIES_DIR);
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(bevy_grimoire::entities::ENTITIES_EXT))
        .collect();
    names.sort();
    let own = format!("{}.{entity}.{component}", bevy_grimoire::entities::ENTITIES_KEY);
    let child = format!(".{}.{entity}.{component}", bevy_grimoire::entities::CHILDREN_KEY);
    for name in names {
        let Ok(text) = std::fs::read_to_string(dir.join(&name)) else { continue };
        // **The component's own key**, from the same walk the loader reads the file with: a mapping node's span starts at its
        // first child, so the node's line is the first line of what the component holds rather than the line it is written on.
        // The key span is that line.
        let found = bevy_grimoire_core::yaml::spans(&text)
            .0
            .into_iter()
            .filter(|span| span.kind == bevy_grimoire_core::yaml::SpanKind::Key && (span.path == own || span.path.ends_with(&child)))
            .map(|span| text[..span.range.start.min(text.len())].bytes().filter(|b| *b == b'\n').count() + 1)
            .min();
        if let Some(line) = found {
            return Some((format!("{}/{name}", bevy_grimoire::entities::ENTITIES_DIR), line.saturating_sub(1)));
        }
    }
    None
}

fn relative_to(project: &Project, path: &Path) -> Result<String, String> {
    path.strip_prefix(&project.root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .map_err(|_| format!("{} is not inside the project", path.display()))
}

/// **Open `rel` in the Files workspace**, at `line` (0-based) when there is one: the file gets its tab, its text is read, and the
/// workspace is asked for. A file that cannot be opened is a problem on the status line and the workspace is not left for it.
///
/// **The one way a file is opened from anywhere but the Files tree**: Open file, a finder result and a followed reference all end here.
pub fn open_in_files(world: &mut World, rel: &str, line: Option<usize>) {
    // **A page of the game's style is read in the Design workspace** (SG-05, #404): a link to one, a finder row and a followed reference
    // are not opened as text. The Files tree's own click still opens it here, because the person chose the file editor.
    if crate::design_view::is_page(rel) {
        crate::design_view::show_file(world, rel, line);
        return;
    }
    if let Err(why) = crate::files::open_file(world, rel) {
        if let Some(mut state) = world.get_resource_mut::<EditorState>() {
            state.status.problem(format!("NOT OPENED: {why}"));
        }
        return;
    }
    if let Some(mut files) = world.get_resource_mut::<crate::files::Files>()
        && let Some(file) = files.get_mut(rel)
    {
        file.reveal = line.filter(|line| *line > 0).map(|line| (line, 0, 0));
    }
    let here = world.get_resource::<Workspace>().copied();
    if here == Some(Workspace::Files) {
        return;
    }
    let Some(mut state) = world.get_resource_mut::<EditorState>() else { return };
    if state.door_request.is_none() && !state.leaving {
        state.door_request = Some(DoorRequest { workspace: Workspace::Files, tab: None });
    } else {
        state.status.problem(crate::editor::LEAVING_QUESTION_REFUSAL.to_owned());
    }
}

/// **Follow a reference to what it names**, through the finder's index and its one way of showing a thing. `Err` says what the value
/// names that the project does not hold — the diagnostic that stands where a dead link would.
pub fn follow_reference(world: &mut World, kind: &str, name: &str) -> Result<(), String> {
    let entry = world.get_resource::<crate::goto::GotoIndex>().and_then(|index| index.named(kind, name)).cloned();
    match entry {
        Some(entry) => {
            crate::goto::show_in_world(world, &entry);
            Ok(())
        }
        None => Err(format!("`{name}` is a {} the project does not hold", crate::goto::Kind::reference_word(kind))),
    }
}

// ── The inspector's block ────────────────────────────────────────────────────

/// **The references block of the Map inspector**: what places the selected piece, and the two buttons that open its files.
#[derive(Component)]
pub struct LinksBlock;

/// The line that says which maps place the selected placement's piece.
#[derive(Component)]
pub struct UsedByLine;

/// **Open file**: the selected placement's row in its map's file.
#[derive(Component)]
pub struct OpenFileButton;

/// **Open piece file**: the selected placement's piece in the library's file.
#[derive(Component)]
pub struct OpenPieceFileButton;

/// What the inspector's line last said, and for what.
#[derive(Default)]
struct Shown {
    key: Option<(String, String)>,
    since: f32,
}

/// **What the references block reads**: the project, the open map and the editor's one lock, as one fact ("what is selected").
#[derive(bevy::ecs::system::SystemParam)]
pub struct Selected<'w, 's> {
    project: Option<Res<'w, Project>>,
    open: Option<Res<'w, OpenMap>>,
    lock: Option<Res<'w, TargetLock>>,
    poses: Query<'w, 's, (Entity, Option<&'static Children>), With<PoseBlock>>,
    blocks: Query<'w, 's, Entity, With<LinksBlock>>,
    lines: Query<'w, 's, &'static mut Text, With<UsedByLine>>,
}

/// **Put the references block under the pose block while a piece is selected**, and keep its `Used by` line true. Spawned with the pose
/// block (and gone with it), never kept hidden.
fn draw_the_links(mut commands: Commands, time: Res<Time<Real>>, mut selected: Selected, mut shown: Local<Shown>) {
    let Some(block) = selected.poses.iter().map(|(entity, _)| entity).next() else {
        shown.key = None;
        return;
    };
    if selected.blocks.is_empty() {
        let dim = chrome::ink().dim;
        commands.entity(block).with_children(|b| {
            b.spawn((LinksBlock, Name::new("references block"), bevy_gooey::column(chrome::GAP_TIGHT_STEPS)))
                .with_children(|l| {
                    chrome::list_heading(l, "USED BY");
                    l.spawn((UsedByLine, Text::new(""), TextColor(dim), chrome::typed(chrome::text::BODY)));
                    chrome::wrapping_row(l, chrome::GAP_TIGHT_STEPS, crate::chrome::NO_MARGIN, AlignItems::Stretch).with_children(|r| {
                        chrome::button(r, OpenFileButton, "Open file", Severity::Plain);
                        chrome::button(r, OpenPieceFileButton, "Open piece file", Severity::Plain);
                    });
                });
        });
        shown.key = None;
        return;
    }
    let (Some(project), Some(open), Some(lock)) = (selected.project.as_deref(), selected.open.as_deref(), selected.lock.as_deref()) else { return };
    let Some(id) = lock.placement_id() else { return };
    let Some(piece) = open.map.placements.iter().find(|p| p.id == id).map(|p| p.descriptor.clone()) else { return };
    shown.since += time.delta_secs();
    let key = (id.to_owned(), piece.clone());
    if shown.key.as_ref() == Some(&key) && shown.since < USED_BY_POLL_S {
        return;
    }
    shown.since = 0.0;
    shown.key = Some(key);
    let words = used_by_line(project, &piece).unwrap_or_else(|why| why);
    for mut line in &mut selected.lines {
        if line.0 != words {
            line.0 = words.clone();
        }
    }
}

/// **A click on Open file or Open piece file**: the file opens at the row, or the status line says why it cannot.
#[allow(clippy::too_many_arguments)]
fn on_open_file(
    activate: On<Activate>,
    files: Query<(), With<OpenFileButton>>,
    pieces: Query<(), With<OpenPieceFileButton>>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    lock: Option<Res<TargetLock>>,
    mut state: Option<ResMut<EditorState>>,
    mut commands: Commands,
) {
    let (is_file, is_piece) = (files.contains(activate.entity), pieces.contains(activate.entity));
    if !is_file && !is_piece {
        return;
    }
    let (Some(project), Some(open), Some(lock)) = (project.as_deref(), open.as_deref(), lock.as_deref()) else { return };
    let target = lock.placement_id().ok_or_else(|| "no piece is selected, so there is no file to open".to_owned()).and_then(|id| {
        if is_file {
            placement_line(project, open, id)
        } else {
            let piece = open.map.placements.iter().find(|p| p.id == id).map(|p| p.descriptor.clone()).ok_or_else(|| format!("`{id}` is no longer on this map"))?;
            piece_line(project, &piece)
        }
    });
    match target {
        Ok((rel, line)) => commands.queue(move |world: &mut World| open_in_files(world, &rel, Some(line))),
        Err(why) => {
            if let Some(state) = state.as_mut() {
                state.status.problem(format!("NOT OPENED: {why}"));
            }
        }
    }
}

/// The plugin: the inspector's block and its two buttons.
pub struct CrossLinksPlugin;

impl Plugin for CrossLinksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, draw_the_links.run_if(in_state(Screen::Editor)).run_if(Workspace::map_is_open)).add_observer(on_open_file);
    }
}
