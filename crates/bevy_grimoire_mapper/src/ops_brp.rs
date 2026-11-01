//! **The agent's door into the editor** — the same operations, over BRP.
//!
//! # Why this module exists
//!
//! The editor's verbs lived in **key handlers**: a `KeyboardInput` message, a match on the key,
//! a write to some state. **An agent has no keyboard.** It could reach the editor over BRP —
//! `bevy_debugger/input` injects into `ButtonInput`, so a key *can* be synthesised — but that is a
//! simulation of a person, and the brief's rule is explicit: *"Never drive the UI by clicking."*
//!
//! So this module registers **typed commands** with the debugger, and each one dispatches an
//! operation through [`crate::ops::dispatch`] — **the same function the key handlers call**.
//! There is no second implementation of anything.
//!
//! # What a command looks like
//!
//! ```text
//! grimoire/operation_list     — every operation, its schema, and how it is undone
//! grimoire/operation_apply    — run one, by name, with its parameters
//! grimoire/operation_preview  — what one would change, and nothing changed
//! grimoire/oplog_read         — the log, as lines
//! ```
//!
//! **The names are `grimoire/`-prefixed because the debugger reserves its own.** A command that
//! took a name the debugger used would be refused at registration — and it would be refused by a
//! **panic**, so the name is checked by `tests/the_operations_have_two_front_ends.rs`.
//!
//! # The shape of an apply
//!
//! **A preview is the default for an operation that changes the map's own shape**, because
//! resizing a map is wide in scope: it moves the ground every piece stands on. **An agent that
//! means it passes `dry_run: false`.**
//!
//! **Every apply returns the operation's id**, so the agent can cite it when it presents the
//! change and the human can revert by it.

use bevy::prelude::*;
use serde::Deserialize;
use serde_json::{Value, json};

use bevy_grimoire_core::descriptor::{Axis, By, Confidence, Descriptor};
use crate::ops::{self, Ctx, Source};

/// **The parameters of `grimoire/operation_apply`.**
///
/// **Everything in one struct**, and `deny_unknown_fields`, so a typo is a refusal rather than a
/// field silently ignored. An agent that misspelled `parameters` would otherwise be told it had
/// applied an operation with no parameters, and it would believe it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyParams {
    /// **Which operation.**
    pub operation: String,
    /// **Its parameters**, as the operation's own schema describes them.
    #[serde(default)]
    pub parameters: Value,
    /// **Whether to actually do it.** `true` previews and changes nothing.
    ///
    /// **The default is `true` here and `false` on the MCP tool**, and the difference is
    /// deliberate. **The BRP surface is a raw door**, and a raw door that changed the editor by
    /// default would be a door an agent could fall into; **the MCP tool is the door with a
    /// description in front of it**, and an agent that reads it knows what it is asking for.
    #[serde(default = "yes")]
    pub dry_run: bool,
    /// **Who is asking, when the caller holds a note (#92).** The name the note was claimed under.
    /// An operation that would change anything outside that note's area plus its margin, or that
    /// needs a question answered first, is refused and changes nothing. Omitted: no fence.
    #[serde(default)]
    pub agent: Option<String>,
}

/// **The parameters of `grimoire/operation_undo`.**
///
/// **`deny_unknown_fields`, and `dry_run` defaults to `true`**, for the reason [`ApplyParams`]
/// gives: this is the raw door, and the MCP tool is the one with a description in front of it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UndoParams {
    /// **The change to take back**, by its operation id. `None`: the newest one not yet taken back.
    #[serde(default)]
    pub operation_id: Option<String>,
    /// **Take back the whole presented turn (E-084)**, newest change first, instead of one logged
    /// change.
    ///
    /// **A flag rather than making the id-less form follow the stack.** An undo on the *log* is well
    /// defined with no presentation, survives a door crossing, and is what `grimoire_read_oplog`
    /// lists; making it silently depend on a UI resource would make one call mean two things on two
    /// screens. The person's Cmd+Z has always meant "the turn", so this gives the agent's door the
    /// same verb rather than changing what its existing one says.
    ///
    /// Refused when nothing has been presented — the turn is a thing the agent declared, and with no
    /// declaration there is nothing to take back as one.
    #[serde(default)]
    pub turn: bool,
    /// **Whether to preview.** `true` changes nothing.
    #[serde(default = "yes")]
    pub dry_run: bool,
}

/// **`true`, the default for the fields that mean "yes".**
fn yes() -> bool {
    true
}

/// **The parameters of `grimoire/operation_preview`.**
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewParams {
    /// **Which operation.**
    pub operation: String,
    /// **Its parameters.**
    #[serde(default)]
    pub parameters: Value,
}

/// **The parameters of `grimoire/oplog_read`.**
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogParams {
    /// **Read from this sequence number on.** `0` is the beginning.
    #[serde(default)]
    pub since_seq: u64,
    /// **How many entries at most.**
    #[serde(default = "fifty")]
    pub limit: usize,
}

/// **Fifty entries, which is a screenful.**
fn fifty() -> usize {
    50
}

/// **Build a context from the editor's own resources.**
///
/// **The two `Option`s, and why they are options.** A map door has an open map and a kit door
/// does not, and a project is present in both — but the resources are `Option` in the signature
/// because a command can be called in a state where one is missing, and **a missing resource must
/// be a refusal and not a panic**. `CLAUDE.md` says it: *"A missing `Res<T>` panics its system."*
fn context(
    project: Option<ResMut<crate::project::Project>>,
    open: Option<ResMut<crate::project::OpenMap>>,
    menu: Option<Res<crate::chooser::MenuOpening>>,
) -> Ctx {
    Ctx {
        // **Cloned out, because the context owns what it carries.** The reason is the registry:
        // a trait with a lifetime on its methods cannot be a `dyn`, and the registry has to be a
        // `dyn` because a list of operations is looked up by name. One copy of a map's settings
        // is a price worth paying for that.
        project: project.map(|p| p.into_inner().clone()),
        // **The menu's root, while the menu is the screen that is up.** There is no `Project`
        // resource until a door opens, so without this every `grimoire/…` command refused on the one
        // screen where a project is chosen — including `map_create`, which is how a map gets made
        // before any door exists.
        root: menu.map(|m| m.root.clone()),
        open: open.map(|o| o.into_inner().clone()),
        source: Source::Agent,
        session: crate::ops::session_id(),
        proposal: None,
    }
}

/// **The context, built from the editor's own resources, and the resources back afterwards.**
///
/// **The change has to land in the RUNNING editor, and not only in the file.** A context that
/// holds a *copy* means the operation edits the copy, writes the file, and the editor's own map
/// is untouched — so the screen shows the old settings until the watcher notices the file, up to
/// a quarter of a second later, and any edit the author makes in that window is thrown away when
/// the watcher swaps the map in.
///
/// **So the context is built from clones and the result is written back**, which is the same
/// thing the UI path does with the map it owns: the operation's answer *is* the editor's new
/// state.
///
/// **`ResMut::into_inner` hands back a borrow, not the value** — so the value is cloned out and
/// the changed one is cloned back. **The clone is the price of the registry's `dyn`**, and it is
/// paid once per operation rather than once per frame.
pub(crate) fn take(
    project: &Option<ResMut<crate::project::Project>>,
    open: &Option<ResMut<crate::project::OpenMap>>,
) -> Ctx {
    Ctx {
        project: project.as_deref().cloned(),
        root: None,
        open: open.as_deref().cloned(),
        source: Source::Agent,
        session: crate::ops::session_id(),
        proposal: None,
    }
}

/// **Carry the standing proposal between the context and the editor's `Proposal` resource.**
///
/// A layout proposal is not map data, so it travels in [`Ctx`] rather than in `OpenMap` — and that
/// means the two front ends have to move it across by hand, the way they move the map. Before the
/// dispatch it is taken out of the resource (`dry_run` clones instead, because a preview must not
/// change anything); after it, whatever the context holds is written back.
///
/// **Only the three layout operations are touched**, and that is deliberate: a `placement_set_yaw`
/// that overwrote the resource with `None` would silently throw away a layout standing on screen,
/// which is the kind of second-order loss this codebase keeps having to fix.
///
/// **Every write is compared first.** Storing the same `Option<Proposed>` through `as_deref_mut`
/// marks the resource changed, and `tests/no_system_writes_every_frame.rs` fails on a system that
/// writes what it did not change.
pub(crate) fn carry_proposal(
    name: &str,
    ctx_proposal: Option<crate::editor::Proposed>,
    proposal: Option<&mut crate::editor::Proposal>,
    clock: Option<&mut crate::editor::ProposalClock>,
) {
    if !matches!(name, "layout_propose" | "layout_accept" | "layout_discard") {
        return;
    }
    let (Some(proposal), Some(clock)) = (proposal, clock) else {
        return;
    };
    // **A fresh proposal gets a fresh generation**, so `sync_proposal_ghosts` can tell a second
    // layout from the first one even when they are the same length.
    let fresh = ctx_proposal.map(|mut p| {
        p.generation = clock.tick();
        p
    });
    if proposal.0 != fresh {
        proposal.0 = fresh;
    }
}

/// **Put the operation's map back into the editor's own resource.**
///
/// **The map, and the rules when an operation changed them.** Most operations read the library and
/// write the file and leave the project alone; a rename is the one that rewrites `rules.ron`.
pub(crate) fn put_back(
    project: &mut Option<ResMut<crate::project::Project>>,
    open: &mut Option<ResMut<crate::project::OpenMap>>,
    ctx: Ctx,
) {
    // **The rules too, and only when they changed.** A rename rewrites `rules.ron`'s examples, and
    // the editor reads the rules it generates from out of the Project resource, so a change that
    // stayed in the clone would leave the next generate reading a map the rename had removed. The
    // comparison comes first: writing through `as_deref_mut` marks the resource changed, and most
    // operations change no rules.
    if let Some(fresh) = ctx.project.as_ref().map(|p| p.rules.clone())
        && project.as_deref().is_some_and(|p| p.rules != fresh)
        && let Some(handle) = project.as_deref_mut()
    {
        handle.rules = fresh;
    }
    // **The library too, when an operation changed it** (E-042). `commit_measured` writes the
    // library and the layers the Map draws from it, so a label kept only in the clone would be on
    // disk and not on screen. Compared first, for the reason the rules are: most operations change
    // no library, and writing through `as_deref_mut` marks the resource changed.
    //
    // **This is also what repaints the kit pane, and no extra trigger is needed (E-048).**
    // `adopt_library` writes through `as_deref_mut`, which marks `Project` changed, and
    // `tiles::rebuild_detail` runs on `resource_exists_and_changed::<Project>`. Measured with the
    // `LabelGeneration` bump below removed: the `KIND` chip block still repainted (9,521 px), so a
    // second trigger would be a second answer to a question already answered.
    if let Some(fresh) = ctx.project.as_ref()
        && project
            .as_deref()
            .is_some_and(|p| p.measured != fresh.measured || p.policy != fresh.policy)
        && let Some(handle) = project.as_deref_mut()
    {
        crate::editor::adopt_library(handle, fresh);
    }
    // **The compositions too** (E-072): `composition_define` writes `compositions.ron` through the
    // clone's project, and the editor's own copy must say so or the next define would not see it.
    // Compared first, like the rules and the library.
    if let Some(fresh) = ctx.project.as_ref()
        && project.as_deref().is_some_and(|p| p.compositions != fresh.compositions)
        && let Some(handle) = project.as_deref_mut()
    {
        handle.compositions = fresh.compositions.clone();
    }
    if let Some(fresh) = ctx.open
        && let Some(handle) = open.as_deref_mut()
    {
        *handle = fresh;
    }
}

/// **`grimoire/project_root` — which project the editor has open.**
///
/// **A read, and the one fact the MCP server cannot know for itself.** The server is told a
/// project on the command line and the editor knows which one it has open, and **the two have to
/// be the same** or a change lands in the wrong place.
fn project_root(
    _: In<serde_json::Value>,
    project: Option<Res<crate::project::Project>>,
    open: Option<Res<crate::project::OpenMap>>,
    menu: Option<Res<crate::chooser::MenuOpening>>,
) -> Result<Value, String> {
    // **The menu's root counts.** The menu is the screen you are on before a door, and it knows
    // which directory it is about — so the server's own "are we pointed at the same project" check
    // can pass there, and `map_create` can be reached before any door is open. Without this the
    // whole agent surface refused on the one screen where a project is chosen.
    let Some(project) = project else {
        if let Some(menu) = menu {
            return Ok(json!({
                "project_root": menu.root.to_string_lossy(),
                "map": serde_json::Value::Null,
                "screen": "menu",
            }));
        }
        return Err("no project is open.".to_owned());
    };
    // **The open map's name, while a map is open.** A rename changes the open map and not the
    // project's name; the project's is only brought up to date on a crossing. Without this an agent
    // asking which map is open is told the name the author has already replaced.
    let map = open
        .map(|o| o.map.name.clone())
        .or_else(|| project.open_map_name.clone());
    Ok(json!({
        "project_root": project.root.to_string_lossy(),
        "map": map,
    }))
}

/// **The editor's own frame clock (E-011).** `frame_time` is wall time between frames, so a loaded
/// machine inflates it; `busy_ms` is the time the main schedules spent working inside one frame
/// (`First` to `Last`), which waiting on the present does not lengthen. Read both: a high frame
/// time with a low busy time is the machine or the present, the reverse is the editor.
#[derive(Resource, Default)]
struct FrameBusy {
    started: Option<std::time::Instant>,
    ms: f64,
}

fn frame_busy_start(mut busy: ResMut<FrameBusy>) {
    busy.started = Some(std::time::Instant::now());
}

fn frame_busy_end(mut busy: ResMut<FrameBusy>) {
    if let Some(s) = busy.started.take() {
        busy.ms = s.elapsed().as_secs_f64() * 1000.0;
    }
}

/// The `frame` section of `grimoire/state`: nulls until a frame has been measured.
fn frame_json(
    store: Option<&bevy::diagnostic::DiagnosticsStore>,
    busy: Option<&FrameBusy>,
) -> Value {
    use bevy::diagnostic::FrameTimeDiagnosticsPlugin as F;
    let get = |p: &bevy::diagnostic::DiagnosticPath| {
        store.and_then(|s| s.get(p)).map(|d| (d.value(), d.average()))
    };
    let time = get(&F::FRAME_TIME);
    let fps = get(&F::FPS);
    json!({
        "frame_ms": time.and_then(|t| t.0),
        "frame_ms_smoothed": time.and_then(|t| t.1),
        "fps": fps.and_then(|t| t.0),
        "busy_ms": busy.filter(|b| b.ms > 0.0).map(|b| b.ms),
        "frame_count": store.and_then(|s| s.get(&F::FRAME_COUNT)).and_then(|d| d.value()),
    })
}

/// The project and the open map, both optional: `editor_state` answers for an editor with neither.
#[derive(bevy::ecs::system::SystemParam)]
struct Loaded<'w> {
    project: Option<Res<'w, crate::project::Project>>,
    open: Option<Res<'w, crate::project::OpenMap>>,
    game: Option<Res<'w, crate::live::LiveGame>>,
    reloads: Option<Res<'w, crate::live::EditorReloads>>,
    live: Option<Res<'w, crate::live::LiveWrite>>,
    diagnostics: Option<Res<'w, bevy::diagnostic::DiagnosticsStore>>,
    busy: Option<Res<'w, FrameBusy>>,
}

/// **The parameters of `grimoire/state`.** Only the pin page is a parameter; the rest of the summary is
/// the state as it stands.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct StateParams {
    /// The first owned placement to name, counted in map order. `0` is the start of the list.
    pub pinned_from: usize,
    /// The first library id to name, counted in library order. `0` is the start of the list.
    pub descriptors_from: usize,
    /// A sent or claimed note's id, to get everything needed to work it (#91): its text, area and
    /// margin, the pieces near it, the kit's palette, the camera, how to get a picture, earlier notes.
    pub note: Option<String>,
}

/// **`grimoire/state` — what the editor is holding right now.**
///
/// **A read, and the answer to "what does the editor look like?"** — the question an agent asks
/// before it changes anything, and the question it asks afterwards to check that the change landed.
///
/// **It is a summary, not a dump.** The editor holds a great deal: a library of hundreds of
/// descriptors, a map of thousands of placements, a grid of cells. **None of that is what an agent
/// needs to know**; what it needs is the *shape* of the current state — which map, how big, how
/// many pieces, what is armed, what is selected, what the editor last said, and what has gone
/// wrong. **So the state is the handful of numbers a person would read off the screen**, and the
/// picture is a separate tool.
///
/// **And it is cheap.** Everything here is already in memory, so the whole command is a read and
/// a `json!` — no scanning, no measuring, no rendering.
// The presentation resource is an eighth parameter. Bundling it into `Loaded` would hide it from the
// ratchet tests that read this signature and would tie a read-only summary to a resource that only
// S1.7 introduced — the same reason every other resource here is its own parameter.
#[allow(clippy::too_many_arguments)]
fn editor_state(
    In(params): In<StateParams>,
    loaded: Loaded,
    state: Option<Res<crate::editor::EditorState>>,
    tiles: Option<Res<crate::tiles::ImportState>>,
    armed: Option<Res<crate::compose::Armed>>,
    door: Option<Res<crate::tiles::Door>>,
    mode: Option<Res<crate::tiles::Mode>>,
    presenting: Option<Res<crate::present::Presenting>>,
    // **Which screen the editor is on, which nothing else here says (E-078).** The map, the door and
    // the mode all read the same on the menu as in the editor — `Door` and `Mode` are `init_resource`d
    // at build and a loaded `OpenMap` is a resource either way — so before this, an agent could not
    // tell *"the map is loaded"* from *"the map is open in front of the person"*. Measured
    // 2026-10-25: that is exactly the pair `tools/loop/accept.py`'s `open_map` step has to tell
    // apart, and its predicate passed on the pre-fix build.
    screen: Option<Res<State<crate::screen::Screen>>>,
    // **Which keys the editor is reading and who holds them** (UI-90), for `ui.focus`.
    keys_live: Option<Res<crate::keys::Live>>,
) -> Result<Value, String> {
    let Loaded { project, open, game, reloads, live, diagnostics, busy } = loaded;
    // **The agent's presentation, and the person's answer to it** (S1.7). `None` until an agent
    // presents something; then an object whose `answer` is `null` while the question is up. This is
    // the only channel an agent has for what a person did after its call returned.
    let presentation = presenting.as_ref().and_then(|p| p.0.as_ref()).map(|p| {
        json!({
            "id": p.id,
            "title": p.title,
            "answer": p.answer,
        })
    });
    // **The pins, named by id and reason.** A pin is the one thing an agent cannot see in the state
    // today, and the list is capped so a state dump stays a summary. `pinned_total` is the true
    // count, so a reader can tell a full list from a cut one.
    const PINNED_CAP: usize = 50;
    let pinned_total = open
        .as_ref()
        .map_or(0, |o| o.map.placements.iter().filter(|p| p.owned).count());
    // **The page the caller asked for.** `pinned_from` counts pins in map order, so the next page
    // starts exactly where this one stopped.
    let pinned_from = params.pinned_from;
    let pinned: Vec<Value> = open.as_ref().map_or_else(Vec::new, |o| {
        o.map
            .placements
            .iter()
            .filter(|p| p.owned)
            .skip(pinned_from)
            .take(PINNED_CAP)
            .map(|p| json!({"id": p.id, "because": p.owned_because}))
            .collect()
    });
    let pinned_next = (pinned_from.saturating_add(PINNED_CAP) < pinned_total)
        .then(|| pinned_from + PINNED_CAP);
    // **The library's ids, a page at a time** (C6 gap 2). A refusal names the closest ids, and this is
    // the whole list an agent can read, capped the way the pins are so a state dump stays a summary.
    const DESCRIPTOR_CAP: usize = 50;
    let descriptors_total = project.as_ref().map_or(0, |p| p.library.descriptors.len());
    let descriptors_from = params.descriptors_from;
    let descriptor_ids: Vec<String> = project.as_ref().map_or_else(Vec::new, |p| {
        p.library
            .descriptors
            .iter()
            .skip(descriptors_from)
            .take(DESCRIPTOR_CAP)
            .map(|d| d.id.clone())
            .collect()
    });
    let descriptor_ids_next = (descriptors_from.saturating_add(DESCRIPTOR_CAP) < descriptors_total)
        .then(|| descriptors_from + DESCRIPTOR_CAP);
    // **The map, and what is in it.**
    let map = open.as_ref().map(|o| {
        json!({
            "name": o.map.name,
            "path": o.map_path.to_string_lossy(),
            "bounds": [o.map.bounds.0, o.map.bounds.1, o.map.bounds.2],
            "origin": [o.map.origin.0, o.map.origin.1, o.map.origin.2],
            // **The rows a consumer will spawn** -- the map's own placements plus every stamp
            // expanded -- because that is the number the game will see and the number an agent
            // should reason about.
            "placements": o.map.placements.len(),
            "stamps": o.map.stamps.len(),
            "bash": o.map.bash,
            "note": o.map.note,
            // **The enclosure wish, so an agent can read back what it set.** Without these two, an
            // operation could change the wish and the state dump would not show it — the agent would
            // have to trust its own parameters rather than the editor's answer (E-005).
            "enclosure_wish": o.map.enclosure.wish,
            "enclosure_weight": o.map.enclosure.weight,
            "pinned": pinned,
            "pinned_total": pinned_total,
            // **Where the next page starts**, and the sentence that says so — the shape `oplog_read`
            // gives a capped result, so a reader that pages one pages the other the same way.
            "pinned_next": pinned_next,
            "how_to_narrow": pinned_next.map(|n| format!("pass pinned_from={n} to read the rest")),
            // **Whether the file and the editor agree.** An agent that changed something needs to
            // know it reached the disk.
            "dirty": o.dirty(),
        })
    });
    // **The project, and what it holds.**
    let project_summary = project.as_ref().map(|p| {
        json!({
            "root": p.root.to_string_lossy(),
            "kit": p.namespace,
            "descriptors": p.library.descriptors.len(),
            // **The ids themselves, a page at a time.** The count alone left an agent that had been
            // refused with no list to choose from.
            "descriptor_ids": descriptor_ids,
            "descriptor_ids_next": descriptor_ids_next,
            "how_to_read_more_ids": descriptor_ids_next
                .map(|n| format!("pass descriptors_from={n} to read the rest")),
            "compositions": bevy_grimoire_core::census::of_catalog(
                &p.library,
                &p.compositions.compositions
            )
            .compositions,
            "kits": p.kits.len(),
            // **The registered game data files and their leaves** (#86), what `data_set` addresses.
            "data_files": data_files_json(&p.grimoire_dir),
        })
    });
    // **What the editor is holding and what it last said.**
    //
    // **The armed piece's id, not its index.** An index means nothing to a reader; the id is what
    // the palette shows and what an agent would name. And the index is into the *library*, so the
    // name is looked up there.
    let armed_piece = match (state.as_ref(), project.as_ref()) {
        (Some(s), Some(p)) => s
            .brush
            .and_then(|i| p.library.descriptors.get(i))
            .map(|d| d.id.clone()),
        _ => None,
    };
    // **What Cmd+Z and Cmd+Shift+Z would do**, named the way the INSPECTOR's UNDO and REDO rows name
    // them, so an agent and a person read the same words.
    let undo_map = open.as_ref().map(|o| &o.map);
    let editor = state.as_ref().map(|s| {
        json!({
            "armed_piece": armed_piece,
            "brush_yaw_deg": s.brush_yaw,
            "tool": format!("{:?}", s.tool),
            "status": s.status.note_text(),
            "problems": s.status.problems().iter().map(|p| p.line()).collect::<Vec<_>>(),
            "journal_entries": s.status.raised(),
            "undo_next": undo_map.and_then(|m| s.undo_next(m)),
            "undo_depth": s.undo_depth(),
            "redo_next": undo_map.and_then(|m| s.redo_next(m)),
            "redo_depth": s.redo_depth(),
            // **What a caller needs to read a waiting door (cycle 8, gap 3).** The question on screen and
            // the door it would cross to, so a waiting door is not invisible to the agent that asked.
            "leaving": s.leaving,
            "door_waiting": s.door_request.map(|r| json!({
                "door": r.door.label(),
                "tab": r.tab.map(|m| m.label()),
            })),
        })
    });
    // **Where the person is looking** (UI-90, #256): the screen, the workspace, the panels and the region with the keys.
    let ui = ui_json(screen.as_deref(), door.as_deref(), keys_live.as_deref());
    Ok(json!({
        "map": map,
        "ui": ui,
        // **The notes an agent can claim, and with `note` the bundle to work one** (#91).
        "notes": open.as_ref().map(|o| crate::ops_notes::state_json(
            o,
            project.as_ref().map(|p| &p.library),
            params.note.as_deref(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()),
        )),
        // **The reference images beside this map** (#124), each with its id to cite and whether its file is there.
        "references": open.as_ref().map(|o| crate::ops_references::state_json(o, project.as_ref().map(|p| p.root.as_path()))),
        // **The agent's plan for this map** (E-069): items with a status each, and a count per status.
        "plan": open.as_ref().map(|o| crate::ops_plan::state_json(o)),
        // **The repeats a player will notice first** (#136): same piece, yaw (any yaw for a round
        // kind, #184) and tip, close together,
        // weighed by role so repeated clutter heads the list above repeated structure. The rules are
        // the project's `repetition.ron` (#139), read on every call so an edit is seen with no restart.
        "repetition": match (open.as_ref(), project.as_ref()) {
            (Some(o), Some(p)) => repetition_json(&o.map.placements, &p.library, &p.project_dir),
            _ => Value::Null,
        },
        // **The kit pieces that break the project's standards** (#138): read from `standards.ron`
        // beside `kits.ron` on every call, so an edit to that file is seen with no restart.
        "standards": project.as_ref().map(|p| standards_json(&p.library, &p.project_dir)),
        // **The kit's dead ends** (#142): every declared face no piece of the kit meets at any
        // quarter turn, through the same seam check the map uses, so a socket nothing can close is
        // named before a map runs into it.
        "dead_ends": project.as_ref().map(|p| dead_ends_json(&p.library, p.lattice.face_bands)),
        // **Whether the kit can loop a hallway back and open it into a room** (#171): the stress
        // tests after `dead_ends`, `adjacency::ring` (#160) and `adjacency::room` (#162), from the
        // library on every call, so an edit to a piece's tokens is in the next answer.
        "stress": project.as_ref().map(|p| stress_json(&p.library, p.lattice.face_bands)),
        // **The intended relations this map breaks** (#137): read from `<map>.relations.ron` on every
        // call, so an edit to that file is seen by the next answer with no restart.
        "relations": open.as_ref().map(|o| relations_json(&o.map.placements, &o.map_path)),
        "project": project_summary,
        "editor": editor,
        "presentation": presentation,
        "frame": frame_json(diagnostics.as_deref(), busy.as_deref()),
        // **The game's verdict on the last reload** (S3.5): null, "pending", or {ok, message, at_unix_ms}.
        "game": game.as_deref().map_or(Value::Null, |g| g.state_json(open.as_ref().map(|o| o.map_path.as_path()))),
        // **The editor's own verdicts** on the files it watches and draws (#200, H3): `{ok, seq, kinds}`,
        // one entry per file with its kind, path, `applied` or `refused`, and the field and reason.
        "reloads": reloads.as_deref().map_or(Value::Null, |r| r.state_json()),
        // **What became of each file's latest edit, as a running game says it** (#203, H6): `{now_unix_ms,
        // game: {connected, watching, at_unix_ms, seq}, files}`, one entry per file of the open project with its
        // kind, path, `state` (`no_game_connected`, `pending`, `game_applied`, `game_refused`,
        // `needs_restart`, `not_read`), field, reason, the game's decision time, the file's last write and
        // the editor's own verdict. The directory is walked on every call, like `standards` and `repetition`.
        "reload_files": project.as_ref().map(|p| {
            let verdict = game.as_deref().and_then(|g| g.verdict.as_ref());
            let editor = reloads.as_deref().map_or(&[][..], |r| r.kinds());
            let files = crate::reload_files::files(&p.project_dir, &p.root, verdict, editor);
            crate::reload_files::state_json(&files, verdict, live.is_some())
        }),
        "armed_composition": armed.and_then(|a| a.id().map(|s| s.to_owned())),
        "selected": tiles.as_ref().map(|t| t.selected_library_id.clone()),
        "door": door.map(|d| d.label()),
        "mode": mode.map(|m| m.label()),
        // **`"menu"` or `"editor"`.** A caller that needs "is the person looking at this map?" reads
        // it here; nothing else in this answer can be told apart between the two screens.
        "screen": screen.map(|s| match *s.get() {
            crate::screen::Screen::Menu => "menu",
            crate::screen::Screen::Editor => "editor",
        }),
    }))
}

/// **`grimoire/state.ui` — where the person is looking** (UI-90, #256; `docs/ui/vision.md` §4).
///
/// `screen` is `launcher` on the chooser and `workbench` in the editor. `workspace` is the open door
/// (the workbench's saved layouts replace the doors, UI-33) and `panels` are the tabs that door shows,
/// until the docks hold panels of their own (UI-30). `focus` is the region with the keyboard: the
/// `viewport` unless a text field, a filter box or the journal is holding it. Read it before
/// `grimoire/run_command`: a command is refused when the region that holds the keys is not its own.
fn ui_json(
    screen: Option<&State<crate::screen::Screen>>,
    door: Option<&crate::tiles::Door>,
    live: Option<&crate::keys::Live>,
) -> Value {
    use crate::keys::{Context, Holder};
    let workbench = screen.is_some_and(|s| *s.get() == crate::screen::Screen::Editor);
    if !workbench {
        return json!({"screen": "launcher", "workspace": null, "panels": [], "focus": "launcher"});
    }
    let focus = match live.map(|l| (l.0, l.2)) {
        Some((Context::Typing, _)) => "text_field",
        Some((_, Holder::Filter)) => "filter",
        Some((_, Holder::Journal)) => "journal",
        _ => "viewport",
    };
    json!({
        "screen": "workbench",
        "workspace": door.map(|d| d.label()),
        "panels": door.map_or_else(Vec::new, |d| d.tabs().iter().map(|m| m.label()).collect()),
        "focus": focus,
    })
}

/// The most repeats `grimoire_state` names; `total` always counts them all.
const REPEATS_CAP: usize = 10;

/// **The visible repeats of a map, as `grimoire_state` reports them**, under the project's rules. A
/// rules file that cannot be read is named in `error` with no repeats, never read as the defaults.
fn repetition_json(
    placements: &[bevy_grimoire_core::map::Placed],
    library: &bevy_grimoire_core::library::Library,
    project_dir: &std::path::Path,
) -> Value {
    use bevy_grimoire_core::repetition::{repeats, Role};
    let rules = match load_repetition(project_dir) {
        Ok(r) => r,
        Err(e) => return json!({"repeats": [], "total": 0, "error": e}),
    };
    let all = repeats(placements, library, &rules);
    let rows: Vec<Value> = all
        .iter()
        .take(REPEATS_CAP)
        .map(|r| {
            json!({
                "descriptor": r.descriptor,
                "patched": r.patch.is_some(),
                "role": match r.role { Role::Structure => "structure", Role::Clutter => "clutter" },
                "pieces": r.pieces,
                "pairs": r.pairs,
                "score": r.score,
            })
        })
        .collect();
    json!({"repeats": rows, "total": all.len(), "error": null})
}

/// `<project_dir>/repetition.ron`, or the default rules when there is no such file; any other
/// failure to read or parse it is the error, naming the file. Shared by `grimoire_state` and the
/// status bar's `REPEATS` line (#139), so the two cannot disagree.
pub(crate) fn load_repetition(
    project_dir: &std::path::Path,
) -> Result<bevy_grimoire_core::repetition::RepetitionRules, String> {
    use bevy_grimoire_core::repetition::{RepetitionRules, FILE};
    let path = project_dir.join(FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => RepetitionRules::parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(RepetitionRules::default()),
        Err(e) => Err(e.to_string()),
    }
    .map_err(|e| format!("{}: {e}", path.display()))
}

/// **The relations beside a map, checked against its placements**, as `grimoire_state` reports them.
/// No file is no relations and no error; a file that cannot be read or parsed is named in `error`,
/// never read as an empty list.
fn relations_json(placements: &[bevy_grimoire_core::map::Placed], map_path: &std::path::Path) -> Value {
    use bevy_grimoire_core::relations::{check_map, path_for_map, Outcome, Relations};
    let path = path_for_map(map_path);
    let loaded = match std::fs::read_to_string(&path) {
        Ok(text) => Relations::parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Relations::default()),
        Err(e) => Err(e.to_string()),
    };
    let relations = match loaded {
        Ok(r) => r,
        Err(e) => {
            return json!({"relations": [], "unmet": 0, "missing": 0, "error": format!("{}: {e}", path.display())})
        }
    };
    let (mut unmet, mut missing) = (0, 0);
    let rows: Vec<Value> = relations
        .relations
        .iter()
        .zip(check_map(placements, &relations))
        .map(|(r, outcome)| {
            let (word, measured, by, missing_id) = match outcome {
                Outcome::Met { measured } => ("met", Some(measured), None, None),
                Outcome::Unmet { measured, by } => {
                    unmet += 1;
                    ("unmet", Some(measured), by, None)
                }
                Outcome::Missing { id } => {
                    missing += 1;
                    ("missing", None, None, Some(id))
                }
            };
            json!({
                "of": r.of,
                "kind": serde_json::to_value(&r.kind).unwrap_or(Value::Null),
                "outcome": word,
                "measured": measured,
                "by": by,
                "missing_id": missing_id,
            })
        })
        .collect();
    json!({"relations": rows, "unmet": unmet, "missing": missing, "error": null})
}

/// The most dead-end faces `grimoire_state` names; `total` always counts them all.
const DEAD_ENDS_CAP: usize = 20;

/// **The kit's dead ends** as `grimoire_state` reports them (#142 slice 2): the first
/// [`DEAD_ENDS_CAP`] of `adjacency::dead_ends`, sorted by piece and side, and the full count.
fn dead_ends_json(library: &bevy_grimoire_core::library::Library, divisions: u32) -> Value {
    use bevy_grimoire_core::adjacency::{dead_ends, dir_name};
    let all = dead_ends(library, divisions);
    let faces: Vec<Value> = all
        .iter()
        .take(DEAD_ENDS_CAP)
        .map(|d| {
            // The piece nearest to closing it (#145), with the cells that keep it from it.
            let closest = d.closest.as_ref().map(|c| {
                let misses: Vec<Value> = c
                    .misses
                    .iter()
                    .map(|m| json!({"at": m.at, "want": m.want, "found": m.found}))
                    .collect();
                json!({"piece": c.piece, "turn": u32::from(c.turn) * 90, "misses": misses})
            });
            json!({
                "piece": d.piece,
                "dir": dir_name(d.dir),
                "face": d.face,
                "closest": closest,
                "message": d.message,
            })
        })
        .collect();
    json!({"faces": faces, "total": all.len()})
}

/// **The kit stress tests that build with the kit** (#171), as `grimoire_state` reports them: the loop
/// `adjacency::ring` closes and the room `adjacency::room` opens, each piece as `{piece, turn}` with
/// its turn in degrees, or null when the kit cannot build it. `declared` counts the pieces that carry
/// an edge token; with none there is no declared seam to try, so both are null without a search and
/// `declared: 0` says the nulls are about an unlabelled kit, not a failing one.
fn stress_json(library: &bevy_grimoire_core::library::Library, divisions: u32) -> Value {
    use bevy_grimoire_core::adjacency::{ring, room};
    let declared = library
        .descriptors
        .iter()
        .filter(|d| {
            d.subgrid
                .as_ref()
                .is_some_and(|g| g.cells.iter().any(|c| c.edge.is_some()))
        })
        .count();
    if declared == 0 {
        return json!({"declared": 0, "ring": null, "room": null});
    }
    let cell = |(piece, q): &(String, u8)| json!({"piece": piece, "turn": u32::from(*q) * 90});
    let ring = ring(library, divisions).map(|r| r.pieces.iter().map(cell).collect::<Vec<_>>());
    let room = room(library, divisions).map(|r| {
        r.pieces
            .iter()
            .map(|row| row.iter().map(cell).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    });
    json!({"declared": declared, "ring": ring, "room": room})
}

/// **The project's standards, checked against its library**, as `grimoire_state` reports them: every
/// breach, every rule that covers no piece, and every pair of rules whose modules do not nest (#168).
/// No file is no rules and no error; a file that cannot be read or parsed is named in `error`, never
/// read as an empty list.
fn standards_json(library: &bevy_grimoire_core::library::Library, project_dir: &std::path::Path) -> Value {
    use bevy_grimoire_core::standards::{check, idle, unnested};
    let standards = match load_standards(project_dir) {
        Ok(s) => s,
        Err(e) => return json!({"breaches": [], "idle": [], "unnested": [], "error": e}),
    };
    let rows: Vec<Value> = check(library, &standards).iter().map(breach_json).collect();
    let pairs: Vec<Value> = unnested(&standards).iter().map(unnested_json).collect();
    json!({"breaches": rows, "idle": idle(library, &standards), "unnested": pairs, "error": null})
}

/// **One piece's breaches of the project's standards** (#138 slice 3), as `piece_import` reports
/// them on admission: `{breaches, error}`, the same rows `grimoire_state` gives, for that piece alone.
/// The second value names each broken rule once, in file order, for a summary; empty when it breaks
/// none or the file could not be read.
pub(crate) fn piece_standards_json(
    piece: &bevy_grimoire_core::descriptor::Descriptor,
    project_dir: &std::path::Path,
) -> (Value, Vec<String>) {
    let breaches = match load_standards(project_dir) {
        Ok(s) => piece_breaches(piece, &s),
        Err(e) => return (json!({"breaches": [], "error": e}), Vec::new()),
    };
    let mut rules: Vec<String> = Vec::new();
    for b in &breaches {
        if !rules.contains(&b.rule) {
            rules.push(b.rule.clone());
        }
    }
    let rows: Vec<Value> = breaches.iter().map(breach_json).collect();
    (json!({"breaches": rows, "error": null}), rules)
}

/// **One piece's breaches of `standards`**, in file order. Shared by `piece_import`'s answer and the
/// Meshes pane (#138), so the two cannot disagree.
pub(crate) fn piece_breaches(
    piece: &bevy_grimoire_core::descriptor::Descriptor,
    standards: &bevy_grimoire_core::standards::Standards,
) -> Vec<bevy_grimoire_core::standards::Breach> {
    use bevy_grimoire_core::library::Library;
    let one = Library {
        version: bevy_grimoire_core::library::LIBRARY_VERSION,
        note: None,
        descriptors: vec![piece.clone()],
    };
    bevy_grimoire_core::standards::check(&one, standards)
}

/// **The pairs of sub-kit modules that do not nest which `piece` is part of** (#168 slice 3): each
/// pair `standards::unnested` names where a rule with a module that covers `piece` is one of the
/// two, in file order. Rules are matched by name, as every report names them. The Meshes pane's.
pub(crate) fn piece_unnested(
    piece: &bevy_grimoire_core::descriptor::Descriptor,
    standards: &bevy_grimoire_core::standards::Standards,
) -> Vec<bevy_grimoire_core::standards::Unnested> {
    let covering: Vec<&str> = standards
        .rules
        .iter()
        .filter(|r| r.multiple_of.is_some() && r.covers(piece))
        .map(|r| r.name.as_str())
        .collect();
    if covering.is_empty() {
        return Vec::new();
    }
    bevy_grimoire_core::standards::unnested(standards)
        .into_iter()
        .filter(|u| u.rules.iter().any(|n| covering.contains(&n.as_str())))
        .collect()
}

/// `<project_dir>/standards.ron`, or no rules when there is no such file; any other failure to read
/// or parse it is the error, naming the file.
pub(crate) fn load_standards(
    project_dir: &std::path::Path,
) -> Result<bevy_grimoire_core::standards::Standards, String> {
    use bevy_grimoire_core::standards::{Standards, FILE};
    let path = project_dir.join(FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => Standards::parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Standards::default()),
        Err(e) => Err(e.to_string()),
    }
    .map_err(|e| format!("{}: {e}", path.display()))
}

fn dim_name(d: bevy_grimoire_core::standards::Dim) -> &'static str {
    use bevy_grimoire_core::standards::Dim;
    match d {
        Dim::Width => "width",
        Dim::Depth => "depth",
        Dim::Height => "height",
    }
}

fn breach_json(b: &bevy_grimoire_core::standards::Breach) -> Value {
    use bevy_grimoire_core::standards::Why;
    let (why, bound) = match b.why {
        Why::Unmeasured => ("unmeasured", None),
        Why::BelowMin(v) => ("below_min", Some(v)),
        Why::AboveMax(v) => ("above_max", Some(v)),
        Why::NotMultipleOf(v) => ("not_multiple_of", Some(v)),
    };
    json!({
        "piece": b.piece,
        "rule": b.rule,
        "dim": dim_name(b.dim),
        "found": b.found,
        "why": why,
        "bound": bound,
    })
}

/// **Two sub-kit modules that do not nest** (#168): the rules, their dimensions and modules in file
/// order, where runs of the two land on one grid line again (`null` when not within
/// `standards::MEET_WITHIN` lengths of the larger), and [`unnested_message`].
fn unnested_json(u: &bevy_grimoire_core::standards::Unnested) -> Value {
    json!({
        "rules": &u.rules,
        "dims": u.dims.map(dim_name),
        "modules": u.modules,
        "meet": u.meet,
        "message": unnested_message(u),
    })
}

/// **The sentence naming two modules that do not nest**, and where they meet again: one wording for
/// `grimoire_state.standards.unnested` and the Meshes pane (#168), so the agent and the person read
/// the same thing.
pub(crate) fn unnested_message(u: &bevy_grimoire_core::standards::Unnested) -> String {
    use bevy_grimoire_core::standards::MEET_WITHIN;
    let [a, b] = &u.rules;
    let [ma, mb] = u.modules;
    let [da, db] = u.dims.map(dim_name);
    let meet = match u.meet {
        Some(m) => format!("they share a grid line only every {m} m"),
        None => format!("they share no grid line within {MEET_WITHIN} lengths of {} m", ma.max(mb)),
    };
    format!("{a} ({da}, {ma} m) and {b} ({db}, {mb} m) do not nest: {meet}")
}

/// **The parameters of `grimoire/pieces`.** Every filter is optional, and they combine: a piece is
/// named only if it passes all of them. `pieces_from` is the page to read, counted in the filtered
/// list.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct PiecesParams {
    /// The first piece to name, counted in the filtered list. `0` is the start of the list.
    pub pieces_from: usize,
    /// Only the pieces that name a mesh. A flat piece with no mesh is a footprint, not a model.
    pub with_mesh: bool,
    /// Only the pieces with no labelled axis: no label record at all, or a record that labels
    /// none of its axes. `false` keeps every piece.
    pub unlabelled: bool,
    /// Only the pieces with at least one axis that a model wrote and no person has confirmed.
    pub by_model: bool,
    /// Only the pieces whose label record says exactly this confidence: `high`, `medium` or `low`.
    pub confidence: Option<String>,
    /// Only the pieces whose id contains this text.
    pub id_contains: Option<String>,
}

/// The six axes a label record can cover, each with the value the descriptor names on it. The
/// order is the schema's, so a row reads the same way every time.
fn axis_values(d: &Descriptor) -> [(&'static str, Axis, Value); 6] {
    [
        ("kind", Axis::Kind, json!(d.kind)),
        ("effects", Axis::Effects, json!(d.effects)),
        ("look", Axis::Look, json!(d.look)),
        ("surfaces", Axis::Surfaces, json!(d.offers.surfaces)),
        ("mount", Axis::Mount, serde_json::to_value(&d.mount).unwrap_or(Value::Null)),
        ("note", Axis::Note, json!(d.note)),
    ]
}

/// **The label provenance of one piece**: the record's model, date and confidence, and each
/// labelled axis with its value and who wrote it. A piece with no record reads the same shape,
/// with every field `null` and no axes, so a row never changes shape with its provenance.
///
/// Returns the provenance, whether the piece is unlabelled (no axis labelled), and whether any
/// axis was written by a model or an agent and never confirmed by a person.
fn provenance(d: &Descriptor) -> (Value, bool, bool) {
    let mut axes = serde_json::Map::new();
    for (name, axis, value) in axis_values(d) {
        let Some(by) = d.labels.as_ref().and_then(|l| l.by.get(axis)) else {
            continue;
        };
        let by = match by {
            By::Model => "model",
            By::Human => "human",
            By::Agent => "agent",
        };
        axes.insert(name.to_owned(), json!({"value": value, "by": by}));
    }
    let by_model = d.labels.as_ref().is_some_and(|l| l.by.any_unconfirmed());
    let unlabelled = axes.is_empty();
    let (model, at, confidence) = match &d.labels {
        Some(l) => (l.model.clone(), Some(l.at.clone()), l.confidence.map(|c| c.to_string())),
        None => (None, None, None),
    };
    let model_at = d.labels.as_ref().and_then(|l| l.model_date()).map(str::to_owned);
    let labels = json!({"model": model, "model_at": model_at, "at": at, "confidence": confidence, "axes": axes});
    (labels, unlabelled, by_model)
}

/// **`grimoire/pieces` — the library's pieces, a page at a time.**
///
/// **A read.** The same paging as `grimoire/state`'s `descriptor_ids`: a capped page, the true
/// `total` for the filtered list so a reader can tell a full list from a cut one, and
/// `pieces_next` for the next page. Each row carries the fields a piece is chosen by (its id, the
/// mesh it is drawn with, and its kinds, effects and looks) and its label provenance: who labelled
/// each axis, the record's model, date and confidence.
fn editor_pieces(In(params): In<PiecesParams>, loaded: Loaded) -> Result<Value, String> {
    const PIECE_CAP: usize = 50;
    let wanted = params.confidence.as_deref().map(str::parse::<Confidence>).transpose()?;
    // **No project is not an empty library.** An empty page here read as "nothing needs labelling".
    let Some(project) = loaded.project else {
        return Err(
            "no project is open, so there is no library to list: open a project (start the editor \
             on its root) and ask again"
                .to_string(),
        );
    };
    let filtered: Vec<(&Descriptor, Value, bool)> = project
        .library
        .descriptors
        .iter()
        .filter_map(|d| {
            let (labels, unlabelled, by_model) = provenance(d);
            let passes = (!params.with_mesh || d.mesh.is_some())
                && (!params.unlabelled || unlabelled)
                && (!params.by_model || by_model)
                && wanted.is_none_or(|w| d.labels.as_ref().and_then(|l| l.confidence) == Some(w))
                && params.id_contains.as_deref().is_none_or(|text| d.id.contains(text));
            passes.then_some((d, labels, unlabelled))
        })
        .collect();
    let total = filtered.len();
    let from = params.pieces_from;
    let pieces: Vec<Value> = filtered
        .iter()
        .skip(from)
        .take(PIECE_CAP)
        .map(|(d, labels, unlabelled)| {
            json!({
                "id": d.id,
                "mesh": d.mesh,
                "kind": d.kind,
                "effects": d.effects,
                "look": d.look,
                "unlabelled": unlabelled,
                "labels": labels,
            })
        })
        .collect();
    let pieces_next = (from.saturating_add(PIECE_CAP) < total).then(|| from + PIECE_CAP);
    let filtered_by = params.with_mesh
        || params.unlabelled
        || params.by_model
        || params.confidence.is_some()
        || params.id_contains.is_some();
    // **Named only on a filtered page**: an unfiltered page is narrowed by nothing, so it names no
    // filter. The `how_to_read_more` sentence is the one that always applies.
    let how_to_narrow = pieces_next.filter(|_| filtered_by).map(|n| {
        format!(
            "the list is capped at {PIECE_CAP} rows: narrow it with with_mesh, unlabelled, by_model, \
             confidence or id_contains, or read the rest with pieces_from={n}"
        )
    });
    Ok(json!({
        "total": total,
        "pieces": pieces,
        "pieces_next": pieces_next,
        "how_to_read_more": pieces_next.map(|n| format!("pass pieces_from={n} to read the rest")),
        "how_to_narrow": how_to_narrow,
    }))
}

/// **`grimoire/capture_sequence_start` — begin a capture sequence.**
///
/// **A refusal, and not a note** — the sequence is an agent's request, and a request that cannot
/// be honoured has to say so where the agent reads it.
fn capture_sequence_start(
    In(p): In<crate::sequencer::SequenceParams>,
    time: Res<Time>,
    mut sequencer: ResMut<crate::sequencer::Sequencer>,
) -> Result<Value, String> {
    let id = sequencer.start(&p, time.elapsed_secs())?;
    Ok(json!({ "id": id }))
}

/// **`grimoire/capture_sequence_status` — what the sequence is doing.**
fn capture_sequence_status(
    In(p): In<crate::sequencer::StatusParams>,
    sequencer: Res<crate::sequencer::Sequencer>,
) -> Result<Value, String> {
    Ok(sequencer.status(p.id))
}

/// **`grimoire/operation_list` — every operation, its schema, and how it is undone.**
///
/// **A discovery tool, not a page of tools.** The MCP server exposes one tool per operation, but
/// the *BRP* surface is one list, because a BRP caller can ask. **The always-loaded MCP toolset
/// stays small** and the rare ones are found here.
///
/// **`{"query": "..."}` searches instead of listing** (#141): only the operations `ops::search`
/// finds, best first, with the query and the registry's `total` beside them, so a caller knows how
/// much it did not read. No query, or a blank one, lists every operation as before.
fn operation_list(In(p): In<serde_json::Value>) -> Result<Value, String> {
    let query = match p.get("query") {
        None | Some(Value::Null) => None,
        Some(Value::String(q)) if q.trim().is_empty() => None,
        Some(Value::String(q)) => Some(q.clone()),
        Some(other) => return Err(format!("`query` is text to search for, not {other}.")),
    };
    let row = |op: &dyn ops::Operation| {
        json!({
            "operation": op.name(),
            "undo": match op.undo() {
                ops::Undo::None => "none",
                ops::Undo::Inverse => "inverse",
                ops::Undo::Snapshot => "snapshot",
            },
            "parameters": op.schema(),
        })
    };
    let Some(query) = query else {
        let list: Vec<Value> = ops::registry().iter().map(|op| row(op.as_ref())).collect();
        return Ok(json!({ "operations": list }));
    };
    let found: Vec<Value> = ops::search(&query).iter().map(|op| row(op.as_ref())).collect();
    Ok(json!({
        "query": query,
        "total": ops::registry().len(),
        "operations": found,
    }))
}

/// **`grimoire/operation_preview` — what an operation would change, and nothing changed.**
///
/// **A pure call.** The operation's `plan` runs and the answer is returned; `commit` never runs,
/// so nothing is written to the map, to the file, or to the log.
fn operation_preview(
    In(p): In<PreviewParams>,
    project: Option<ResMut<crate::project::Project>>,
    open: Option<ResMut<crate::project::OpenMap>>,
    menu: Option<Res<crate::chooser::MenuOpening>>,
) -> Result<Value, String> {
    let Some(op) = ops::find(&p.operation) else {
        return Err(format!(
            "`{}` is not an operation this editor knows.",
            p.operation
        ));
    };
    let ctx = context(project, open, menu);
    match ops::preview(&ctx, op.as_ref(), &p.parameters) {
        Ok(plan) => Ok(json!({
            "operation": p.operation,
            "outcome": "previewed",
            "would_change": plan,
        })),
        Err(e) => Ok(refused_answer(&p.operation, &e)),
    }
}

/// **Why the Meshes shelf cannot select this piece now, or `None` when it can (E-044).**
///
/// The plan reads the library and nothing else. The door, the job and the shelf filter are editor
/// state, so they are read here, where the editor's resources are in hand.
pub(crate) fn piece_select_refusal(
    door: Option<crate::tiles::Door>,
    mode: Option<crate::tiles::Mode>,
    project: Option<&crate::project::Project>,
    filters: Option<&crate::filter::Filters>,
    id: &str,
) -> Option<String> {
    use crate::tiles::{Door, Mode};
    if door != Some(Door::Kit) || mode != Some(Mode::Meshes) {
        return Some(
            "the Meshes shelf is not on screen. Open the kit door on its meshes panel first (door_open with door kit and tab meshes), then select the piece."
                .to_string(),
        );
    }
    // No project is open: the plan refuses it with the editor's own sentence.
    let (Some(project), Some(filters)) = (project, filters) else {
        return None;
    };
    if !filters.keeps(crate::filter::Pane::Candidates, id) {
        return Some(format!(
            "the shelf filter hides `{id}`. Clear the filter, then select the piece."
        ));
    }
    let known = project.library.descriptors.iter().any(|d| d.id == id);
    if known && project.measured.get(id).is_none() {
        if let Some(ns) = project.other_kit_of(id) {
            return Some(format!(
                "`{id}` belongs to kit `{ns}`, not the kit being authored (`{}`).",
                project.namespace
            ));
        }
        return Some(format!(
            "`{id}` is not on the shelf, because it has no measurement yet."
        ));
    }
    None
}

/// **`grimoire/operation_apply` — run one operation.**
///
/// **The one way an agent changes the editor.** It dispatches through
/// [`crate::ops::dispatch`], which is the same function a key handler calls — so the two front
/// ends cannot drift, and the log records the agent's work in the same file the human's work is
/// recorded in.
// Each resource is its own parameter, so an app without one still runs, and the ratchet tests that
// read this signature see each one; a bundle would hide them from both.
#[allow(clippy::too_many_arguments)]
fn operation_apply(
    In(p): In<ApplyParams>,
    // **For the one thing a `ResMut` cannot do: put a resource where there was none.** `map_open` can
    // load a project from the menu's root, and installing it is `Commands::insert_resource`.
    mut commands: Commands,
    mut project: Option<ResMut<crate::project::Project>>,
    mut open: Option<ResMut<crate::project::OpenMap>>,
    mut due: Option<ResMut<crate::watch::Due>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    confirm: Option<Res<crate::confirm::Confirm>>,
    door: Option<Res<crate::tiles::Door>>,
    mode: Option<Res<crate::tiles::Mode>>,
    filters: Option<Res<crate::filter::Filters>>,
    mut import: Option<ResMut<crate::tiles::ImportState>>,
    menu: Option<Res<crate::chooser::MenuOpening>>,
    mut proposal: Option<ResMut<crate::editor::Proposal>>,
    mut clock: Option<ResMut<crate::editor::ProposalClock>>,
    // **Which screen the operation is being applied on (E-078).** `map_open` means two different
    // things and only the screen tells them apart: from a door it asks for the Map door, and from the
    // menu it is an *entry* — the same crossing the MAPS column's Enter makes. **Read the state, not
    // the presence of a resource**: `MenuOpening` is inserted at plugin build and is on every screen,
    // and `EditorState`'s absence is a fact about the editor, not a name for the menu.
    screen: Option<Res<State<crate::screen::Screen>>>,
    mut screen_next: Option<ResMut<NextState<crate::screen::Screen>>>,
    // **An agent's armed composition is the Map's (E-002)**, as a picker row's click arms it.
    mut composition_armed: Option<ResMut<crate::compose::Armed>>,
) -> Result<Value, String> {
    let Some(op) = ops::find(&p.operation) else {
        return Err(format!(
            "`{}` is not an operation this editor knows. The known ones are: {}.",
            p.operation,
            ops::registry()
                .iter()
                .map(|o| o.name())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    };
    // **A door is refused before anything is planned when the editor cannot cross to it now (gaps 6,
    // 10 and 11).** A question on screen, a leaving question, or a door already waiting owns the next
    // frame. The refusal is the editor's own sentence, and nothing is written or logged.
    if p.operation == "door_open" {
        let asking = confirm.as_deref().is_some_and(|c| c.is_open());
        if let Some(why) = state
            .as_deref()
            .and_then(|s| crate::editor::door_refusal(s, asking))
        {
            if let Some(state) = state.as_mut() {
                state.problem_agent_refusal(&p.operation, why);
            }
            return Ok(json!({
                "operation": p.operation,
                "outcome": "refused",
                "refusal": why,
            }));
        }
    }

    // **A piece is selected only from the Meshes shelf, as the shelf shows it (E-044).** A refusal
    // changes nothing and is logged nowhere, as a refused door is not.
    if p.operation == "piece_select" {
        let id = p.parameters.get("id").and_then(Value::as_str).unwrap_or_default();
        if let Some(why) = piece_select_refusal(
            door.as_deref().copied(),
            mode.as_deref().copied(),
            project.as_deref(),
            filters.as_deref(),
            id,
        ) {
            if let Some(state) = state.as_mut() {
                state.problem_agent_refusal(&p.operation, &why);
            }
            return Ok(json!({
                "operation": p.operation,
                "outcome": "refused",
                "refusal": why,
            }));
        }
    }
    // **The fence of a held note (#92)**, found before anything is written, for a preview and an
    // apply alike: a preview never says "would change" for a change the apply would refuse.
    if let Some(agent) = p.agent.as_deref() {
        let read = Ctx {
            root: None,
            project: project.as_deref().cloned(),
            open: open.as_deref().cloned(),
            source: Source::Agent,
            session: crate::ops::session_id(),
            proposal: proposal.as_deref().and_then(|r| r.0.clone()),
        };
        if let Some(why) = crate::ops_notes::fence_refusal(&read, agent, &p.operation, &p.parameters) {
            return Ok(json!({
                "operation": p.operation,
                "outcome": "refused",
                "refusal": why,
            }));
        }
    }
    // **A preview is the plan, and nothing else.**
    //
    // **And a preview takes the resources by *reference*, not by ownership** — a preview that
    // took them mutably would mark them changed, and every system gated on a change would
    // rebuild the interface for a call that changed nothing.
    if p.dry_run {
        let read_project = project.as_deref();
        let read_open = open.as_deref();
        let ctx = Ctx {
            root: menu.as_ref().map(|m| m.root.clone()),
            project: read_project.cloned(),
            open: read_open.cloned(),
            source: Source::Agent,
            session: crate::ops::session_id(),
            // **Cloned, not taken**, because a preview must not change anything — including the
            // layout standing on screen, which `layout_accept` reads.
            proposal: proposal.as_deref().and_then(|r| r.0.clone()),
        };
        return match ops::preview(&ctx, op.as_ref(), &p.parameters) {
            Ok(plan) => Ok(json!({
                "operation": p.operation,
                "outcome": "previewed",
                "would_change": plan,
            })),
            Err(e) => Ok(refused_answer(&p.operation, &e)),
        };
    }
    // **And the real thing**, which takes the editor's own map and project and gives them back.
    let mut ctx = take(&project, &open);
    // **The menu's root, when the menu is up**, so `map_create` has a directory before any door.
    ctx.root = menu.as_ref().map(|m| m.root.clone());
    // **The proposal travels by value**, in and out, because it is not map data.
    ctx.proposal = proposal.as_deref_mut().and_then(|r| r.0.take());
    // **A proposal with no seed named gets the editor's own next one.** This is what makes two
    // `layout_propose` calls differ the way two `G` presses do; a caller that names a seed gets
    // exactly that seed, which is what makes a layout reproducible.
    let mut params = p.parameters.clone();
    if p.operation == "layout_propose"
        && params.get("seed").is_none()
        && let Some(state) = state.as_deref_mut()
    {
        params["seed"] = json!(state.next_seed());
    }
    // **What the change broke is measured against what was already there (E-074).** Taken before the
    // dispatch because `ctx` still holds the project and the map it is about to change, and consumed
    // by `put_back` below.
    let checked = crate::checks::wanted(&p.operation);
    let before = crate::checks::before(&p.operation, &ctx);
    let applied = ops::dispatch(&mut ctx, &p.operation, &params);
    // **And what is there now**, while `ctx` still owns both. `None` unless the operation applied and
    // changes a document: a refusal changed nothing, and a preview never reaches here.
    let checks = (checked && matches!(applied, Ok(ops::Applied::Done { .. })))
        .then(|| crate::checks::find(&ctx).map(|after| crate::checks::report(before.as_ref(), &after)))
        .flatten();
    let carried = ctx.proposal.take();
    // **A project the operation LOADED is installed here, before the map goes back.** `map_open` can
    // load the project from the menu's root — that is what lets an agent make a map and then work in
    // it — and a write-back that skipped it left the editor holding a map and no project, which no
    // door can show. `put_back` cannot do it: it holds `Option<ResMut>`, and a `ResMut` cannot be
    // built outside a world. Measured: `map_create` applied, `map_open` applied, and the next call
    // reported an empty library because `Project` was still absent.
    // **A project the operation LOADED is installed, before the map goes back.** `map_open` can load
    // the project from the menu's root — that is what lets an agent make a map and then work in it —
    // and a write-back that skipped it left the editor holding a map and no project, which no door
    // can show. `put_back` cannot do it: it holds `Option<ResMut>`, and a `ResMut` cannot be built
    // outside a world. Measured: `map_create` applied, `map_open` applied, and the next call
    // reported an empty library because `Project` was still absent.
    //
    // The project and the map are taken together, because the map is a resource the editor can only
    // show *with* its project.
    let loaded = if project.is_none() {
        ctx.project.clone()
    } else {
        None
    };
    let opened = if open.is_none() { ctx.open.clone() } else { None };
    // **The map goes back either way** — changed, refused, or not — so the editor's own state is
    // never left holding a half-applied edit.
    put_back(&mut project, &mut open, ctx);
    // **On the menu, `map_open` enters the editor (E-078)** — and it does it the way the MAPS
    // column's Enter does, not by installing resources and hoping the screen follows.
    //
    // `Chosen` is what a crossing carries: `screen::open_the_door` consumes it on `OnExit(Menu)` and
    // installs the project and the map through `Opened::insert_into`, an exclusive system, so
    // `OnEnter(Editor)` — the very next schedule — finds them. Inserting `Project` here instead would
    // put a loaded project on a screen that has no door, and a `NextState` read one pass late would
    // run `OnEnter(Editor)` against a world with no project: the panic `open_the_door`'s own note
    // records. **So there is exactly one way into the editor, and this is it.**
    let on_menu = screen
        .as_deref()
        .is_some_and(|s| *s.get() == crate::screen::Screen::Menu);
    let entering = matches!(&applied, Ok(ops::Applied::Done { .. }))
        && on_menu
        && p.operation == "map_open";
    if entering {
        match (loaded, opened) {
            (Some(mut fresh), Some(map)) => {
                // **The project remembers the map's name**, mirroring `args::open`: a crossing from
                // the Kit door back to Maps reads it to know which map to reopen.
                fresh.open_map_name = Some(map.map.name.clone());
                commands.insert_resource(crate::chooser::Chosen(crate::args::Opened {
                    project: fresh,
                    open_map: Some(map),
                    door: crate::tiles::Door::Map,
                    mode: crate::tiles::Door::Map.opens_on(),
                }));
                if let Some(next) = screen_next.as_deref_mut() {
                    next.set(crate::screen::Screen::Editor);
                }
            }
            // **Refused rather than half-entered.** `map_open::commit` sets both on the menu, so this
            // is the shape of a defect, not of a use — and an entry that installed a project without
            // its map is the one state the editor cannot be entered in.
            _ => {
                return Ok(json!({
                    "operation": p.operation,
                    "outcome": "refused",
                    "refusal": "map_open applied but the project or the map was not loaded, so the \
                                editor was not entered",
                }));
            }
        }
    } else if matches!(&applied, Ok(ops::Applied::Done { .. })) {
        if let Some(fresh) = loaded {
            commands.insert_resource(fresh);
        }
        if let Some(fresh) = opened {
            commands.insert_resource(fresh);
        }
    }
    carry_proposal(
        &p.operation,
        carried,
        proposal.as_deref_mut(),
        clock.as_deref_mut(),
    );
    // **The screen is redrawn when the write-back saved.** The watcher adopts a file whose
    // contents already match the open map and does nothing, so nothing else would tell the
    // screen; a piece the agent turned stayed where it was until the editor was reopened.
    if let (Some(due), Ok(ops::Applied::Done { outcome, .. })) = (due.as_mut(), &applied) {
        due.map_replaced |= outcome.saved;
    }
    // **An armed piece is the editor's brush (E-037)**: the answer names the piece, and the editor
    // holds it as a palette click would.
    if let (Ok(ops::Applied::Done { outcome, .. }), Some(state), Some(project)) =
        (&applied, state.as_deref_mut(), project.as_deref())
        && p.operation == "brush_arm"
    {
        crate::editor::apply_armed(state, project, outcome.after.as_ref());
    }
    if let (Ok(ops::Applied::Done { outcome, .. }), Some(armed)) =
        (&applied, composition_armed.as_deref_mut())
        && p.operation == "composition_arm"
    {
        crate::editor::apply_composition_armed(armed, outcome.after.as_ref());
    }
    // **An agent's door is crossed by the editor in the next frame (`door_open`)**, the same crossing
    // the keys make. It is stored here and carried out by `editor::apply_pending_door`, because the
    // leaving question may need the author's answer first.
    if let (Ok(ops::Applied::Done { outcome, .. }), Some(state)) = (&applied, state.as_deref_mut())
        && p.operation == "door_open"
    {
        crate::editor::request_door(state, outcome.after.as_ref());
    }
    // **From a door, `map_open` asks for the Map door.** From the menu, it has already entered the
    // editor through `Chosen`, as Enter on the MAPS column does (E-078).
    if let (Ok(ops::Applied::Done { .. }), Some(state)) = (&applied, state.as_deref_mut())
        && p.operation == "map_open"
        && !on_menu
    {
        state.door_request = Some(crate::editor::DoorRequest {
            door: crate::tiles::Door::Map,
            tab: None,
        });
    }

    // **An agent's selection is the row a click would be (E-044).** The answer names the piece, and
    // the Meshes shelf holds it as a click holds it.
    if let (Ok(ops::Applied::Done { outcome, .. }), Some(import)) =
        (&applied, import.as_deref_mut())
        && p.operation == "piece_select"
    {
        crate::editor::apply_selected(import, outcome.after.as_ref());
    }
    // **An agent's removal leaves the shelf where a key's does (#129 slice 8)**: its mesh is a candidate
    // again, with the cursor on it.
    if let (Ok(ops::Applied::Done { outcome, .. }), Some(import), Some(project)) =
        (&applied, import.as_deref_mut(), project.as_deref())
        && p.operation == "piece_remove"
    {
        crate::tiles::follow_piece_removal(project, import, outcome.after.as_ref());
    }
    // **The agent's change joins the person's one undo history (B-019).** Logged here, so Cmd+Z
    // takes back the newest change whoever made it, not the newest the person made by hand. An
    // operation that changes no document (`Undo::None`, such as arming a piece) has nothing to take
    // back, so it stays out of the history.
    if let Ok(ops::Applied::Done {
        outcome,
        operation_id,
        ..
    }) = &applied
        && op.undo() != ops::Undo::None
    {
        join_history(
            op.writes_library(),
            state.as_deref_mut(),
            import.as_deref_mut(),
            operation_id,
            &outcome.summary,
        );
    }

    // **The person's receipt says what the agent did (B-020).** A refusal goes to the journal as a
    // problem: it is an alarm, and it must not overwrite the receipt.
    //
    // **Both receipts, because the person reads the one belonging to the door they are on (E-048).**
    // `EditorState::status` is the Map door's receipt; the kit door shows `ImportState::status`, and
    // the agent path wrote only the first — so with the kit door open, an agent's label change
    // reached the file, the live library and the chips, and said nothing in the row the author was
    // looking at. Measured on the running editor: the labels row read `agent set kind` and the words
    // `+crop -tile` appeared nowhere on screen.
    let says = match &applied {
        Ok(ops::Applied::Done { outcome, .. }) => Ok(outcome.summary.clone()),
        Ok(ops::Applied::Refused(e)) | Err(e) => Err(e.problem()),
    };
    if let Some(state) = state.as_mut() {
        match &says {
            Ok(summary) => state.receipt_agent_change(&p.operation, summary),
            Err(why) => state.problem_agent_problem(&p.operation, why.clone()),
        }
    }
    if let Some(import) = import.as_mut() {
        match &says {
            Ok(summary) => import
                .status
                .note(format!("agent: {}: {summary}", p.operation)),
            Err(why) => import.status.raise(&format!("agent: {}: ", p.operation), why.clone()),
        }
    }
    match applied {
        Ok(ops::Applied::Done {
            outcome,
            operation_id,
            entry,
        }) => {
            // **A door the editor will ask about is not yet open (cycle 8, gap 4).** The outcome is a
            // stable word, `waiting`, and `waiting_on` names what it waits for, so nobody reads "applied"
            // as "the door is open" and a caller can branch on the word.
            let waiting = outcome
                .after
                .as_ref()
                .and_then(|after| after["waiting"].as_str())
                .filter(|_| p.operation == "door_open");
            Ok(json!({
                "operation": p.operation,
                "outcome": if waiting.is_some() { "waiting" } else { "applied" },
                "waiting_on": waiting,
                "summary": outcome.summary,
                "affected": outcome.affected,
                "saved": outcome.saved,
                // **The brief's "validators run after every mutation; their failures appear in the
                // tool result" (E-074).** `null` means the operation changes no document, or no map
                // is open. A finding is not a refusal — the change happened and `outcome` says so.
                "checks": checks,
                // **The id the agent cites and the human reverts by.** The brief asks for it on every
                // mutating tool, and an operation whose id nobody can name is an operation nobody can
                // present.
                "operation_id": operation_id,
                "log_entry": entry,
            }))
        }
        // **A refusal from the dispatch itself** (`Err`) — the plan refused, or the log could not be
        // written — is a refusal like any other, and it says why.
        Ok(ops::Applied::Refused(e)) | Err(e) => Ok(refused_answer(&p.operation, &e)),
    }
}

/// **The answer to a refused operation** (UI-05): `refusal` is the sentence it always was, and
/// `problem` is the [`bevy_grimoire_core::problem::Problem`] the toast and the journal row were
/// printed from, with the `where`, `why` and `fix` the refusal has. An agent reads the same parts a
/// person sees.
pub(crate) fn refused_answer(operation: &str, refusal: &ops::Refusal) -> Value {
    json!({
        "operation": operation,
        "outcome": "refused",
        "refusal": refusal.0,
        "problem": refusal.problem().to_json(),
    })
}

/// **`grimoire/operation_undo` — take one change back, by its id.**
///
/// **A front end and nothing more.** [`ops::undo`] resolves the line, dispatches its recorded
/// inverse through the registry and appends the new line; this builds the context from the
/// editor's own resources and puts the map back, exactly as `grimoire/operation_apply` does.
fn operation_undo(
    In(p): In<UndoParams>,
    mut project: Option<ResMut<crate::project::Project>>,
    mut open: Option<ResMut<crate::project::OpenMap>>,
    mut due: Option<ResMut<crate::watch::Due>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    mut import: Option<ResMut<crate::tiles::ImportState>>,
    presenting: Option<Res<crate::present::Presenting>>,
) -> Result<Value, String> {
    // **`turn: true` takes back what was presented (E-084)**, newest first, so the agent's door and
    // the person's Cmd+Z take back the same amount. It is a loop over the id form rather than a
    // second undo mechanism: each step is the same `ops::undo(ctx, Some(id))` the id form makes, and
    // the log stays the only writer.
    if p.turn {
        let ids = presenting
            .as_ref()
            .and_then(|p| p.0.as_ref())
            .map(|p| p.operation_ids.clone());
        let Some(ids) = ids else {
            return Ok(json!({
                "operation": "undo",
                "outcome": "refused",
                "refusal": "nothing has been presented, so there is no turn to take back. \
                            Present a result first, or name one change with `operation_id`.",
            }));
        };
        if ids.is_empty() {
            return Ok(json!({
                "operation": "undo",
                "outcome": "refused",
                "refusal": "the presentation named no operations, so the turn changed nothing \
                            and there is nothing to take back.",
            }));
        }
        if p.dry_run {
            return Ok(json!({
                "operation": "undo",
                "outcome": "previewed",
                "turn": ids,
                "would_take_back": ids.len(),
            }));
        }
        // **Newest first**, because the log refuses a change while a newer one in the same scope is
        // still in effect — the same order Cmd+Z walks, and the reason `group_turn` reverses.
        let mut took_back = 0usize;
        let mut stopped: Option<String> = None;
        // **What the person's one redo must put back (E-076)**, in authoring order: the undo walked
        // newest-first, so its own lines are collected and reversed below.
        let mut redo_lines: Vec<(String, String)> = Vec::new();
        let mut took: Vec<String> = Vec::new();
        for id in ids.iter().rev() {
            let mut ctx = take(&project, &open);
            let undone = ops::undo(&mut ctx, Some(id));
            put_back(&mut project, &mut open, ctx);
            match undone {
                Ok((plan, ops::Applied::Done { outcome, operation_id, .. })) => {
                    took_back += 1;
                    if let Some(due) = due.as_mut() {
                        due.map_replaced |= outcome.saved;
                    }
                    took.push(id.clone());
                    redo_lines.push((operation_id, undo_phrase(&plan)));
                    if let Some(state) = state.as_mut() {
                        state.receipt_agent_change("undo", &outcome.summary);
                    }
                }
                Ok((_, ops::Applied::Refused(e))) | Err(e) => {
                    stopped = Some(format!("stopped at `{id}`: {}", e.0));
                    break;
                }
            }
        }
        // **One group on the redo stack, in authoring order.** The loop above ran newest-first because
        // that is the order the log allows; `record_undone_turn` reverses it, so the person's redo
        // replays the turn the way it was written and cannot be refused for reaching a change while a
        // newer one is still in effect.
        redo_lines.reverse();
        if let Some(state) = state.as_mut() {
            // **Forget the changes that were taken back, not the lines that took them back.** Each
            // `plan.line.operation_id` above is the *undo's own* new id; forgetting that leaves the
            // turn's group holding all three children, so the stack still reads "placed crate@3 …,
            // +2 …" after the turn is gone and Cmd+Z would try to take back rows that are not there.
            // Measured: `took_back: 3` with `undo_depth: 1` and the group's own phrase still on the
            // receipt row.
            let undid: Vec<&str> = took.iter().map(String::as_str).collect();
            state.record_undone_turn(&undid, redo_lines);
        }
        return Ok(json!({
            "operation": "undo",
            "outcome": if took_back == 0 { "refused" } else { "applied" },
            "took_back": took_back,
            "stopped": stopped,
        }));
    }
    if p.dry_run {
        // **A preview reads the log and plans the inverse, and writes nothing.**
        let ctx = Ctx {
            root: None,
            project: project.as_deref().cloned(),
            open: open.as_deref().cloned(),
            source: Source::Agent,
            session: crate::ops::session_id(),
            proposal: None,
        };
        let resolved = ops::undo_target(&ctx, p.operation_id.as_deref()).and_then(|plan| {
            let op = ops::find(&plan.line.op).ok_or_else(|| {
                ops::Refusal::new(format!(
                    "`{}` is not an operation this editor knows.",
                    plan.line.op
                ))
            })?;
            op.plan(&ctx, &plan.inverse).map(|_| plan)
        });
        let resolved = match rows_refusal(state.as_deref(), &ctx, p.operation_id.as_deref()) {
            Some(refusal) => Err(refusal),
            None => resolved,
        };
        return Ok(match resolved {
            Ok(plan) => json!({
                "operation": plan.line.op,
                "outcome": "previewed",
                "change": plan.change,
                "undoes": plan.line.operation_id,
                "would_set": plan.inverse,
            }),
            Err(e) => refused_answer("undo", &e),
        });
    }
    let mut ctx = take(&project, &open);
    let undone = match rows_refusal(state.as_deref(), &ctx, p.operation_id.as_deref()) {
        Some(refusal) => Err(refusal),
        None => ops::undo(&mut ctx, p.operation_id.as_deref()),
    };
    // **The map goes back either way**, so a refused undo leaves the editor holding what it held.
    put_back(&mut project, &mut open, ctx);
    // **Redrawn for the same reason as `operation_apply`:** the watcher ignores the write-back.
    if let (Some(due), Ok((_, ops::Applied::Done { outcome, .. }))) = (due.as_mut(), &undone) {
        due.map_replaced |= outcome.saved;
    }
    // **The change this undo took back leaves the person's history too (B-019)**, so the next Cmd+Z
    // reaches the newest change still in effect instead of one the log no longer holds. A put-back is
    // the opposite: the change is in effect again, so it is the newest change and joins the history.
    //
    // **And an undo, not a put-back, leaves a redo (E-076).** A person's own Cmd+Z has always done
    // both — dropped the taken-back entry and stacked the undo's line for redo — while this door only
    // dropped, so an agent could take a turn away and the person's REDO row said "nothing to redo".
    if let Ok((plan, ops::Applied::Done { outcome, operation_id, .. })) = &undone {
        // **An agent's undo or redo of a removal moves the shelf as the removal did**, so a restored
        // piece is not also listed as a candidate.
        if plan.line.op == "piece_remove"
            && let (Some(import), Some(project)) = (import.as_mut(), project.as_deref())
        {
            crate::tiles::follow_piece_removal(project, import, outcome.after.as_ref());
        }
        if plan.restores {
            let writes = ops::find(&plan.line.op).is_some_and(|o| o.writes_library());
            join_history(
                writes,
                state.as_deref_mut(),
                import.as_deref_mut(),
                operation_id,
                &outcome.summary,
            );
        } else {
            // **The phrase names the change being put back**, which is what the person's own redo
            // entry says — `plan.change` is that line's id and `outcome.summary` names the undo, so
            // the wording comes from the line's own result rather than the undo's.
            let phrase = undo_phrase(plan);
            if let Some(state) = state.as_mut() {
                state.record_undone(&plan.line.operation_id, operation_id.clone(), phrase.clone());
            }
            if let Some(import) = import.as_mut() {
                import.forget_line(&plan.line.operation_id);
                import.push_step(
                    false,
                    crate::tiles::Step::Logged {
                        line: operation_id.clone(),
                        phrase,
                    },
                );
            }
        }
    }

    // **The person's receipt says what the agent undid (B-020)**, named `undo`.
    if let Some(state) = state.as_mut() {
        match &undone {
            Ok((_, ops::Applied::Done { outcome, .. })) => {
                state.receipt_agent_change("undo", &outcome.summary);
            }
            Ok((_, ops::Applied::Refused(e))) | Err(e) => {
                state.problem_agent_problem("undo", e.problem());
            }
        }
    }
    Ok(match undone {
        Ok((
            plan,
            ops::Applied::Done {
                outcome,
                operation_id,
                entry,
            },
        )) => json!({
            "operation": entry.op,
            "outcome": "applied",
            // **The change taken back or put back**, by the id of the line that made it, and the
            // line whose inverse did it.
            "change": plan.change,
            "undoes": entry.undoes,
            "summary": outcome.summary,
            "saved": outcome.saved,
            "set": entry.params,
            // **The undo's own id**, which is what a redo names.
            "operation_id": operation_id,
            "log_entry": entry,
        }),
        Ok((_, ops::Applied::Refused(e))) | Err(e) => refused_answer("undo", &e),
    })
}

/// **An undo of a removal waits while a change recorded by place sits above it (gaps 1 and 4).** The
/// removal's undo reinserts or drops a row, or puts a stamp back at its index, and every change the
/// person made after it names its piece by that place. A placement removal's undo moves rows, so a
/// row-recorded change above it counts; a stamp removal's undo moves stamps, so an index-recorded
/// stamp change above it counts. Undoing it now would make that change point at the wrong thing later.
/// The person takes their later changes back first, and only then does the agent's undo go through.
/// The editor's own stack holds those changes, which the operation log does not, so this checks that
/// stack rather than the log.
fn rows_refusal(
    state: Option<&crate::editor::EditorState>,
    ctx: &Ctx,
    target: Option<&str>,
) -> Option<ops::Refusal> {
    let state = state?;
    let plan = ops::undo_target(ctx, target).ok()?;
    let id = &plan.line.operation_id;
    match plan.line.op.as_str() {
        "placement_remove" if state.row_recorded_above(id) => Some(ops::Refusal::new(format!(
            "Undoing `{id}` would move every piece after its row, and the person has changed the map since \
             with a change recorded by row number (a move, a turn, a lift, a tip, or a placement that replaced pieces). That \
             change would then point at the wrong piece. Have the person take their later changes back \
             (Cmd+Z) first, then undo `{id}`."
        ))),
        "stamp_remove" if state.stamp_recorded_above(id) => Some(ops::Refusal::new(format!(
            "Undoing `{id}` would put a stamp back in its place and move every stamp after it, and the \
             person has changed the stamps since with a change recorded by place (a stamp taken off or \
             put back by Cmd+Z). That change would then point at the wrong stamp. Have the person take \
             their later changes back (Cmd+Z) first, then undo `{id}`."
        ))),
        _ => None,
    }
}

/// **`grimoire/oplog_read` — the log, as lines.**
///
/// **Capped and paged, and it says how to narrow.** A log is append-only and a busy session makes
/// a long one, so the answer carries the next sequence number to ask from and a sentence saying
/// so — the brief's rule for a result that hits its cap.
fn oplog_read(
    In(p): In<LogParams>,
    project: Option<ResMut<crate::project::Project>>,
) -> Result<Value, String> {
    let Some(project) = project else {
        return Err("no project is open, so there is no log to read.".to_owned());
    };
    let path = ops::oplog_path(&project);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let total = lines.len() as u64;
    let from = (p.since_seq as usize).min(lines.len());
    let take = p.limit.clamp(1, 200);
    let end = (from + take).min(lines.len());
    let entries: Vec<Value> = lines[from..end]
        .iter()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    let next = if end < lines.len() {
        Some(end as u64)
    } else {
        None
    };
    Ok(json!({
        "entries": entries,
        "total": total,
        "next_seq": next,
        "how_to_narrow": next.map(|n| format!("pass since_seq={n} to read the rest")),
    }))
}

/// **The target of `grimoire/frame_view`, as the caller writes it.** A bare word names a target (`"map"`
/// or `"selection"`); an object names placements or a rectangle.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum FrameTargetParam {
    Named(String),
    Placements(PlacementsParam),
    Rect(RectParam),
}

/// `{"placements": [ids]}` — the ids as the map lists them.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementsParam {
    pub placements: Vec<String>,
}

/// `{"rect": [x0, z0, x1, z1]}` — in map metres.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RectParam {
    pub rect: [f32; 4],
}

/// **The parameters of `grimoire/frame_view`.** `deny_unknown_fields`, so a typo is a refusal.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameParams {
    pub target: FrameTargetParam,
}

/// **The parameters of `grimoire/run_command`.** `deny_unknown_fields`, so a typo is a refusal.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunCommandParams {
    /// The command's id, `<category>.<verb>` (`edit.undo`), as the command registry names it.
    pub id: String,
}

/// **`grimoire/run_command` — run a registered command by id** (UI-90, #256).
///
/// **The agent's door onto [`crate::commands`].** The command's chord is pressed for one frame and the
/// editor's own dispatcher answers, so `edit.undo` undoes exactly as Cmd+Z does, through the same
/// system, with the same receipt and the same undo stack ([`crate::command_run`] says why a key and
/// not a call). **It runs nothing the key would refuse**: a command that is not enabled now is refused
/// with the registry's own reason ([`crate::commands::Command::enabled`]) and nothing is pressed.
///
/// **`ran` means the chord is queued, not that the verb finished**; the change lands in the next frame
/// or two and the caller reads it from `grimoire/state`. An unknown id is refused and names the ids
/// nearest to it.
fn run_command(
    In(p): In<RunCommandParams>,
    screen: Option<Res<State<crate::screen::Screen>>>,
    live: Option<Res<crate::keys::Live>>,
    chords: Option<ResMut<crate::command_run::Chords>>,
    state: Option<ResMut<crate::editor::EditorState>>,
) -> Result<Value, String> {
    fn refused(id: &str, why: impl Into<String>) -> Value {
        json!({"outcome": "refused", "id": id, "why": why.into()})
    }
    let Some(command) = crate::commands::by_id(&p.id) else {
        let ids: Vec<&str> = crate::commands::all().into_iter().map(|c| c.id).collect();
        let mut near: Vec<(usize, &str)> = ids
            .iter()
            .map(|id| (bevy_grimoire_core::vocab::edit_distance(id, &p.id), *id))
            .collect();
        near.sort();
        let mut did_you_mean: Vec<&str> = near.into_iter().take(3).map(|(_, id)| id).collect();
        // **An id that is a part of another names it too**: `undo` finds `edit.undo` and `tiles.undo`.
        let parts: Vec<&str> = ids
            .iter()
            .copied()
            .filter(|id| !p.id.is_empty() && id.contains(p.id.as_str()) && !did_you_mean.contains(id))
            .take(5)
            .collect();
        did_you_mean.extend(parts);
        return Ok(json!({
            "outcome": "refused",
            "id": p.id,
            "why": format!("`{}` is not a command id; ids are `<category>.<verb>`, such as `edit.undo`", p.id),
            "did_you_mean": did_you_mean,
        }));
    };
    if screen.as_deref().is_some_and(|s| *s.get() != crate::screen::Screen::Editor) {
        return Ok(refused(command.id, "The launcher is open; commands run in the workbench"));
    }
    let (Some(live), Some(mut chords)) = (live, chords) else {
        return Ok(refused(command.id, "The editor is not ready for keys yet"));
    };
    if let Err(why) = command.enabled(*live) {
        return Ok(refused(command.id, why));
    }
    chords.queue(&command);
    if let Some(mut state) = state {
        state.status.note(format!("an agent ran {}", command.title));
    }
    Ok(json!({"outcome": "ran", "id": command.id, "title": command.title, "shortcut": command.shortcut_label()}))
}

/// **`grimoire/frame_view` — frame the Map camera on a box.**
///
/// **Camera state only.** It moves the view, not the map: it is not an operation, it is not in the
/// registry or the operation log, and it is not undoable. It resolves and applies through
/// [`crate::editor::resolve_frame`] and [`crate::editor::apply_frame`], the one place a frame is computed.
///
/// **The person sees why the view moved**: a framed call leaves a receipt naming what was framed and
/// that an agent asked, so a camera that jumps under the author's eyes is never unexplained.
///
/// **A refusal moves nothing.** Every name is resolved before the camera is touched.
fn frame_view(
    In(p): In<FrameParams>,
    project: Option<Res<crate::project::Project>>,
    open: Option<Res<crate::project::OpenMap>>,
    lock: Option<Res<crate::editor::TargetLock>>,
    rig: Option<ResMut<crate::view::Rig>>,
    cameras: Query<&Camera, With<crate::view::MainCamera>>,
    state: Option<ResMut<crate::editor::EditorState>>,
) -> Result<Value, String> {
    fn refused(why: impl Into<String>) -> Value {
        json!({"outcome": "refused", "why": why.into()})
    }
    let (Some(project), Some(open), Some(mut rig)) = (project, open, rig) else {
        return Ok(refused("no map is open, so there is nothing to frame"));
    };
    let target = match p.target {
        FrameTargetParam::Named(name) => match name.as_str() {
            "map" => crate::editor::FrameTarget::Map,
            "selection" => crate::editor::FrameTarget::Selection,
            other => {
                return Ok(refused(format!(
                    "`{other}` is not a frame target: say \"map\", \"selection\", {{\"placements\": [ids]}} or {{\"rect\": [x0, z0, x1, z1]}}"
                )));
            }
        },
        FrameTargetParam::Placements(list) => {
            crate::editor::FrameTarget::Placements(list.placements)
        }
        FrameTargetParam::Rect(r) => crate::editor::FrameTarget::Rect(r.rect),
    };
    let locked = lock
        .as_deref()
        .and_then(|l| l.placement_id().map(str::to_owned));
    let framing = match crate::editor::resolve_frame(&target, &project, &open, locked.as_deref()) {
        Ok(framing) => framing,
        Err(why) => return Ok(refused(why)),
    };
    let Some(aspect) = cameras.single().ok().and_then(crate::editor::aspect_of) else {
        return Ok(refused(
            "the Map camera has no viewport yet, so there is nothing to frame against",
        ));
    };
    let (focus, height) = crate::editor::apply_frame(&mut rig, &framing, aspect, &open);
    if let Some(mut state) = state {
        state.status.note(format!(
            "an agent framed {} ({:.0} x {:.0} m)",
            framing.what, framing.size.x, framing.size.z,
        ));
    }
    Ok(json!({
        "outcome": "framed",
        "what": framing.what,
        "focus": [focus.x, focus.y, focus.z],
        "height_m": height,
    }))
}

/// **Register the agent's commands.
///
/// **Under the `debugger` feature**, because the transport is what carries them and a build
/// without it has no way to reach them.
pub struct OpsBridgePlugin;

impl Plugin for OpsBridgePlugin {
    fn build(&self, app: &mut App) {
        use bevy_debugger_bevy::{DebugCommandInfo, DebugCommandsAppExt};

        if !app.is_plugin_added::<bevy::diagnostic::FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default());
        }
        app.init_resource::<FrameBusy>()
            .add_systems(First, frame_busy_start)
            .add_systems(Last, frame_busy_end);
        if !app.is_plugin_added::<crate::command_run::CommandRunPlugin>() {
            app.add_plugins(crate::command_run::CommandRunPlugin);
        }

        app.add_debug_command(
            DebugCommandInfo::new(
                "grimoire/project_root",
                "Which project the editor has open, and which map. A read.",
            )
            .example(json!({})),
            project_root,
        );

        app.add_debug_command(
            DebugCommandInfo::new(
                "grimoire/state",
                "What the editor is holding right now: the open map and its bounds, how many \
                 pieces it has, what is armed, what is selected, which door and tab are open, and \
                 what the editor last said, and which placements are pinned and why. A read.",
            )
            .example(json!({})),
            editor_state,
        );

        app.add_debug_command(
            DebugCommandInfo::new(
                "grimoire/pieces",
                "The library's pieces, a page at a time: each row's id, the mesh it is drawn \
                 with, its kinds, effects and looks, and its label provenance (who labelled each \
                 axis, the record's model, date and confidence). Filters combine: with_mesh, \
                 unlabelled, by_model, confidence (high, medium or low), id_contains. pieces_from \
                 is the page to read, counted in the filtered list; total is the true filtered \
                 count and pieces_next says where the next page starts. A read.",
            )
            .params("pieces_from: the first piece to name in the filtered list (default 0). with_mesh: true to leave out pieces with no mesh (default false). unlabelled: true to keep only pieces no one has labelled (default false). by_model: true to keep pieces with an axis a model or an agent wrote and no person confirmed (default false). confidence: high, medium or low, keeps only that label confidence (default none). id_contains: keeps pieces whose id contains this text (default none).")
            .example(json!({"by_model": true, "confidence": "low"})),
            editor_pieces,
        );

        app.add_debug_command(
            DebugCommandInfo::new(
                "grimoire/capture_sequence_start",
                "Begin a capture sequence: the editor takes the frames, so frame 0 is the state \
                 before the operation and the operation is applied after it. Returns the run's id; \
                 poll grimoire/capture_sequence_status with it.",
            )
            .params("label: a snake_case name for the sequence. out_dir: the absolute directory the \
                     frames are written to. seconds: how long the sequence runs. fps: 4 by default; any \
                     whole rate from 2 to 20 a second, and a higher rate costs frame time while recording. operation: an optional operation to apply once frame 0 is \
                     in.")
            .example(json!({
                "label": "fill_preview",
                "out_dir": "/tmp/seq",
                "seconds": 2.0,
                "fps": 4,
            })),
            capture_sequence_start,
        );

        app.add_debug_command(
            DebugCommandInfo::new(
                "grimoire/capture_sequence_status",
                "What a capture sequence is doing: running, done, or failed, and the frames it \
                 wrote. A read.",
            )
            .params("id: the run's id, as grimoire/capture_sequence_start returned it.")
            .example(json!({"id": 1})),
            capture_sequence_status,
        );

        app.add_debug_command(
            DebugCommandInfo::new(
                "grimoire/operation_list",
                "Every operation this editor knows, with its parameters' JSON schema and how it is \
                 undone. Read this before calling `grimoire/operation_apply`.",
            )
            .example(json!({})),
            operation_list,
        );

        app.add_debug_command(
            DebugCommandInfo::new(
                "grimoire/operation_preview",
                "What an operation would change, and nothing changed. The map, the file and the log \
                 are all left alone. Use this to check a proposal before applying it.",
            )
            .params("operation: the operation's name, as `grimoire/operation_list` gives it. \
                     parameters: the operation's own parameters, as its schema describes them.")
            .example(json!({
                "operation": "map_set_settings",
                "parameters": {"bounds": [64.0, 8.0, 64.0]}
            })),
            operation_preview,
        );

        app.add_debug_command(
            DebugCommandInfo::new(
                "grimoire/operation_apply",
                "Run one operation and record it in the operation log. `dry_run` is false by \
                 default for a small edit and true for one that changes the map's shape; set it \
                 explicitly. A refusal changes nothing.",
            )
            .params(
                "operation: the operation's name. parameters: the operation's own parameters. \
                     dry_run: true previews and writes nothing; false applies.",
            )
            .example(json!({
                "operation": "map_set_settings",
                "parameters": {"bounds": [64.0, 8.0, 64.0]},
                "dry_run": true
            })),
            operation_apply,
        );

        app.add_debug_command(
            DebugCommandInfo::new(
                "grimoire/operation_undo",
                "Take one change back by its operation id: the editor dispatches the inverse the \
                 change recorded and appends a new log line naming the line it undoes. Refuses \
                 when a newer change exists, naming it. Previews unless dry_run is false.",
            )
            .params(
                "operation_id: the id to take back; omitted means the newest change not yet \
                     taken back. dry_run: true (the default) previews and changes nothing.",
            )
            .example(json!({"operation_id": "0003-ab12", "dry_run": false})),
            operation_undo,
        );

        app.add_debug_command(
            DebugCommandInfo::new(
                "grimoire/oplog_read",
                "The operation log, newest last, as parsed entries. Every operation writes one \
                 line, from a person or an agent, and undo is a new line rather than a deletion.",
            )
            .params(
                "since_seq: read from this sequence number on; 0 is the beginning. limit: how \
                     many entries at most, 1 to 200.",
            )
            .example(json!({"since_seq": 0, "limit": 50})),
            oplog_read,
        );

        app.add_debug_command(
            DebugCommandInfo::new(
                "grimoire/frame_view",
                "Frame the Map camera on the whole map, on the locked placement, on named placements, or \
                 on a rectangle in map space. Moves only the camera: the map does not change, and the \
                 change is not logged or undoable. A refusal moves nothing.",
            )
            .params(
                "target: \"map\"; \"selection\" (the locked placement, refused when nothing is locked); \
                     {\"placements\": [ids]}; or {\"rect\": [x0, z0, x1, z1]} in map metres.",
            )
            .example(json!({"target": {"rect": [200.0, 100.0, 400.0, 300.0]}})),
            frame_view,
        );
        app.add_debug_command(
            DebugCommandInfo::new(
                "grimoire/run_command",
                "Run a registered editor command by id, as its key would: `edit.undo` undoes exactly \
                 as Cmd+Z does. Refused, with the registry's reason, when the command is not enabled \
                 now (the stance, the tab or a text field refuses it); an unknown id names the nearest \
                 ids. `ran` means the key is queued: read grimoire/state for the result.",
            )
            .params("id: the command's id, `<category>.<verb>`, such as `edit.undo` or `edit.redo`.")
            .example(json!({"id": "edit.undo"})),
            run_command,
        );
        app.add_debug_command(
            DebugCommandInfo::new(
                "grimoire/present",
                "Put a result in front of the person: a title, what was made, what they asked for, \
                 what you decided on your own, and the operations it covers. The editor asks them \
                 Keep or Undo all, and their answer comes back in grimoire/state's \
                 presentation.answer. This returns at once and changes no map data. Parameters: \
                 title and summary (required); specified and inferred (strings); check, up to \
                 three {what, placements}; operation_ids, the operations it covers.",
            )
            .example(json!({
                "title": "courtyard",
                "summary": "A 12 x 4 x 12 m courtyard with two exits, floor filled and walls placed.",
                "specified": ["a courtyard", "two exits"],
                "inferred": ["12 x 4 x 12 m"],
                "check": [{"what": "the east exit is clear", "placements": []}],
                "operation_ids": [],
            })),
            editor_present,
        );
    }
}

/// **`grimoire/present`** — put a result in front of the person.
///
/// **It returns at once**, because an MCP call that blocked on a person would deadlock with the
/// editor's own frame. The answer is read from `grimoire/state`, which is the only channel an agent
/// has for what happened after its call returned.
fn editor_present(
    In(p): In<crate::present::PresentParams>,
    project: Option<Res<crate::project::Project>>,
    mut confirm: Option<ResMut<crate::confirm::Confirm>>,
    mut presenting: Option<ResMut<crate::present::Presenting>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
) -> Result<Value, String> {
    let Some(confirm) = confirm.as_mut() else {
        return Err("this editor has no confirm modal, so nothing can be asked".to_owned());
    };
    if confirm.is_open() {
        return Err("another question is on screen — answer it first".to_owned());
    }
    if p.title.trim().is_empty() {
        return Err("a presentation needs a title".to_owned());
    }
    if p.check.len() > crate::present::MAX_CHECKS {
        return Err(format!(
            "at most {} places to check — pick the ones most worth a look",
            crate::present::MAX_CHECKS
        ));
    }
    // **Every operation id has to be in the log**, because Undo all walks them and an id the log does
    // not hold would make the walk stop on something that never happened.
    //
    // **The log is read through `ops::read_entries`**, the same parse `grimoire/oplog_read` makes one
    // screen over — two parses would be two answers to "is this id real".
    let log = project
        .as_ref()
        .map(|p| ops::read_entries(&ops::oplog_path(p)))
        .unwrap_or_default();
    for id in &p.operation_ids {
        if !log.iter().any(|e| &e.operation_id == id) {
            return Err(format!("`{id}` is not in the operation log"));
        }
    }
    // **The steps are the log's own `result` lines**, never raw logs: that is the rule the Mixar beat
    // carries, and it is why the agent cites ids rather than describing what it did. The order is the
    // caller's, so the person reads the turn as the agent narrated it.
    let steps: Vec<String> = p
        .operation_ids
        .iter()
        .filter_map(|id| {
            log.iter()
                .find(|e| &e.operation_id == id)
                .and_then(|e| e.result.clone())
        })
        .collect();
    let body = crate::present::body(&p, &steps);
    confirm.ask(
        crate::confirm::Asked::Presentation,
        p.title.clone(),
        body,
        // **Yes is the destructive answer**, so `Enter` answers nothing and the button says how much
        // it takes back: a turn's worth of changes must not be discarded by a label that reads "Yes".
        format!("Undo {} operation{}", p.operation_ids.len(), if p.operation_ids.len() == 1 { "" } else { "s" }),
        "Keep",
    );
    let Some(presenting) = presenting.as_mut() else {
        return Err("this editor has no presentation slot".to_owned());
    };
    let n = presenting
        .0
        .as_ref()
        .and_then(|prev| prev.id.rsplit('-').next())
        .and_then(|n| n.parse::<u32>().ok())
        .map(|n| n + 1)
        .unwrap_or(1);
    let id = format!("present-{n}");
    presenting.0 = Some(crate::present::Presented {
        id: id.clone(),
        title: p.title.clone(),
        operation_ids: p.operation_ids.clone(),
        answer: None,
    });
    // **The turn is one undo step from here on (E-071).** Done at presentation rather than at the
    // answer, so Cmd+Z during the conversation takes the turn back — a person who does not want the
    // result does not have to find the confirmation first, and `Undo all` and Cmd+Z then take back
    // the same amount.
    let grouped = state
        .as_mut()
        .map(|state| state.group_turn(&p.operation_ids))
        .unwrap_or(0);
    Ok(json!({
        "presentation": id,
        "steps": steps.len(),
        // **How many of the turn's entries became one.** `0` means nothing was grouped — a turn of
        // one operation, or ids the stack no longer holds — and the answer says so rather than
        // claiming a single step it did not make.
        "grouped": grouped,
    }))
}

/// **What a redo of this undo would put back, in the author's words (E-076).**
///
/// The person's own Cmd+Z leaves a redo entry whose phrase names the change being put back, and the
/// agent's door must say the same words or the two REDO rows would read differently for the same
/// state. `plan.line` is the line whose inverse was dispatched — the change itself — so its own
/// `result` is that name; a line written before `Entry::result` existed, or one whose commit answered
/// no summary, falls back to the operation's name rather than inventing a sentence.
fn undo_phrase(plan: &ops::UndoPlan) -> String {
    plan.line
        .result
        .clone()
        .unwrap_or_else(|| plan.line.op.clone())
}

/// **Which history a logged change joins (cycle 9 gap 1).** A library write joins the Meshes history,
/// beside the person's Meshes edits, so one Cmd+Z there takes back the newest library change whoever
/// made it. Every other change joins the Map history, as it always has. `TilesPlugin` inserts
/// `ImportState`, so a library write falls back to the Map history only in an app built without it.
pub(crate) fn join_history(
    writes_library: bool,
    state: Option<&mut crate::editor::EditorState>,
    import: Option<&mut crate::tiles::ImportState>,
    line: &str,
    summary: &str,
) {
    match (writes_library, import, state) {
        (true, Some(import), _) => import.record_logged(line.to_string(), summary.to_string()),
        (_, _, Some(state)) => state.record_logged(line.to_string(), summary.to_string(), true),
        _ => {}
    }
}

/// **`data.ron`'s files, with each one's leaf count**, read from disk so a hand edit shows at once.
/// Empty with no registry; a malformed one reads as `{"error": ..}`.
fn data_files_json(grimoire_dir: &std::path::Path) -> Value {
    use bevy_grimoire_core::data_files::{DataFiles, leaves};
    match DataFiles::load(grimoire_dir) {
        Ok(None) => json!([]),
        Err(e) => json!({"error": e}),
        Ok(Some(d)) => d
            .files
            .iter()
            .map(|f| {
                let n = std::fs::read_to_string(d.resolve(f)).ok().and_then(|t| leaves(&t).ok());
                json!({"id": f.id, "path": f.path, "leaves": n.map(|l| l.len())})
            })
            .collect(),
    }
}
