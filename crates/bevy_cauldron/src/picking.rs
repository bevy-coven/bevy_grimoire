//! **Who owns the pointer, and therefore whether picking runs at all.**
//!
//! `bevy::picking::PickingSettings::is_enabled` is the one switch that stops a button seeing a click,
//! and **exactly one system writes it.** Two gestures want it off, for two different reasons and while
//! two different things are true:
//!
//! | gesture | picking is off while |
//! |---|---|
//! | a snip (`snip.rs`) | the snip is armed, drawing a box or holding a note |
//! | a feedback report (`report.rs`) | a person is still pointing at what to report on |
//!
//! They each *ask* here. They used to each write it, and the failure is not hypothetical: the two
//! systems ran in an order Bevy chose, whichever went last won, and
//! `a_snip_armed_over_a_button_does_not_press_it` caught it the first time both features existed
//! (2026-11-05). It is the same defect the crate names everywhere else — a fact stated twice drifts —
//! with the extra bite that this one is a plain `bool`, so nothing can tell a stale write from a fresh
//! one.
//!
//! **A third gesture adds a term to [`claimed`], not a fourth writer.**
//!
//! # Turning it off is not enough on its own — B-032
//!
//! `button_on_pointer_click` (`bevy_ui_widgets-0.20.0/src/button.rs:60-72`) activates a button that
//! carries [`Pressed`](bevy::ui::Pressed) when a `PointerClick` arrives, and the press that set
//! `Pressed` may have happened *before* the gesture claimed the pointer. Picking off stops new presses;
//! it cannot stop a release from completing one already in flight, so a person presses a button,
//! decides mid-press to snip instead, and the button fires anyway. So a gesture that claims the pointer
//! **drops every `Pressed`** on that transition, and that is what [`drop_presses`] is for: each
//! gesture's claiming system calls it once, on the frame it becomes the owner.

use bevy::prelude::*;

use crate::report::ReportEdit;
use crate::snip::SnipEdit;

/// **Does any gesture own the pointer right now?** The only question that turns picking off.
pub fn claimed(snip: &SnipEdit, report: &ReportEdit) -> bool {
    snip.claims_pointer() || report.pointing()
}

/// The one writer of [`bevy::picking::PickingSettings::is_enabled`].
fn gate_picking(
    snip: Res<SnipEdit>,
    report: Res<ReportEdit>,
    settings: Option<ResMut<bevy::picking::PickingSettings>>,
) {
    let Some(mut settings) = settings else {
        return;
    };
    let wanted = !claimed(&snip, &report);
    if settings.is_enabled != wanted {
        settings.is_enabled = wanted;
    }
}

/// **Drop every press in flight**, on the frame a gesture becomes the pointer's owner. Each gesture
/// calls this for itself, from the system that notices the transition — see the module note and B-032.
pub fn drop_presses(in_flight: &mut Query<Entity, With<bevy::ui::Pressed>>, commands: &mut Commands) {
    for entity in in_flight.iter_mut() {
        commands.entity(entity).remove::<bevy::ui::Pressed>();
    }
}

/// **The one pointer gate.** In the shared plugin list: both gestures are in it, and a build with one
/// of them alone would otherwise have no writer at all.
pub struct PickingPlugin;

impl Plugin for PickingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, gate_picking);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Neither gesture, or either one, and the pointer is claimed** — the rule the one writer reads.
    #[test]
    fn either_gesture_claims_the_pointer() {
        let idle = (SnipEdit::default(), ReportEdit::default());
        assert!(!claimed(&idle.0, &idle.1), "nothing is going on");

        let snip = SnipEdit {
            armed: true,
            ..Default::default()
        };
        assert!(claimed(&snip, &idle.1), "the snip is armed");

        let report = ReportEdit::pointing_for_test();
        assert!(claimed(&idle.0, &report), "a report is pointing");
    }
}
