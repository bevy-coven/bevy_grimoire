//! **The file watcher** — a change on disk, in the editor, without a restart.
//!
//! # The gap this closes
//!
//! The editor read its project **once**, at open, and never again. `Project::open` parses
//! `kits.ron`, the vocabulary, every bound kit's `library.ron` and `project.ron`, and the map —
//! and that is the whole load. The only on-demand re-read in the crate is
//! [`crate::project::Project::reload_vocab`], called after the vocabulary prompt appends a token.
//!
//! **So a developer who edited a data file in their editor saw nothing happen.** The file was
//! correct on disk and the editor was showing the old copy, and the difference between the two
//! was invisible until the editor was restarted. That is the slowest possible feedback loop for
//! the person whose whole job is to see what a change looks like, and it is the reason a
//! configuration-driven editor is not configuration-driven at all: **the configuration is read once
//! and then frozen.**
//!
//! # What it watches
//!
//! **The project's own files, by name** — not a directory scan. A scan would pick up the editor's
//! own output (`oplog.jsonl`) and every file the project does not own, and the set of files that
//! matter is small and known: the vocabulary, the kits, each bound kit's library and policy, and
//! the open map.
//!
//! # How it decides something changed
//!
//! **By size and modification time**, polled on a timer. **Not a hash**, because a hash means
//! reading every file every tick, and the editor is a game about maps — it must not spend a frame
//! on checking whether a text file changed. The size and the mtime are enough to notice an edit
//! and cheap enough to notice it every tick.
//!
//! # What it does when a file changes
//!
//! **Re-reads it, and adopts it only if it parses.** A malformed file is reported **with its
//! file, its field and its line**, and **the last good state stays loaded** — the editor does not
//! empty itself because somebody saved a half-written file. That is the rule the crate states
//! everywhere else: *"a tool that writes level data while unsure which line it is writing is
//! worse than no tool"*, and its mirror image — a tool that empties itself on a bad file is worse
//! than no tool.
//!
//! # The one-second target
//!
//! **250 ms between polls.** The target is about a second from a save to a visible change, and
//! the poll is the dominant term in it. The editor's own `anim_watch` polls at one second, which
//! is right for a mesh bench and too slow for a text file the author is watching.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use bevy::prelude::*;

use crate::project::{OpenMap, Project};

/// **How often the files are checked.** A quarter of a second.
const POLL: std::time::Duration = std::time::Duration::from_millis(250);

/// **What a file looked like the last time it was checked.**
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
}

/// **The files being watched, and what they looked like last time.**
#[derive(Resource, Default)]
pub struct Watched {
    files: HashMap<PathBuf, Stamp>,
    /// **When the last tick happened**, so the poll is on a timer and not on a frame.
    ///
    /// **Wall-clock, and not the app's `Time`.** The editor's frame time is its own business, and
    /// a headless run — which is where this is measured — has no frames to speak of. **A watcher
    /// that counts frames measures the frame rate, not the file.**
    last: Option<std::time::Instant>,
    /// **The project these stamps belong to.**
    ///
    /// **A door change can land on a different project**, and the stamps are the watcher's memory
    /// of *that* project's files. Carried across, the first tick on the new project would compare
    /// the new files' stamps against the old project's and see every one of them as changed — so
    /// the watcher's memory is dropped when the project changes, and the new project's files are
    /// sighted afresh.
    project: Option<PathBuf>,
    /// **The watched paths that had no file at the last tick** (#158).
    ///
    /// `repetition.ron`, `standards.ron`, `rules.ron` and `compositions.ron` are optional, so the
    /// first edit an author makes to one is creating it. A file with no earlier stamp is only a
    /// first sighting when the watcher has never looked at its path; one it saw missing and now
    /// finds is a file written while the editor was open, and that is a change.
    absent: HashSet<PathBuf>,
}

impl Watched {
    /// **The files this editor watches**, and their current stamps.
    ///
    /// **Derived from the project and the open map, every time the watch runs.** The set of files
    /// changes when a kit is bound or a map is opened, and a fixed list would be stale — which is
    /// the same failure as a hard-coded list of a project's kits.
    fn files_of(project: &Project, open: Option<&OpenMap>) -> Vec<PathBuf> {
        // **The project's own files, by the names the core uses to find them.**
        //
        // **The names are the core's constants, not literals here.** The first version watched
        // `<root>/library.ron` — **a file that does not exist** — because the library lives in
        // each kit's own directory and the root holds `kits.ron` instead. A watcher that names a
        // file nobody writes is a watcher that never sees a change it should.
        let mut out = vec![
            project.root.join(crate::project::VOCAB),
            project.project_dir.join(bevy_grimoire_core::kits::KITS_FILE),
            project
                .project_dir
                .join(bevy_grimoire_core::composition::Compositions::FILE),
            project.project_dir.join(bevy_grimoire_core::rules::FILE),
            // The project's kit standards (#138): the Meshes pane reads them on each rebuild, so a
            // change only has to reach it.
            project.project_dir.join(bevy_grimoire_core::standards::FILE),
            // The project's repetition rules (#139): the REPEATS line re-reads them when the
            // project changes, so a change only has to reach it.
            project.project_dir.join(bevy_grimoire_core::repetition::FILE),
        ];
        // **Every bound kit's own files**, because each kit has a library and a policy of its own.
        for kit in project.kits.iter() {
            out.push(kit.dir.join("library.ron"));
            out.push(kit.dir.join("project.ron"));
        }
        if let Some(open) = open {
            out.push(open.map_path.clone());
        }
        out
    }
}

/// **The watcher plugin.**
pub struct WatchPlugin;

impl Plugin for WatchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Watched>()
            .init_resource::<Due>()
            .add_systems(
                Update,
                (
                    // **The tick decides; the act does.** A system that takes the project and the
                    // map **every frame** holds them against every other system that wants them,
                    // and Bevy resolves a conflict by **deferring one of the two** — so the badge
                    // layout, which measures the interface in `Phase::Act`, was being pushed past
                    // the frame by a watcher that had nothing to do. **So the tick holds nothing
                    // but its own resource, and the act — which takes the project and the map —
                    // runs only when the tick found something.**
                    tick_watch,
                    adopt_changes,
                )
                    .chain()
                    .in_set(crate::keys::Phase::Sense),
            );
    }
}

/// **What the poll found, and what it is owed.**
#[derive(Resource, Default)]
pub struct Due {
    /// **The files whose stamp changed since the last tick.**
    pub changed: Vec<PathBuf>,
    /// **The open map was replaced by a map read from disk.**
    ///
    /// **The editor's map and the editor's screen are two different things**, and this is the
    /// signal that says so. The map lives in [`crate::project::OpenMap`], and the pieces on
    /// screen are entities — and **the entities were spawned once, when the editor opened**.
    ///
    /// So a person editing `town.map.ron` in a text editor got the map and not the screen: the
    /// inspector's numbers changed, because they are read from the map every frame, and the
    /// world did not, because the pieces are the ones spawned at startup. **That is the worst
    /// shape a bug can have** — the editor looks like it took the edit.
    ///
    /// **The flag is consumed by the redraw**, which respawns every placement from the map the
    /// watcher just read.
    pub map_replaced: bool,
}

/// **Look at the files, and note what changed.**
///
/// **Every frame, and it holds nothing but its own resource.** The whole point is that a watcher
/// which does nothing must not stand in the way of a system which does.
pub fn tick_watch(
    mut watched: ResMut<Watched>,
    mut due: ResMut<Due>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
) {
    due.changed.clear();
    // **The poll is on the wall clock**, and the first tick happens at once — so a file that
    // changed between the editor's open and the first frame is not missed.
    let now = std::time::Instant::now();
    if watched
        .last
        .is_some_and(|last| now.duration_since(last) < POLL)
    {
        return;
    }
    watched.last = Some(now);
    let Some(project) = project.as_ref() else {
        return;
    };

    // **A different project, a different set of files.** The watcher's memory of the old
    // project's files is dropped, so the new project's files are sighted afresh — **and a file
    // that has not changed is not seen as changed**, which is what the first sighting is for.
    if watched.project.as_deref() != Some(project.root.as_path()) {
        watched.files.clear();
        watched.absent.clear();
        watched.project = Some(project.root.clone());
    }

    // **The open map's file is in the list**, because a person editing `town.map.ron` in a text
    // editor is the case this module exists for, and the map file is the most-edited file in a
    // project.
    let files = Watched::files_of(project, open.as_deref());
    let mut seen: HashMap<PathBuf, Stamp> = HashMap::with_capacity(files.len());
    let mut absent = HashSet::new();
    for path in files {
        let stamp = stamp_of(&path);
        // **A file that is not there is not a change.** A map that has never been saved has no
        // file, and the watcher must not report a deletion the author did not make. Its absence is
        // remembered, so its creation is seen.
        let Some(stamp) = stamp else {
            absent.insert(path);
            continue;
        };
        seen.insert(path.clone(), stamp.clone());
        let previous = watched.files.get(&path);
        if previous == Some(&stamp) {
            continue;
        }
        //
        // **The first sighting is not a change either** — it is the editor noticing what was
        // already there, and treating it as an edit would reload the project the moment it
        // opened. A path last seen with no file is not a first sighting: the file was created.
        if previous.is_none() && !watched.absent.contains(&path) {
            continue;
        }
        due.changed.push(path);
    }
    watched.files = seen;
    watched.absent = absent;
}

/// **Adopt what the tick found, if there is anything.**
///
/// **Runs only when there is something to adopt**, and only then takes the project and the map
/// — so a frame in which nothing changed costs one `stat` per file and nothing else.
pub fn adopt_changes(
    mut due: ResMut<Due>,
    project: Option<ResMut<Project>>,
    mut open: Option<ResMut<OpenMap>>,
    mut status: Option<ResMut<crate::tiles::ImportState>>,
    // **The Map door's own status**, because a refusal goes where the author is looking (B-016).
    // It went to `ImportState` only, the kit door's status, so on the Map door a map file saved
    // with a typo was refused silently: the old map stayed, which is right, and nothing said why.
    mut map_status: Option<ResMut<crate::editor::EditorState>>,
) {
    if due.changed.is_empty() {
        return;
    }
    let Some(res) = project else { return };
    let project = res.into_inner();
    // **The paths are taken, so the flag can be set while they are walked.** The borrow is the
    // whole of the loop's body, and the flag is set inside it.
    let changed_paths = std::mem::take(&mut due.changed);
    let mut map_replaced = false;
    for path in &changed_paths {
        // **The open map's own file, re-read.**
        //
        // **A person editing `town.map.ron` in a text editor is the case this whole module
        // exists for**, and the map file is the most-edited file in a project. So it is
        // watched like every other authored file — **and the editor's own save is not a
        // change**, because the file the editor writes is the file the editor already has.
        //
        // **The "is this our own save?" question is answered by comparing the file's contents
        // with what the editor holds**, not by ignoring the file: a save that changed something
        // the editor does not have is a change, and a save that changed nothing is not.
        if let Some(o) = open.as_deref_mut()
            && o.map_path == *path
        {
            match adopt_map(path, o) {
                Ok(changed) => {
                    // **The map was replaced, and the screen has to hear about it.**
                    //
                    // **The map and the screen are two different things.** The map is the
                    // resource the inspector reads; the screen is the entities, and those were
                    // spawned once, when the editor opened. **So the map can be right and the
                    // screen wrong**, and a developer who trusts the number is wrong about the
                    // picture.
                    map_replaced |= changed;
                }
                Err(e) => {
                    // The open map is the Map door's, so its refusal is the Map door's.
                    if let Some(state) = map_status.as_deref_mut() {
                        state
                            .status
                            .problem(format!("{e} — the editor kept what it had."));
                    }
                }
            }
            continue;
        }
        // **A change in a file the editor reads.**
        //
        // **And the editor's own writes are not changes.** The editor writes a kit's
        // `library.ron` when a piece is committed, and the file's stamp changes — so the
        // watcher would re-open the project over the editor's own back, **in the frame after the
        // commit**, and reset the kit's state the author (or a test) is looking at.
        //
        // **So the file is compared before the project is re-opened.** A file whose content is
        // what the editor already holds is not a change, whatever its stamp says; and the
        // comparison is on the *content*, because a stamp cannot tell the editor's own write
        // from a person's edit.
        if !differs(path, project) {
            continue;
        }
        if let Err(e) = adopt(path, project) {
            // **The refusal names the file, and the editor keeps what it had.** The last good
            // state stays loaded, which is the whole point of adopting only on success. It goes
            // to the door that is showing: the Map door when a map is open, else the kit door.
            let why = format!("{e} — the editor kept what it had.");
            match (
                open.is_some(),
                map_status.as_deref_mut(),
                status.as_deref_mut(),
            ) {
                (true, Some(map), _) => map.status.problem(why),
                (_, _, Some(kit)) => kit.status.problem(why),
                _ => {}
            }
        }
    }
    // **And the flag goes up after the walk**, so the redraw runs once with the map the
    // watcher settled on, and not once per file.
    due.map_replaced |= map_replaced;
}

/// **Does this file say something the project does not already hold?**
///
/// **Compared on the content, and not on the stamp.** The stamp changes on every write, and the
/// editor writes — so a stamp cannot tell the editor's own write from a person's edit. **The
/// content can**, and a file that says what the editor already says is not a change.
///
/// **A file that cannot be read is a change**, because the editor may be about to read it and
/// find it gone.
fn differs(path: &Path, project: &Project) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return true;
    };
    // **The kit's own library, compared against the layer the editor holds.**
    //
    // **Compared by PARSING, and not by the bytes.** A hand-written `library.ron` carries
    // comments and its own formatting, so a byte comparison against the editor's own
    // serialisation would say "different" for a file the editor itself wrote — and the watcher
    // would re-open the project over the editor's back every time. **Parsing both sides and
    // comparing the values is what "the same library" means.**
    if path.file_name().and_then(|n| n.to_str()) == Some("library.ron")
        && let Some(kit) = project
            .kits
            .iter()
            .find(|k| k.dir.join("library.ron") == path)
    {
        // **The editor's copy is the *measured* layer**, which is what it writes and what it
        // reads back; the layered one is that plus the kit's policy patches.
        return match bevy_grimoire_core::library::Library::parse(&text) {
            Ok(fresh) => fresh != kit.measured,
            // **A file that does not parse IS a change** — the editor has to try to read it, and
            // the refusal that comes back names the file and the parser's message.
            Err(_) => true,
        };
    }
    // **Anything else the editor does not hold a copy of** — the vocabulary, a policy — is a
    // change, because the editor cannot compare what it does not have.
    true
}

/// **Re-read the open map's file and adopt it, if it differs from what the editor holds.**
///
/// **Compared, not assumed.** The editor saves the map itself, and a save rewrites the file — so
/// without the comparison the editor would re-read its own save on every poll, and the map would
/// be re-opened every 250 ms for the rest of the session.
///
/// **And the comparison is on the content, not on the stamp**: the editor's save changes the
/// stamp, and a person's edit changes it too, and the only question worth asking is whether the
/// file now says something the editor does not.
/// **Re-read the map file and put it in the editor — and say whether the map changed.**
///
/// **The "did it change?" answer matters**, because the editor's own save writes this file and
/// the watcher sees the write. A redraw on every one of the editor's own saves would respawn
/// every piece after every edit, for no reason.
///
/// # Compared against what was last saved, not against what the editor holds (B-049)
///
/// This compared the re-read file with `open.map` — **the in-memory map, which is where the
/// person's unsaved edits live** — and replaced it whenever the two differed. So a save from
/// anywhere (an agent's operation, another editor, a `cp`) followed by an unsaved edit inside the
/// 250 ms poll replaced the edit with the file, **silently**: the map on screen and the map on
/// disk then agree, so nothing looks wrong, and the work is gone. The intermittent failure of
/// `a_person_removes_a_piece_and_undoes_it::an_agents_undo_of_a_removal_waits_while_the_person_has_moved_a_piece_since`
/// was this, reported as flakiness.
///
/// The decision order below is what "the file says something the editor does not" turns into once
/// a third state exists — the file, what was saved, and what the person has since typed:
///
/// 1. **The file says what the editor holds** → nothing happened; it is this editor's own save
///    landing, and `mark_saved` keeps the map from being called unsaved for a save just made.
/// 2. **The file says what was last saved or read** → it is the file the editor already knows
///    about, so an unsaved edit is the only difference and it is the person's. Nothing to do.
/// 3. **The person has unsaved edits** → **refuse, and say why.** A re-read here would choose
///    between two versions of somebody's work on their behalf. The caller puts this in the Map
///    door's `status.problem`, so it is an alarm rather than a note.
/// 4. **Otherwise** → a clean editor and a file that says something new: adopt it.
fn adopt_map(path: &Path, open: &mut OpenMap) -> Result<bool, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let fresh =
        bevy_grimoire_core::map::Map::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if fresh == open.map {
        // **The file says what the editor already says.** The editor's own save, or a person
        // saving an identical file. **Nothing to do.**
        open.mark_saved();
        return Ok(false);
    }
    if fresh == *open.saved() {
        // **The file is exactly what was last saved or read.** The only difference from `map` is
        // the person's own unsaved editing, which no poll has any business touching.
        return Ok(false);
    }
    if open.dirty() {
        // **Two versions, and one of them is unsaved work.** Refusing is the only answer that
        // cannot destroy either: the file is still on disk, and the editor still holds what the
        // person typed. The route out is in the message, because "it refused" without one is a
        // person who cannot save.
        return Err(format!(
            "`{}` changed on disk while this map has unsaved edits, so the file was not read. Save \
             to keep your version; to take the file's, leave the map without saving and open it \
             again",
            path.display()
        ));
    }
    // **The new map replaces the old one, and the editor's own save record is refreshed** — so
    // the editor does not think the map is unsaved when it is not.
    open.map = fresh;
    open.mark_saved();
    Ok(true)
}

/// **Re-read one file and adopt it, or refuse with the file, the field and the line.**
///
/// **One file at a time, and only the ones the editor reads.** The vocabulary is re-read through
/// the editor's own gate — [`Project::reload_vocab`], which resolves the masks and swaps
/// all-or-nothing — and a failure there is a refusal with the file and the parser's own message.
///
/// # A kit's save re-reads that kit, and the project-wide files still go through the open (E-003)
///
/// A kit's `library.ron` or `project.ron` can only change that kit: which directory answers for
/// which namespace is `kits.ron`'s business and a different file. So the two kit files go through
/// [`Project::reload_kit`], which re-reads the one directory, re-merges, and re-does every check
/// the open does over the result — instead of re-opening the project, its GLBs and every other kit.
///
/// **`kits.ron`, `vocab.ron`, `compositions.ron` and `rules.ron` are the project's**, and a change
/// to one of them still goes through a whole open: the binding decides which directories are read
/// at all, and the other two are parsed and validated against the merge.
fn adopt(path: &Path, project: &mut Project) -> Result<(), String> {
    if path.ends_with("vocab.ron") {
        return project.reload_vocab();
    }
    // **The kit standards and the repetition rules are read where they are shown** (the Meshes
    // pane, the REPEATS line, `grimoire_state`, `piece_import`), so nothing is re-opened: adopting is
    // checking that the file parses, and the caller's mutable borrow of the project is what tells
    // the readers to look again.
    if path.parent() == Some(project.project_dir.as_path()) {
        match path.file_name().and_then(|n| n.to_str()) {
            Some(bevy_grimoire_core::standards::FILE) => {
                return crate::ops_brp::load_standards(&project.project_dir).map(|_| ());
            }
            Some(bevy_grimoire_core::repetition::FILE) => {
                return crate::ops_brp::load_repetition(&project.project_dir).map(|_| ());
            }
            _ => {}
        }
    }
    // **Which kit, if any, this file belongs to.** Both of a kit's files are named by the core's
    // own constants, so a kit directory cannot be recognised by a filename pattern that drifts.
    let kit_dir = project
        .kits
        .iter()
        .find(|k| path.parent() == Some(k.dir.as_path()))
        .map(|k| k.dir.clone());
    if let Some(dir) = kit_dir {
        // **And the Map is told which pieces changed, through the same comparison the Tiles tab
        // uses** ([`crate::tiles::changed_ids`]).
        //
        // Without this the re-read is a change to the editor's *memory* only: the placed pieces
        // were spawned from the library the editor held then, and the systems that draw them are
        // driven by [`Project::touched`] — which only `tiles::commit_measured` filled. So a person
        // editing a piece's `stretch_y`, `scale`, `extent` or `mesh` in a text editor got the
        // palette and the inspector updated and **the world unchanged**: E-007's shape, one file
        // over, where the number agrees with the file and the picture is a lie.
        //
        // `touched` is compared against the library being replaced, not against the file, so a
        // hand-written `library.ron` with its own formatting cannot read as a change — and
        // [`Project::reload_kit`] returns every refusal before it touches `self`, so a bad file
        // leaves the editor holding the last good project whole.
        let touched = project.reload_kit(&dir)?;
        project.touched.extend(touched);
        return Ok(());
    }
    // **A project-wide file.** Nothing smaller than the open knows how to re-read these: the
    // binding decides which kits are loaded at all, and the other three are validated against the
    // merge it produces.
    let root = project.root.clone();
    let kit = project
        .kits
        .iter()
        .find(|k| k.namespace == project.namespace)
        .and_then(|k| k.dir.file_name())
        .map(|n| n.to_string_lossy().into_owned());
    let fresh = Project::open(&root, kit.as_deref())?;
    let touched = crate::tiles::changed_ids(&project.library, &fresh.library);
    // **And the map the author has open is still the map they have open.** `Project::open`
    // starts with `open_map_name: None`, which is right for a first open and wrong here: this
    // is the same project, re-read, and the name is the author's intent about which map they
    // are working on. Dropping it made a kit edit the same as never having opened a map, so
    // the next crossing from the Kit door back to the Map door refused with "none has been
    // open".
    let open_map_name = project.open_map_name.clone();
    let mut fresh = fresh;
    fresh.open_map_name = open_map_name;
    *project = fresh;
    project.touched.extend(touched);
    Ok(())
}

/// **A file's size and modification time.**
///
/// **`None` for a file that is not there**, which is different from a file of zero length.
fn stamp_of(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some(Stamp {
        len: meta.len(),
        modified: meta.modified().ok(),
    })
}
