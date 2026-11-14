//! **The Agent workspace: everything about the Familiar in one place** (NV-10, #387; series getting around, #377).
//!
//! It holds
//!
//! - **the AGENT block at full size** ([`crate::familiar::spawn_the_block`], the one builder the Map's compact block also uses): when the
//!   agent was last heard, the newest change, the plan, Pause, Undo and Look, and **the waiting wishes and lessons** as its rows (AW-02, AG-06);
//! - **the agent's history**: the operation log filtered to the changes the Familiar made, newest first, each with **Look**, which shows
//!   *that change's own page* — its pieces framed on the Map — through the reveal ([`crate::reveal::Revealing::request_by_person`]), the
//!   same answer to "where is this seen" a finder row and an agent's own change use.
//!
//! **One list of waiting things.** The rows are `wish_view`'s, shown here and counted on the Agent icon of the activity bar
//! ([`crate::activity_bar::badge`]); there is no second list.
//!
//! # The history reads the log, not the undo stack — and only when the log changed
//!
//! The undo stack belongs to a workspace and is reset by a switch, so a person who walks from the Map to here would find it empty. The
//! operation log (`oplog.jsonl`) is the project's, so what is listed is every applied change whose source is the agent, that is not
//! itself an undo and has not been undone. The file is looked at every frame by its length and modification time and **read only when
//! either moved**, and then only its last [`TAIL_BYTES`] (the whole file only when that tail holds fewer than [`HISTORY_ROWS`] of them).
//! [`oplog_reads`] counts the reads, which is what the test holds still.
//!
//! The compact block stays a toggle of the Map's inspector (View ▸ Show agent) so a person can watch the agent from Map; the same block is
//! never spawned in two places at once, because each is spawned by its own workspace.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::SystemTime;

use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy_gooey::Activate;
use serde_json::Value;

use crate::chrome::{self, Frame, FrameSystems, Severity, Side};
use crate::familiar::{AgentPause, HandBackHere};
use crate::project::{OpenMap, Project};
use crate::screen::Screen;
use crate::workspace::Workspace;

/// How many of the agent's last changes the history lists.
pub const HISTORY_ROWS: usize = 3;

/// **How much of the end of the operation log is read** when it changes, in bytes. A log is one line per operation, so this holds
/// hundreds of them; the whole file is read only when it does not hold [`HISTORY_ROWS`] of the agent's.
pub const TAIL_BYTES: u64 = 64 * 1024;

/// How many times the operation log has been read for the history, whole or tail.
static OPLOG_READS: AtomicUsize = AtomicUsize::new(0);

/// **How many times the history has read the operation log**, for the test that holds it still while the file does not change.
pub fn oplog_reads() -> usize {
    OPLOG_READS.load(Ordering::Relaxed)
}

/// The Agent panel's root.
#[derive(Component)]
pub struct AgentRoot;
/// The Agent panel's scrolling body.
#[derive(Component)]
pub struct AgentPane;
/// The column of the agent's last changes.
#[derive(Component)]
pub struct AgentHistory;
/// One row of the history: the change, in words.
#[derive(Component)]
pub struct AgentChangeRow;
/// **The Look button of a history row**, carrying the change it looks at: the operation and its parameters, from which the page it is
/// seen on is asked of the operation itself.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct AgentChangeLook {
    pub operation: String,
    pub params: Value,
    /// The change in words, which the follow's chip says when it goes there.
    pub text: String,
}

/// **The Undo button of a history row**, carrying the id of the change it takes back (NV-10, #387). The inverse is the one the log recorded, run
/// through the operation layer as the person's own undo, so it lands in the log and on the editor's history like any undo.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct AgentChangeUndo(pub String);

/// **The text of a kept hand-back's row** (BM-07, #417).
#[derive(Component)]
pub struct AgentHandBackRow;

/// **The Make a benchmark task button of a kept hand-back**, carrying the hand-back's id (`present-1`). Pressing it writes the draft under
/// `.dark_factory/bench/drafts/` and opens it in the Files workspace; a hand-back whose setup cannot be rebuilt has it disabled, with the
/// operation named in its tooltip.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct AgentMakeTask(pub String);

/// **One of the agent's changes**, as the history lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub operation: String,
    pub params: Value,
    /// `op: what it did`.
    pub text: String,
    /// **The operation's id in the log**, which is what an undo names (`operation_undo`'s own key).
    pub operation_id: String,
}

/// What the history last looked at, so the log is read when it moved and the column is rebuilt when its rows did.
#[derive(Default)]
struct Watch {
    root: Option<PathBuf>,
    length: Option<u64>,
    modified: Option<SystemTime>,
    rows: Vec<Change>,
    hands: Vec<HandBackRow>,
    drawn: bool,
    /// Whether a map was open when the hand-backs were drawn: a Make a benchmark task is disabled without one.
    had_map: bool,
}

pub struct AgentViewPlugin;

impl Plugin for AgentViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingLook>()
            .add_systems(OnEnter(Screen::Editor), spawn_agent_panel.run_if(Workspace::agent_is_open).after(FrameSystems))
            .add_systems(Update, draw_the_history.run_if(in_state(Screen::Editor)).run_if(Workspace::agent_is_open))
            // Runs in every workspace: the Look is carried out on the Map, after it has been left for.
            .add_systems(Update, finish_the_look.run_if(in_state(Screen::Editor)))
            .add_observer(on_look)
            .add_observer(on_undo)
            .add_observer(on_make_task);
    }
}

/// **The agent's changes among `entries`, newest first**: applied, not an undo, and not undone by a later entry.
fn agent_changes_of(entries: &[crate::ops::Entry], count: usize) -> Vec<Change> {
    let undone: HashSet<&str> = entries.iter().filter_map(|e| e.undoes.as_deref()).collect();
    entries
        .iter()
        .rev()
        .filter(|e| {
            e.source == crate::ops::Source::Agent && e.outcome == "applied" && e.undoes.is_none() && !undone.contains(e.operation_id.as_str())
        })
        .take(count)
        .map(|e| Change {
            operation: e.op.clone(),
            operation_id: e.operation_id.clone(),
            params: e.params.clone(),
            text: match e.result.as_deref().filter(|r| !r.is_empty()) {
                Some(result) => format!("{}: {result}", e.op),
                None => e.op.clone(),
            },
        })
        .collect()
}

/// **The last [`TAIL_BYTES`] of the log, as entries.** A line cut by the window is dropped; one that does not read is skipped, as every
/// reader of the log skips it.
fn tail_entries(path: &std::path::Path) -> Option<(Vec<crate::ops::Entry>, bool)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    let start = length.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.take(TAIL_BYTES).read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let mut lines = text.lines();
    if start > 0 {
        lines.next();
    }
    let entries = lines.filter_map(|line| serde_json::from_str::<crate::ops::Entry>(line).ok()).collect();
    Some((entries, start > 0))
}

/// **The agent's last `count` applied changes, newest first**, from the project's operation log. Counts as one read.
pub fn agent_changes(root: &std::path::Path, count: usize) -> Vec<Change> {
    OPLOG_READS.fetch_add(1, Ordering::Relaxed);
    let path = crate::ops::oplog_path_at(root);
    if let Some((entries, cut)) = tail_entries(&path) {
        let found = agent_changes_of(&entries, count);
        if found.len() >= count || !cut {
            return found;
        }
    }
    agent_changes_of(&crate::ops::read_entries(&path), count)
}

/// **The Agent workspace's panel**: the block, and under it the history.
fn spawn_agent_panel(mut commands: Commands, frame: Res<Frame>, pause: Res<AgentPause>, here: Res<HandBackHere>) {
    let root = chrome::panel_root(&mut commands, &frame, Side::Left, chrome::CONTROLS_W, true, false, "AGENT").insert(AgentRoot).id();
    let mut pane = Entity::PLACEHOLDER;
    commands.entity(root).with_children(|p| {
        chrome::title(p, "FAMILIAR");
        // FOLLOW-OK: a block of lines, buttons and rows, not a list the arrows walk: the scroll exists for height.
        pane = chrome::scroll_list(p, AgentPane).id();
    });
    let block = crate::familiar::spawn_the_block(&mut commands, pause.0, here.0);
    commands.entity(pane).add_child(block);
    commands.entity(pane).with_children(|s| {
        // The history keeps its height when the pane is short.
        s.spawn(bevy_gooey::column_with(chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() }))
            .with_children(|h| {
                chrome::section(h, "HISTORY");
                h.spawn((AgentHistory, bevy_gooey::column(chrome::GAP_TIGHT_STEPS)));
            });
    });
}

/// **One kept hand-back, as the history lists it** (BM-07, #417): its id, its title, the draft already made from it, and why one cannot be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandBackRow {
    pub id: String,
    pub title: String,
    /// The draft's path under the project, once Make a benchmark task was pressed.
    pub draft: Option<String>,
    /// **Why no task can be made of it**, which is the disabled button's tooltip.
    pub cannot: Option<String>,
}

/// **The kept hand-backs of this project, newest first**, each with the reason it cannot be made a task, when it cannot.
///
/// **No open map is a reason**: the draft is named for the map the hand-back was made on, so with none open there is nothing to rebuild it from.
fn kept_handbacks(project: &Project, has_map: bool, handbacks: &crate::present::HandBacks) -> Vec<HandBackRow> {
    handbacks
        .kept_in(&project.root)
        .take(HISTORY_ROWS)
        .map(|h| HandBackRow {
            id: h.id.clone(),
            title: h.title.clone(),
            draft: h.draft.clone(),
            cannot: if h.draft.is_some() {
                None
            } else if !has_map {
                Some("no map is open, so the map before this hand-back cannot be rebuilt: open the map it was made on".to_owned())
            } else {
                crate::bench_draft::cannot(project, h)
            },
        })
        .collect()
}

/// **Draw the agent's last changes**, and the hand-backs the person kept, when the log or the hand-backs moved and they are different.
fn draw_the_history(
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    handbacks: Option<Res<crate::present::HandBacks>>,
    list: Query<(Entity, Option<&Children>), With<AgentHistory>>,
    mut watch: Local<Watch>,
    mut commands: Commands,
) {
    let Some(project) = project else { return };
    let Ok((container, children)) = list.single() else {
        // The panel is not up: forget what was drawn, so the next one draws.
        watch.drawn = false;
        return;
    };
    let path = crate::ops::oplog_path_at(&project.root);
    let meta = std::fs::metadata(&path).ok();
    let (length, modified) = (meta.as_ref().map(std::fs::Metadata::len), meta.and_then(|m| m.modified().ok()));
    let unchanged = watch.root.as_ref() == Some(&project.root) && watch.length == length && watch.modified == modified;
    let hands_moved = handbacks.as_ref().is_some_and(|h| h.is_changed());
    let has_map = open.is_some();
    if unchanged && watch.drawn && !hands_moved && watch.had_map == has_map {
        return;
    }
    let rows = if unchanged && !watch.rows.is_empty() { watch.rows.clone() } else { agent_changes(&project.root, HISTORY_ROWS) };
    let hands = handbacks.as_deref().map(|h| kept_handbacks(&project, has_map, h)).unwrap_or_default();
    watch.had_map = has_map;
    watch.root = Some(project.root.clone());
    watch.length = length;
    watch.modified = modified;
    if watch.drawn && watch.rows == rows && watch.hands == hands {
        return;
    }
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    watch.rows.clone_from(&rows);
    watch.hands.clone_from(&hands);
    watch.drawn = true;
    commands.entity(container).with_children(|c| {
        if rows.is_empty() {
            chrome::row_value(c, "The agent has made no changes yet", chrome::ink().dim, AgentChangeRow);
        }
        for row in &rows {
            chrome::gapped_row(c, chrome::GAP_TIGHT_STEPS, crate::chrome::NO_MARGIN, AlignItems::FlexStart)
                .with_children(|r| {
                    r.spawn(bevy_gooey::fill_column(0.0)).with_children(|col| {
                        chrome::row_value(col, row.text.clone(), chrome::ink().text, AgentChangeRow);
                    });
                    chrome::button(
                        r,
                        AgentChangeLook { operation: row.operation.clone(), params: row.params.clone(), text: row.text.clone() },
                        "Look",
                        Severity::Plain,
                    );
                    chrome::button(r, AgentChangeUndo(row.operation_id.clone()), "Undo this", Severity::Plain);
                });
        }
        // **The hand-backs the person kept** (BM-07, #417): a result is a benchmark task once somebody accepted it. Drawn only when there is
        // one, so a workspace that has kept nothing holds no extra button.
        for hand in &hands {
            chrome::gapped_row(c, chrome::GAP_TIGHT_STEPS, crate::chrome::NO_MARGIN, AlignItems::FlexStart)
                .with_children(|r| {
                    r.spawn(bevy_gooey::fill_column(0.0)).with_children(|col| {
                        chrome::row_value(col, format!("kept: {}", hand.title), chrome::ink().text, AgentHandBackRow);
                    });
                    let label = if hand.draft.is_some() { "Open the benchmark task" } else { "Make a benchmark task" };
                    let mut button = chrome::button(r, AgentMakeTask(hand.id.clone()), label, Severity::Plain);
                    if let Some(why) = &hand.cannot {
                        button.insert((InteractionDisabled, bevy_gooey::Tooltip::new(why.clone())));
                    }
                });
        }
    });
}

/// **Make a benchmark task of a kept hand-back** (BM-07, #417): the draft is written, the person's hand-back remembers it, and the file opens in
/// the Files workspace to be read. A refusal is the sentence on the status line, never a silent no-op. The open map is never touched.
fn on_make_task(
    activate: On<Activate>,
    buttons: Query<&AgentMakeTask>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    mut handbacks: Option<ResMut<crate::present::HandBacks>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    mut commands: Commands,
) {
    let Ok(AgentMakeTask(id)) = buttons.get(activate.entity) else { return };
    let (Some(project), Some(handbacks), Some(state)) = (project.as_deref(), handbacks.as_mut(), state.as_mut()) else { return };
    let Some(hand) = handbacks.kept_in(&project.root).find(|h| &h.id == id).cloned() else { return };
    let Some(open) = open.as_deref() else {
        state.status.problem("no benchmark task was made: no map is open, so the map before this hand-back cannot be rebuilt".to_owned());
        return;
    };
    match crate::bench_draft::make(project, open, &hand) {
        Ok(made) => {
            handbacks.set_draft(&hand.id, &project.root, made.rel.clone());
            state.status.note(if made.written {
                format!("made the benchmark task `{}`: {}", made.id, made.rel)
            } else {
                format!("the benchmark task `{}` is already drafted: {}", made.id, made.rel)
            });
            let rel = made.rel;
            commands.queue(move |world: &mut World| crate::crosslinks::open_in_files(world, &rel, None));
        }
        Err(why) => state.status.problem(format!("no benchmark task was made: {why}")),
    }
}

/// **A Look asked for and not yet carried out.** A `Session` resource: the switch it asks for resets every workspace's own state, and a
/// request kept there would be erased by the very switch it needs.
#[derive(Resource, Default)]
pub struct PendingLook(Option<AgentChangeLook>);

/// **Look on a change goes to the Map first** — the workspace asks "Leave this map?" when the map has unsaved edits, **before anything moves**
/// — and the camera is asked for once the Map is up ([`finish_the_look`]).
fn on_look(activate: On<Activate>, looks: Query<&AgentChangeLook>, mut pending: ResMut<PendingLook>, mut commands: Commands) {
    let Ok(look) = looks.get(activate.entity) else { return };
    pending.0 = Some(look.clone());
    commands.queue(|world: &mut World| {
        if world.contains_resource::<crate::editor::EditorState>() {
            crate::editor::open_door(world, Workspace::Map, None);
        }
    });
}

/// **Undo on a history row takes that change back**, or says on the status line why the log refuses (a newer change in the same scope still
/// stands, the map changed by hand since): the refusal is the operation layer's own sentence, never a silent no-op.
fn on_undo(
    activate: On<Activate>,
    undos: Query<&AgentChangeUndo>,
    mut project: Option<ResMut<Project>>,
    mut open: Option<ResMut<OpenMap>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    mut due: Option<ResMut<crate::watch::Due>>,
) {
    let Ok(AgentChangeUndo(id)) = undos.get(activate.entity) else { return };
    let (Some(project), Some(open), Some(state)) = (project.as_mut(), open.as_mut(), state.as_mut()) else { return };
    match crate::present::undo_logged(project, open, state, id) {
        Ok(()) => state.status.note(format!("took back the agent's change `{id}`")),
        Err(why) => state.status.problem(why),
    }
    // The screen is drawn again through the one flag that means "the map was replaced", as every other undo does.
    if let Some(due) = due.as_mut() {
        due.map_replaced = true;
    }
}

/// **Carry out a Look**: asked of the operation itself for its parameters — a place on the map (its pieces, framed) goes to the follow, once the
/// Map is the workspace and its camera has a viewport to frame in; anything else goes to the reveal, which opens its own page. A "Stay" to the
/// leaving question drops the Look: nothing moved, and the camera is where it was. The same two mechanisms and one decision as the finder's row.
#[allow(clippy::too_many_arguments)]
fn finish_the_look(
    mut pending: ResMut<PendingLook>,
    workspace: Option<Res<Workspace>>,
    cameras: Query<&Camera, With<crate::view::MainCamera>>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    confirm: Option<Res<crate::confirm::Confirm>>,
    revealing: Option<ResMut<crate::reveal::Revealing>>,
    following: Option<ResMut<crate::follow::Following>>,
    state: Option<ResMut<crate::editor::EditorState>>,
) {
    let Some(look) = pending.0.as_ref() else { return };
    let (Some(mut revealing), Some(mut following), Some(mut state)) = (revealing, following, state) else { return };
    let ctx = crate::ops::Ctx { project: project.as_deref().cloned(), open: open.as_deref().cloned(), source: crate::ops::Source::Human, ..crate::ops::Ctx::default() };
    let Some(op) = crate::ops::registry().into_iter().find(|op| op.name() == look.operation) else {
        state.status.problem(format!("the agent's change `{}` is not an operation this editor knows, so there is no page to show it on", look.operation));
        pending.0 = None;
        return;
    };
    let shown = crate::ops::preview_shows(&ctx, op.as_ref(), &look.params);
    let Some(request) = crate::follow::request_of(&shown) else {
        revealing.request_by_person(&look.operation, &shown);
        pending.0 = None;
        return;
    };
    let on_the_map = workspace.as_deref().is_some_and(|w| *w == Workspace::Map);
    if !on_the_map {
        let asking = confirm.as_deref().is_some_and(crate::confirm::Confirm::is_open) || state.door_request.is_some() || state.leaving;
        if !asking {
            // The question was answered "Stay" (or the switch was refused): nothing moved, and nothing will.
            pending.0 = None;
        }
        return;
    }
    if !cameras.iter().any(|camera| camera.logical_viewport_rect().is_some_and(|rect| rect.width() > 0.0 && rect.height() > 0.0)) {
        // The Map has just been built; its camera has no viewport to frame in yet.
        return;
    }
    following.look_now();
    following.request_named(request, format!("went to the agent's change: {}", look.text));
    pending.0 = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(seq: u64, id: &str, source: &str, op: &str, undoes: Option<&str>) -> crate::ops::Entry {
        serde_json::from_value(serde_json::json!({
            "seq": seq, "operation_id": id, "at": "2026-11-12T00:00:00Z", "source": source, "session": "s", "op": op,
            "params": {"id": "table@0"}, "outcome": "applied", "undoes": undoes,
        }))
        .unwrap_or_else(|e| panic!("{e}"))
    }

    /// **A change the agent made and a person undid is not the agent's change any more**: the history lists neither the undo line
    /// nor the original it took back, and not a person's own edit.
    #[test]
    fn an_undone_change_and_its_undo_are_not_listed_and_nor_is_a_persons_edit() {
        let entries = vec![
            entry(0, "a", "agent", "placement_set_yaw", None),
            entry(1, "b", "agent", "placement_remove", None),
            entry(2, "u", "human", "placement_remove", Some("b")),
            entry(3, "c", "human", "placement_set_yaw", None),
        ];
        let listed: Vec<String> = agent_changes_of(&entries, 5).into_iter().map(|c| c.operation).collect();
        assert_eq!(listed, ["placement_set_yaw"], "only the agent's change that stands is listed");
    }
}
