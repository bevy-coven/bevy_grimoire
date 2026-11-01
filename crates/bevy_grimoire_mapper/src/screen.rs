//! **Which half of the editor is live** — the menu, or a door.
//!
//! # One application, two screens
//!
//! These were two processes. The chooser built its own `App`, ran to completion, launched the editor
//! as a child, and a supervisor spawned a fresh menu per lap — because **winit builds at most one
//! event loop per process**, so a second `App::run` in one process dies on `RecreationAttempt`.
//!
//! That shape was chosen for a second reason which is the one that actually mattered: `Project` is
//! inserted before the editor's plugins, and in Bevy 0.19 a **missing `Res<T>` panics its system**
//! rather than skipping it. Around a hundred parameter positions take `Res<Project>`, so a menu
//! living inside the editor's `App` meant gating every one of them — *"Gating is feasible… it is the
//! **cost** that is the argument"* (`chooser.rs`).
//!
//! Asked for at the keyboard, 2026-08-16: *"can we not open a whole another editing window? I'd like
//! to keep the same bevy application running across whether it's the UI or the editor."* So the cost
//! gets paid. This state is what pays it: every editor system runs `in_state(Screen::Editor)`, so in
//! [`Screen::Menu`] none of them run and none of them fetch a `Project` that is not there.
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
    /// The chooser: kits, maps, and what each one is.
    #[default]
    Menu,
    /// A door, with a [`crate::project::Project`] loaded and a [`crate::tiles::Door`] chosen.
    Editor,
}

/// **The plugin that owns the two transitions.**
///
/// `OnExit(Menu)` opens the project, because a state transition runs every `OnExit` before any
/// `OnEnter` — so by the time the editor's own spawns run, `Project` is already in the World and the
/// hundred systems that take it are not looking at an empty slot.
///
/// `OnExit(Editor)` puts everything back: entities despawned, resources dropped. See the module note
/// on why that is a full reload rather than a swap.
pub struct ScreenPlugin;

impl Plugin for ScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<Screen>()
            .add_systems(OnExit(Screen::Menu), open_the_door)
            .add_systems(OnExit(Screen::Editor), close_the_door);
    }
}

/// Put what the menu opened into the World. An exclusive system, so the resources exist the instant
/// this returns rather than whenever a command queue is next flushed — `OnEnter(Editor)` is the very
/// next schedule and every system in it assumes a loaded project.
///
/// **This cannot fail, and that is the point.** It used to call `args::open` here and, on an error,
/// write the reason onto the chooser and set `NextState(Menu)`. That never worked: a
/// `StateTransition` runs `ExitSchedules` then `EnterSchedules` in one pass, so the `NextState` was
/// not read until the following pass and `OnEnter(Editor)` ran regardless — against a world with no
/// `Project` and no `Door`, which panics rather than returning. The open moved to
/// `chooser::drive_chooser` and to `main`, both of which can refuse in place; what arrives here is a
/// door already open. See [`crate::chooser::Chosen`].
fn open_the_door(world: &mut World) {
    let Some(chosen) = world.remove_resource::<crate::chooser::Chosen>() else {
        // Reached only when something set the state directly. Loud, because the alternative is an
        // editor with no project and a hundred systems finding out one at a time.
        error!("entering a door with nothing chosen — staying on the menu");
        if let Some(mut next) = world.get_resource_mut::<NextState<Screen>>() {
            next.set(Screen::Menu);
        }
        return;
    };
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
/// **And the chooser never appears.** A same-state transition runs neither `OnExit(Menu)` nor
/// `OnEnter(Menu)`, so `ChooserPlugin`'s `build_chooser` / `spawn_screen` / `spawn_menu_bars` never
/// run. The author sees the editor tear down and the other door build, in one frame.
///
/// The alternative — hopping through `Screen::Menu` — would build the whole chooser UI and tear it
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
    // **The chooser never appears.** A same-state transition runs neither `OnExit(Menu)` nor
    // `OnEnter(Menu)`, so `ChooserPlugin`'s `build_chooser` / `spawn_screen` / `spawn_menu_bars`
    // never run. The author sees the editor tear down and the other door build, in one frame.
    //
    // **The alternative — hopping through `Screen::Menu` — would build the whole chooser UI and
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
    crate::editor::NotesFilter,
    crate::editor::RunDrag,
    crate::editor::RunGhostDrag,
    crate::editor::FineAnchor,
    crate::editor::Proposal,
    crate::editor::Rung,
    crate::snip::SnipEdit,
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
];

/// **Despawn every root a screen could have spawned.** The one implementation, called by both
/// screens' teardowns.
///
/// It was two: this and `chooser::tear_down_menu`, which spelled the same reachability rule as its
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
    ("bevy_grimoire_mapper::project::Project", Ownership::Project),
    ("bevy_grimoire_mapper::project::OpenMap", Ownership::Project),
    ("bevy_grimoire_mapper::tiles::Door", Ownership::Project),
    ("bevy_grimoire_mapper::tiles::Mode", Ownership::Project),
    // Derived from the kit's meshes: a different kit means different thumbnails.
    ("bevy_grimoire_mapper::thumbs::Thumbnails", Ownership::Project),
    (
        "bevy_grimoire_mapper::thumbs::ThumbGeneration",
        Ownership::Project,
    ),
    (
        "bevy_grimoire_mapper::labels::Suggestions",
        Ownership::Project,
    ),
    // **`Project`, because the model is read off the project's own `.env`.** A different project may
    // configure a different endpoint, and the resource latches that it has read one — so carrying it
    // across would leave the band naming a model this project never asked for. The link state goes
    // with it, which is right: it is evidence about a round trip that was made for that project.
    ("bevy_grimoire_mapper::labels::Labeler", Ownership::Project),
    (
        "bevy_grimoire_mapper::anim_watch::BenchReports",
        Ownership::Project,
    ),
    (
        "bevy_grimoire_mapper::anim_watch::RigWatch",
        Ownership::Project,
    ),
    // **The file watcher's stamps, and they are `Project` because the files are the project's.**
    // A door change can land on a different project — the chooser's own door is a different
    // project — and the stamps are the watcher's memory of *those* files. Carried across, the
    // watcher's first tick on the new project would compare the new files' stamps against the
    // old project's and see every one of them as changed.
    ("bevy_grimoire_mapper::watch::Watched", Ownership::Project),
    // **And the tick's own scratch, for the same reason** — it names the files that changed
    // since the last tick, and those are the previous project's files.
    ("bevy_grimoire_mapper::watch::Due", Ownership::Project),
    // **`Session`, and that is the whole point of it.** Every other problem list in this editor is a
    // tab's working state, capped and cleared by `Esc`; this one answers *"what has gone wrong since
    // the application started"*, which a value thrown away with the door cannot.
    ("bevy_grimoire_mapper::chrome::Journal", Ownership::Session),
    // The agent operations whose checks are owed: a fact about this project's log.
    (
        "bevy_grimoire_mapper::after_edit::AfterEditQueue",
        Ownership::Project,
    ),
    // The channels the check threads and screenshots answer on, and operations waiting for a picture:
    // plumbing true for as long as the app runs, never a value read off a project.
    ("bevy_grimoire_mapper::after_edit::Reports", Ownership::Session),
    ("bevy_grimoire_mapper::after_edit::Shots", Ownership::Session),
    ("bevy_grimoire_mapper::after_edit::ShotWait", Ownership::Session),
    ("bevy_grimoire_mapper::after_edit::PictureMemory", Ownership::Session),
    // ── The door's own working state ────────────────────────────────────────────────────────────
    // The undo stack is the one this list was written for: `docs/2026-08-17-one-application.md` §1
    // names it as the value that survives into the next kit today.
    ("bevy_grimoire_mapper::build::TileHistory", Ownership::Door),
    ("bevy_grimoire_mapper::build::Build", Ownership::Door),
    ("bevy_grimoire_mapper::editor::EditorState", Ownership::Door),
    ("bevy_grimoire_mapper::editor::MapFraming", Ownership::Door),
    ("bevy_grimoire_mapper::editor::CloneDrag", Ownership::Door),
    ("bevy_grimoire_mapper::editor::MoveDrag", Ownership::Door),
    ("bevy_grimoire_mapper::editor::PlaceDrag", Ownership::Door),
    ("bevy_grimoire_mapper::editor::RemovalDrag", Ownership::Door),
    ("bevy_grimoire_mapper::editor::NoteBox", Ownership::Door),
    ("bevy_grimoire_mapper::editor::NotesFilter", Ownership::Door),
    ("bevy_grimoire_mapper::editor::RunDrag", Ownership::Door),
    ("bevy_grimoire_mapper::editor::RunGhostDrag", Ownership::Door),
    ("bevy_grimoire_mapper::editor::FineAnchor", Ownership::Door),
    ("bevy_grimoire_mapper::editor::Proposal", Ownership::Door),
    // **`Session`, and that is the entire reason the resource exists.** `EditorState` — where the
    // counter used to live as `seed` — is rebuilt on every door crossing, while the thing comparing
    // against it (`sync_proposal_ghosts`' `Local`) is not. A counter that a door can reset is a
    // counter that eventually reports a proposal as already drawn. See `ProposalClock`'s own doc.
    ("bevy_grimoire_mapper::editor::ProposalClock", Ownership::Session),
    ("bevy_grimoire_mapper::editor::Rung", Ownership::Door),
    ("bevy_grimoire_mapper::editor::SizeEdit", Ownership::Door),
    ("bevy_grimoire_mapper::editor::FamilyEdit", Ownership::Door),
    ("bevy_grimoire_mapper::editor::DataEdit", Ownership::Door),
    ("bevy_grimoire_mapper::editor::TerrainEdit", Ownership::Door),
    ("bevy_grimoire_mapper::editor::CrowdEdit", Ownership::Door),
    ("bevy_grimoire_mapper::balloon::BalloonPreview", Ownership::Door),
    // The game's newest verdict on a reload, read from `.live/game.ron`: true for as long as the app runs.
    ("bevy_grimoire_mapper::live::LiveGame", Ownership::Session),
    // **The editor's own verdicts on the files it draws from** (#200): true for as long as the app runs, like the
    // game's; `sync_terrain_view` forgets and re-decides them whenever one of its files or its map changes.
    ("bevy_grimoire_mapper::live::EditorReloads", Ownership::Session),
    // **Derived from the open map's notes file**: the last good notes the signs and the panel draw (#200). Another map's file starts it afresh.
    ("bevy_grimoire_mapper::notes_view::NotesView", Ownership::Project),
    ("bevy_grimoire_mapper::family_host::FamilySettings", Ownership::Session),
    // **`Door`, like the camera it sits beside**: the hour the Environment panel's clock buttons
    // hold the preview at (#98). It is a view of the open project, never written to its file, so a
    // door change lets the next project's own clock run.
    #[cfg(feature = "bevy_elements")]
    ("bevy_grimoire_mapper::environment::ClockScrub", Ownership::Door),
    #[cfg(feature = "bevy_light_grid")]
    ("bevy_grimoire_mapper::light_overlay::LightOverlay", Ownership::Door),
    ("bevy_grimoire_mapper::editor::Folded", Ownership::Door),
    ("bevy_grimoire_mapper::editor::StampPicture", Ownership::Door),
    ("bevy_grimoire_mapper::editor::TargetLock", Ownership::Door),
    ("bevy_grimoire_mapper::editor::UnderCursor", Ownership::Door),
    // The palette's own order, rewritten whenever it rebuilds: what the booth bakes first is a fact
    // about what this door is showing, not about the kit.
    ("bevy_grimoire_mapper::thumbs::ThumbOrder", Ownership::Door),
    ("bevy_grimoire_mapper::editor::EdgeFaults", Ownership::Door),
    ("bevy_grimoire_mapper::tiles::ImportState", Ownership::Door),
    ("bevy_grimoire_mapper::tiles::CellEdit", Ownership::Door),
    ("bevy_grimoire_mapper::tiles::DerivedEdges", Ownership::Door),
    ("bevy_grimoire_mapper::tiles::HeightEdit", Ownership::Door),
    ("bevy_grimoire_mapper::tiles::LatticePick", Ownership::Door),
    ("bevy_grimoire_mapper::present::Presenting", Ownership::Door),
    ("bevy_grimoire_mapper::tiles::MapView", Ownership::Door),
    ("bevy_grimoire_mapper::tiles::NoteEdit", Ownership::Door),
    ("bevy_grimoire_mapper::tiles::ScaleEdit", Ownership::Door),
    ("bevy_grimoire_mapper::tiles::StagedLift", Ownership::Door),
    // The library row you were last on. `Door`, because a different kit's ids mean nothing here —
    // a remembered `furniture/sofa` pointing into `site` would land the cursor on nothing.
    ("bevy_grimoire_mapper::tiles::LibraryCursor", Ownership::Door),
    // What the automatic scan has done and what the list is drawn from: both keyed by mesh path
    // or row text, and two kits sharing either would have the second one's first frame silently
    // skipped — the `badges::ShowingFor` hazard, one panel over.
    ("bevy_grimoire_mapper::tiles::AutoScanned", Ownership::Door),
    ("bevy_grimoire_mapper::tiles::DrawnRows", Ownership::Door),
    ("bevy_grimoire_mapper::compose::ComposeState", Ownership::Door),
    ("bevy_grimoire_mapper::compose::Budget", Ownership::Door),
    (
        "bevy_grimoire_mapper::compose::StagedCarousel",
        Ownership::Door,
    ),
    ("bevy_grimoire_mapper::anim_tab::BenchState", Ownership::Door),
    (
        "bevy_grimoire_mapper::anim_tab::AdoptExclude",
        Ownership::Door,
    ),
    ("bevy_grimoire_mapper::anim_stage::BenchAb", Ownership::Door),
    (
        "bevy_grimoire_mapper::anim_stage::BenchCamera",
        Ownership::Door,
    ),
    (
        "bevy_grimoire_mapper::anim_stage::BenchScrub",
        Ownership::Door,
    ),
    (
        "bevy_grimoire_mapper::anim_watch::MeasureQueue",
        Ownership::Door,
    ),
    ("bevy_grimoire_mapper::labels::LabelQueue", Ownership::Door),
    ("bevy_grimoire_mapper::filter::Filters", Ownership::Door),
    ("bevy_grimoire_mapper::snip::SnipEdit", Ownership::Door),
    ("bevy_grimoire_mapper::notice::Showing", Ownership::Door),
    // **`Door`, because it is what is on screen right now.** A toast is an event's tail — carrying
    // one into the next kit would announce a refusal about a door the author has already left.
    ("bevy_grimoire_mapper::notice::Toast", Ownership::Door),
    // Holds entity ids from the frame this screen spawned, so it is meaningless the moment those
    // are despawned — and `chrome::spawn_frame` replaces it on every entry.
    ("bevy_grimoire_mapper::chrome::Frame", Ownership::Door),
    // The badge layer's entities go with the door, so the key that says which tab they were
    // built for has to go with them — a stale key would suppress the first rebuild on the next
    // kit and the badges would simply never appear.
    ("bevy_grimoire_mapper::badges::ShowingFor", Ownership::Door),
    // What the live tab is drawing into the viewport, re-derived every frame in `Phase::Sense` —
    // so a stale value survives no frame and this classification changes nothing. `Door` all the
    // same: it describes the door's own world, and a resource that says "the Map's cube is here"
    // while the Kit door is open would be a lie for exactly as long as it took to notice.
    ("bevy_grimoire_mapper::view::Rig", Ownership::Door),
    // A half-typed token draft is the door's working state — a draft naming a token for one
    // kit's vocabulary is meaningless standing in another.
    (
        "bevy_grimoire_mapper::token_prompt::TokenPrompt",
        Ownership::Door,
    ),
    // ── True for as long as the application runs ────────────────────────────────────────────────
    // The surface is how this application DRAWS — see `crate::surface`, and `scene_roots`, which
    // excludes its entities for the same reason.
    ("bevy_grimoire_mapper::surface::Surface", Ownership::Session),
    // **The sequencer's own state is `Session`, and the reason is the same as the surface's.**
    // A capture in flight is bound to *this* application — to the renderer that can read the
    // pixels back and to the clock that is counting the frames. **A door change does not
    // invalidate it**: the run carries on, and the frames it takes are of the editor as it is,
    // which is the point of the run. The run's *work* — the operation it applies — is dispatched
    // through the operation registry against the map that is open, and the map is `Project`, so
    // the map it acts on is the one the door change reloads.
    (
        "bevy_grimoire_mapper::sequencer::Sequencer",
        Ownership::Session,
    ),
    // **And whether this application can write a frame at all** — settled once, at startup, from
    // the render world's presence. It cannot change while the application runs, so it is
    // `Session` for the same reason: a door change does not make a headless editor able to
    // render.
    (
        "bevy_grimoire_mapper::sequencer::CanRender",
        Ownership::Session,
    ),
    ("bevy_grimoire_mapper::keys::Live", Ownership::Session),
    ("bevy_grimoire_mapper::keys::Repeat", Ownership::Session),
    // **The one prompt outlives a door on purpose**, because it is asked on both sides of one:
    // `chooser` raises delete and quit on the menu, `editor` raises the leaving question from
    // inside a door, and the resource that carries them cannot belong to either. Its *panel* is
    // per-screen (`ConfirmPlugin` spawns on both `OnEnter`s) because `scene_roots` sweeps roots on
    // a screen change — the state is session-wide, the pixels are not.
    ("bevy_grimoire_mapper::confirm::Confirm", Ownership::Session),
    ("bevy_grimoire_mapper::view::Pointer", Ownership::Session),
    // Persisted to disk under `target/`, so that STALE is truthful at startup rather than after the
    // bench's first audit.
    (
        "bevy_grimoire_mapper::anim_cache::BenchCache",
        Ownership::Session,
    ),
    (
        "bevy_grimoire_mapper::anim_watch::BenchGeneration",
        Ownership::Session,
    ),
    (
        "bevy_grimoire_mapper::anim_plots::BenchPlots",
        Ownership::Session,
    ),
    (
        "bevy_grimoire_mapper::anim_stage::GhostMaterial",
        Ownership::Session,
    ),
    (
        "bevy_grimoire_mapper::label_booth::ShotRig",
        Ownership::Session,
    ),
    (
        "bevy_grimoire_mapper::labels::LabelGeneration",
        Ownership::Session,
    ),
    // In-flight vision-model work. Dropping it mid-request would strand the task, not cancel it.
    ("bevy_grimoire_mapper::labels::LabelTasks", Ownership::Session),
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
    ("bevy_grimoire_mapper::compose::Armed", Ownership::Session),
    // **The composition picker's cursor, and the same answer.** It is the author's cursor in the
    // author's list, and it is where the arm is going to be taken from — so it belongs with the arm
    // rather than with the door's own working state.
    (
        "bevy_grimoire_mapper::compose::picker::Picker",
        Ownership::Session,
    ),
];

/// What a door change does to this resource, or `None` if nobody has said.
pub fn ownership(type_path: &str) -> Option<Ownership> {
    OWNERSHIP
        .iter()
        .find(|(name, _)| *name == type_path)
        .map(|(_, class)| *class)
}
