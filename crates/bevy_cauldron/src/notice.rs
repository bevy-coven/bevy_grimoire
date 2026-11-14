//! **The two verbs that act on what a tab has just said**: read it out, and take it down.
//!
//! [`crate::chrome::Status`] is where a message lives and what makes a refusal stick. This is the
//! other end — the author is finished with it, either because they have read it (`Esc`) or because
//! they have taken it somewhere it can be acted on (`Cmd+C`).
//!
//! # Why one module knows about all four tabs
//!
//! Because the alternative is four copies of one behaviour. Each tab keeps its own `Status` inside
//! the state it already owns, so a system that reads "the live tab's status" has to name all four —
//! and `editor::not_typing` set that precedent for exactly this shape, listing five kinds of state
//! from two tabs with the note that *"every field is listed here and nowhere else, so adding one is
//! adding a line."* Four near-identical `dismiss` systems, one per tab file, is the drift
//! `crate::chrome` exists to prevent, one level up.
//!
//! # Copying is for an agent, and that is why it is Global
//!
//! `bevy_ui` has no selectable text. A refusal an author cannot get out of the window is one that
//! has to be retyped out of a screenshot before anybody else can help with it — so the verb belongs
//! wherever a refusal can happen, which is now every tab. It was `Context::Meshes`, copying that
//! tab's detail pane.

use bevy::prelude::*;

use crate::keys::{self, Action};
use crate::tiles::Mode;

/// **A panel whose text is worth copying**, and which tabs it belongs to.
///
/// The tabs are on the marker rather than inferred from the panel's visibility, because a hidden
/// panel's `Text` still exists — `chrome::panel_root` hides with `Display::None`, which keeps the
/// entities. Asking "is this node drawn" would mean walking ancestors; naming the tabs is one field.
///
/// **A list**, because the Meshes and Tiles tabs share one detail pane: tagging it `Meshes` alone made
/// `Cmd+C` on Tiles harvest the status lines and none of the tile — no id, no envelope, no member
/// list — in an editor where `bevy_ui` offers no other way to get that text out of the window. It is
/// the only marker left with a tab list; `chrome::ProblemBanner` had one and it decided nothing, so
/// it is a bare marker now.
#[derive(Component, Clone, Copy)]
pub struct CopyPane(pub &'static [Mode]);

/// What the notice panels currently show, so they are rebuilt on a change and not on a frame.
///
/// `badges::ShowingFor` does the same for the key badges, for the same reason: *"static text
/// respawned sixty times a second would be sixty times the work for one picture."* It matters more
/// here — `EditorState` is written at drag rate, so a system gated on `is_changed()` would rebuild
/// the log while the cursor moves.
#[derive(Resource, Default, PartialEq, Eq)]
pub(crate) struct Showing {
    /// Which tab's status is live, as [`source_of`] says it: the Meshes and Tiles tabs share one.
    source: Option<u8>,
}

/// **Which status a tab reads and writes.** The Meshes and Tiles tabs report through one
/// `ImportState`, so switching between them is not arriving on a different list of refusals: a toast
/// the person dismissed must not come back because they changed tab.
fn source_of(tab: Mode) -> u8 {
    match tab {
        Mode::Map | Mode::World | Mode::Sound | Mode::Game | Mode::Agent | Mode::Files | Mode::Project | Mode::Design => 0,
        Mode::Meshes | Mode::Tiles => 1,
        Mode::Anim => 2,
        Mode::Compose => 3,
    }
}

/// **What the toast is about, and whether a new one is owed.**
///
/// `shown` is the line, so the same problem does not re-toast every frame — and a *repeated* one
/// does, because `chrome::Problem::line` folds consecutive repeats into `(x2)` and that is a
/// different string. Pressing a refused key again therefore says so again, which is the whole point
/// of feedback on a gesture. `cause` is what the refusal names ([`crate::chrome::Problem::cause`]).
#[derive(Resource, Default)]
pub(crate) struct Toast {
    shown: Option<String>,
    cause: Option<String>,
    /// The line or the tab changed since the toast was last raised: [`raise_toast`] answers it once.
    fresh: bool,
}

/// A toast this module raised for the live tab's newest refusal, and what that refusal names.
#[derive(Component)]
struct RefusalToast {
    cause: Option<String>,
}

/// The id of the Show button's action.
const SHOW: &str = "show";

pub struct NoticePlugin;

impl Plugin for NoticePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Showing>()
            .init_resource::<Toast>()
            .init_resource::<crate::chrome::Journal>()
            .add_observer(show_the_cause)
            .add_systems(
                OnEnter(crate::screen::Screen::Editor),
                crate::chrome::journal_panel.after(crate::chrome::FrameSystems),
            )
            .add_systems(
                Update,
                ((
                    (dismiss, copy_out, toggle_journal).in_set(keys::Phase::Act),
                    // **Before the paint**, so a refusal raised this frame is in the journal before
                    // anything reads it — and outside `Phase::Act`, because it watches state rather
                    // than keys.
                    record_problems.after(keys::Phase::Act),
                    record_steps.after(keys::Phase::Act),
                    paint_journal.after(record_problems).after(record_steps),
                    // After the verbs, so a problem raised this frame is on screen this frame.
                    paint_notices.after(keys::Phase::Act),
                    // And after the paint, so a toast is raised in the frame its refusal is. **Before gooey's style set** (UI-R06, #336):
                    // gooey gives a toast its entrance (the slide in from its edge, `begin_entrances`) in the frame it first sees it, and a
                    // toast raised after that set stood at its place for one drawn frame and then jumped to where the slide begins.
                    (raise_toast, mark_the_words).chain().after(paint_notices).before(bevy_gooey::GooeySystems::Style),
                    finish_show,
                ),)
                    .run_if(in_state(crate::screen::Screen::Editor)),
            );
    }
}

/// The four tabs' status sources, read together. Only the live tab's is looked at.
#[derive(bevy::ecs::system::SystemParam)]
struct TabStatuses<'w> {
    editor: Res<'w, crate::editor::EditorState>,
    import: Res<'w, crate::tiles::ImportState>,
    bench: Res<'w, crate::anim_tab::BenchState>,
    compose: Res<'w, crate::compose::ComposeState>,
}

/// The same four, for writing: `copy_out` answers the live tab's status by dismissing it.
#[derive(bevy::ecs::system::SystemParam)]
struct TabStatusesMut<'w> {
    editor: ResMut<'w, crate::editor::EditorState>,
    import: ResMut<'w, crate::tiles::ImportState>,
    bench: ResMut<'w, crate::anim_tab::BenchState>,
    compose: ResMut<'w, crate::compose::ComposeState>,
}

/// The UI nodes `copy_out` reads text from: the copy panes, their children, their text and layout.
#[derive(bevy::ecs::system::SystemParam)]
struct UiTree<'w, 's> {
    panes: Query<'w, 's, (Entity, &'static CopyPane)>,
    children: Query<'w, 's, &'static Children>,
    texts: Query<'w, 's, &'static Text>,
    nodes: Query<'w, 's, &'static Node>,
}

/// The roots `copy_out` walks, the frame first. See the note on the param.
#[derive(bevy::ecs::system::SystemParam)]
struct UiRoots<'w, 's> {
    frame: Option<Res<'w, crate::chrome::Frame>>,
    other_roots: Query<'w, 's, Entity, (With<Node>, Without<ChildOf>)>,
}

/// **What the live tab's newest problem is, and whether the toast owes a new one.**
///
/// One system rather than four, because the four tabs' statuses are one list read one way — four
/// tabs each painting their own was how the severity rule drifted four ways in the first place.
///
/// Only the live tab is read, and that is enough: `chrome::panel_root` hides an inactive tab with
/// `Display::None`, so the other three panels are not on screen to be wrong.
fn paint_notices(mode: Res<Mode>, tabs: TabStatuses, mut showing: ResMut<Showing>, mut toast: ResMut<Toast>) {
    let TabStatuses { editor, import, bench, compose } = tabs;
    let tab = *mode;
    let status = match tab {
        Mode::Map | Mode::World | Mode::Sound | Mode::Game | Mode::Agent | Mode::Files | Mode::Project | Mode::Design => &editor.status,
        // **Both tabs report through `ImportState`.** The Tiles tab's own verbs write there too —
        // one status line for one file's worth of work, rather than a second channel to keep in step.
        Mode::Meshes | Mode::Tiles => &import.status,
        Mode::Anim => &bench.status,
        Mode::Compose => &compose.status,
    };

    // **The newest problem is what the toast says**, and a change to it is what raises a new toast.
    // A tab change raises one too: arriving on a tab that already has a refusal should say so, and
    // `showing.source` is still the tab's we are leaving at this point in the frame.
    let newest = status.problems().last().map(|p| p.line());
    if toast.shown != newest || showing.source != Some(source_of(tab)) {
        toast.cause = status.problems().last().and_then(|p| p.cause()).map(str::to_owned);
        toast.shown = newest;
        toast.fresh = true;
    }
    // **Remembered here, and this is load-bearing.** The check above compares against it, so a
    // `Showing` nothing ever wrote would make every frame look like a tab change and the toast would
    // be raised again every frame.
    if showing.source != Some(source_of(tab)) {
        showing.source = Some(source_of(tab));
    }
}

/// Everything a tab has to say, in the order an agent wants to read it.
///
/// **The problem first.** The old Tiles-only copy put the status line last, after the pane, on the
/// argument that it was the line the pane did not hold — true, but it buried the one line somebody
/// pasting this into a message is usually asking about.
fn harvest(
    status: &crate::chrome::Status,
    tab: Mode,
    panes: &Query<(Entity, &CopyPane)>,
    roots: &[Entity],
    children: &Query<&Children>,
    texts: &Query<&Text>,
    nodes: &Query<&Node>,
) -> Vec<String> {
    let mut lines = Vec::new();
    // **The whole log, not just the newest.** The banner shows one; a copy is what somebody pastes
    // into a message for help, and the run is what makes a sequence of failures diagnosable.
    // Newest first, matching the order on screen.
    for p in status.problems().iter().rev() {
        lines.push(format!("problem: {}", p.line()));
    }
    if status.dropped() > 0 {
        lines.push(format!(
            "problem: (+{} earlier, dropped at {})",
            status.dropped(),
            crate::chrome::MAX_PROBLEMS
        ));
    }
    // **Everything on the screen, not just this tab's detail pane.**
    //
    // Asked for at the keyboard, 2026-08-18, and the reason was visible in the request: the author
    // was hand-transcribing the mesh list and the header into a message because the copy did not
    // reach them. It carried the `CopyPane` panes and nothing else — so the id, the measurements
    // and the findings came through, while the list they were chosen from, the kit the header
    // names, and the tab you were on did not. A paste that needs a covering note to be legible is
    // not a copy of the screen.
    //
    // Walked from the frame's root in child order, which IS reading order: `chrome::spawn_frame`
    // builds a column of chrome bar, door strip, body — itself a row of left dock, viewport, right
    // dock — and status bar. Overlays are roots of their own and follow.
    for root in roots {
        collect_visible(*root, children, texts, nodes, &mut lines);
    }
    // The note is on screen and therefore already harvested; `CopyPane` is now only a marker of
    // where the panes are, which nothing here needs to consult.
    let _ = (panes, tab);
    lines
}

/// Depth-first text harvest under one UI node, in child order — which is the order it reads.
///
/// **`Display::None` prunes the whole subtree**, and that is what makes walking from the root safe
/// rather than absurd: every tab's panel exists at once and the four that are not showing are
/// hidden exactly that way (`chrome::panel_root`, which chose `Display::None` over `Visibility` so
/// a hidden panel holds no layout and answers no hover). Without this check a copy of the Meshes
/// tab would arrive carrying the Map, Anim and Compose panels underneath it.
fn collect_visible(
    root: Entity,
    children: &Query<&Children>,
    texts: &Query<&Text>,
    nodes: &Query<&Node>,
    out: &mut Vec<String>,
) {
    if nodes.get(root).is_ok_and(|n| n.display == Display::None) {
        return;
    }
    if let Ok(t) = texts.get(root)
        && !t.0.trim().is_empty()
    {
        out.push(t.0.clone());
    }
    if let Ok(kids) = children.get(root) {
        for kid in kids {
            collect_visible(*kid, children, texts, nodes, out);
        }
    }
}

/// **Take the problem block down** — `Esc`, the census's one key for "not that".
///
/// **The Map tab is deliberately absent from this match.** `Action::Cancel` peels one layer per
/// press there — a piece in hand, an armed tool, an armed piece — and the block is the outermost of
/// them, so `editor::keys` clears it itself and returns rather than letting one press do two things.
/// Clearing it here as well would take the block down *and* put the held piece back on the same
/// keystroke, which is exactly the promise that comment makes and this would break.
fn dismiss(
    keyboard: Res<ButtonInput<KeyCode>>,
    live: Res<keys::Live>,
    mode: Res<Mode>,
    mut import: ResMut<crate::tiles::ImportState>,
    mut bench: ResMut<crate::anim_tab::BenchState>,
    mut compose: ResMut<crate::compose::ComposeState>,
    mut journal: Query<&mut Node, With<crate::chrome::JournalPanel>>,
) {
    if !keys::just_pressed(&keyboard, *live, Action::Cancel) {
        return;
    }
    // **The innermost thing first.** `Esc` with the journal open puts the journal away and leaves
    // the tab's problems alone — clearing them under a panel the author is reading would empty the
    // list in front of them.
    let mut closed = false;
    for mut node in &mut journal {
        if node.display != Display::None {
            node.display = Display::None;
            closed = true;
        }
    }
    if closed {
        return;
    }
    match *mode {
        Mode::Map | Mode::World | Mode::Sound | Mode::Game | Mode::Agent | Mode::Files | Mode::Project | Mode::Design => {}
        Mode::Meshes | Mode::Tiles => import.status.dismiss(),
        Mode::Anim => bench.status.dismiss(),
        Mode::Compose => compose.status.dismiss(),
    }
}

/// **`Cmd+C`: this tab's text into the clipboard, and the block comes down with it.**
///
/// Dismissing on success is the point rather than a convenience: copying a refusal is the strongest
/// evidence there is that somebody has read it, and a block still shouting after you have pasted it
/// into a message is a block you learn to stop looking at.
///
/// **Only on success.** A clipboard that could not be reached leaves the block exactly where it was
/// and raises a second problem over it — the one case where the author has *not* got the text out,
/// and the worst possible moment to take it off the screen.
fn copy_out(
    // Pins the system to the main thread. macOS's pasteboard is not promised to tolerate any
    // other, and a clipboard that works four times in five is worse than none — inherited from the
    // Tiles-only verb this replaces, where it was learned.
    _main_thread: bevy::ecs::system::NonSendMarker,
    input: keys::KeyInput,
    mode: Res<Mode>,
    tabs: TabStatusesMut,
    tree: UiTree,
    // **The frame first, then every other root.** `Frame::root` is the window's own column and
    // therefore reads top-to-bottom; the overlays (the shortcut card, the name box, a guide step)
    // are roots of their own and belong after it rather than interleaved. `Option` because
    // `Screen::Editor` can run a pass before the door is built, and a missing `Res` panics in 0.19.
    screen_roots: UiRoots,
) {
    let keys::KeyInput { keyboard, live } = input;
    let TabStatusesMut { mut editor, mut import, mut bench, mut compose } = tabs;
    let UiTree { panes, children, texts, nodes } = tree;
    let UiRoots { frame, other_roots } = screen_roots;
    if !keys::just_pressed(&keyboard, *live, Action::CopyInfo) {
        return;
    }
    let tab = *mode;
    let mut roots: Vec<Entity> = Vec::new();
    if let Some(frame) = frame.as_ref() {
        roots.push(frame.root);
    }
    // Sorted, so two runs of this key on an unchanged screen produce the same text — entity
    // iteration order is not stable across `App`s and a copy that reshuffled would be a poor thing
    // to paste into a bug report twice.
    let mut rest: Vec<Entity> = other_roots
        .iter()
        .filter(|e| Some(*e) != frame.as_ref().map(|f| f.root))
        .collect();
    rest.sort();
    roots.extend(rest);
    // One borrow of the live tab's status, used to read and then to answer. `match` on the mode
    // rather than a helper returning `&mut Status`, because four `ResMut` params cannot be handed
    // to one function and returned from it without borrowing all four for the rest of the system.
    let status: &mut crate::chrome::Status = match tab {
        Mode::Map | Mode::World | Mode::Sound | Mode::Game | Mode::Agent | Mode::Files | Mode::Project | Mode::Design => &mut editor.status,
        Mode::Meshes | Mode::Tiles => &mut import.status,
        Mode::Anim => &mut bench.status,
        Mode::Compose => &mut compose.status,
    };
    let lines = harvest(status, tab, &panes, &roots, &children, &texts, &nodes);
    if lines.is_empty() {
        status.note("nothing on this tab to copy");
        return;
    }
    let count = lines.len();
    match arboard::Clipboard::new().and_then(|mut c| c.set_text(lines.join("\n"))) {
        Ok(()) => {
            status.dismiss();
            status.note(format!("copied {count} line(s) — the whole {} screen", tab.label()));
        }
        Err(e) => status.problem(format!("could not reach the clipboard: {e}")),
    }
}

/// **The toast a refusal raises, and the layer it stands in** (UI-39, #233).
///
/// The newest refusal of the live tab, when it is new, becomes one `bevy_gooey` alert toast that
/// **stands until it is dismissed** (its Dismiss button, `Esc`, or the next refusal replacing it), with
/// a **Show** button when the refusal names what it is about ([`crate::chrome::Problem::cause`]). The
/// journal behind `Cmd+E` keeps the line either way. The toast stands down for as long as the journal
/// is up: the journal's first line IS the toast's sentence, and both are at the top of the viewport.
///
/// The strip ([`crate::chrome::ToastLayer`]) is shown in the frame the toast is raised, and hidden when
/// none stands, so a quiet editor is quiet. Its `Display` is compared before it is written.
fn raise_toast(
    mut commands: Commands,
    mut raised: ResMut<Toast>,
    project: Option<Res<crate::project::Project>>,
    open: Option<Res<crate::project::OpenMap>>,
    journals: Query<&Node, With<crate::chrome::JournalPanel>>,
    standing: Query<Entity, With<RefusalToast>>,
    mut layers: Query<&mut bevy_gooey::Shown, (With<crate::chrome::ToastLayer>, Without<crate::chrome::JournalPanel>)>,
) {
    let journal_open = journals.iter().any(|n| n.display != Display::None);
    let mut up = !standing.is_empty();
    if raised.fresh {
        raised.fresh = false;
        for toast in &standing {
            commands.entity(toast).despawn();
        }
        up = false;
        if let (Some(line), false) = (raised.shown.as_deref(), journal_open) {
            let mut bundle = bevy_gooey::toast_alert(line).sticky();
            // Show only for a thing that is there: a refusal about one that is already gone has nowhere to go.
            let is_there = |id: &str| {
                project.as_ref().is_some_and(|p| p.library.get(id).is_some())
                    || open.as_ref().is_some_and(|o| o.map.placements.iter().any(|p| p.id == id))
            };
            if raised.cause.as_deref().is_some_and(is_there) {
                bundle = bundle.action("Show", SHOW);
            }
            commands.spawn((bundle, RefusalToast { cause: raised.cause.clone() }));
            up = true;
        }
    } else if journal_open && up {
        for toast in &standing {
            commands.entity(toast).despawn();
        }
        up = false;
    }
    for mut shown in &mut layers {
        if shown.0 != up {
            shown.0 = up;
        }
    }
}

/// A text no refusal has claimed yet.
type Unmarked = (With<Text>, Without<crate::chrome::ProblemBanner>);

/// **Mark the one text that is on screen as the refusal's**, so `Cmd+C` and the tests that ask "what
/// does the card say" find exactly one. gooey builds a toast's body in the frame it is spawned, so the
/// text exists by the time this runs (it is chained after [`raise_toast`]).
fn mark_the_words(
    mut commands: Commands,
    words: Query<(Entity, &bevy_gooey::Role, &ChildOf), Unmarked>,
    parents: Query<&ChildOf>,
    toasts: Query<(), With<RefusalToast>>,
) {
    for (words, role, body) in &words {
        if *role != bevy_gooey::Role::ToastText {
            continue;
        }
        if parents.get(body.parent()).is_ok_and(|toast| toasts.contains(toast.parent())) {
            commands.entity(words).insert(crate::chrome::ProblemBanner);
        }
    }
}

/// **Show: take the person to what the refusal is about.** A placement on the open map is targeted
/// (the gold marker the Map's `H` verb draws); a library piece is selected on the Kit door's Meshes
/// shelf, through the `piece_select` operation a click on its row dispatches. What is neither says so
/// on the status line, because the thing may have been removed since the refusal was raised.
fn show_the_cause(action: On<bevy_gooey::ToastAction>, toasts: Query<&RefusalToast>, mut commands: Commands) {
    if action.id != SHOW {
        return;
    }
    let Some(id) = toasts.get(action.toast).ok().and_then(|t| t.cause.clone()) else {
        return;
    };
    commands.queue(move |world: &mut World| show(world, &id));
}

fn show(world: &mut World, id: &str) {
    let on_map = world.get_resource::<crate::project::OpenMap>().is_some_and(|o| o.map.placements.iter().any(|p| p.id == id));
    let in_library = world.get_resource::<crate::project::Project>().is_some_and(|p| p.library.get(id).is_some());
    let (door, tab) = if on_map {
        (crate::workspace::Workspace::Map, None)
    } else if in_library {
        (crate::workspace::Workspace::Kit, Some(Mode::Meshes))
    } else {
        world.resource_mut::<crate::editor::EditorState>().status.note(format!("`{id}` is not on this map or in the library any more"));
        return;
    };
    let here = world.get_resource::<crate::workspace::Workspace>() == Some(&door) && tab.is_none_or(|t| world.get_resource::<Mode>() == Some(&t));
    if !here {
        crate::editor::open_door(world, door, tab);
    }
    world.spawn(ShowAfterDoor(id.to_owned()));
}

/// **What Show is waiting to do once the door it asked for is crossed.** A door switching resets the
/// state a selection lives in (`ImportState`, `TargetLock` and the status line are the door's own), so
/// the selection is made on the far side. A component and not a resource: a resource would need a
/// classification of what a door does to it, and this is gone in a frame or two.
#[derive(Component)]
struct ShowAfterDoor(String);

/// **Select what Show named, once no door is waiting.** A switching that was asked for and did not
/// happen (the person answered "stay" to the leaving question) leaves nothing selected and says nothing:
/// they chose to stay.
fn finish_show(world: &mut World) {
    let waiting: Vec<(Entity, String)> = world.query::<(Entity, &ShowAfterDoor)>().iter(world).map(|(e, s)| (e, s.0.clone())).collect();
    if waiting.is_empty() {
        return;
    }
    let busy = world.contains_resource::<crate::screen::Crossing>()
        || world.get_resource::<crate::editor::EditorState>().is_some_and(|s| s.door_request.is_some() || s.leaving);
    if busy {
        return;
    }
    for (entity, id) in waiting {
        world.entity_mut(entity).despawn();
        let door = world.get_resource::<crate::workspace::Workspace>().copied();
        if door == Some(crate::workspace::Workspace::Map) && crate::editor::lock_on_placement(world, &id) {
            world.resource_mut::<crate::editor::EditorState>().status.note(format!("targeting `{id}`, which the refusal was about"));
        } else if door == Some(crate::workspace::Workspace::Kit) && world.get_resource::<Mode>() == Some(&Mode::Meshes) {
            world.resource_scope(|world, mut import: Mut<crate::tiles::ImportState>| {
                if let Some(project) = world.get_resource::<crate::project::Project>() {
                    crate::editor::select_piece(&mut import, project, &id);
                }
            });
        }
    }
}

/// **The agent's steps, read from the operation log (E-068).**
///
/// Follows the log's own sequence count rather than a watermark of its own: [`crate::ops::read_entries`]
/// is a pure read of the file, so re-reading when the count moved is the whole of it, and a log edited
/// by hand is picked up on the next dispatch rather than being invisible until a restart.
///
/// **Reads the open map's project, not a state flag.** An agent's steps are a fact about the project
/// on disk; a watermark on a resource would miss every operation a person took between two frames.
fn record_steps(
    project: Option<Res<crate::project::Project>>,
    mut journal: ResMut<crate::chrome::Journal>,
    mut seen: Local<usize>,
) {
    let Some(project) = project else {
        return;
    };
    let path = crate::ops::oplog_path(project.as_ref());
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    // **The count of non-empty lines is the log's own clock**, so a file nobody appended to costs
    // one `lines()` and nothing else.
    let lines = text.lines().filter(|l| !l.trim().is_empty()).count();
    if lines == *seen {
        return;
    }
    *seen = lines;
    let rows = crate::steps::from_oplog(&text, &[]);
    // **The dropped count is derived, not tracked**: `from_oplog` caps, so what is missing is what
    // the log holds minus what the feed kept.
    let applied = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<crate::ops::Entry>(l).ok())
        .filter(|e| e.result.is_some())
        .count();
    journal.steps_dropped = applied.saturating_sub(rows.len());
    journal.steps_unreadable = crate::steps::unreadable_lines(&text);
    journal.steps = rows;
}

/// **Everything that has gone wrong, kept where `Esc` cannot reach it.**
///
/// Watches [`crate::chrome::Status::raised`] — a counter that only goes up — so the journal learns
/// about a refusal without either side holding the other's list. See [`crate::chrome::Journal`] for
/// why a status cannot simply hand it over.
///
/// All four tabs, because a session log that only recorded the tab you happened to be on would be a
/// log with holes exactly where a batch left them.
fn record_problems(
    editor: Res<crate::editor::EditorState>,
    import: Res<crate::tiles::ImportState>,
    bench: Res<crate::anim_tab::BenchState>,
    compose: Res<crate::compose::ComposeState>,
    mut journal: ResMut<crate::chrome::Journal>,
    // One watermark per tab, in the order below. A `Local` rather than a field on the journal: it is
    // this system's bookkeeping about what it has read, not a fact about the log.
    mut seen: Local<[u64; 4]>,
) {
    let statuses = [
        &editor.status,
        &import.status,
        &bench.status,
        &compose.status,
    ];
    for (i, status) in statuses.into_iter().enumerate() {
        let now = status.raised();
        if now <= seen[i] {
            continue;
        }
        // **Every refusal since the last look, each with its own text.** `raised` counts calls to
        // `Status::problem` — folds included — so it says how far back to look and nothing more; the
        // TEXT comes from the entries themselves. Recording `problem_text()` that many times was one
        // sentence standing in for several: `labels::check_stale` raises a distinct refusal per stale
        // result inside one loop, and the journal kept only the last of them, N times over.
        let problems = status.problems();
        // Newest first, spending the budget. The boundary entry may already be partly recorded — a
        // repeat folds into it across frames — so it contributes only what is left rather than its
        // whole count.
        let mut budget = (now - seen[i]) as usize;
        let mut back = 0usize;
        let mut oldest_times = 0usize;
        for p in problems.iter().rev() {
            if budget == 0 {
                break;
            }
            oldest_times = p.count.min(budget);
            budget -= oldest_times;
            back += 1;
        }
        // Oldest of the tail first, so the journal reads in the order the refusals happened.
        for (n, p) in problems[problems.len() - back..].iter().enumerate() {
            let times = if n == 0 { oldest_times } else { p.count };
            journal.record_problem(&p.text, times, p.problem.as_ref());
        }
        // **The watermark moves by what was recorded, not by what was raised.** It moved first, and
        // to `now`, so a raise whose text was already gone by the time this looked — `Esc` or a
        // successful `Cmd+C` clears the tab in the same `Phase::Act` this runs after — was written
        // off rather than still owed. A clock advanced past events it never read is a clock that
        // silently forgets, which is `Status::dropped`'s argument one scope up.
        //
        // The tradeoff, stated: holding the watermark back attributes an unrecordable raise to the
        // *next* refusal on that tab, inflating its `(xN)`. Advancing fully loses it outright. The
        // reachable window is only raises created *and* cleared inside one `Act`, and in both real
        // cases (`copy_out`, `editor::keys`) the refusal being cleared was raised on an earlier frame
        // and is already recorded, so `budget` is zero.
        seen[i] = now - budget as u64;
    }
}

/// **`Cmd+E` opens the journal, and pressing it again puts it away.**
///
/// A toggle rather than a modal: it is a reference panel, not a question, so it takes no scrim and
/// blocks nothing. `Esc` closes it too — see [`dismiss`], which now backs out of the innermost thing
/// rather than always clearing the tab.
fn toggle_journal(
    keyboard: Res<ButtonInput<KeyCode>>,
    live: Res<keys::Live>,
    mut panels: Query<&mut Node, With<crate::chrome::JournalPanel>>,
) {
    // **Behind a key EDGE, and the lint can see that now.** `just_pressed` is false on every frame
    // but the one where `Cmd+E` went down, so the one write below happens once per press and the
    // change flag it raises is exactly the news the layout wants.
    if !keys::just_pressed(&keyboard, *live, Action::ShowErrors) {
        return;
    }
    // **The key dispatches the operation an agent calls** (E-002, #129). Up or down is the operation's
    // answer, read from the panel's own `Display` and written back after it — so the key and an MCP
    // call change the same thing, and `view_toggle_journal` is the one place that decides what a toggle
    // means.
    let open = panels.iter().any(|n| n.display != Display::None);
    let mut ctx = crate::ops::Ctx {
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        view: Some(crate::ops_view::View { journal_open: open, ..Default::default() }),
        ..crate::ops::Ctx::default()
    };
    let outcome = crate::ops::dispatch(&mut ctx, "view_toggle_journal", &serde_json::json!({}));
    let open = ctx.view.map(|v| v.journal_open).unwrap_or(open);
    // **The toast stands down when the journal comes up** ([`raise_toast`] sees the open panel): both
    // are at the top of the viewport, and the journal's first line IS the toast's sentence.
    let _ = outcome;
    for mut node in &mut panels {
        node.display = if open { Display::Flex } else { Display::None };
    }
}

/// **Rebuild the list when the journal changes or the panel opens**, and not on a frame.
///
/// `chrome::Follow`'s argument: static text respawned sixty times a second is sixty times the work
/// for one picture. The guard is what it would produce, exactly as the old per-panel log's was.
fn paint_journal(
    mut commands: Commands,
    journal: Res<crate::chrome::Journal>,
    panels: Query<&Node, With<crate::chrome::JournalPanel>>,
    lists: Query<Entity, With<crate::chrome::JournalList>>,
    lines: Query<Entity, With<crate::chrome::ProblemLogLine>>,
    // **The journal's ink, from the editor's skin** (UI-02, #210).
    skin: Option<Res<bevy_gooey::GooeySkin>>,
    mut was_open: Local<bool>,
) {
    let ink = crate::compose::PanelInk::of(skin.as_deref());
    let open = panels.iter().any(|n| n.display != Display::None);
    if !open {
        // Nothing to draw and nothing to keep: the next open rebuilds from the journal, which is the
        // only copy that matters.
        *was_open = false;
        return;
    }
    // **Rebuilt when the journal changed or the panel just opened — never on a count.** This
    // guarded on the line count once, and a count is exactly what the journal keeps constant: a
    // repeat folds into the last entry's `(xN)` without adding a line, and at [`crate::chrome::JOURNAL_CAP`]
    // every new entry replaces an old one — so an open panel showed stale tallies and, at the cap,
    // froze for the rest of the session. `Journal` is only dereferenced mutably when a refusal was
    // recorded, so its change flag is the honest clock — and cheaper than rendering 200 lines a
    // frame to compare them.
    // A project that restyles itself while the journal is up repaints it, so the refusals a reader is
    // looking at do not keep the old skin's ink until the next refusal arrives.
    let reskinned = skin.as_ref().is_some_and(|s| s.is_changed());
    let rebuild = journal.is_changed() || !*was_open || reskinned;
    *was_open = true;
    if !rebuild {
        return;
    }
    // **The steps first, newest last, then a rule, then the refusals (E-068).** One column, because
    // a person watching an agent wants the two together: a refusal is a step that did not happen.
    // The steps read downward in the order they happened — the opposite of the refusal list, which
    // is newest-first because a toast just showed its head — so the section is labelled rather than
    // left to be inferred from the order.
    let mut want: Vec<String> = Vec::new();
    if !journal.steps.is_empty() {
        want.push(format!("STEPS ({} this session)", journal.steps.len()));
        want.extend(journal.steps.iter().map(|s| {
            let mark = if s.capture.is_some() { " ▪" } else { "" };
            format!("  {}{mark}", s.text)
        }));
        if journal.steps_dropped > 0 {
            want.push(format!(
                "  +{} earlier, dropped at {}",
                journal.steps_dropped,
                crate::steps::CAP
            ));
        }
        if journal.steps_unreadable > 0 {
            // **Said, not swallowed.** A feed shorter than the file reads as a complete session.
            want.push(format!(
                "  {} line(s) of the log could not be read",
                journal.steps_unreadable
            ));
        }
        if !journal.entries().is_empty() {
            want.push(String::new());
        }
    }
    want.extend(
        journal
            .entries()
            .iter()
            .rev()
            .map(|p| p.line())
            .chain(match journal.dropped() {
                0 => None,
                n => Some(format!(
                    "+{n} earlier, dropped at {}",
                    crate::chrome::JOURNAL_CAP
                )),
            }),
    );
    for e in &lines {
        commands.entity(e).despawn();
    }
    for list in &lists {
        commands.entity(list).with_children(|p| {
            if want.is_empty() {
                p.spawn((
                    bevy::prelude::Text::new("no steps and nothing has gone wrong yet".to_owned()),
                    bevy::prelude::TextColor(ink.quiet),
                    crate::chrome::typed(crate::chrome::text::LABEL),
                    crate::chrome::ProblemLogLine,
                ));
                return;
            }
            for (i, line) in want.iter().enumerate() {
                // Newest first, and the newest is the one the toast just showed — so the top of this
                // list and the thing that interrupted you agree.
                let colour = if i == 0 { ink.danger } else { ink.quiet };
                crate::chrome::problem_log_line(p, line, colour);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::chrome::Status;

    /// The rule `copy_out` turns on, stated where it can be checked without an `App`: a copy that
    /// worked takes the block down, and a copy that failed must not.
    #[test]
    fn only_a_copy_that_worked_takes_the_block_down() {
        let mut ok = Status::default();
        ok.problem("NOT SAVED: read-only file system");
        ok.dismiss();
        ok.note("copied 9 line(s) from the map tab");
        assert!(!ok.has_problem());

        let mut failed = Status::default();
        failed.problem("NOT SAVED: read-only file system");
        failed.problem("could not reach the clipboard: no display");
        assert!(
            failed.has_problem(),
            "a failed copy is the one moment the text must stay on screen"
        );
    }
}
