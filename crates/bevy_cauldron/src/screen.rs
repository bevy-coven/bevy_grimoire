//! **Which half of the editor is live** — the menu, or a door.
//!
//! # One application, two screens
//!
//! These were two processes. The menu built its own `App`, ran to completion, launched the editor
//! as a child, and a supervisor spawned a fresh menu per lap — because **winit builds at most one
//! event loop per process**, so a second `App::run` in one process dies on `RecreationAttempt`.
//!
//! That shape was chosen for a second reason which is the one that actually mattered: `Project` is
//! inserted before the editor's plugins, and in Bevy 0.19 a **missing `Res<T>` panics its system**
//! rather than skipping it. Around a hundred parameter positions take `Res<Project>`, so a menu
//! living inside the editor's `App` meant gating every one of them — *"Gating is feasible… it is the
//! **cost** that is the argument"* (the chooser, which the project page replaced).
//!
//! Asked for at the keyboard, 2026-08-16: *"can we not open a whole another editing window? I'd like
//! to keep the same bevy application running across whether it's the UI or the editor."* So the cost
//! gets paid. This state is what pays it: every editor system runs `in_state(Screen::Editor)`, so in
//! [`Screen::Project`] none of them run and none of them fetch a `Project` that is not there.
//!
//! # A door change is a full teardown
//!
//! Chosen at the keyboard on the same day, over keeping the project loaded between doors: leaving a
//! door despawns everything it made and drops the project; entering the next one loads it fresh.
//!
//! That is exactly what the two processes did, minus the process. The alternative — keep the library,
//! vocabulary and thumbnails warm and swap only the map — is faster and is the whole point of one
//! window, but it makes every entity, resource and history stack I forget to clear a stale value
//! that survives into the next door. A reload cannot be wrong; a partial teardown can be, silently,
//! and the bug lands weeks later looking like something else.
//!
//! # Doors are reached through the menu — and now also through a key
//!
//! **The menu is still the way a door is _opened_**, and it is still the only one. A key does not
//! open a door; it **changes** which door is open, from inside one.
//!
//! It was declined once, on a premise that has expired. The note said: *"It became affordable once
//! the process boundary went — the design doc had rejected it when it meant a relaunch — and it was
//! declined anyway: with full teardown the saving is the menu round-trip rather than the reload."*
//! **The relaunch is gone** — the menu and the editor are one application — and so is the reload it
//! was weighed against: a crossing **keeps the project** (see [`cross`]), which is what the
//! [`Ownership`] table has always said a same-kit door change should do.
//!
//! Measured on 2026-10-21: the round trip was **three keystrokes and a full project reload** each
//! way — `Cmd+O`, walk a column, `Enter` — re-reading `kits.ron`, `vocab.ron` and `rules.ron`,
//! re-validating 332 library descriptors and 16 compositions, and **re-measuring every GLB's
//! triangle count** from its JSON chunk. None of that changes when the author walks from one door
//! to the other to look at the thing they just made.
//!
//! **And the two are different acts**, which is why they are different actions: the menu *opens* a
//! door, and the crossing *changes* one. A second way to do one thing is the pattern this crate
//! spends its refusals on; this is not that.

use bevy::prelude::*;

/// The menu, or a door.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Screen {
    /// The launcher: the projects opened before, and a way to open another (`launcher.rs`). `main` starts here
    /// when no project is named; a bare `ROOT` starts at [`Screen::Project`] and a door or a map at [`Screen::Loading`].
    #[default]
    Launcher,
    /// A project being opened as named stages, with a spinner and a progress bar when it takes long
    /// (`loading.rs`). `main` starts here when the command line names a door; it ends in [`Screen::Editor`],
    /// or back in [`Screen::Launcher`] when it is cancelled or the project will not open.
    Loading,
    /// The project page: a project's maps and kits (`project_page.rs`), where a project opened from the home lands before a door opens.
    Project,
    /// A door, with a [`crate::project::Project`] loaded and a [`crate::tiles::Door`] chosen.
    Editor,
}

/// **The plugin that owns the transitions into and out of a door** (the menu's exit, the loader's, and the editor's).
///
/// The project page's exit to the editor opens the project, because a state transition runs every `OnExit` before any
/// `OnEnter` — so by the time the editor's own spawns run, `Project` is already in the World and the
/// hundred systems that take it are not looking at an empty slot.
///
/// `OnExit(Editor)` puts everything back: entities despawned, resources dropped. See the module note
/// on why that is a full reload rather than a swap.
pub struct ScreenPlugin;

impl Plugin for ScreenPlugin {
    fn build(&self, app: &mut App) {
        // `main` starts a bare `bevy_cauldron` on the launcher by inserting the state first; leave that one alone.
        if !app.world().contains_resource::<State<Screen>>() {
            app.init_state::<Screen>();
        }
        app.add_systems(OnTransition { exited: Screen::Project, entered: Screen::Editor }, open_the_door)
            // The loader leaves for the editor only with a door in hand; leaving for the launcher (cancelled,
            // or a project that would not open) has nothing to open, so it is the one exit that is not
            // `OnExit(Loading)`.
            .add_systems(OnTransition { exited: Screen::Loading, entered: Screen::Editor }, open_the_door)
            .add_systems(OnExit(Screen::Editor), close_the_door)
            // A project opened from inside the editor is pointed at once its door is closed.
            .add_systems(OnExit(Screen::Editor), crate::project_assets::apply_pending_open.after(close_the_door));
    }
}

/// **The door the project page, the loader or an agent chose, already opened.**
///
/// Still one parser: the argv `[root, map?, --door, d, --kit, k]` goes to `args::open`. The *opening* happens **before** the state moves,
/// not after: the transition into the editor used to carry the argv and let [`open_the_door`] open it, whose failure branch could not work —
/// Bevy runs `ExitSchedules` and `EnterSchedules` in one `StateTransition` pass, so a `NextState` written during `OnExit` is not read until
/// the *next* pass and `OnEnter(Editor)` ran anyway, against a world with no `Project`. Opening first means the only way into
/// `Screen::Editor` is with a door in hand, and a door that will not open is refused where a person is looking.
#[derive(Resource)]
pub struct Chosen(pub crate::args::Opened);

/// Put what the project page opened into the World. An exclusive system, so the resources exist the instant
/// this returns rather than whenever a command queue is next flushed — `OnEnter(Editor)` is the very
/// next schedule and every system in it assumes a loaded project.
///
/// **This cannot fail, and that is the point.** It used to call `args::open` here and, on an error,
/// write the reason onto the page and set `NextState(Project)`. That never worked: a
/// `StateTransition` runs `ExitSchedules` then `EnterSchedules` in one pass, so the `NextState` was
/// not read until the following pass and `OnEnter(Editor)` ran regardless — against a world with no
/// `Project` and no `Door`, which panics rather than returning. The open moved to
/// the loading screen (`loading.rs`) and an agent's `map_open`, both of which can refuse in place; what arrives here is a
/// door already open. See [`Chosen`].
fn open_the_door(world: &mut World) {
    let Some(chosen) = world.remove_resource::<Chosen>() else {
        // Reached only when something set the state directly. Loud, because the alternative is an
        // editor with no project and a hundred systems finding out one at a time.
        error!("entering a door with nothing chosen — staying on the project page");
        if let Some(mut next) = world.get_resource_mut::<NextState<Screen>>() {
            next.set(Screen::Project);
        }
        return;
    };
    // **One place records an opening**, because this is the one way into the editor: the menu, the
    // command line and an agent's `map_open` all arrive as a `Chosen`. Only where a file is named
    // (`main` names it), and a list that cannot be written never stops the project opening.
    if let Some(file) = world.get_resource::<crate::recent::RecentProjects>().map(|r| r.0.clone())
        && let Err(e) = crate::recent::record(&file, &chosen.0.project.root)
    {
        warn!("recent projects not updated: {e}");
    }
    chosen.0.insert_into(world);
}

/// Everything the door made, gone.
///
/// The entity sweep is by root — everything the editor spawns hangs off one, and a teardown naming
/// each marker is a list that goes stale the first time somebody adds a camera. The resources are
/// named, because there is no such reachability rule for them and a stale `Project` is precisely the
/// thing that must not survive into the next door.
///
/// **`pub` so a test can drive the teardown.** The headless harness does **not** register
/// [`ScreenPlugin`] — it inserts the state directly, because a harness that stands up a door is not
/// a harness that changes one — so `OnExit(Editor)` never runs in a test, and this teardown had
/// **no test at all** until 2026-10-21. That is the crate's most delicate machinery: it is what
/// keeps a stale value from surviving into the next door, and it was the one thing a test could not
/// reach. `tests/the_arm_crosses_the_door.rs` calls it directly, which is the only way to ask *"what
/// does a door change do to this resource?"* without standing up a second application.
pub fn close_the_door(world: &mut World) {
    despawn_scene(world);
    // **A crossing is in flight: the door's own resources stay.**
    //
    // The crossing has *already* put the new door's `Project`, `OpenMap`, `Door` and `Mode` into
    // the world — it is a state change, not a place the project is kept. So the teardown must not
    // take them away, or it would undo the crossing.
    //
    // **What the teardown still does, unchanged:** the entity sweep and the door's own working
    // state. The teardown is a teardown; only the *door's own* resources are spared, and they are
    // spared because they now belong to the *next* door.
    if world.remove_resource::<Crossing>().is_some() {
        // **The receipt is carried over the reset (cycle 8, gap 7).** The reset puts `EditorState`
        // back empty, which erased the receipt of whoever asked for this door: the window changed
        // doors with no word of who did it. The receipt is what the person reads on arrival, so it is
        // kept. A problem is not: the journal already holds it.
        let receipt = world
            .get_resource::<crate::editor::EditorState>()
            .map(|state| state.status.note_text().to_owned());
        reset_door_state(world);
        if let Some(receipt) = receipt.filter(|r| !r.is_empty())
            && let Some(mut state) = world.get_resource_mut::<crate::editor::EditorState>()
        {
            state.status.note(receipt);
        }
        return;
    }
    crate::args::Opened::remove_from(world);
    reset_door_state(world);
}

/// **A door change is in flight** — the teardown is part of a crossing, not of a leave.
///
/// The mark is what tells [`close_the_door`] that the `Project` and the `OpenMap` in the world are
/// already the **new** door's, and must survive the teardown that is about to run.
#[derive(Resource)]
pub struct Crossing;

/// **Cross to the other door, keeping the project.**
///
/// # What it replaces
///
/// The round trip. Measured on 2026-10-21: **`Cmd+O`, `right`, `Enter`** each way — three
/// keystrokes and a **full project reload**, to move between two halves of one project. The reload
/// re-reads `kits.ron`, `vocab.ron` and `rules.ron`, re-validates every bound kit's
/// `library.ron`, and **re-measures the triangle count of every GLB** by reading its JSON chunk.
/// Nothing in any of that changes when the author walks from one door to the other.
///
/// The [`crate::screen::Ownership`] table already says so: `Project` is *"reloaded when the kit is
/// not"*. **This is the crossing that obeys the table.**
///
/// # How it crosses
///
/// **`NextState::set(Screen::Editor)`, while already in `Screen::Editor`.** Bevy 0.20 runs
/// `OnExit` and `OnEnter` for a same-state transition — `NextState::set` is `Pending`, and only
/// `PendingIfDifferent` suppresses the schedules. So `OnExit(Editor)` runs [`close_the_door`] — the
/// teardown, unchanged — and `OnEnter(Editor)` runs the spawns.
///
/// **And the project page never appears.** A same-state transition runs neither `OnExit(Project)` nor
/// `OnEnter(Project)`, so the project page's `build_page` / `fill_page` never
/// run. The author sees the editor tear down and the other door build, in one frame.
///
/// The alternative — hopping through `Screen::Project` — would build the whole page's UI and tear it
/// down, which is a flash of the wrong screen on every crossing.
///
/// # The `Project` never leaves the world
///
/// **The first version of this took the `Project` out of the world** and put it back in
/// `Chosen` — which is what `close_the_door` removes and `open_the_door` restores. **And the live
/// test found it in one frame:** twenty editor systems ran with no `Project` to read, and Bevy 0.19
/// **panics a system whose `Res<T>` is missing** rather than skipping it. The crossing tore the
/// door down and left the editor without a project for the rest of the frame.
///
/// So the `Project` is **moved, not copied** — it is not `Clone` — and it is moved **into the new
/// door's place**: the crossing takes it out and puts it straight back, so the world holds a
/// project at every moment of the crossing, and the *teardown* is what removes it, exactly as it
/// does today.
///
/// The crossing therefore does the swap itself, rather than letting `close_the_door` and
/// `open_the_door` do it in two schedules with a gap between them.
pub fn cross(world: &mut World, to: crate::tiles::Door) -> Result<(), String> {
    // **The project is TAKEN, not copied** — it is not `Clone`.
    let Some(mut project) = world.remove_resource::<crate::project::Project>() else {
        return Err("there is no project open to carry across".to_owned());
    };
    // **The map's name, carried across.** The Kit door has no map — so without this, crossing back
    // to the Map door has nothing to open, and the refusal it gives is *true* and useless: the
    // author was on a map a moment ago.
    //
    // **The name is the project's own**, because it is the author's intent about which map they are
    // working on, and it outlives the door it was opened in — the same argument the armed
    // composition makes.
    let carried = world
        .get_resource::<crate::project::OpenMap>()
        .map(|m| m.map.name.clone())
        .or_else(|| project.open_map_name.clone());
    // **The target's resources, built from the project that is already loaded.** The Kit door needs
    // a project and no map; the Map door needs a project and a map — and the *map* is re-read,
    // because a map is a map and the Map door's subject is the one the author is editing.
    let (open_map, mode) = match to {
        crate::tiles::Door::Map => {
            let Some(name) = carried else {
                world.insert_resource(project);
                return Err("the Map door needs a map, and none has been open".to_owned());
            };
            // **Through the registry, and that is what makes an agent's `map_open` and a person's
            // crossing one thing.** This read the file itself (`OpenMap::open`) while `map_open`
            // did the same — two answers to "what does opening a map mean", which is the second
            // front end the parity rule forbids. The context is built from clones and the result is
            // written back, the way `ops_brp::take`/`put_back` do it: a crossing is not a place to
            // move the `Project` into a context and hope.
            let mut ctx = crate::ops::Ctx {
                project: Some(project.clone()),
                root: None,
                open: None,
                source: crate::ops::Source::Human,
                session: crate::ops::session_id(),
                ..crate::ops::Ctx::default()
            };
            match crate::ops::dispatch(&mut ctx, "map_open", &serde_json::json!({"name": name})) {
                Ok(crate::ops::Applied::Done { outcome, .. })
                    if outcome.after.as_ref().is_some_and(|a| a.get("map").is_some()) =>
                {
                    // **And the project remembers it**, so the next crossing back knows.
                    project.open_map_name = Some(name.clone());
                    let fresh = ctx.open.clone();
                    (fresh, crate::tiles::Mode::Map)
                }
                Ok(crate::ops::Applied::Done { .. }) => {
                    world.insert_resource(project);
                    return Err(format!("`{name}` could not be opened"));
                }
                Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
                    // **The project goes back.** The crossing failed and the editor is untouched.
                    world.insert_resource(project);
                    return Err(e.0);
                }
            }
        }
        other => {
            // **The project keeps the name when no map is open.** A rename changes the open map's
            // name and not the project's, so without this a Kit → Map crossing asks for the old
            // name — and `OpenMap::open` makes a new, empty map under a name it cannot find.
            project.open_map_name = carried;
            (None, other.opens_on())
        }
    };
    // **And it goes straight back in.** The world holds a project at every instant of the
    // crossing: the editor's systems run in the same frame as the keypress, and **twenty** of
    // them take a `Res<Project>`, and Bevy 0.19 **panics a system whose `Res<T>` is missing**
    // rather than skipping it. The first version of this function left the world without a
    // project for the rest of the frame, and the live test found it in one frame.
    world.insert_resource(project);
    match open_map {
        Some(m) => {
            world.insert_resource(m);
        }
        None => {
            world.remove_resource::<crate::project::OpenMap>();
        }
    }
    world.insert_resource(to);
    world.insert_resource(mode);
    // **And a `Chosen` for the teardown to give back.**
    //
    // `close_the_door` — which the transition runs — **removes** the `Project`, and something has
    // to put the new door's back. `install_the_door` is that something, and it reads a `Chosen`.
    //
    // **But the `Project` is not `Clone`, and it is already in the world.** So the `Chosen` cannot
    // hold it as well.
    //
    // **The answer is that the teardown does not need to take the project away at all.** The
    // crossing has *already* put the new door's resources in the world — the project, the map, the
    // door and the mode. What the teardown has to do is take away everything else the old door
    // made, and reset the door's own working state. The project and the map are **the new door's**
    // by then, and taking them away would be undoing the crossing.
    //
    // So the crossing **marks** the transition, and `close_the_door` reads the mark: a crossing is
    // in flight, so the door's *own* resources stay and only the old door's entities and working
    // state go.
    world.insert_resource(crate::screen::Crossing);
    // **And the transition does the teardown and the rebuild.**
    //
    // **`NextState::set(Screen::Editor)`, while already in `Screen::Editor`.** Bevy 0.20 runs
    // `OnExit` and `OnEnter` for a same-state transition — `NextState::set` is `Pending`, and only
    // `PendingIfDifferent` suppresses the schedules. So `OnExit(Editor)` runs [`close_the_door`] —
    // the teardown, unchanged — and `OnEnter(Editor)` runs the spawns.
    //
    // **The project is put back by `install_the_door`**, which reads the `Chosen` the crossing
    // left behind. That is the one thing `close_the_door` cannot do on its own: it *removes* the
    // project, and something has to put the new door's back.
    //
    // **The project page never appears.** A same-state transition runs neither `OnExit(Project)` nor
    // `OnEnter(Project)`, so the project page's `build_page` / `fill_page`
    // never run. The author sees the editor tear down and the other door build, in one frame.
    //
    // **The alternative — hopping through `Screen::Project` — would build the whole page's UI and
    // tear it down**, which is a flash of the wrong screen on every crossing.
    if let Some(mut next) = world.get_resource_mut::<NextState<Screen>>() {
        next.set(Screen::Editor);
    }
    Ok(())
}

/// **Put every [`Ownership::Door`] resource back to its opening value.**
///
/// # This is the fix for the worst bug on the 2026-09-03 list
///
/// [`OWNERSHIP`] classified fifty-odd resources as the door's own working state and **nothing acted
/// on it** — the table was a declaration, and `close_the_door` removed exactly four resources. The
/// test beside it says so in as many words: *"The door change is **already** the partial teardown
/// the comment is spent avoiding, and the bug class it warns about is already open."*
///
/// Reproduced on camera: open `furniture`, `Ctrl+O`, open `scp` — a kit with **zero** pieces — and
/// the chrome bar reads `KIT · scp` while the inspector shows `hoover`, the shelf chips read
/// `NOT IMPORTED (270) / MESHES (90)`, the list shows the other kit's tile and the other kit's mesh
/// is on the stage. `ImportState.scanned` is what pins it: both scan entrances are gated on
/// `!state.scanned`, which means "first look", and a door change is a first look.
///
/// # Reset, not remove, and the difference is a panic
///
/// A missing `Res<T>` **panics its system** in Bevy 0.19, and these are `init_resource`d once at
/// plugin build — so removing them would take the next door down rather than clean it. Each is
/// removed and immediately re-initialised, which is the same value the application started with and
/// costs nothing a fresh boot does not.
///
/// # The list is checked against the table
///
/// A hand-written list of forty types is exactly the thing that goes stale, which is the failure
/// this whole file is about. `the_door_resets_what_it_says_it_owns` compares this macro's types
/// against every `Ownership::Door` row in [`OWNERSHIP`] and fails on either half of a drift — a type
/// classified `Door` and not reset, or reset and not classified.
///
/// A type only one cargo feature registers carries that feature's `#[cfg]`, here and on its
/// [`OWNERSHIP`] row, so either build compares the list and the table it actually has.
macro_rules! door_state {
    ($($(#[$attr:meta])* $t:ty),* $(,)?) => {
        /// **The same reset, for a test** that opens a door and asks whether a resource went back.
        pub fn reset_door_state_for_test(world: &mut World) {
            reset_door_state(world);
        }

        pub(crate) fn reset_door_state(world: &mut World) {
            $(
                $(#[$attr])*
                {
                    world.remove_resource::<$t>();
                    world.init_resource::<$t>();
                }
            )*
        }

        /// The types [`reset_door_state`] puts back, by type path, for the ratchet.
        pub fn door_state_type_paths() -> Vec<&'static str> {
            vec![$( $(#[$attr])* std::any::type_name::<$t>() ),*]
        }
    };
}

door_state![
    crate::build::TileHistory,
    crate::build::Build,
    crate::editor::EditorState,
    crate::editor::MapFraming,
    crate::editor::CloneDrag,
    crate::editor::MoveDrag,
    crate::editor::PlaceDrag,
    crate::editor::RemovalDrag,
    crate::editor::NoteBox,
    // **The note a reveal selected** (AF-03, #359): it names a row of *this map's* `.notes.ron`, so a
    // selection carried across a door would name a note the next map does not have.
    crate::editor::SelectedNote,
    crate::editor::NotesFilter,
    // **The outliner's eye** (UI-51, #237): it holds ids of *this map's* placements, so another map's rows
    // must not inherit them.
    crate::outliner::Eyes,
    crate::editor::RunDrag,
    crate::editor::RunGhostDrag,
    crate::editor::FineAnchor,
    crate::editor::Proposal,
    crate::editor::Rung,
    crate::snip::SnipEdit,
    crate::report::ReportEdit,
    crate::editor::SizeEdit,
    crate::editor::FamilyEdit,
    crate::editor::DataEdit,
    crate::editor::TerrainEdit,
    crate::editor::CrowdEdit,
    crate::balloon::BalloonPreview,
    crate::editor::Folded,
    crate::editor::StampPicture,
    crate::editor::TargetLock,
    crate::editor::UnderCursor,
    crate::thumbs::ThumbOrder,
    crate::jobs::Jobs,
    crate::editor::EdgeFaults,
    crate::tiles::ImportState,
    crate::tiles::CellEdit,
    crate::tiles::DerivedEdges,
    crate::tiles::HeightEdit,
    crate::tiles::LatticePick,
    // **A question belongs to the door it was asked on.** A presentation names the operations it
    // covers, and the undo it offers walks them on the *open* map — so carried across a door
    // change it would answer about a map the person never saw, which is the stale-value class
    // this list exists for.
    crate::present::Presenting,
    // **The answer to that question goes with it**: a button pressed on one door and not yet taken must not answer the next door's question.
    crate::present::PresentationReply,
    crate::tiles::MapView,
    crate::tiles::NoteEdit,
    crate::tiles::ScaleEdit,
    crate::tiles::StagedLift,
    crate::tiles::LibraryCursor,
    crate::tiles::AutoScanned,
    crate::tiles::DrawnRows,
    crate::compose::ComposeState,
    crate::compose::Budget,
    crate::compose::StagedCarousel,
    crate::anim_tab::BenchState,
    crate::anim_tab::AdoptExclude,
    crate::anim_stage::BenchAb,
    crate::anim_stage::BenchCamera,
    crate::anim_stage::BenchScrub,
    crate::anim_watch::MeasureQueue,
    crate::labels::LabelQueue,
    crate::filter::Filters,
    crate::notice::Showing,
    crate::notice::Toast,
    crate::badges::ShowingFor,
    crate::view::Rig,
    crate::token_prompt::TokenPrompt,
    // The Environment panel's held hour (#98): what this door's preview is showing, released with it.
    #[cfg(feature = "bevy_elements")]
    crate::environment::ClockScrub,
    // The light map's shown/hidden and its last bake (#103): a view of this door's map, released with it.
    #[cfg(feature = "bevy_light_grid")]
    crate::light_overlay::LightOverlay,
    // The inspector's pose view (UI-50, #236): which entity gooey's reflect inspector edits, which is
    // this door's selection, so a door change starts afresh.
    crate::inspector_view::PoseInspector,
    // The audio panel's buses, cues and selection (S-01): a view of this door's project's sound.
    #[cfg(feature = "bevy_siren")]
    crate::audio_panel::AudioPanel,
];

/// **Despawn every root a screen could have spawned.** The one implementation, called by both
/// screens' teardowns.
///
/// It was two: this and the page's own teardown, which spelled the same reachability rule as its
/// own query — and the copies drifted the first time the rule gained a clause. `crate::surface` was
/// excluded from [`scene_roots`] and swept away by the other one anyway, which left the editor with
/// no camera for its interface, seventy-six UI nodes computing a target of nothing, and a window
/// that still drew the world because the map camera holds the image handle directly. Nothing logged.
///
/// So the rule lives in [`scene_roots`] and nobody writes it a second time.
pub(crate) fn despawn_scene(world: &mut World) {
    for e in scene_roots(world) {
        if let Ok(entity) = world.get_entity_mut(e) {
            entity.despawn();
        }
    }
}

/// **Every root a screen could have spawned** — and nothing else.
///
/// `Or<(With<Transform>, With<Node>)>` rather than "everything without a parent", and the difference
/// is a crash: the broad version despawned Bevy's own bookkeeping entities — observers and the like —
/// and the next command touching one panicked with *"the entity with ID 0v0 is invalid; its index
/// now has generation 1"*. A screen only ever spawns things in the world or things on the screen, so
/// those two components are the whole surface, and anything the engine keeps for itself has neither.
///
/// Windows and monitors are excluded because they outlive both screens; that is the one thing here
/// that is genuinely shared.
pub(crate) fn scene_roots(world: &mut World) -> Vec<Entity> {
    world
        .query_filtered::<Entity, (
            Or<(With<Transform>, With<Node>)>,
            Without<ChildOf>,
            Without<Window>,
            Without<bevy::window::Monitor>,
            // **And the surface, for the same reason.** It is not something a screen made — it is
            // how this application draws at all, spawned once at `Startup` and shared by both
            // screens. Sweeping it away on a door change left the next screen with no camera to
            // render into and no clue why.
            Without<crate::surface::SurfaceRig>,
            // **And the guide card, which is the same shape and the same mistake.** It is spawned
            // once in `Startup` by `bevy_debugger_bevy::GuideOverlayPlugin` and never respawned, so
            // one sweep took the agent-to-human channel down for the rest of the process:
            // `paint_guide` then iterates nothing, `tick_guide` goes on scoring checkpoints, and
            // `guide` answers OK over a card that is not on screen. Every launch reached it —
            // `--door` transitions out of the menu on the first frame.
            NotTheGuideCard,
        )>()
        .iter(world)
        .collect()
}

/// The guide overlay's exclusion from [`scene_roots`], in the one shape that survives the feature
/// being off: an empty filter matches everything, which is what "there is no card" means.
#[cfg(feature = "debugger")]
type NotTheGuideCard = Without<bevy_debugger_bevy::GuideOverlay>;
#[cfg(not(feature = "debugger"))]
type NotTheGuideCard = ();

/// **What a door change does to a resource** — the answer, for every one of them, in one place.
///
/// # Why this exists
///
/// `screen.rs` used to claim, a few lines up, that *"leaving a door despawns everything it made and
/// drops the project"*, and defend it: *"A reload cannot be wrong; a partial teardown can be,
/// silently, and the bug lands weeks later looking like something else."*
///
/// **That is not what the code does**, measured on 2026-08-17 and unchanged since: entities are
/// swept by reachability and four resources are named, and the other **fifty-six are not touched at
/// all**. Nothing resets them on entry either — every `OnEnter(Editor)` system is a spawn. So the
/// door change is *already* the partial teardown the comment is spent avoiding, unnamed and
/// unchecked, and the bug class it warns about is already open: edit a tile in kit A, leave, open
/// kit B, and A's undo stack is there to be replayed into B.
///
/// So the list is the deliverable. **This changes no behaviour** — it is `docs/2026-08-17-one-application.md`
/// §6 step 1, the step that makes the rest safe and that stands alone if the rest is dropped. What
/// it buys today is that a new resource is a deliberate answer to "what happens to this when the
/// door changes", asked when it is added rather than three weeks later when its stale value
/// surfaces as something else.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ownership {
    /// Derived from files on disk. Kept while the kit is unchanged, reloaded when it is not.
    Project,
    /// This door's own working state — selections, drags, edit buffers, undo stacks. **Reset.**
    Door,
    /// True for as long as the application runs: caches, generations, input, the surface. Kept.
    Session,
}

/// **Every resource the editor's own plugins register, and what a door does to it.**
///
/// Keyed by the full type path, because two crates may name a type alike and the ratchet compares
/// against what the world reports. Unsure cases are classified [`Ownership::Door`] on purpose: it is
/// the class that matches what a full teardown would do, so a wrong guess here is conservative
/// rather than a stale value nobody looks for.
pub const OWNERSHIP: &[(&str, Ownership)] = &[
    // ── What was opened, and what was read off disk for it ──────────────────────────────────────
    ("bevy_cauldron::project::Project", Ownership::Project),
    // **`Session`: it is the one thing that says which project the asset server reads**, moved by
    // `project_assets::open_project` and by nothing a door change does.
    // **The wish queue is the editor session's, not a door's** (#353, review M4). A wish is what the
    // editor has learned about its own tools in this sitting, and a hand-back it cites can be on any
    // door — carried across, both stay readable, which is the rule the issue states: "the queue lives
    // in the editor session and nothing is lost".
    //
    // **What a door change used to break was the ids**, and that is fixed at the source:
    // `HandBacks::asked` numbers presentations for the whole session, and a note's key carries the
    // map it is on, so the one-wish rule cannot read two hand-backs as one.
    ("bevy_cauldron::present::HandBacks", Ownership::Session),
    ("bevy_cauldron::present::Wishes", Ownership::Session),
    ("bevy_cauldron::ui_budget::UiFrame", Ownership::Session),
    // **`Session`, for the queue's own reason**: which rows the person has unfolded, and what GitHub
    // said about the wishes in them, are facts about this sitting's wishes — the queue outlives a door
    // change, so the rows that read it must too or a wish would be folded again on every crossing.
    ("bevy_cauldron::wish_view::WishOpenRows", Ownership::Session),
    ("bevy_cauldron::wish_view::WishSearches", Ownership::Session),
    // **`Session`**: whether this run asks GitHub nothing is the run's own fact, true for as long as
    // the app runs (a test inserts it once and every row reads it).
    ("bevy_cauldron::wish_view::WishSearchOff", Ownership::Session),
    ("bevy_cauldron::project_assets::ProjectRoot", Ownership::Session),
    // The editor's face is installed over the default font id once and lives as long as the app does (`harness::install_font_in`).
    ("bevy_cauldron::harness::EditorFont", Ownership::Session),
    ("bevy_cauldron::project::OpenMap", Ownership::Project),
    ("bevy_cauldron::tiles::Door", Ownership::Project),
    ("bevy_cauldron::tiles::Mode", Ownership::Project),
    // Derived from the kit's meshes: a different kit means different thumbnails.
    ("bevy_cauldron::thumbs::Thumbnails", Ownership::Project),
    (
        "bevy_cauldron::thumbs::ThumbGeneration",
        Ownership::Project,
    ),
    (
        "bevy_cauldron::labels::Suggestions",
        Ownership::Project,
    ),
    // **`Project`, because the model is read off the project's own `.env`.** A different project may
    // configure a different endpoint, and the resource latches that it has read one — so carrying it
    // across would leave the band naming a model this project never asked for. The link state goes
    // with it, which is right: it is evidence about a round trip that was made for that project.
    ("bevy_cauldron::labels::Labeler", Ownership::Project),
    (
        "bevy_cauldron::anim_watch::BenchReports",
        Ownership::Project,
    ),
    (
        "bevy_cauldron::anim_watch::RigWatch",
        Ownership::Project,
    ),
    // **The file watcher's stamps, and they are `Project` because the files are the project's.**
    // A door change can land on a different project — the chooser's own door is a different
    // project — and the stamps are the watcher's memory of *those* files. Carried across, the
    // watcher's first tick on the new project would compare the new files' stamps against the
    // old project's and see every one of them as changed.
    ("bevy_cauldron::watch::Watched", Ownership::Project),
    // **And the tick's own scratch, for the same reason** — it names the files that changed
    // since the last tick, and those are the previous project's files.
    ("bevy_cauldron::watch::Due", Ownership::Project),
    // **`Session`, and that is the whole point of it.** Every other problem list in this editor is a
    // tab's working state, capped and cleared by `Esc`; this one answers *"what has gone wrong since
    // the application started"*, which a value thrown away with the door cannot.
    ("bevy_cauldron::chrome::Journal", Ownership::Session),
    // Chords an agent or the command palette has asked to press: a key one frame from going down belongs to
    // whoever is at the keyboard, not to the map the door is leaving.
    ("bevy_cauldron::command_run::Chords", Ownership::Session),
    // **`Session`: it is recomputed from the pointer every frame.** `OverUi` answers "is a panel under
    // the cursor" for the gesture drivers, and the cursor belongs to whoever is at the machine rather
    // than to the map a door is leaving. Deriving it again costs one `contains_point` per hovered node.
    ("bevy_cauldron::view::OverUi", Ownership::Session),
    // The agent operations whose checks are owed: a fact about this project's log.
    (
        "bevy_cauldron::after_edit::AfterEditQueue",
        Ownership::Project,
    ),
    // The channels the check threads and screenshots answer on, and operations waiting for a picture:
    // plumbing true for as long as the app runs, never a value read off a project.
    ("bevy_cauldron::after_edit::Reports", Ownership::Session),
    ("bevy_cauldron::after_edit::Shots", Ownership::Session),
    ("bevy_cauldron::after_edit::ShotWait", Ownership::Session),
    ("bevy_cauldron::after_edit::PictureMemory", Ownership::Session),
    // ── The door's own working state ────────────────────────────────────────────────────────────
    // The undo stack is the one this list was written for: `docs/2026-08-17-one-application.md` §1
    // names it as the value that survives into the next kit today.
    ("bevy_cauldron::build::TileHistory", Ownership::Door),
    ("bevy_cauldron::build::Build", Ownership::Door),
    ("bevy_cauldron::editor::EditorState", Ownership::Door),
    ("bevy_cauldron::editor::MapFraming", Ownership::Door),
    ("bevy_cauldron::editor::CloneDrag", Ownership::Door),
    ("bevy_cauldron::editor::MoveDrag", Ownership::Door),
    ("bevy_cauldron::editor::PlaceDrag", Ownership::Door),
    ("bevy_cauldron::editor::RemovalDrag", Ownership::Door),
    ("bevy_cauldron::editor::NoteBox", Ownership::Door),
    ("bevy_cauldron::editor::SelectedNote", Ownership::Door),
    ("bevy_cauldron::editor::NotesFilter", Ownership::Door),
    // **The eye of the outliner** (UI-51, #237): the pieces a person hid are rows of the map that was open.
    ("bevy_cauldron::outliner::Eyes", Ownership::Door),
    ("bevy_cauldron::editor::RunDrag", Ownership::Door),
    ("bevy_cauldron::editor::RunGhostDrag", Ownership::Door),
    ("bevy_cauldron::editor::FineAnchor", Ownership::Door),
    ("bevy_cauldron::editor::Proposal", Ownership::Door),
    // **`Session`, and that is the entire reason the resource exists.** `EditorState` — where the
    // counter used to live as `seed` — is rebuilt on every door crossing, while the thing comparing
    // against it (`sync_proposal_ghosts`' `Local`) is not. A counter that a door can reset is a
    // counter that eventually reports a proposal as already drawn. See `ProposalClock`'s own doc.
    ("bevy_cauldron::editor::ProposalClock", Ownership::Session),
    ("bevy_cauldron::editor::Rung", Ownership::Door),
    ("bevy_cauldron::editor::SizeEdit", Ownership::Door),
    ("bevy_cauldron::editor::FamilyEdit", Ownership::Door),
    ("bevy_cauldron::editor::DataEdit", Ownership::Door),
    ("bevy_cauldron::editor::TerrainEdit", Ownership::Door),
    ("bevy_cauldron::editor::CrowdEdit", Ownership::Door),
    ("bevy_cauldron::balloon::BalloonPreview", Ownership::Door),
    // The game's newest verdict on a reload, read from `.live/game.ron`: true for as long as the app runs.
    ("bevy_cauldron::live::LiveGame", Ownership::Session),
    // **The editor's own verdicts on the files it draws from** (#200): true for as long as the app runs, like the
    // game's; `sync_terrain_view` forgets and re-decides them whenever one of its files or its map changes.
    ("bevy_cauldron::live::EditorReloads", Ownership::Session),
    // **Derived from the open map's notes file**: the last good notes the signs and the panel draw (#200). Another map's file starts it afresh.
    ("bevy_cauldron::notes_view::NotesView", Ownership::Project),
    // **Derived from the open project's `checks.ron`**: the last good checks the after-edit run asks (#200). Another project's file starts it afresh.
    ("bevy_cauldron::checks_view::ChecksView", Ownership::Project),
    // **Derived from the open project's `oplog.jsonl`**: what the editor last found when it read the log (#200). Another project's log starts it afresh.
    ("bevy_cauldron::oplog_view::OplogView", Ownership::Project),
    // **Derived from the open project's `data.ron` and the files it lists**: what the editor last found when it read them (#200). Another project's registry starts it afresh.
    ("bevy_cauldron::data_view::DataView", Ownership::Project),
    // **Derived from the open map's `references.ron` and the pictures it lists**: what the editor last found when it read their headers (#200). Another map or project starts it afresh.
    ("bevy_cauldron::reference_view::ReferenceView", Ownership::Project),
    ("bevy_cauldron::family_host::FamilySettings", Ownership::Session),
    // **The editor's settings** (UI-70): the defaults, the user's file and the project's laid over them. The user's layer is the app's and the project's layer follows the open project's path (`sync_settings` starts afresh when it moves), so both are true for as long as the app runs.
    ("bevy_cauldron::settings::EditorSettings", Ownership::Session),
    ("bevy_cauldron::settings::SettingsView", Ownership::Session),
    // **`Door`, like the camera it sits beside**: the hour the Environment panel's clock buttons
    // hold the preview at (#98). It is a view of the open project, never written to its file, so a
    // door change lets the next project's own clock run.
    #[cfg(feature = "bevy_elements")]
    ("bevy_cauldron::environment::ClockScrub", Ownership::Door),
    #[cfg(feature = "bevy_light_grid")]
    ("bevy_cauldron::light_overlay::LightOverlay", Ownership::Door),
    // **`Door`, like the selection it mirrors**: the entity the inspector edits belongs to the open
    // project's map, and another door's map is another selection.
    ("bevy_cauldron::inspector_view::PoseInspector", Ownership::Door),
    // **`Session`**: where the person is looking, not this door's state. A door change keeps it, and the
    // pieces that are not on the new map are dropped the next frame.
    // **`Session`**: whether the agent may write is the person's call about the whole sitting, not a door's.
    ("bevy_cauldron::familiar::AgentPause", Ownership::Session),
    ("bevy_cauldron::familiar::AgentActivity", Ownership::Session),
    // **`Session`**: which client is talking is true for the whole sitting, not for one door — the
    // agent that made a wish about the last map is still the agent after a crossing.
    ("bevy_cauldron::present::AgentClient", Ownership::Session),
    // **`Session`**: whether hand-backs are answered in the AGENT block is the person's choice for the sitting, like the pause.
    ("bevy_cauldron::familiar::HandBackHere", Ownership::Session),
    // **`Session`, and it has to be**: a follow raised while the Kit door is open asks for the Map,
    // and the crossing that answers it resets the door's own state — so a request kept in a `Door`
    // resource would be erased by the very crossing it asked for. `Camera state only`, like `Rig`,
    // but it outlives the door for that one reason.
    ("bevy_cauldron::follow::Following", Ownership::Session),
    // **`Session`, and for `Following`'s exact reason**: a reveal raised on the Kit door asks the
    // editor to open the Map, and the crossing that answers it resets the door's own state — so a
    // request kept in a `Door` resource would be erased by the very crossing it asked for. AF-03, #359.
    ("bevy_cauldron::reveal::Revealing", Ownership::Session),
    // **`Session`**: which inspector heading was last brought into view is where the person's eye
    // was sent, not a fact about this door's documents.
    ("bevy_cauldron::reveal::InspectorReveal", Ownership::Session),
    // **`Session`**, derived from `Following` every frame: a chip's sentence is about the sitting's
    // agents, not about this door's documents, and it is rebuilt on the next frame from a queue that
    // is itself `Session` — a `Door` classification would blank it for one frame for no reason.
    ("bevy_cauldron::follow::LookChip", Ownership::Session),
    // **`Session`**: what the person has hidden of the shell (the left panel, Zen mode) is their call for the sitting, not a door's;
    // the next door's frame is folded the same way the frame of this one was.
    ("bevy_cauldron::shell_view::ShellView", Ownership::Session),
    ("bevy_cauldron::selection::Selection", Ownership::Session),
    // **`Door`, like the light map**: the audio panel's buses, cue names and selection are read from
    // this door's `family/bevy_siren.ron`, and another project's sound is another file.
    #[cfg(feature = "bevy_siren")]
    ("bevy_cauldron::audio_panel::AudioPanel", Ownership::Door),
    // **`Session`, not `Door`**: the icon sheet is decoded from the binary (`icons::ATLAS_PNG`) once and
    // its handles live in `Assets`, which outlives a door crossing. Decoding it again on every door
    // would leak one image per crossing.
    ("bevy_cauldron::toolbar::ToolbarAtlas", Ownership::Session),
    ("bevy_cauldron::editor::Folded", Ownership::Door),
    ("bevy_cauldron::editor::StampPicture", Ownership::Door),
    ("bevy_cauldron::editor::TargetLock", Ownership::Door),
    ("bevy_cauldron::editor::UnderCursor", Ownership::Door),
    // The palette's own order, rewritten whenever it rebuilds: what the booth bakes first is a fact
    // about what this door is showing, not about the kit.
    ("bevy_cauldron::thumbs::ThumbOrder", Ownership::Door),
    // **`Door`, because a job is this door's work** (UI-22, #223). A bake carried across a door change
    // would name a portrait of a project nobody is looking at, and a stop request would land on a job
    // that is no longer the one holding the booth.
    ("bevy_cauldron::jobs::Jobs", Ownership::Door),
    ("bevy_cauldron::editor::EdgeFaults", Ownership::Door),
    ("bevy_cauldron::tiles::ImportState", Ownership::Door),
    ("bevy_cauldron::tiles::CellEdit", Ownership::Door),
    ("bevy_cauldron::tiles::DerivedEdges", Ownership::Door),
    ("bevy_cauldron::tiles::HeightEdit", Ownership::Door),
    ("bevy_cauldron::tiles::LatticePick", Ownership::Door),
    ("bevy_cauldron::present::Presenting", Ownership::Door),
    ("bevy_cauldron::present::PresentationReply", Ownership::Door),
    ("bevy_cauldron::tiles::MapView", Ownership::Door),
    ("bevy_cauldron::tiles::NoteEdit", Ownership::Door),
    ("bevy_cauldron::tiles::ScaleEdit", Ownership::Door),
    ("bevy_cauldron::tiles::StagedLift", Ownership::Door),
    // The library row you were last on. `Door`, because a different kit's ids mean nothing here —
    // a remembered `furniture/sofa` pointing into `site` would land the cursor on nothing.
    ("bevy_cauldron::tiles::LibraryCursor", Ownership::Door),
    // What the automatic scan has done and what the list is drawn from: both keyed by mesh path
    // or row text, and two kits sharing either would have the second one's first frame silently
    // skipped — the `badges::ShowingFor` hazard, one panel over.
    ("bevy_cauldron::tiles::AutoScanned", Ownership::Door),
    ("bevy_cauldron::tiles::DrawnRows", Ownership::Door),
    ("bevy_cauldron::compose::ComposeState", Ownership::Door),
    ("bevy_cauldron::compose::Budget", Ownership::Door),
    (
        "bevy_cauldron::compose::StagedCarousel",
        Ownership::Door,
    ),
    ("bevy_cauldron::anim_tab::BenchState", Ownership::Door),
    (
        "bevy_cauldron::anim_tab::AdoptExclude",
        Ownership::Door,
    ),
    ("bevy_cauldron::anim_stage::BenchAb", Ownership::Door),
    (
        "bevy_cauldron::anim_stage::BenchCamera",
        Ownership::Door,
    ),
    (
        "bevy_cauldron::anim_stage::BenchScrub",
        Ownership::Door,
    ),
    (
        "bevy_cauldron::anim_watch::MeasureQueue",
        Ownership::Door,
    ),
    ("bevy_cauldron::labels::LabelQueue", Ownership::Door),
    ("bevy_cauldron::filter::Filters", Ownership::Door),
    ("bevy_cauldron::snip::SnipEdit", Ownership::Door),
    // **A report cannot survive the door change it may be reporting on.** It holds a frozen frame
    // of the old door and entity ids from its layout, so carrying it across would record the screen
    // a person has already left.
    ("bevy_cauldron::report::ReportEdit", Ownership::Door),
    ("bevy_cauldron::notice::Showing", Ownership::Door),
    // **`Door`, because it is what is on screen right now.** A toast is an event's tail — carrying
    // one into the next kit would announce a refusal about a door the author has already left.
    ("bevy_cauldron::notice::Toast", Ownership::Door),
    // Holds entity ids from the frame this screen spawned, so it is meaningless the moment those
    // are despawned — and `chrome::spawn_frame` replaces it on every entry.
    ("bevy_cauldron::chrome::Frame", Ownership::Door),
    // The badge layer's entities go with the door, so the key that says which tab they were
    // built for has to go with them — a stale key would suppress the first rebuild on the next
    // kit and the badges would simply never appear.
    ("bevy_cauldron::badges::ShowingFor", Ownership::Door),
    // What the live tab is drawing into the viewport, re-derived every frame in `Phase::Sense` —
    // so a stale value survives no frame and this classification changes nothing. `Door` all the
    // same: it describes the door's own world, and a resource that says "the Map's cube is here"
    // while the Kit door is open would be a lie for exactly as long as it took to notice.
    ("bevy_cauldron::view::Rig", Ownership::Door),
    // A half-typed token draft is the door's working state — a draft naming a token for one
    // kit's vocabulary is meaningless standing in another.
    (
        "bevy_cauldron::token_prompt::TokenPrompt",
        Ownership::Door,
    ),
    // ── True for as long as the application runs ────────────────────────────────────────────────
    // The surface is how this application DRAWS — see `crate::surface`, and `scene_roots`, which
    // excludes its entities for the same reason.
    ("bevy_cauldron::surface::Surface", Ownership::Session),
    // **The sequencer's own state is `Session`, and the reason is the same as the surface's.**
    // A capture in flight is bound to *this* application — to the renderer that can read the
    // pixels back and to the clock that is counting the frames. **A door change does not
    // invalidate it**: the run carries on, and the frames it takes are of the editor as it is,
    // which is the point of the run. The run's *work* — the operation it applies — is dispatched
    // through the operation registry against the map that is open, and the map is `Project`, so
    // the map it acts on is the one the door change reloads.
    (
        "bevy_cauldron::sequencer::Sequencer",
        Ownership::Session,
    ),
    // **And whether this application can write a frame at all** — settled once, at startup, from
    // the render world's presence. It cannot change while the application runs, so it is
    // `Session` for the same reason: a door change does not make a headless editor able to
    // render.
    (
        "bevy_cauldron::sequencer::CanRender",
        Ownership::Session,
    ),
    ("bevy_cauldron::keys::Live", Ownership::Session),
    ("bevy_cauldron::keys::Repeat", Ownership::Session),
    // **The one prompt outlives a door on purpose**, because it is asked on both sides of one:
    // `chooser` raises delete and quit on the menu, `editor` raises the leaving question from
    // inside a door, and the resource that carries them cannot belong to either. Its *panel* is
    // per-screen (`ConfirmPlugin` spawns on both `OnEnter`s) because `scene_roots` sweeps roots on
    // a screen change — the state is session-wide, the pixels are not.
    ("bevy_cauldron::confirm::Confirm", Ownership::Session),
    ("bevy_cauldron::view::Pointer", Ownership::Session),
    // Persisted to disk under `target/`, so that STALE is truthful at startup rather than after the
    // bench's first audit.
    (
        "bevy_cauldron::anim_cache::BenchCache",
        Ownership::Session,
    ),
    (
        "bevy_cauldron::anim_watch::BenchGeneration",
        Ownership::Session,
    ),
    (
        "bevy_cauldron::anim_plots::BenchPlots",
        Ownership::Session,
    ),
    (
        "bevy_cauldron::anim_stage::GhostMaterial",
        Ownership::Session,
    ),
    (
        "bevy_cauldron::label_booth::ShotRig",
        Ownership::Session,
    ),
    (
        "bevy_cauldron::labels::LabelGeneration",
        Ownership::Session,
    ),
    // In-flight vision-model work. Dropping it mid-request would strand the task, not cancel it.
    ("bevy_cauldron::labels::LabelTasks", Ownership::Session),
    // **The armed composition, and the same answer.** It is the author's intent — *which
    // composition the next click stamps* — and it is a name, not a cursor. Measured on 2026-10-21:
    // the COMPOSE tab armed a house and said *"the map tab stamps it"*, and there was no map tab
    // in that process, because the door is chosen at startup. The author's only route to a stamp
    // was to leave, come back through the menu, re-open the map, and **re-arm the composition by
    // hand** — the arm had been reset with the rest of the door's working state. Sixteen house
    // compositions were defined in Sanctuary's project and not one was stamped on either map.
    //
    // `Door` here would be the class that matches what a full teardown does, and the teardown is
    // right about everything *except* this: a door change is not a new author. See
    // `compose::Armed`.
    ("bevy_cauldron::compose::Armed", Ownership::Session),
    // **The composition picker's cursor, and the same answer.** It is the author's cursor in the
    // author's list, and it is where the arm is going to be taken from — so it belongs with the arm
    // rather than with the door's own working state.
    (
        "bevy_cauldron::compose::picker::Picker",
        Ownership::Session,
    ),
    // **The terminal's shells, and this is the one resource the whole feature turns on.**
    //
    // `Session`, and it is the reason the terminal survives a door at all: a person starts
    // `cargo build` on the Map door, crosses to Kit, and the build is still running with its
    // scrollback intact. `Door` here would kill the child (`TerminalSession`'s `Drop` hangs it up), and
    // the cost of that is somebody's twenty-minute build.
    //
    // It is also *not* a `Node`, so [`scene_roots`]' sweep cannot reach it — a fact this table states
    // rather than leaves to be discovered by a `cargo build` that vanished.
    ("bevy_cauldron::terminal::Terminal", Ownership::Session),
    // **`Session`, because it says who is at the keyboard.** A door change does not change who is
    // sitting there; and a `Door` classification would clear the flag on the way out, which is exactly
    // what `OnExit(Editor)` already does deliberately (`give_the_keyboard_back`). Two things resetting
    // it is not a bug, but the second one is the one that means something.
    (
        "bevy_cauldron::terminal_pane::TerminalFocus",
        Ownership::Session,
    ),
    // **`Session`: the cell count the pane last asked for, the caret's blink, a drag in progress.**
    // Not `Door`, because none of it is per-project and all of it is re-derived: the resize system
    // recomputes the cells from the body's layout on the first frame of the next door, and a stale
    // count is one frame that asks the shell for the size it already has — which `resize` refuses.
    (
        "bevy_cauldron::terminal_pane::TerminalPane",
        Ownership::Session,
    ),
    // **`Session`, like the focus it refines** (#321): it says whether the terminal's find box is open
    // and has the keys. The box is a child of the pane's body, so the door's sweep takes it, and
    // `forget_a_find_box_that_went` closes the flag on the next frame; a `Door` reset would be a second
    // thing closing it.
    (
        "bevy_cauldron::terminal_pane::TerminalFind",
        Ownership::Session,
    ),
    // **`Session`: the measured advance of one glyph in the shipped face.** It is a fact about the
    // font, not about a door, and re-measuring it costs a frame of `TextLayoutInfo`.
    (
        "bevy_cauldron::terminal_view::CellMetrics",
        Ownership::Session,
    ),
    // **`Session`, and the reason is that this is a drawing cache rather than a value.**
    //
    // `Door` would mean `reset_door_state` removes and re-initialises it — one more type in a
    // forty-type macro whose list and table are compared by a ratchet, for a `Vec` that the draw
    // system overwrites the first time it finds a row whose cells differ. What the row *says* is that
    // the cache is not a fact about a project, which is the question the table asks.
    ("bevy_cauldron::terminal_view::Drawn", Ownership::Session),
];

/// What a door change does to this resource, or `None` if nobody has said.
pub fn ownership(type_path: &str) -> Option<Ownership> {
    OWNERSHIP
        .iter()
        .find(|(name, _)| *name == type_path)
        .map(|(_, class)| *class)
}
