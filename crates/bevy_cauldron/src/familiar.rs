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

use bevy::prelude::*;
use bevy_gooey::{Checked, ValueChange};

use crate::screen::Screen;

/// **Whether the agent's writes are paused.**
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AgentPause(pub bool);

/// The View ▸ Pause agent writes row.
#[derive(Component)]
pub struct PauseItem;

pub struct FamiliarPlugin;

impl Plugin for FamiliarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AgentPause>().add_systems(Update, tick_the_menu_row.run_if(in_state(Screen::Editor)));
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
