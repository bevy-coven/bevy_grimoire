//! **The Agent workspace: everything about the Familiar in one place** (NV-10, #387; series getting around, #377).
//!
//! It holds
//!
//! - **the AGENT block at full size** ([`crate::familiar::spawn_the_block`], the one builder the Map's compact block also uses): when the
//!   agent was last heard, the newest change, the plan, Pause, Undo and Look, and **the waiting wishes and lessons** as its rows (AW-02, AG-06);
//! - **the agent's history**: the operation log filtered to the changes the Familiar made, newest first, each with **Look**, which switches to
//!   the Map and frames the change there ([`crate::follow::Following::look_now`]).
//!
//! **One list of waiting things.** The rows are `wish_view`'s, shown here and counted on the Agent icon of the activity bar
//! ([`crate::activity_bar::badge`]); there is no second list.
//!
//! # The history reads the log, not the undo stack
//!
//! The undo stack belongs to a workspace and is reset by a switch, so a person who walks from the Map to here would find it empty. The
//! operation log (`oplog.jsonl`) is the project's, so what is listed is every applied change whose source is the agent and that is not itself an
//! undo, read when the file's length changes.
//!
//! The compact block stays a toggle of the Map's inspector (View ▸ Show agent) so a person can watch the agent from Map; the same block is
//! never spawned in two places at once, because each is spawned by its own workspace.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_gooey::Activate;

use crate::chrome::{self, Frame, FrameSystems, Severity, Side};
use crate::familiar::{AgentPause, HandBackHere};
use crate::project::Project;
use crate::screen::Screen;
use crate::workspace::Workspace;

/// How many of the agent's last changes the history lists.
pub const HISTORY_ROWS: usize = 3;

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
/// The Look button of a history row.
#[derive(Component)]
pub struct AgentChangeLook;

/// What the history rows were last drawn with, so the column is rebuilt only when it changed.
#[derive(Default)]
struct Drawn {
    rows: Vec<String>,
    root: Option<PathBuf>,
}

pub struct AgentViewPlugin;

impl Plugin for AgentViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Editor), spawn_agent_panel.run_if(Workspace::agent_is_open).after(FrameSystems))
            .add_systems(Update, draw_the_history.run_if(in_state(Screen::Editor)).run_if(Workspace::agent_is_open))
            .add_observer(on_look);
    }
}

/// **The agent's last `count` applied changes, newest first**, as `op: what it did`, from the project's operation log.
pub fn agent_changes(root: &std::path::Path, count: usize) -> Vec<String> {
    let entries = crate::ops::read_entries(&crate::ops::oplog_path_at(root));
    entries
        .iter()
        .rev()
        .filter(|e| e.source == crate::ops::Source::Agent && e.outcome == "applied" && e.undoes.is_none())
        .take(count)
        .map(|e| match e.result.as_deref().filter(|r| !r.is_empty()) {
            Some(result) => format!("{}: {result}", e.op),
            None => e.op.clone(),
        })
        .collect()
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
        s.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(chrome::GAP_TIGHT), flex_shrink: 0.0, ..default() })
            .with_children(|h| {
                chrome::section(h, "HISTORY");
                h.spawn((AgentHistory, Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(chrome::GAP_TIGHT), ..default() }));
            });
    });
}

/// **Draw the agent's last changes** when the log has a different set of them.
fn draw_the_history(
    project: Option<Res<Project>>,
    list: Query<(Entity, Option<&Children>), With<AgentHistory>>,
    mut drawn: Local<Drawn>,
    mut commands: Commands,
) {
    let Some(project) = project else { return };
    let Ok((container, children)) = list.single() else {
        // The panel is not up: forget what was drawn, so the next one draws.
        drawn.rows.clear();
        drawn.root = None;
        return;
    };
    let rows = agent_changes(&project.root, HISTORY_ROWS);
    let populated = children.is_some_and(|c| !c.is_empty());
    if populated && drawn.rows == rows && drawn.root.as_ref() == Some(&project.root) {
        return;
    }
    for child in children.into_iter().flatten() {
        commands.entity(*child).despawn();
    }
    drawn.rows.clone_from(&rows);
    drawn.root = Some(project.root.clone());
    commands.entity(container).with_children(|c| {
        if rows.is_empty() {
            chrome::row_value(c, "The agent has made no changes yet", chrome::ink().dim, AgentChangeRow);
        }
        for row in &rows {
            c.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::FlexStart, column_gap: Val::Px(chrome::GAP_TIGHT), ..default() })
                .with_children(|r| {
                    r.spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, min_width: Val::Px(0.0), ..default() }).with_children(|col| {
                        chrome::row_value(col, row.clone(), chrome::ink().text, AgentChangeRow);
                    });
                    chrome::button(r, AgentChangeLook, "Look", Severity::Plain);
                });
        }
    });
}

/// **Look on a change: switch to the Map and frame what the Familiar changed there.**
fn on_look(activate: On<Activate>, buttons: Query<(), With<AgentChangeLook>>, mut commands: Commands) {
    if !buttons.contains(activate.entity) {
        return;
    }
    commands.queue(|world: &mut World| {
        if !world.contains_resource::<crate::editor::EditorState>() {
            return;
        }
        if let Some(mut following) = world.get_resource_mut::<crate::follow::Following>() {
            following.look_now();
        }
        crate::editor::open_door(world, Workspace::Map, None);
    });
}
