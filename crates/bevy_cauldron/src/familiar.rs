//! **The Familiar's switch: View ▸ Pause agent writes** (UI-60, #246).
//!
//! An agent drives the editor through `grimoire/operation_apply`, and every change it makes is already
//! attributed (the history row says "the Familiar", the INSPECTOR's LAST line says `agent:`) and takes back
//! with one Cmd+Z. What a person lacked was a way to say *not now*: to stop the agent changing the map while
//! they look at it, think, or take over, without disconnecting it or losing its place.
//!
//! [`AgentPause`] is that switch, one boolean for the whole editor. While it is on:
//!
//! - **`grimoire/operation_apply` and `grimoire/operation_undo` refuse every write**, with the editor's own
//!   sentence ([`refusal`]) in the answer (`outcome: "refused"`) and on the status line as a problem, and change
//!   nothing and log nothing. The agent can read what the sentence says and wait, which is what a person asking
//!   a colleague to hold on would expect.
//! - **A preview is not a write.** `dry_run` answers as it always does, so an agent can keep planning.
//! - **A person's own edits are untouched.** The switch is about the agent's writes, not the editor's.
//!
//! The row in the View menu is ticked while the switch is on, and takes the tick off when it is turned off by
//! any other means. Nothing is saved: a new session starts unpaused, because a pause a person forgot should
//! not outlive the thing they were looking at.
//!
//! # The AGENT block
//!
//! View ▸ Show agent puts a block at the top of the Map door's INSPECTOR panel that answers the three things a
//! person beside an agent wants to know without opening a log (Amershi's G1, G2 and G7):
//!
//! - **When the editor last heard from it**, and by which call: `Last heard: operation_apply, 3 s ago`. This is
//!   what the editor can know, not a claim of a connection: an agent that has asked nothing is not heard.
//! - **Whose the newest change is**, in words: `the agent's: turned table@0 to 90 deg` or `yours: ...`, from the
//!   same history the History panel lists.
//! - **A way to take the agent's last change back, and to stop it.** The Undo button presses Undo's own chord, so
//!   it is exactly Cmd+Z and lands in the history as any undo does; it is greyed unless the newest change is the
//!   agent's, because a button that undid *your* edit under the agent's name would be a lie. The switch is the
//!   View row's twin.
//!
//! The block is not there until it is asked for, for the reason the outliner's is not: screens that count every
//! button in the world would find these ones.

use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy_gooey::{Activate, Checked, ValueChange, button, label, observe, toggle};

use crate::commands;
use crate::editor::{EditorState, MapControlsPane};
use crate::project::OpenMap;
use crate::screen::Screen;

/// **Whether the agent's writes are paused.**
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AgentPause(pub bool);

/// The View ▸ Pause agent writes row.
#[derive(Component)]
pub struct PauseItem;

/// What the editor last heard from an agent: the one fact it can know about it. Session state, never saved.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct AgentActivity {
    heard: u32,
    last: Option<Heard>,
}

/// One call from an agent.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    /// The method, without its `grimoire/` prefix.
    pub method: &'static str,
    /// Seconds on the editor's real clock.
    pub at_s: f64,
}

impl AgentActivity {
    /// An agent called `method` at `at_s`.
    pub fn hear(&mut self, method: &'static str, at_s: f64) {
        self.heard += 1;
        self.last = Some(Heard { method, at_s });
    }

    /// How many calls have been heard.
    pub fn heard(&self) -> u32 {
        self.heard
    }

    /// The newest call, if any was heard.
    pub fn last(&self) -> Option<&Heard> {
        self.last.as_ref()
    }

    /// The line the AGENT block says it in.
    pub fn sentence(&self, now_s: f64) -> String {
        match &self.last {
            None => "not heard this session".to_owned(),
            Some(Heard { method, at_s }) => {
                let ago = (now_s - at_s).max(0.0).round() as u64;
                let calls = if self.heard == 1 { "1 call".to_owned() } else { format!("{} calls", self.heard) };
                format!("{method}, {ago} s ago ({calls})")
            }
        }
    }
}

pub struct FamiliarPlugin;

impl Plugin for FamiliarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AgentPause>().init_resource::<AgentActivity>().add_systems(
            Update,
            (tick_the_menu_row, tick_the_familiar_row, follow_the_agent, follow_the_pause).run_if(in_state(Screen::Editor)),
        );
    }
}

/// The View ▸ Show agent row.
#[derive(Component)]
pub struct FamiliarItem;

/// The AGENT block in the INSPECTOR panel.
#[derive(Component)]
pub struct Familiar;

/// One of the block's lines of text.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FamiliarLine {
    /// When the agent was last heard.
    Heard,
    /// Whose the newest change is, and what it was.
    Newest,
    /// What the agent handed back, and what the person answered.
    HandBack,
}

/// The block's pause switch.
#[derive(Component)]
pub struct FamiliarSwitch;

/// The block's Undo button.
#[derive(Component)]
pub struct FamiliarUndo;

/// **View ▸ Show agent puts the AGENT block into the Map door's INSPECTOR panel, first among its rows, and takes it
/// out again.**
pub fn toggle_the_familiar(
    change: On<ValueChange<bool>>,
    panes: Query<Entity, With<MapControlsPane>>,
    blocks: Query<Entity, With<Familiar>>,
    pause: Res<AgentPause>,
    mut commands: Commands,
) {
    if !change.value {
        for block in &blocks {
            commands.entity(block).despawn();
        }
        return;
    }
    if !blocks.is_empty() {
        return;
    }
    let Ok(pane) = panes.single() else { return };
    let block = commands
        .spawn((
            Familiar,
            Name::new("agent"),
            // **Never squeezed**: the INSPECTOR's rows scroll, and a flex item that may shrink is made shorter than its text when
            // the panel's rows are taller than the panel, which draws one line over the next (found in a capture, where the
            // headless layout, whose panel is not full, shows nothing wrong).
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            },
        ))
        .with_children(|block| {
            crate::chrome::list_heading(block, "AGENT");
            // **The INSPECTOR's own row**: a label column and a value column that wraps beside it. A bare gooey `label` here
            // was measured one line short of what it drew once it wrapped at this panel's width, and the next line was
            // drawn over it (found in a capture; the headless layout does not show it); the rows beside this block are the
            // builders that have been drawn at this width since the readout began, so these are theirs.
            for (line, name) in [(FamiliarLine::Heard, "HEARD"), (FamiliarLine::Newest, "NEWEST"), (FamiliarLine::HandBack, "HAND-BACK")] {
                block.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::FlexStart, flex_shrink: 0.0, ..default() }).with_children(|row| {
                    crate::chrome::row_label(row, crate::editor::READOUT_LABEL_W, name);
                    row.spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, min_width: Val::Px(0.0), ..default() }).with_children(|col| {
                        crate::chrome::row_value(col, "", crate::chrome::TEXT, line);
                    });
                });
            }
            block.spawn((toggle("Pause agent writes", pause.0), FamiliarSwitch, observe(toggle_the_pause)));
            block.spawn((button("Undo the agent's change"), FamiliarUndo, observe(undo_the_agents_change)));
        })
        .id();
    commands.entity(pane).insert_children(0, &[block]);
}

/// **The Undo button presses Undo's chord** when the newest change is the agent's, and otherwise says why not.
fn undo_the_agents_change(
    activate: On<Activate>,
    buttons: Query<(), With<FamiliarUndo>>,
    mut chords: ResMut<crate::command_run::Chords>,
    live: Res<crate::keys::Live>,
    mut state: Option<ResMut<EditorState>>,
) {
    if !buttons.contains(activate.entity) {
        return;
    }
    let Some(state) = state.as_deref_mut() else { return };
    if !newest_is_the_agents(state) {
        state.status.note("the newest change is yours, not the agent's: nothing was undone".to_owned());
        return;
    }
    let Some(command) = commands::by_id("edit.undo") else { return };
    chords.queue(&command, *live);
    state.status.note("asked for Undo from the AGENT block".to_owned());
}

/// Is the newest change on the undo stack the agent's?
pub fn newest_is_the_agents(state: &EditorState) -> bool {
    let depth = state.undo_depth();
    state.history_window(depth, depth).first().is_some_and(|row| row.agent)
}

/// What the block says about the newest change: whose it is and what it was.
fn newest_sentence(state: &EditorState, map: &bevy_grimoire_core::map::Map) -> String {
    match state.undo_next(map) {
        None => "none yet".to_owned(),
        Some(what) if newest_is_the_agents(state) => format!("the agent's: {what}"),
        Some(what) => format!("yours: {what}"),
    }
}

/// What the block says about the agent's last hand-back (`grimoire/present`): waiting on the person, or how they answered.
/// Empty when nothing has been handed back this session.
fn hand_back_sentence(presenting: &crate::present::Presenting) -> String {
    match &presenting.0 {
        None => String::new(),
        Some(presented) => match &presented.answer {
            None => format!("{}: waiting for your answer", presented.title),
            Some(answer) => format!("{}: you answered {answer}", presented.title),
        },
    }
}

/// **The block follows the editor**: its lines are rewritten when the words change, and the Undo button is greyed
/// unless pressing it would undo the agent's change.
#[allow(clippy::too_many_arguments)]
fn follow_the_agent(
    activity: Res<AgentActivity>,
    time: Res<Time<Real>>,
    state: Option<Res<EditorState>>,
    open: Option<Res<OpenMap>>,
    presenting: Option<Res<crate::present::Presenting>>,
    mut lines: Query<(&mut Text, &FamiliarLine)>,
    undo: Query<(Entity, Has<InteractionDisabled>), With<FamiliarUndo>>,
    mut commands: Commands,
) {
    for (mut text, line) in &mut lines {
        let said = match line {
            FamiliarLine::Heard => activity.sentence(time.elapsed_secs_f64()),
            FamiliarLine::Newest => match (&state, &open) {
                (Some(state), Some(open)) => newest_sentence(state, &open.map),
                _ => continue,
            },
            FamiliarLine::HandBack => presenting.as_deref().map(hand_back_sentence).unwrap_or_default(),
        };
        if text.0 != said {
            text.0 = said;
        }
    }
    let Some(state) = state else { return };
    let can = newest_is_the_agents(&state);
    for (button, disabled) in &undo {
        match (can, disabled) {
            (true, true) => {
                commands.entity(button).remove::<InteractionDisabled>();
            }
            (false, false) => {
                commands.entity(button).insert(InteractionDisabled);
            }
            _ => {}
        }
    }
}

/// The block's switch says what the switch says, whichever way it was set.
fn follow_the_pause(pause: Res<AgentPause>, switches: Query<(Entity, Has<Checked>), With<FamiliarSwitch>>, mut commands: Commands) {
    for (switch, ticked) in &switches {
        match (pause.0, ticked) {
            (true, false) => {
                commands.entity(switch).insert(Checked);
            }
            (false, true) => {
                commands.entity(switch).remove::<Checked>();
            }
            _ => {}
        }
    }
}

/// The tick on View ▸ Show agent says whether the block is there.
fn tick_the_familiar_row(items: Query<(Entity, Has<Checked>), With<FamiliarItem>>, blocks: Query<(), With<Familiar>>, mut commands: Commands) {
    let present = !blocks.is_empty();
    for (item, ticked) in &items {
        match (present, ticked) {
            (true, false) => {
                commands.entity(item).insert(Checked);
            }
            (false, true) => {
                commands.entity(item).remove::<Checked>();
            }
            _ => {}
        }
    }
}

/// What a refused write says, to the agent in its answer and to the person on the status line.
pub fn refusal(operation: &str) -> String {
    format!(
        "the agent's writes are paused, so `{operation}` changed nothing. A person turned them off with View > Pause agent writes; \
         turn that off to let the agent write again. Previews still work."
    )
}

/// The row was chosen: the switch follows the tick.
pub fn toggle_the_pause(change: On<ValueChange<bool>>, mut pause: ResMut<AgentPause>) {
    if pause.0 != change.value {
        pause.0 = change.value;
    }
}

/// The tick says what the switch says, however it was set.
fn tick_the_menu_row(pause: Res<AgentPause>, items: Query<(Entity, Has<Checked>), With<PauseItem>>, mut commands: Commands) {
    for (item, ticked) in &items {
        match (pause.0, ticked) {
            (true, false) => {
                commands.entity(item).insert(Checked);
            }
            (false, true) => {
                commands.entity(item).remove::<Checked>();
            }
            _ => {}
        }
    }
}
