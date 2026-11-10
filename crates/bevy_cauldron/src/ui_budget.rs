//! **The UI's own clock: what the content-following systems cost on one frame** (UI-93, #259).
//!
//! `docs/ui/vision.md` P6 takes Nielsen Norman Group's 0.1 s as the ceiling for input to visual, and the UI's share of it is
//! [`MAX_MS_PER_FRAME`] (2 ms). **A whole frame's time is not the editor's cost** — a headless harness, the terminal, the
//! thumbnails and this machine's other work all land in it, and on `bmb` the empty editor's own frame is ~4.3 ms before any
//! panel has content (measured 2026-11-09). A budget written against that number would fail for reasons that are not the
//! panels.
//!
//! So each system whose work grows with what a panel shows times itself and adds it here: [`begin`] at the top of the body,
//! [`end`] at the bottom. The number the test reads is therefore the panels' own work, which is the question the issue asks
//! ("the UI systems"), and a system that joins the list is measured the frame it is added.
//!
//! **The systems in the list are the ones that read content**: the history list, and the outliner's tree. A system that reads
//! no content cannot grow with it, so measuring it would only add noise to the sum.

use std::time::Instant;

use bevy::prelude::*;

/// **The most the content-following UI systems may cost in one frame, milliseconds.** 2 ms of Nielsen Norman Group's 0.1 s,
/// which leaves 98 ms for reading the input, doing what was asked and composing the picture.
pub const MAX_MS_PER_FRAME: f64 = 2.0;

/// **What the content-following systems cost**, this frame and the last.
///
/// Two totals because of when a reader runs: a test (or `grimoire/state`) reads this from outside `Update`, after the
/// systems that would fill this frame's total — so what it can see is the frame that finished.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct UiFrame {
    /// The milliseconds the listed systems have spent in the frame being run.
    pub ms: f64,
    /// How many of them timed themselves in that frame.
    pub systems: u32,
    /// The same two numbers for the frame that finished.
    pub last_ms: f64,
    pub last_systems: u32,
}

impl UiFrame {
    /// **Whether the frame that finished was inside the budget.** The one predicate the test and the report share, so a
    /// measurement and a gate cannot disagree about what "under 2 ms" means.
    pub fn within_budget(&self) -> bool {
        self.last_ms < MAX_MS_PER_FRAME
    }
}

/// **A system's clock, started.** Hold the answer and give it to [`end`] when the body is done — at every exit, or the total
/// is short by that frame's work.
#[must_use]
pub fn begin() -> Instant {
    Instant::now()
}

/// **Add what `started` has measured to this frame's total**, and count the system that spent it.
pub fn end(started: Instant, frame: &mut UiFrame) {
    frame.ms += started.elapsed().as_secs_f64() * 1000.0;
    frame.systems += 1;
}

/// **The finished frame is kept and the new one starts at zero**, before any system in `Update` can add to it.
fn roll_the_frame(mut frame: ResMut<UiFrame>) {
    frame.last_ms = frame.ms;
    frame.last_systems = frame.systems;
    frame.ms = 0.0;
    frame.systems = 0;
}

pub struct UiBudgetPlugin;

impl Plugin for UiBudgetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiFrame>().add_systems(First, roll_the_frame);
    }
}
