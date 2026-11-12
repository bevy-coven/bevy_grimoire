//! **The UI's own clock: what the content-following systems cost on one frame** (UI-93, #259).
//!
//! `docs/ui/vision.md` P6 takes Nielsen Norman Group's 0.1 s as the ceiling for input to visual, and the UI's share of it is
//! [`MAX_MS_PER_FRAME`] (2 ms). **A whole frame's time is not the editor's cost** — the harness, the terminal, the thumbnails
//! and this machine's other work all land in one, and on `bmb` the empty editor's frame is ~4.3 ms before any panel has
//! content (measured 2026-11-09). A budget written against that number fails for reasons that are not the panels.
//!
//! So each system whose work grows with what a panel shows times itself into [`UiFrame`], and gooey's own table and tree
//! drawing (`update_trees` to `update_tables`) is timed as a span of its own ([`UiFrame::gooey_ms`]), because the virtualized
//! lists are what a long session spends its frames in. The number a reader sees is therefore the panels' own work, which is
//! the question the issue asks ("the UI systems"), and a system that joins the list is measured the frame it is added.
//!
//! # What a frame is, and which frames the budget covers
//!
//! A frame where nothing moved is **settled**, and a settled frame must cost nothing that grows with the content: a long
//! session would otherwise lose the headroom the acting frame needs. That is what [`MAX_MS_PER_FRAME`] is asserted on.
//!
//! A frame where a piece arrived or a search was typed is a **rebuild**: the outliner's and the history's lists changed (a
//! search builds the outliner's tree whole; a piece placed, taken back or marked, and a step of the history, put the one
//! row or two they touch into gooey's tree and leave the rest, `Outliner::patches`). It is reported separately
//! ([`UiFrame::last_rebuild_ms`]) rather than held to the settled ceiling by [`UiFrame::within_budget`], which refuses to
//! answer for a frame that had a rebuild in it at all, so a caller cannot pass the settled gate on a frame that was not
//! settled. **A change has numbers of its own**: every frame from a piece placed to the settled frame is held to
//! [`MAX_MS_PER_FRAME`], at 10,000 outliner nodes and 5,000 history lines, in
//! `tests/the_frame_a_change_lands_on_stays_inside_two_milliseconds.rs`; and the frame where the lists are built whole (a
//! search) is held to its measured floor with headroom in `tests/the_ui_frame_covers_a_change_and_the_input_path.rs`.
//!
//! **What is not measured here**: the present and the display, which a headless test cannot see. The editor's share of
//! the 0.1 s — a keystroke to the frame the narrowed tree stands in — is measured by the same file, and named as the
//! editor's share, not the whole.
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
    /// **How much of that time was gooey's own table and tree drawing** (`update_trees` to `update_tables`), which the
    /// editor's own two systems do not include and a long session spends its frames in. It is part of [`UiFrame::ms`], not
    /// beside it. A wall-clock span over two systems of a chain, so anything that ran beside them is in it too: an upper
    /// bound on gooey's cost, which is the safe direction for a ceiling.
    pub gooey_ms: f64,
    /// The same five numbers for the frame that finished.
    pub last_ms: f64,
    pub last_systems: u32,
    pub last_rebuild_ms: f64,
    pub last_rebuilds: u32,
    pub last_gooey_ms: f64,
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
    frame.last_gooey_ms = frame.gooey_ms;
    frame.ms = 0.0;
    frame.systems = 0;
    frame.rebuild_ms = 0.0;
    frame.rebuilds = 0;
    frame.gooey_ms = 0.0;
}

/// **When gooey's table and tree drawing began this frame**, for [`end_the_gooey_span`] to read.
#[derive(Resource, Default)]
struct GooeySpan(Option<Instant>);

/// **Start the clock on gooey's own drawing**, just before `update_trees`.
fn begin_the_gooey_span(mut span: ResMut<GooeySpan>) {
    span.0 = Some(Instant::now());
}

/// **Stop it just after `update_tables`** and add what passed to the frame, as a timed system. gooey chains its style
/// systems, so `update_trees` and `update_tables` run back to back: the span is those two and anything unrelated the
/// scheduler ran beside them.
fn end_the_gooey_span(mut span: ResMut<GooeySpan>, mut frame: ResMut<UiFrame>) {
    let Some(began) = span.0.take() else { return };
    let ms = began.elapsed().as_secs_f64() * 1000.0;
    frame.ms += ms;
    frame.gooey_ms += ms;
    frame.systems += 1;
}

pub struct UiBudgetPlugin;

impl Plugin for UiBudgetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiFrame>()
            .init_resource::<GooeySpan>()
            .add_systems(First, roll_the_frame)
            // **Pinned between gooey's own neighbours**, so the span is not the whole of `Update`: the style systems are a
            // chain, `update_splits` then `update_trees` then `update_tables` then `update_scroll_glides`, and the two
            // ends sit in the gaps either side of the two systems being timed.
            .add_systems(
                Update,
                (
                    begin_the_gooey_span.after(bevy_gooey::update_splits).before(bevy_gooey::update_trees),
                    end_the_gooey_span.after(bevy_gooey::update_tables).before(bevy_gooey::update_scroll_glides),
                ),
            );
    }
}
