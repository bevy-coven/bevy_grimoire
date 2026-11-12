//! **Go to anything: one finder over everything a developer can name** (NV-05, #382; series getting
//! around, #377).
//!
//! Cmd/Ctrl+P opens the editor's **one** finder. It is not a second palette: it is `bevy_gooey`'s
//! `CommandPalette` — the widget UI-32 (#226) already put on Cmd+Shift+P — with a **second source**.
//! Typing `>` in front of the query swaps the source to the command registry and back again, so a
//! person learns one chord, one field and one Enter.
//!
//! # One index, and a result is a `Shown`
//!
//! [`GotoIndex`] is the list of everything the project names, and every row is a [`Shown`] — the one
//! answer per thing to *where it is seen* (AF-01, #357). Choosing a row therefore calls the reveal
//! ([`crate::reveal::Revealing`]) for a page and the follow ([`crate::follow::Following`]) for a place
//! on the map, exactly as an agent's change does. **This module adds no navigation of its own**: a
//! second path would disagree with the first about which page a piece is on the moment a panel moved.
//!
//! # `Shown` and a problem's `Location`, compared
//!
//! The issue asks whether they name the same things and, if they do, whether they become one type.
//! They name the same *subjects* from opposite ends — a `Shown` says where a change is, a
//! `Location` says where a fault is — and both carry a file and a line, or an entity. They are **not
//! one type here**, for two reasons that are each a field the other lacks: `Location` carries no page
//! (a workspace, a panel, a row) and `Shown` carries no fault, so a fused type would carry half of
//! each on every use, and `Location` lives in `bevy_grimoire_core`, which is engine-free and knows
//! nothing about workspaces. **Nothing maps one to the other yet**: the problem list's Go to does not
//! ask the reveal for a page, so there is no second path to unify, and a mapping with no caller would
//! be a function that answers a question nobody asks (a `Location` carries an entity id and not the
//! footprint a `Shown::Map` frames). The day the problem list reveals, that mapping is written where
//! it is first called. This part of the issue is `Part of #382`'s remainder.
//!
//! # Rebuilt from the reload events, so a piece an agent adds is findable at once
//!
//! The index is not read once at open. [`rebuild_the_index`] watches the watcher's own adopter
//! ([`crate::watch::Due::adopted`], the reload contract's record of *this file was taken again*) and
//! rebuilds when that count, the project, the open map, the library's size or the notes' generation
//! changes. A piece an agent writes into `library.yaml` is in the finder on the frame the editor adopts
//! the file.
//!
//! # The rest never reorder by use
//!
//! Recent results sit under their own `Recent` heading and everything else keeps its kind's heading
//! and its name (P5, `docs/ui/vision.md` §3.5: a list that reorders itself cannot be remembered). That
//! is `bevy_gooey`'s own `PaletteHistory`, whose ranking puts a history entry first and leaves the
//! rest on its label — the two rules are one mechanism rather than a second ranker beside it.
//!
//! # What is not in the index, and why
//!
//! **A thing the editor cannot show is not a result.** A result that reveals nothing is the dead link
//! NV-07 (#384) exists to forbid, so the rows are the pages there are: the open map and its
//! placements, library pieces and their lattices, compositions, rigs and each rig's clips (NV-11, #391: a clip
//! is a slot of the rig's manifest entry, and its page is the rig on the bench with that slot soloed), registered data files, notes,
//! settings sections and the built workspaces. A cue has no page until the Sound workspace lands
//! (NV-08, #385), a project file none until the Files workspace does (GD-11, #373), and a map other
//! than the open one would have to be *opened* rather than shown. The three are `Part of #382`'s
//! remainder, named in the commit that adds this file.

use bevy::prelude::*;
use bevy_gooey::{CommandList, PaletteCommand, PaletteHistory, RunCommand};

use crate::keys::Live;
use crate::ops::Shown;
use crate::project::{OpenMap, Project};
use crate::workspace::Workspace;

/// **How a goto row's id starts**: `goto:<kind>:<key>`. The palette runs it through the same
/// `RunCommand` a command uses, and this prefix is how [`show_the_chosen_row`] tells which of the two
/// sources the choice came from.
pub const ID_PREFIX: &str = "goto:";

/// **The character that swaps the finder to commands**, and back when it is removed.
pub const COMMANDS: char = '>';

/// **The heading a recent result is listed under.** A group of its own, so a person who has used the
/// finder sees what they used last without it moving their other rows.
pub const RECENT: &str = "Recent";

/// **The heading a command is listed under**: `> Edit`, so the query that finds a command is
/// `> edit` — the same characters a person typed, matched against the same row. The `>` here is not
/// decoration: it is what makes the one character in the query an ordinary part of the label rather
/// than a mode the matcher would have to know about.
pub const COMMANDS_HEADING: &str = ">";

/// **One thing a developer can name, and where the editor shows it.**
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// What kind of thing it is, which is the heading it is listed under.
    pub kind: Kind,
    /// **The row's identity inside its kind**: the library id, the placement id, the rig's name. It
    /// is what [`Self::id`] is built from, so it is stable while the thing is.
    pub key: String,
    /// The words the row shows: the name, and the little that says which of two alike it is.
    pub title: String,
    /// **Where it lives**, in the editor's words: `Kit ▸ Meshes`, `the map`, `Settings ▸ Map`. Shown
    /// beside the title so a person choosing between two rows knows which page they are asking for.
    pub lives: String,
    /// **Where the editor shows it** — the one answer, shared with the reveal and the follow.
    pub shown: Shown,
    /// **The workspace this row goes to, when it is not shown by one of its own** — a workspace row
    /// names the workspace itself. `None` for everything a `Shown` answers.
    pub opens: Option<Workspace>,
}

impl Entry {
    /// The palette id: `goto:<kind>:<key>`.
    pub fn id(&self) -> String {
        format!("{ID_PREFIX}{}:{}", self.kind.id(), self.key)
    }
}

/// **The kinds of thing the finder knows**, in the order it lists them: the map before the pieces on
/// it, the things a person authors before the settings that tune them.
///
/// Named rather than a free string because the headings are part of what a person reads, and two
/// spellings of one kind would be two headings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Map,
    Placement,
    Piece,
    Tile,
    Composition,
    Rig,
    /// One of a rig's clips: a slot of its manifest entry, on the bench (NV-11, #391).
    Clip,
    /// A registered data file, whose page is the DATA table.
    Data,
    Note,
    Settings,
    Workspace,
}

impl Kind {
    /// The lower-case word in a row's id.
    pub const fn id(self) -> &'static str {
        match self {
            Kind::Map => "map",
            Kind::Placement => "placement",
            Kind::Piece => "piece",
            Kind::Tile => "tile",
            Kind::Composition => "composition",
            Kind::Rig => "rig",
            Kind::Clip => "clip",
            Kind::Data => "data",
            Kind::Note => "note",
            Kind::Settings => "settings",
            Kind::Workspace => "workspace",
        }
    }

    /// The heading, as a person reads it.
    pub const fn heading(self) -> &'static str {
        match self {
            Kind::Map => "Map",
            Kind::Placement => "Placement",
            Kind::Piece => "Piece",
            Kind::Tile => "Tile",
            Kind::Composition => "Composition",
            Kind::Rig => "Rig",
            Kind::Clip => "Clip",
            Kind::Data => "Data file",
            Kind::Note => "Note",
            Kind::Settings => "Setting",
            Kind::Workspace => "Workspace",
        }
    }

    /// **The order the headings appear in**, which is [`GotoIndex`]'s own order.
    pub const ALL: [Kind; 11] = [
        Kind::Map,
        Kind::Placement,
        Kind::Piece,
        Kind::Tile,
        Kind::Composition,
        Kind::Rig,
        Kind::Clip,
        Kind::Data,
        Kind::Note,
        Kind::Settings,
        Kind::Workspace,
    ];
}

/// **Everything a developer can name, and a record of what it was built from.**
///
/// A `Session` resource (`screen::OWNERSHIP`): the same index serves every workspace, and a workspace
/// change does not change what a piece is called.
#[derive(Resource, Default)]
pub struct GotoIndex {
    entries: Vec<Entry>,
    /// **What the entries were built from**, so a frame that changed nothing costs a comparison
    /// rather than a second read of the project. See [`Built`].
    built: Option<Built>,
}

/// **The facts an index depends on.** Compared, not hashed.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Built {
    project: std::path::PathBuf,
    map: Option<std::path::PathBuf>,
    /// **How many files the watcher has adopted since the editor opened**, which is the reload events
    /// in one number: any file the editor read again may have added, renamed or removed something
    /// namable. A counter rather than the list, because `Due::changed` is emptied by the system that
    /// adopts it and this is one of several readers.
    adopted: u64,
    /// The library's descriptor count, so a piece added *inside* the frame that also adopted a file is
    /// seen even when the count above happens to be unchanged.
    pieces: usize,
    /// **The notes' generation**, because the notes file is written by the panel rather than by an
    /// operation and the watcher may not have adopted it on the frame a note appears.
    notes: u64,
}

impl GotoIndex {
    /// Everything the finder lists, in [`Kind::ALL`] order and then by title.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// **The entry a palette id names**, or `None` when the id is not this finder's or the thing has
    /// gone since the list was built — a piece an agent deleted while the person was choosing it.
    pub fn find(&self, id: &str) -> Option<&Entry> {
        let rest = id.strip_prefix(ID_PREFIX)?;
        let (kind, key) = rest.split_once(':')?;
        self.entries.iter().find(|e| e.kind.id() == kind && e.key == key)
    }

    /// **Every entry of one kind**, for a panel that lists what it holds (NV-07's `Used by`).
    pub fn of_kind(&self, kind: Kind) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(move |e| e.kind == kind)
    }
}

/// **Build the palette rows**, taking the recents under their own heading.
///
/// A recent entry is listed **once**, under [`RECENT`]; the others keep their kind's heading. A row's
/// own words carry where it lives, because that is the question a person choosing between two rows
/// asks, and it is what the issue's *"each shows where it lives"* means.
pub fn rows(index: &GotoIndex, recents: &[String]) -> Vec<PaletteCommand> {
    index
        .entries
        .iter()
        .map(|entry| {
            let heading = if recents.iter().any(|id| *id == entry.id()) {
                RECENT
            } else {
                entry.kind.heading()
            };
            PaletteCommand::new(entry.id(), format!("{} — {}", entry.title, entry.lives))
                .category(heading)
        })
        .collect()
}

/// **The command registry as the same finder lists it**: the `>` a person typed in front of the query
/// in front of every heading, so the one character is what narrows the list to commands.
pub fn command_rows(live: Live) -> Vec<PaletteCommand> {
    crate::command_palette::list(live)
        .into_iter()
        .map(|mut row| {
            let heading = row
                .category
                .as_deref()
                .map_or_else(|| COMMANDS_HEADING.to_owned(), |c| format!("{COMMANDS_HEADING} {c}"));
            row.category = Some(heading);
            row
        })
        .collect()
}

/// **Which source the open finder is showing.**
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    /// Everything the project names.
    #[default]
    Everything,
    /// The command registry, because the query begins with [`COMMANDS`].
    Commands,
}

impl Source {
    /// The source a query names: `>` first is commands, anything else is everything.
    pub fn of(query: &str) -> Source {
        if query.trim_start().starts_with(COMMANDS) { Source::Commands } else { Source::Everything }
    }
}

/// **Whether the everything-finder is open, and the context it was opened from**, so the source swap
/// runs for the everything-finder and for nothing else: the command palette's own chords open the same
/// widget on the command list and must not have it swapped under them.
///
/// Written by `command_palette::toggle` when Cmd/Ctrl+P spawns the palette.
///
/// **The context is the one the person was in when they pressed the chord.** While the palette's field
/// has focus the editor's [`Live`] is `Typing` and every command would be judged "a text field is taking
/// the keys"; a `>` typed later swaps in the command list, and it has to be judged by the context the
/// finder was opened from, exactly as the palette's own list is (`command_palette`'s module note).
#[derive(Resource, Default, Debug)]
pub struct Finder(Option<Live>);

impl Finder {
    /// Whether the open palette is the everything-finder.
    pub fn is_open(&self) -> bool {
        self.0.is_some()
    }

    /// **The context the open finder was opened from**, which the command list is judged by.
    pub fn judged_in(&self) -> Option<Live> {
        self.0
    }

    /// **The finder closed** — the palette is gone, so the next chord may open either source again.
    pub fn closed(&mut self) {
        self.0 = None;
    }

    /// **The finder opened**, from `live`.
    pub fn opened(&mut self, live: Live) {
        self.0 = Some(live);
    }
}

/// **Read the project into the index**, once per change of the things it depends on.
fn rebuild_the_index(
    mut index: ResMut<GotoIndex>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    due: Option<Res<crate::watch::Due>>,
    notes: Option<Res<crate::notes_view::NotesView>>,
    bench: Option<Res<crate::anim_tab::BenchState>>,
) {
    let Some(project) = project.as_deref() else {
        if !index.entries.is_empty() {
            index.entries.clear();
        }
        index.built = None;
        return;
    };
    let built = Built {
        project: project.root.clone(),
        map: open.as_deref().map(|o| o.map_path.clone()),
        adopted: due.as_deref().map_or(0, |d| d.adopted),
        pieces: project.library.descriptors.len(),
        notes: notes.as_deref().map_or(0, crate::notes_view::NotesView::generation),
    };
    if index.built.as_ref() == Some(&built) && !index.entries.is_empty() {
        return;
    }
    index.entries.clear();
    index.entries.extend(collect_the_map(project, open.as_deref()));
    index.entries.extend(collect_the_library(project));
    index.entries.extend(collect_the_rest(project, notes.as_deref(), bench.as_deref()));
    // **One order, the index's own**, so a rebuild cannot reorder the list under a person's finger.
    index.entries.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.title.cmp(&b.title)));
    index.built = Some(built);
}

/// **The open map itself**, and every placement on it by its id.
fn collect_the_map(project: &Project, open: Option<&OpenMap>) -> Vec<Entry> {
    let Some(open) = open else { return Vec::new() };
    let mut out = Vec::with_capacity(open.map.placements.len() + 1);
    out.push(Entry {
        kind: Kind::Map,
        key: open.map.name.clone(),
        title: if open.map.name.is_empty() { "the open map".to_owned() } else { open.map.name.clone() },
        lives: format!(
            "Map ▸ {}",
            open.map_path.file_name().map_or_else(|| "the open map".into(), |n| n.to_string_lossy().into_owned())
        ),
        shown: Shown::MapWhole,
        opens: None,
    });
    for row in &open.map.placements {
        out.push(Entry {
            kind: Kind::Placement,
            key: row.id.clone(),
            title: format!("{} — {}", row.id, row.descriptor),
            lives: "Map".to_owned(),
            // **A placement is a place, not a page**: the camera frames its own footprint, which is
            // the measurement AF-02 uses for a change too, so the two cannot disagree.
            shown: Shown::Map { rect: crate::ops::row_box(&project.library, row), placements: vec![row.id.clone()] },
            opens: None,
        });
    }
    out
}

/// **Every piece and lattice in the library**, plus every composition.
fn collect_the_library(project: &Project) -> Vec<Entry> {
    let mut out = Vec::with_capacity(project.library.descriptors.len() + project.compositions.compositions.len());
    for d in &project.library.descriptors {
        // **A lattice is the same page as its piece** (AF-03 measured it): `Shown::Tile` and
        // `Shown::Piece` both open the Meshes panel, so the heading is what tells them apart.
        let lattice = d.subgrid.is_some();
        out.push(Entry {
            kind: if lattice { Kind::Tile } else { Kind::Piece },
            key: d.id.clone(),
            title: d.id.clone(),
            lives: "Kit ▸ Meshes".to_owned(),
            shown: if lattice { Shown::Tile { id: d.id.clone() } } else { Shown::Piece { id: d.id.clone() } },
            opens: None,
        });
    }
    for c in &project.compositions.compositions {
        out.push(Entry {
            kind: Kind::Composition,
            key: c.id.clone(),
            title: c.id.clone(),
            lives: "Kit ▸ Compose".to_owned(),
            shown: Shown::Composition { id: c.id.clone() },
            opens: None,
        });
    }
    out
}

/// **Rigs, registered data files, notes, settings sections and workspaces.** Each is a page the editor
/// already draws; a kind with no page is left out (see the module note).
fn collect_the_rest(
    project: &Project,
    notes: Option<&crate::notes_view::NotesView>,
    bench: Option<&crate::anim_tab::BenchState>,
) -> Vec<Entry> {
    let mut out = Vec::new();
    if let Some(bench) = bench {
        for name in bench.names() {
            out.push(Entry {
                kind: Kind::Rig,
                key: name.to_owned(),
                title: name.to_owned(),
                lives: "Animation".to_owned(),
                shown: Shown::Rig { id: name.to_owned() },
                opens: None,
            });
        }
    }
    // **A rig's clips, read from the manifest** (NV-11, #391), not from the bench: the bench reads `rigs.ron` when the Animation
    // workspace is first opened, and a finder that could name a clip only after a person had been to the page it names would not be
    // a finder. A clip is a slot of the rig, and the manifest is the one file that says what the slots are.
    out.extend(collect_the_clips(project));
    // **A registered data file's page is the DATA table**, which is where the reveal puts
    // `Shown::File` when `data.yaml` lists it — the same resolution, so the two cannot disagree.
    if let Ok(Some(reg)) = bevy_grimoire_core::data_files::DataFiles::load(&project.grimoire_dir) {
        for file in &reg.files {
            let absolute = reg.resolve(file);
            let Ok(relative) = absolute.strip_prefix(&project.root) else { continue };
            out.push(Entry {
                kind: Kind::Data,
                key: file.id.clone(),
                title: file.id.clone(),
                lives: "Map ▸ DATA".to_owned(),
                shown: Shown::File { path: relative.to_string_lossy().replace('\\', "/"), lines: None },
                opens: None,
            });
        }
    }
    if let Some(notes) = notes {
        for note in &notes.notes().notes {
            out.push(Entry {
                kind: Kind::Note,
                key: note.id.clone(),
                // **The first line of the note**, because `note-1` tells a person nothing and the
                // sentence they wrote tells them everything.
                title: format!("{} — {}", note.id, first_line(&note.text)),
                lives: "Map ▸ NOTES".to_owned(),
                shown: Shown::Note { id: note.id.clone() },
                opens: None,
            });
        }
    }
    // **The sections a `Shown::Settings` names**, from the one list the settings screen draws.
    for section in crate::setting_catalogue::CATEGORIES {
        out.push(Entry {
            kind: Kind::Settings,
            key: section.to_lowercase(),
            title: (*section).to_owned(),
            lives: "Settings".to_owned(),
            shown: Shown::Settings { section: (*section).to_owned() },
            opens: None,
        });
    }
    for section in ["packs", "kits"] {
        out.push(Entry {
            kind: Kind::Settings,
            key: section.to_owned(),
            title: section.to_owned(),
            lives: "Project configuration".to_owned(),
            shown: Shown::Settings { section: section.to_owned() },
            opens: None,
        });
    }
    // **A workspace is not a thing the editor shows, it is where you are** — so it is the one row
    // without a `Shown`, and its action is the same `DoorRequest` the chord makes.
    for workspace in Workspace::ALL.into_iter().filter(|w| w.is_built()) {
        out.push(Entry {
            kind: Kind::Workspace,
            key: workspace.id().to_owned(),
            title: workspace.title().to_owned(),
            lives: "every workspace".to_owned(),
            shown: Shown::Nowhere,
            opens: Some(workspace),
        });
    }
    out
}

/// **What ends the word that names a clip in a slot's note**: the note's first clause is the word a person types (`walk`, from
/// `walk — the phase reference all the others are measured against`).
const CLIP_NOTE_SEPARATOR: &str = " — ";

/// **What a rig's slot is called**: the asset's own clip name when the manifest recorded one (`provenance`), else the first clause of the
/// slot's note, else `clip <index>`, which is how the bench's slot table spells a slot that has no other name.
pub fn clip_name(rig: &bevy_grimoire_core::rigs::Rig, slot: usize) -> Option<String> {
    let def = rig.slots.get(slot)?;
    let from_asset = rig
        .provenance
        .as_ref()
        .and_then(|p| p.clip_names.get(def.clip))
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .map(str::to_owned);
    let from_note = || {
        def.note
            .as_deref()
            .and_then(|note| note.split(CLIP_NOTE_SEPARATOR).next())
            .map(str::trim)
            .filter(|clause| !clause.is_empty())
            .map(str::to_owned)
    };
    Some(from_asset.or_else(from_note).unwrap_or_else(|| format!("clip {}", def.clip)))
}

/// **Every rig's every clip**, from `assets/grimoire/rigs.ron`: a project with no manifest, or one that does not read, has none (the
/// bench says which on its own page).
fn collect_the_clips(project: &Project) -> Vec<Entry> {
    let Ok(text) = std::fs::read_to_string(project.root.join(crate::anim_tab::RIGS_FILE)) else { return Vec::new() };
    let Ok(rigs) = bevy_grimoire_core::rigs::Rigs::parse(&text) else { return Vec::new() };
    let mut out = Vec::new();
    for (name, rig) in &rigs.rigs {
        for slot in 0..rig.slots.len() {
            let Some(clip) = clip_name(rig, slot) else { continue };
            out.push(Entry {
                kind: Kind::Clip,
                key: format!("{name}#{slot}"),
                title: format!("{name} — {clip}"),
                lives: "Animation".to_owned(),
                shown: Shown::Clip { rig: name.clone(), slot },
                opens: None,
            });
        }
    }
    out
}

/// The first line of a note, cut to a row's width, or the empty string for a note nobody has written
/// in yet.
pub fn first_line(text: &str) -> String {
    /// How many characters of a note a row shows.
    const NOTE_TITLE_CHARS: usize = 48;
    let line = text.lines().next().unwrap_or("").trim();
    if line.chars().count() <= NOTE_TITLE_CHARS {
        return line.to_owned();
    }
    let cut: String = line.chars().take(NOTE_TITLE_CHARS.saturating_sub(1)).collect();
    format!("{cut}…")
}

/// **A chosen row opens what it names**: the camera for a place on the map, the page for everything
/// else, and the workspace for a workspace.
fn show_the_chosen_row(
    chosen: On<RunCommand>,
    index: Res<GotoIndex>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    mut revealing: Option<ResMut<crate::reveal::Revealing>>,
    mut following: Option<ResMut<crate::follow::Following>>,
) {
    let Some(entry) = index.find(&chosen.id).cloned() else {
        return;
    };
    // **The workspace row is where you are**, so it is asked for as the chord asks for it.
    if let Some(workspace) = entry.opens {
        if let Some(state) = state.as_mut()
            && state.door_request.is_none()
            && !state.leaving
        {
            state.door_request = Some(crate::editor::DoorRequest { workspace, tab: None });
        }
        return;
    }
    // **A chosen row is the person's own instruction to move**, so it does not wait for the idle
    // clock a *typing* person sets: choosing from the finder is pressing Enter and then reading, and a
    // two-second wait after Enter would read as the finder being broken. This is `look_now`'s own
    // rule (AF-04, #360) — the follow's "show it now" — applied to the person's navigation, and it is
    // the same call so the two cannot disagree about whether they are being held back.
    if let Some(following) = following.as_mut() {
        following.look_now();
    }
    // **Two mechanisms and one decision, exactly as an agent's change takes it**: the camera for a
    // place on the map, the reveal for a page. A second path here would be a second answer to "which
    // page is this thing on", and the two would disagree the first time a panel moved.
    if let Some(request) = crate::follow::request_of(&entry.shown) {
        if let Some(following) = following.as_mut() {
            following.request_named(request, format!("went to `{}`", entry.title));
        }
        return;
    }
    if let Some(revealing) = revealing.as_mut() {
        // **The person asked for this page by pressing Enter**, so it is not held back by the idle
        // clock their own key set, and the key that asked does not cancel it.
        revealing.request_by_person("go_to", &entry.shown);
    }
}

/// **Swap the open finder's source when the query crosses `>`.** The list is written once per source
/// change, not per frame, and it is written into `CommandList` — the palette's own list — so the rows,
/// their ranking and their Enter are the widget's rather than a second implementation's.
fn choose_the_source(
    finder: Res<Finder>,
    index: Res<GotoIndex>,
    history: Res<PaletteHistory>,
    open: Query<(&bevy_gooey::PaletteInput, &bevy_gooey::TextInputValue)>,
    mut list: ResMut<CommandList>,
    mut shown: Local<Option<Source>>,
) {
    let Some(live) = finder.judged_in() else {
        *shown = None;
        return;
    };
    // **The palette's field carries the query**, and it is the only place it exists.
    let Some((_, query)) = open.iter().next() else {
        *shown = None;
        return;
    };
    let want = Source::of(&query.0);
    if *shown == Some(want) {
        return;
    }
    *shown = Some(want);
    list.0 = match want {
        Source::Commands => command_rows(live),
        Source::Everything => rows(&index, &history.0),
    };
}

/// The plugin: the index, the source swap, and the choice.
pub struct GotoPlugin;

impl Plugin for GotoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GotoIndex>()
            .init_resource::<Finder>()
            .add_systems(
                Update,
                (rebuild_the_index, choose_the_source).run_if(in_state(crate::screen::Screen::Editor)),
            )
            .add_observer(show_the_chosen_row);
    }
}
