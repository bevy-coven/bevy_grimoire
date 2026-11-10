//! **The UI's own clock: what the content-following systems cost on one frame** (UI-93, #259).
//!
//! `docs/ui/vision.md` P6 takes Nielsen Norman Group's 0.1 s as the ceiling for input to visual, and the UI's share of it is
//! [`MAX_MS_PER_FRAME`] (2 ms). **A whole frame's time is not the editor's cost** — the harness, the terminal, the thumbnails
//! and this machine's other work all land in one, and on `bmb` the empty editor's frame is ~4.3 ms before any panel has
//! content (measured 2026-11-09). A budget written against that number fails for reasons that are not the panels.
//!
//! So each system whose work grows with what a panel shows times itself into [`UiFrame`]. The number a reader sees is
//! therefore the panels' own work, which is the question the issue asks ("the UI systems"), and a system that joins the list
//! is measured the frame it is added.
//!
//! # What a frame is, and which frames the budget covers
//!
//! A frame where nothing moved is **settled**, and a settled frame must cost nothing that grows with the content: a long
//! session would otherwise lose the headroom the acting frame needs. That is what [`MAX_MS_PER_FRAME`] is asserted on.
//!
//! A frame where a piece arrived or a search was typed is a **rebuild**, and it is measured and reported separately
//! ([`UiFrame::last_rebuild_ms`]) rather than asserted: it is one frame of work a person asked for, its cost is what it buys
//! (ten thousand rows in the tree), and asserting a settled frame's ceiling on it would be the gate pretending the work does
//! not happen. `UiFrame::within_budget` refuses to answer for a frame that had a rebuild in it at all, so a caller cannot pass
//! the settled gate on a frame that was not settled.
//!
//! **What is not measured here**: the 0.1 s itself — a keypress to the redrawn picture. That is the editor's frame plus the
//! present plus the display, and a headless test cannot see it. It is named as not covered, not claimed.
//!
//! # The guard
//!
//! [`measure`] returns a guard that adds itself to the frame's total when it is dropped, so a system that returns early,
//! falls through an `else` or panics cannot leave its work out of the number. One line at the top of a body is the whole
//! instrumentation.

use std::time::Instant;

use bevy::prelude::*;

/// **The most a settled frame's content systems may cost, milliseconds.** 2 ms of Nielsen Norman Group's 0.1 s, which leaves
/// 98 ms for reading the input, doing what was asked and composing the picture.
pub const MAX_MS_PER_FRAME: f64 = 2.0;

/// **What the content-following systems cost**, this frame and the last.
///
/// Two totals because of when a reader runs: a test (or `grimoire/state`) reads this from outside `Update`, after the systems
/// that would fill this frame's total — so what it can see is the frame that finished.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct UiFrame {
    /// The milliseconds the timed systems have spent in the frame being run.
    pub ms: f64,
    /// How many of them timed themselves in that frame.
    pub systems: u32,
    /// How much of that time was list rebuilds.
    pub rebuild_ms: f64,
    /// **How many spans said they were a rebuild.** Counted rather than inferred from the time, because a rebuild that is
    /// fast enough to round to zero milliseconds is still a rebuild, and a frame is settled or not independently of how long
    /// its work took.
    pub rebuilds: u32,
    /// The same four numbers for the frame that finished.
    pub last_ms: f64,
    pub last_systems: u32,
    pub last_rebuild_ms: f64,
    pub last_rebuilds: u32,
}

impl UiFrame {
    /// **Whether the frame that finished was settled and inside the budget.** `false` for a frame that had a rebuild in it,
    /// however cheap that was: the budget is a statement about frames where nothing moved, and answering for a rebuild would
    /// be the gate claiming a frame it did not measure.
    pub fn within_budget(&self) -> bool {
        self.settled() && self.last_ms < MAX_MS_PER_FRAME
    }

    /// **Whether the frame that finished was settled** — nothing was rebuilt in it.
    pub fn settled(&self) -> bool {
        self.last_rebuilds == 0
    }
}

/// **A running measurement.** Drop it and the time it measured is added to the frame's total, on every path out of the
/// system including a panic; call [`Measured::rebuilding`] to say the span is a list rebuild rather than follow-up work.
pub struct Measured<'a> {
    started: Instant,
    frame: &'a mut UiFrame,
    rebuild: bool,
}

impl<'a> Measured<'a> {
    /// **This span is a list rebuild**, so the frame it lands in is reported as one and the settled gate will not answer for
    /// it.
    pub fn rebuilding(mut self) -> Self {
        self.rebuild = true;
        self
    }
}

impl Drop for Measured<'_> {
    fn drop(&mut self) {
        let ms = self.started.elapsed().as_secs_f64() * 1000.0;
        self.frame.ms += ms;
        self.frame.systems += 1;
        if self.rebuild {
            self.frame.rebuild_ms += ms;
            self.frame.rebuilds += 1;
        }
    }
}

/// **Start timing a system's body.** Bind it (`let _spent = …`) so it lives to the end of the body: a bare call is dropped at
/// once and measures nothing.
#[must_use]
pub fn measure(frame: &mut UiFrame) -> Measured<'_> {
    Measured { started: Instant::now(), frame, rebuild: false }
}

/// **The finished frame is kept and the new one starts at zero**, before any system in `Update` can add to it.
fn roll_the_frame(mut frame: ResMut<UiFrame>) {
    frame.last_ms = frame.ms;
    frame.last_systems = frame.systems;
    frame.last_rebuild_ms = frame.rebuild_ms;
    frame.last_rebuilds = frame.rebuilds;
    frame.ms = 0.0;
    frame.systems = 0;
    frame.rebuild_ms = 0.0;
    frame.rebuilds = 0;
}

pub struct UiBudgetPlugin;

impl Plugin for UiBudgetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiFrame>().add_systems(First, roll_the_frame);
    }
}
