//! **The editing loop** — a palette, a ghost, a click, a file.
//!
//! # The ghost is the contract
//!
//! Everything an author does here goes through a preview that stands where the thing will actually
//! be. That is not polish; it is the rule `containment::cordon` states and the Site editor had to
//! learn twice: a preview drawn somewhere the piece will not end up *"is worse than no preview,
//! because it is a promise the game then breaks."* So the ghost is snapped, aimed, and lifted onto
//! its host exactly as the real placement will be.
//!
//! # Aiming happens before placing
//!
//! `[` and `]` turn the **brush**, never the last thing placed. The Site editor bound them to the
//! selection and it made rotation feel broken: placing selects, so the next `]` turned the piece you
//! had just put down while the ghost — the only thing on screen showing a facing — sat still.

use bevy::camera::visibility::ViewVisibility;
use crate::filter::FilterTexts;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{Activate, Button as UiButton};
use bevy_grimoire_core::map::Placed;
use bevy_grimoire_core::stack::{descriptor_of, descriptor_with};

use crate::chrome::{
    ACCENT, BOUNDS_LINE, CHIP_PAD, DANGER, DIM, FOCUS_BG, GRID_LINE, HEADER_BG, LABEL, LIST_W,
    ROW_BG, ROW_HOVER, ROW_SELECTED, SLOT_BG, TEXT,
};
use crate::keys::{self, Action};
use crate::project::{OpenMap, Project};
use crate::view::{MainCamera, cursor_ground};

/// **The authoring grid, from the one place that defines it.**
///
/// This was `const SNAP: f32 = 0.5` — a second copy of `bevy_grimoire_core::grid::SNAP`, in the one file
/// that snaps, while `lift_step` two thousand lines below already used the real one. `grid`'s own
/// module note says why that is not allowed: *"Nothing here is clever. It is here so there is
/// exactly one of it."* It had already drifted from a third statement of the same fact — the drawn
/// grid, which `view.rs` set to 1.0 m under a comment insisting it was the snap.
use bevy_grimoire_core::grid::{self, SnapLevel};
/// Yaw snap, degrees.
const YAW_STEP: f32 = 15.0;

/// Where a descriptor with no `kind` goes. Named rather than hidden: an untagged piece is work to do,
/// and a palette that quietly omitted it would be a palette missing pieces.
const UNSORTED: &str = "unsorted";
// The bounds wireframe and the grid lines are `chrome::{BOUNDS_LINE, GRID_LINE}` — world ink two
// tabs state, so the values live with the palette rather than drifting here and in `tiles`.

/// **The editable area's own ground.** Lighter than the void it sits in — `ClearColor` here is
/// `srgb(0.035, 0.033, 0.030)`, so this reads as a slab laid on nothing rather than as a tint.
///
/// A wireframe box said where the bounds were and an author still had to trace it with their eye to
/// tell inside from outside; `Map::bounds` is the thing every refusal in `fill` and `flood` is about,
/// and it was the one piece of state the world did not show.
const BOUNDS_FILL: Color = Color::srgb(0.105, 0.100, 0.092); // CHROME-OK: world ink — the slab under this map, read against chrome::VOID, no panel role

// The map grid's line is `chrome::GRID_LINE` — world ink two tabs state, one value in the palette.
/// **The dressing rungs' lines, quieter than the module's.**
///
/// A minor line that reads as loud as a major one is not a second scale, it is noise: the eye has to
/// count to find the tile boundary instead of seeing it. Darker rather than a different hue, because
/// they are the same ladder and a colour change would say they were not.
/// **A cell a pending layout would take.**
///
/// The same slate `chrome::SUGGEST` uses for the VLM labeler's machine-proposed state, and for the
/// same reason its note gives: a proposal is *a question*, so it must read as neither an answer
/// (amber, a live edit of yours) nor an alarm (red, something wrong).
pub(crate) const PROPOSAL_LINE: Color = Color::srgb(0.42, 0.55, 0.68); // CHROME-OK: world ink — a pending layout's cell gizmo, SUGGEST's role in the world

/// The **least** the slab drops below the datum, metres — enough to beat z-fighting against a piece
/// lying at `y_offset: 0.0`. See [`ground_drop`], which is usually deeper.
const BOUNDS_FILL_CLEARANCE: f32 = 0.005;

/// **How far the slab sits under the drawn ground**, metres (#267). The ground is at
/// `origin.y - ground_drop + height`, so a flat ground at height 0 was in the slab's own plane and
/// the depth buffer drew the two in either order, hatched. Over the widest depth range the Map camera
/// is given (a 1.1 km map: about 3 km) the buffer separates surfaces closer than 0.7 mm; a centimetre
/// is well clear of that and too small to see from the camera. The grid and the gizmos stay at
/// [`ground_drop`], on the ground.
pub const BOUNDS_FLOOR_BELOW_GROUND: f32 = 0.01;

/// **How far under the datum the backdrop sits** — deeper than anything the library can sink.
///
/// This was a flat 5 mm, on the assumption that a floor piece rests *on* the datum. It does not:
/// `stack::datum` adds `align.y_offset` on top of whatever the mount decided, and says why in as
/// many words — *"a floor grate is 6 cm into its floor"*. The site kit's `site/floor` is authored at
/// `y_offset: -0.06`, so it sat **below** a 5 mm slab and the backdrop drew over it. Every floor
/// tile in the map vanished under grey and got sliced by grid lines, while `site/floor_button` at
/// `+0.002` was untouched — which is exactly how it was reported.
///
/// Measured from the library rather than picked, so a kit that sinks something deeper still works
/// and no constant has to be revisited. The grid lines take the same depth: real geometry should
/// occlude them, and there is nothing to draw a cell for where a floor already fills it.
pub fn ground_drop(project: &Project,
) -> f32 {
    let deepest = project
        .library
        .descriptors
        .iter()
        .filter_map(|d| d.align.y_offset)
        .fold(0.0f32, f32::min);
    // `deepest` is zero or negative; the slab goes under it by the clearance.
    -deepest + BOUNDS_FILL_CLEARANCE
}

/// **The ghost's hint over bare floor, short enough for the INSPECTOR's LAST slot.**
///
/// A surface piece's refusal is the core's sentence, which runs to three body lines in a slot two lines
/// tall, and the slot's extra height then moves MAP SIZE down under the pointer. The ghost writes the
/// short sentence for that one refusal, with the piece's id left out so it fits two lines. Every other
/// refusal (a datum error, or a piece that has its surface) passes through unchanged.
pub fn ghost_hint(d: &bevy_grimoire_core::descriptor::Descriptor, refusal: &str) -> String {
    match bevy_grimoire_core::stack::needs_surface(d) {
        Some(class) if refusal == bevy_grimoire_core::stack::no_surface_sentence(d, class) => {
            format!("goes on a `{class}` surface — none here")
        }
        _ => refusal.to_owned(),
    }
}

/// The removal marker. Red because it is the one destructive tool here, and translucent because the
/// thing it covers is the thing being asked about — an opaque marker would hide the answer.
const REMOVE_TINT: Color = Color::srgba(0.86, 0.20, 0.16, 0.38); // CHROME-OK: world ink — a translucent tool tint over geometry, not panel text

/// The clone tool's marker tint — cool where removal is hot, because the two rectangles make
/// opposite promises and an author reads the colour before the status line.
const CLONE_TINT: Color = Color::srgba(0.20, 0.55, 0.86, 0.32); // CHROME-OK: world ink — see REMOVE_TINT

/// The target lock's tint — gold: neither removal's threat nor clone's copy, just "this one".
const TARGET_TINT: Color = Color::srgba(0.90, 0.72, 0.20, 0.35); // CHROME-OK: world ink — see REMOVE_TINT
/// How far the marker floats above the floor, metres. Enough to beat z-fighting against a floor tile
/// it is lying exactly on top of, small enough to still read as flat on the ground.
const MARKER_LIFT: f32 = 0.02;
/// A press-and-release inside this many metres is a click, not a drag — so "click one piece" and
/// "drag a box" are the same gesture told apart by distance rather than by a second modifier.
const CLICK_EPS: f32 = 0.2;

/// Scene triangle counts worth colouring. Not limits — this machine draws far more than either — but
/// the points at which an author should know what they have built.
const BUSY_SCENE: usize = 1_000_000;
const HEAVY_SCENE: usize = 5_000_000;

/// The chrome bar's own inset, either side of the readout. Named so [`COST_COL`] is derived from the
/// same number the node applies rather than from a copy of it. `pub` for the same reason
/// [`TriangleTotal`] is: the test that checks the column measures it against what the node was given
/// rather than against a literal copied out of here.
pub const COST_PAD_X: f32 = 8.0;

/// **The column the triangle count holds**, so the map's name beside it does not move.
///
/// Derived from what the readout can draw, not from a colour threshold. [`HEAVY_SCENE`] tints the
/// digits and bounds nothing — `refresh_triangle_total` sums every visible mesh, and this file's own
/// note names a flood fill that costs forty million triangles. Sized at [`HEAVY_SCENE`]'s order of
/// magnitude the column was one character short: `10,000,000 tris drawn` is twenty-one characters,
/// 138.6 px of glyphs in a 132 px content box, and because it is `min_width` the node grew and
/// shoved `WhereYouAre` — the exact bouncing the constant exists to stop.
///
/// So it is the widest string the format can produce: `with_thousands(usize::MAX)` is twenty digits
/// and six separators, and ` tris drawn` is eleven more. `Node::box_sizing` defaults to
/// `BoxSizing::BorderBox`, so [`COST_PAD_X`] is inside the width and has to be added here.
const COST_CHARS: f32 = 26.0 + 11.0;
pub const COST_COL: f32 = COST_CHARS * crate::chrome::BODY_CHAR_W + 2.0 * COST_PAD_X;

#[derive(Resource)]
pub struct EditorState {
    /// Index into the library — what a click would place, or `None` for **nothing armed**.
    ///
    /// This was a bare `usize`, so index 0 was always armed and "I am not placing anything" was a
    /// state the editor could not be in. There was therefore nothing for `Esc` to clear, and no way to
    /// put the cursor over the map without a piece following it — which matters most for the two tools
    /// that are *about* pieces already on the map rather than about the palette.
    ///
    /// `None` is a real answer everywhere it is read: no ghost, no placement, no highlighted row, and
    /// `BRUSH  none` on the status block.
    pub brush: Option<usize>,
    pub brush_yaw: f32,
    /// **The tip the next placement lands at** — quarter turns about X and Z, `Placed::tip`'s own
    /// units. The brush had a yaw and no tip, so `Y`/`U` had nothing to write and could only ever
    /// act on a piece already down; that asymmetry is what made the rotate cluster act on two
    /// different subjects. Reset by [`keys::Action::Straighten`] along with the yaw.
    pub brush_tip: (u8, u8),
    /// See [`crate::chrome::Status`]. The colour used to be decided here by
    /// `status.starts_with("NOT SAVED")`, so that one message was red and `stamp refused:`,
    /// `is not a number of metres` and every `cannot draw it:` were the same grey as a receipt.
    pub status: crate::chrome::Status,
    /// The ghost's running commentary. Written every frame; never mixed with [`Self::status`].
    pub hint: String,
    /// Monotonic counter behind generated placement ids, so two crates never share a name.
    next_id: u32,
    /// Advanced on every solve, so pressing `G` twice offers a different arrangement rather than the
    /// same one — a generator that cannot be asked again is one you have to undo to disagree with.
    seed: u64,
    /// A pin waiting for its reason: the placement id, and what has been typed so far.
    ///
    /// `Placed::owned_because` is *a reason, never a bool*, on the schema's own argument: a bool lets
    /// "I could not be bothered" and "this is the cell block's only entrance" look identical in a
    /// diff. A canned reason supplied by the editor would be that bool wearing a sentence, so pinning
    /// asks.
    pinning: Option<(String, String)>,
    /// **The name being typed for a group being captured**, or `None`.
    ///
    /// `pinning`'s shape and for the same reason: a group's name is the author's, and a canned one
    /// supplied by the editor would be a field nobody reads wearing a name nobody chose. The set it
    /// names is `CloneDrag::held`, which stays in hand until the name is committed or abandoned.
    pub grouping: Option<String>,
    /// **Which existing composition the open name field has armed for replacement.**
    ///
    /// Capturing over a name that exists redefines it, which every stamp of it in every map follows.
    /// That is destructive enough to ask twice, and the arm is what makes the second `Enter` mean
    /// "yes, redefine it" rather than "I pressed Enter again". Cleared the moment the name is edited,
    /// because the confirmation was for THAT name — the rule `tiles::DemoteArm` already follows.
    pub replacing: Option<String>,
    /// The raw text being typed into the name, or `None` when not renaming.
    ///
    /// Raw, with the snake_case spelling applied for display and on commit — so a backspace undoes a
    /// keystroke rather than an underscore the transform inserted, and "Site 67" reads back as
    /// `site_67` while it is still being typed.
    renaming: Option<String>,
    /// The raw text being typed into the rule's seed, or `None` when not editing it. Raw for the
    /// same reason `renaming` is: a backspace undoes a keystroke, not a transform's insertion.
    ///
    /// **Public, as `grouping` is**, so a test can open the field and prove the typing guard holds —
    /// the one property of a text field that no screenshot can show.
    pub rule_seed: Option<String>,
    /// What can be undone, most recent last. See [`Undo`].
    undo: Vec<Undo>,
    /// What has been undone and can be put back, most recent last.
    ///
    /// **Cleared by any new edit**, which is what stops a redo from replaying an operation against a
    /// map that has moved on underneath it — `Undo` addresses rows by index, so a redo across an
    /// intervening edit would put pieces back at positions that now mean something else.
    ///
    /// It is **not** cleared by changing tabs. The Tiles tab keeps its own pair (`ImportState::undo`)
    /// and neither can reach the other's, so leaving a tab is not a reason to forget what you did on
    /// it.
    redo: Vec<Undo>,
    /// Refusals of the top undo entry in a row. The first keeps the entry (B-026); the second drops
    /// it, because an entry that refuses twice will not apply and would sit above every older change.
    /// Any applied change and any new edit reset it. See [`EditorState::refused_entry`].
    refused_undo: u8,
    /// The same count for the redo stack.
    refused_redo: u8,
    /// Which tool the next click belongs to. See [`Tool`].
    ///
    /// **The tool here and the drag in [`RemovalDrag`] / [`MoveDrag`]**, not one struct:
    /// `rebuild_palette` runs on `resource_changed::<EditorState>`, and a drag corner written every
    /// frame would tear the whole palette down and rebuild it at frame rate. This changes twice per
    /// use; those change every frame, so they live elsewhere.
    pub tool: Tool,
    /// **A question is up: leaving for the chooser would lose unsaved work.**
    ///
    /// This began as a flat refusal — `Cmd+O` said *"unsaved changes, `Cmd+S` first"* and did
    /// nothing, on the argument that a refusal cannot lose anything. Asked for at the keyboard:
    /// *"[it] should probably prompt the user for any unsaved changes if they go back to the main
    /// menu."* A refusal is safe and it is also a dead end — the author still wants to leave, and
    /// the editor made them work out how on their own.
    ///
    /// So it asks, and every answer is a key that names what it does: `S` save and go, `D` discard
    /// and go, `Esc` stay. `Enter` deliberately answers nothing — it is the most-pressed key in this
    /// editor, and a question about discarding work must not be answerable by reflex. That is the
    /// same rule the chooser's delete prompt follows, where `Y` agrees and `Enter` does not.
    pub leaving: bool,
    /// **A door an operation asked for, waiting to be crossed to.** `door_open` sets it, from the keys
    /// and from an agent alike. It is taken by the crossing, or held while the leaving question is up.
    pub door_request: Option<DoorRequest>,
    /// **A command of the project's waiting for the person to allow it** (Send, Play), and where their
    /// trust store is. See [`crate::trusted`].
    pub trust: crate::trusted::TrustState,
}

impl EditorState {
    /// **The solver's seed for the next proposal**, advanced by exactly one.
    ///
    /// A generator that cannot be asked again is one the author has to undo to disagree with, so
    /// every press advances this. It lives on the state rather than in the proposer so that a
    /// proposal made by an agent (which supplies its own seed) and one made by a key behave the
    /// same way on the *next* press.
    pub fn next_seed(&mut self) -> u64 {
        self.seed = self.seed.wrapping_add(1);
        self.seed
    }

    /// **How many steps back are available**, for a test that needs to assert an act recorded
    /// *nothing* — discarding a proposal, for one, which writes nothing and so must take nothing
    /// back. The stack itself stays private: this answers a question, it does not hand over the rows.
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// **The newest undo, named**, for the INSPECTOR's UNDO row and `grimoire/state`.
    pub fn undo_next(&self, map: &bevy_grimoire_core::map::Map) -> Option<String> {
        self.undo.last().map(|op| op.describe(map, false))
    }

    /// **The newest redo, named the same way**, for the REDO row and `grimoire/state`.
    pub fn redo_next(&self, map: &bevy_grimoire_core::map::Map) -> Option<String> {
        self.redo.last().map(|op| op.describe(map, true))
    }

    /// **How many redos are stacked**, for the REDO row's ` (+N)`.
    pub fn redo_depth(&self) -> usize {
        self.redo.len()
    }

    /// **The history, oldest first (E-018, UI-53)**, for the history list: the undo stack, then the
    /// redo stack in the order redo would replay it. Row `n` is the entry at position `n`, and a
    /// jump to position `n` leaves that entry applied.
    ///
    /// **Only the rows asked for.** The list shows a window around the current position
    /// ([`EditorState::history_window`]), and the whole list is 5,000 entries on a long session, so
    /// building every label to throw all but seven away was an allocation per entry per refresh
    /// (UI-53's "5,000 log lines scroll within the UI frame budget"). `first` and `last` are
    /// 1-based positions, both inclusive; a position past the end is not returned.
    pub fn history_window(&self, first: usize, last: usize) -> Vec<HistoryRow> {
        let total = self.undo.len() + self.redo.len();
        let first = first.max(1);
        let last = last.min(total);
        if first > last {
            return Vec::new();
        }
        (first..=last)
            .filter_map(|position| self.history_row(position))
            .collect()
    }

    /// **One row of the history by position**, or `None` past the end: the undo stack first, then the
    /// redo stack in the order redo would replay it.
    pub fn history_row(&self, position: usize) -> Option<HistoryRow> {
        if position == 0 || position > self.undo.len() + self.redo.len() {
            return None;
        }
        let (op, redo) = if position <= self.undo.len() {
            (&self.undo[position - 1], false)
        } else {
            let back = position - self.undo.len();
            (self.redo.get(self.redo.len().checked_sub(back)?)?, true)
        };
        Some(HistoryRow { position, agent: op.by_agent(), label: op.kind(redo) })
    }

    /// **The whole history, oldest first (E-018)**, for a caller that wants all of it: the undo stack,
    /// then the redo stack in the order redo would replay it. The panel uses
    /// [`EditorState::history_window`] instead, so 5,000 entries do not cost 5,000 labels a frame.
    pub fn history(&self) -> Vec<HistoryRow> {
        let undone = self.redo.iter().rev().map(|op| (op, true));
        self.undo
            .iter()
            .map(|op| (op, false))
            .chain(undone)
            .enumerate()
            .map(|(i, (op, redo))| HistoryRow {
                position: i + 1,
                agent: op.by_agent(),
                label: op.kind(redo),
            })
            .collect()
    }

    /// **Record an edit**, and drop anything that was waiting to be redone.
    ///
    /// One place, so no edit site can forget the second half. A redo stack that survived a new edit
    /// would replay operations against a map that had moved on under them — and `Undo` addresses rows
    /// by index, so "put these three back at 7, 8 and 9" means something different once anything else
    /// has been inserted or removed.
    fn record(&mut self, op: Undo) {
        self.undo.push(op);
        self.redo.clear();
        self.settle();
    }

    /// **Collapse one agent turn's entries into a single undo step (E-071).**
    ///
    /// The brief: *"An agent's multi-step action is grouped into one undo step, so a single undo
    /// reverts the whole agent step."* Every dispatched operation makes its own [`Undo::Logged`], so
    /// an eight-operation turn was eight Cmd+Z presses and the person had no way to know where the
    /// turn ended.
    ///
    /// **The boundary is the presentation**, because the agent already declares it: `grimoire/present`
    /// takes `operation_ids` — *"the operations this covers"* — and `Undo all` walks them. Making that
    /// list one entry on the stack is what makes Cmd+Z and `Undo all` take back the **same amount**
    /// instead of being two answers to one question.
    ///
    /// # Only the ids it names, and only leaves
    ///
    /// An entry is taken when it is a `Logged` for one of `ids`. Everything else on the stack is left
    /// exactly where it is — the person's own edit before the turn, a `Pin`, a `Settings`, a
    /// `Group` some earlier act already made. **Nothing newer than the newest named id is touched**,
    /// because an operation the agent did *after* presenting is not part of the turn it presented.
    ///
    /// A turn of one operation is left alone: a group of one is the entry it already has, and
    /// wrapping it would make the receipt read `1 change` for a single click.
    ///
    /// Returns how many entries were collapsed, so the caller can say so.
    pub(crate) fn group_turn(&mut self, ids: &[String]) -> usize {
        if ids.len() < 2 {
            return 0;
        }
        let named = |op: &Undo| matches!(op, Undo::Logged { line, .. } if ids.iter().any(|id| id == line));
        let found = self.undo.iter().filter(|op| named(op)).count();
        if found < 2 {
            return 0;
        }
        let mut taken: Vec<Undo> = Vec::with_capacity(found);
        let mut rest: Vec<Undo> = Vec::with_capacity(self.undo.len() - found);
        for op in self.undo.drain(..) {
            if named(&op) {
                taken.push(op);
            } else {
                rest.push(op);
            }
        }
        // **The children are reversed, because a turn is undone newest-first.**
        //
        // The stack is ordered oldest-first and `apply` runs a group's children in order — which is
        // right for the inverses the group *produces*, and wrong for the oplog walk the children
        // drive. `ops::undo` refuses a change while a newer one in the same scope is still in
        // effect, so undoing a three-placement turn in authoring order refused the first two and
        // took back only the last: Cmd+Z said "took away 1 row(s)" and left two on the map. **The
        // group's own phrase said "3 changes" the whole time**, which is why this needed a count
        // printed rather than a reading of the code.
        //
        // Reversing here keeps `apply`'s composition rule intact — it still inverts by reversing
        // what it ran — and makes what it runs the order the log can answer.
        taken.reverse();
        rest.push(Undo::Group { ops: taken });
        self.undo = rest;
        self.settle();
        found
    }

    /// **No entry is being refused any more**: an applied change, or a new edit, moved the stacks on.
    fn settle(&mut self) {
        self.refused_undo = 0;
        self.refused_redo = 0;
    }

    /// **A refused entry goes back, and a second refusal in a row drops it (gap 3).** The first keeps
    /// the entry, so the author sees why and can try again (B-026). An entry that refuses twice will
    /// not apply, and left on the stack it would sit above every older change for good, so the second
    /// refusal drops it and says so.
    fn refused_entry(&mut self, op: Undo, redo_side: bool) {
        let count = if redo_side {
            &mut self.refused_redo
        } else {
            &mut self.refused_undo
        };
        *count += 1;
        if *count < 2 {
            let stack = if redo_side {
                &mut self.redo
            } else {
                &mut self.undo
            };
            stack.push(op);
        } else {
            *count = 0;
            self.status.problem(
                "that entry refused twice, so it is dropped and cannot be taken back",
            );
        }
    }

    /// **Record a change another front end made through the registry**, by its operation-log line, so
    /// it sits in the one history Cmd+Z walks (B-019). Clears redo like any edit.
    pub(crate) fn record_logged(&mut self, line: String, phrase: String, agent: bool) {
        self.record(Undo::Logged {
            line,
            phrase,
            agent,
        });
    }

    /// **An agent's change, on the person's receipt (B-020).**
    ///
    /// **One line naming the operation and what it did.** The receipt row is where a person looks
    /// after an edit. A receipt left from their own last edit says nothing happened while the map
    /// says it did, so the agent's change replaces it.
    pub(crate) fn receipt_agent_change(&mut self, operation: &str, summary: &str) {
        self.status.note(format!("agent: {operation}: {summary}"));
    }

    /// **An agent's refusal, in the journal (B-020).** A refusal is an alarm, so it is a problem and
    /// not a note: it reaches the toast and the journal, and it does not overwrite the person's
    /// receipt.
    pub(crate) fn problem_agent_refusal(&mut self, operation: &str, refusal: &str) {
        self.problem_agent_problem(operation, bevy_grimoire_core::problem::Problem::new(refusal));
    }

    /// [`Self::problem_agent_refusal`] for a refusal that has its parts (UI-05): the row keeps the
    /// problem, so the journal reads the same `where` and `fix` the agent's answer carries.
    pub(crate) fn problem_agent_problem(&mut self, operation: &str, problem: bevy_grimoire_core::problem::Problem) {
        self.status.raise(&format!("agent: {operation}: "), problem);
    }

    /// **Take an agent's undo onto the person's stacks (E-076).**
    ///
    /// Two things, in this order, because both are required and each alone is a bug:
    ///
    /// 1. **The taken-back change leaves both stacks** ([`Self::forget_line`], B-019). It is no longer
    ///    in effect, so a Cmd+Z that reached it could only refuse.
    /// 2. **The undo's own line goes on the redo stack**, so the person can put back what the agent
    ///    took away. This is what a person's own Cmd+Z already did — `apply`'s `Undo::Logged` arm
    ///    returns the redo line and `undo()` pushes it — and the agent's door did not, so an agent
    ///    could take a turn away and leave the person holding a REDO row that said nothing.
    ///
    /// `phrase` names the change being put back, as the person's own redo entry does.
    pub(crate) fn record_undone(&mut self, undone: &str, line: String, phrase: String) {
        self.forget_line(undone);
        // **`push`, not `record`.** `record` clears the redo stack, which is right for a new edit and
        // exactly wrong here: it would throw away every redo the person already had waiting — the
        // same reason `redo()` moves an entry between the stacks instead of recording it.
        self.redo.push(Undo::Logged {
            line,
            phrase,
            agent: true,
        });
        self.settle();
    }

    /// **Take an agent's undo of a whole turn onto the person's stacks (E-076, `turn: true`).**
    ///
    /// The same two rules as [`Self::record_undone`], with the turn's boundary kept: the undo walked
    /// `operation_ids` newest-first (the order the log allows), so putting the turn back must walk them
    /// **oldest-first** — the order they were authored — because `undo_target` refuses a change while a
    /// newer one in the same scope is still in effect. One [`Undo::Group`] rather than a redo stack of
    /// `n`, so the person's single redo puts back exactly the amount the agent's single undo took away;
    /// `group_turn` makes the other half of that promise for undo.
    pub(crate) fn record_undone_turn(&mut self, undone: &[&str], lines: Vec<(String, String)>) {
        for line in undone {
            self.forget_line(line);
        }
        if !lines.is_empty() {
            let ops: Vec<Undo> = lines
                .into_iter()
                .map(|(line, phrase)| Undo::Logged {
                    line,
                    phrase,
                    agent: true,
                })
                .collect();
            self.redo.push(Undo::Group { ops });
        }
        self.settle();
    }

    /// **Forget the entries for a change an agent's undo took back.** Once the log no longer has that
    /// line in effect, a Cmd+Z that reached it would only refuse; dropping it means Cmd+Z takes back the
    /// newest change still in effect.
    pub(crate) fn forget_line(&mut self, line: &str) {
        /// **Groups too.** A box's removals are children of one group, so the entry for one of them sits
        /// inside it. A group left with no children goes with its last child.
        fn forget_in(history: &mut Vec<Undo>, line: &str) {
            history.retain_mut(|op| match op {
                Undo::Logged { line: l, .. }
                | Undo::Pin { line: l, .. }
                | Undo::Settings { line: l, .. } => l.as_str() != line,
                Undo::Group { ops } => {
                    forget_in(ops, line);
                    !ops.is_empty()
                }
                _ => true,
            });
        }
        forget_in(&mut self.undo, line);
        forget_in(&mut self.redo, line);
    }

    /// The entries above a registry line's entry, leaves only, in stack order. **Groups are walked
    /// too.** A box's removals sit inside one group, so the line can be a leaf of it, and a group's
    /// children are taken in the order they run. If the line has no entry on the stack, every entry
    /// counts: the conservative answer when the stack cannot say where the line sits.
    fn entries_above(&self, line: &str) -> Vec<&Undo> {
        fn leaves<'a>(ops: &'a [Undo], out: &mut Vec<&'a Undo>) {
            for op in ops {
                match op {
                    Undo::Group { ops } => leaves(ops, out),
                    leaf => out.push(leaf),
                }
            }
        }
        let mut all: Vec<&Undo> = Vec::new();
        leaves(&self.undo, &mut all);
        let start = all
            .iter()
            .position(|op| matches!(op, Undo::Logged { line: l, .. } if l == line))
            .map_or(0, |at| at + 1);
        all.split_off(start)
    }

    /// **Is a change recorded by row number sitting above a registry line's entry?** An undo of a
    /// `placement_remove` puts a row back or takes one out, and every row-recorded change above it
    /// names its piece by a number the undo has just moved. `Added` names its pieces from the tail,
    /// which a row landing before them leaves alone, so it does not count. Stamps index `map.stamps`,
    /// not placements, so they are [`EditorState::stamp_recorded_above`]'s business.
    pub(crate) fn row_recorded_above(&self, line: &str) -> bool {
        fn by_row(op: &Undo) -> bool {
            match op {
                Undo::RemovedMany { .. }
                | Undo::Moved { .. }
                | Undo::RemoveAt { .. } => true,
                Undo::Group { ops } => ops.iter().any(by_row),
                Undo::Added { .. }
                | Undo::Stamped { .. }
                | Undo::UnstampedMany { .. }
                | Undo::UnstampAt { .. }
                | Undo::MovedStamp { .. }
                | Undo::Pin { .. }
                | Undo::Settings { .. }
                | Undo::Logged { .. } => false,
            }
        }
        self.entries_above(line).into_iter().any(by_row)
    }

    /// **Is a stamp change recorded by place sitting above a registry line's entry?** `UnstampAt` and
    /// `UnstampedMany` name stamps by their index in `map.stamps`, so an undo of a `stamp_remove` that
    /// puts a stamp back in the middle moves them. `Stamped` drains from the tail, which such an undo
    /// leaves alone, and `MovedStamp` names its stamp by id, so neither counts. The same fallback as
    /// [`EditorState::row_recorded_above`] when the line has no entry.
    pub(crate) fn stamp_recorded_above(&self, line: &str) -> bool {
        self.entries_above(line)
            .into_iter()
            .any(|op| matches!(op, Undo::UnstampAt { .. } | Undo::UnstampedMany { .. }))
    }

    /// Where the id mint stands — read-only, so the harness can check [`next_id_after`]'s seed
    /// really landed.
    pub fn minted(&self) -> u32 {
        self.next_id
    }
}

/// **What a click on the map does.** Exactly one of these at a time.
///
/// This was `removing: bool`. A second bool beside it for the move tool would have made "both armed"
/// and "neither armed" expressible states that mean nothing, and every reader would then have had to
/// decide which one won — two paths where the author only ever has one tool in hand.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    /// Click to put the armed piece down. The default, and what every other tool returns to.
    #[default]
    Place,
    /// Click a piece to delete it; drag a box to delete everything inside it.
    Remove,
    /// Click a piece to pick it up, click again to put it down. See [`MoveDrag`].
    Move,
    /// Drag a box to take a copy of everything inside it; click to stamp the set — as many times
    /// as it is held. See [`CloneDrag`].
    Clone,
    /// **Drag a line and the armed piece repeats along it**, at its own pitch.
    ///
    /// The fourth tool, and the one the other three could not do. A flood lays a *grid* — one
    /// piece, one yaw, one pitch, over a region — so it cannot draw a fence that turns and cannot
    /// draw one with a gap in it. And the grid's pitch is the *project's* lattice, which is not
    /// the piece's: Sanctuary's fence posts stand 2.032 m apart, and the lattice's rungs are
    /// 1.000, 0.333 and 0.111 m, so a fill lays them 3.000, 2.333 or 2.111 m apart — never
    /// 2.032, and 8 cm out at best, which is a visible gap in every joint.
    ///
    /// A run's pitch is the **piece's own**, and the corners are the same piece turned.
    ///
    /// A mode, and Raskin's objection is a mode you can *forget* — so it is drawn: the inspector
    /// names it, and the posts it would lay stand on the map as a ghost while the drag is in
    /// progress. A click leaves it, like every other tool.
    Run,
    /// **Drag a rectangle on the ground and type a note for an agent about it** (#89); click a
    /// standing note's area to read or edit it. See [`NoteBox`].
    Note,
}

impl Tool {
    /// How it reads in a status line.
    fn label(self) -> &'static str {
        match self {
            Tool::Place => "placing",
            Tool::Remove => "removal mode",
            Tool::Move => "move mode",
            Tool::Clone => "clone mode",
            Tool::Run => "run mode",
            Tool::Note => "note mode",
        }
    }
}

/// The rectangle being dragged out, in map space. Deliberately its own resource — see
/// [`EditorState::tool`] for why it is not a field on the state everything else watches.
#[derive(Resource, Default)]
pub struct RemovalDrag {
    /// Where the button went down, or `None` while only hovering.
    from: Option<(f32, f32)>,
}

/// **Which placement is in hand**, under [`Tool::Move`] — by `Placed::id`, never by index.
///
/// The id is the reference for the reason `Placed::id`'s own doc gives for being a string rather than
/// an index, and a carry is exactly the window where it matters: `Cmd+Z`, `F` and `G` all fire while a
/// piece is held and all of them can insert or remove rows. A stored index would then be pointing at
/// whatever slid into that slot, and the next click would move the wrong piece — silently, because an
/// index is always valid-looking.
///
/// Its own resource for the reason [`RemovalDrag`] gives: this is read every frame by the ghost, and
/// `rebuild_palette` watches `EditorState`.
///
/// # Nothing moves until the piece is put down
///
/// Picking up records *only* the index. The placement keeps its position, its `on` and its entity for
/// the whole carry, and the cursor is previewed by the same translucent ghost that previews the brush.
/// So cancelling is `held = None` — there is no half-applied move to reverse, no entity to put back,
/// and no window in which the file on disk disagrees with the screen. The alternative, moving the row
/// as the cursor moves, would have made `Esc` a second implementation of the move that had to undo
/// exactly what the first one did.
#[derive(Resource, Default)]
pub struct MoveDrag {
    pub held: Option<Held>,
}

/// **What the move tool has picked up.**
///
/// Ids, never indices, for [`MoveDrag`]'s own reason: an undo or a fill between the grab and the
/// drop can move a row, and an index would then name a different piece. The two arms are the two
/// things a click can land on — see [`Subject`], which answers *which* and is resolved to this.
///
/// FVS-R-14: a stamp moves as **one thing**. Grabbing a member and dragging it would be reaching
/// *through* the instance, which is what `composition.rs`'s encapsulation rule forbids and what the
/// missing `Placement` on stamped rows used to prevent by making it unreachable.
#[derive(Clone, Debug, PartialEq)]
pub enum Held {
    /// A loose placement, by [`bevy_grimoire_core::map::Placed::id`]. Its riders come with it.
    Piece(String),
    /// A stamp, by [`bevy_grimoire_core::composition::Stamped::id`]. Moving it writes `Stamped::at`; the
    /// rows follow because `redraw_stamps` rebuilds them from the list.
    Stamp(String),
}

/// The translucent red marker: the hovered piece's footprint, or the dragged rectangle.
#[derive(Component)]
struct RemovalTile;

/// The clone tool's marker — the box being dragged out, then the held set's bounds riding the
/// cursor. Its own component so the two tools' rectangles can never claim each other's colour.
#[derive(Component)]
struct CloneTile;

/// One piece of a held clone set, stored **relative to the set's anchor** so the set's internal
/// geometry — every flush edge, every offset a lamp keeps from its table's centre — survives the
/// trip exactly. Everything authored rides along: yaw, tip, lift, note, even the pin, because a
/// copied barricade is still a barricade.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClonePiece {
    pub descriptor: String,
    pub offset: (f32, f32),
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub tip: (u8, u8),
    #[serde(default)]
    pub lift: f32,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub owned: bool,
    #[serde(default)]
    pub owned_because: Option<String>,
    /// The placement's own `patch`, carried so a copy of a road is as long as the road it was taken
    /// from, and measured at that length while it is held.
    #[serde(default)]
    pub patch: Option<bevy_grimoire_core::descriptor::Descriptor>,
    #[serde(default)]
    pub on: CloneHost,
}

/// What a cloned piece rests on — resolved at capture, applied at stamp.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloneHost {
    /// Its own layer: floor, wall, ceiling — the map answers, nothing to carry.
    #[default]
    Layer,
    /// Its host was caught in the same box, by index into the set — repointed to the host's fresh
    /// id at stamp, so the cloned lamp stands on the cloned table.
    InSet(usize),
    /// Its host stayed behind, so each stamp re-seats it on whatever offers the surface at the new
    /// spot — `host_under`'s answer, and a stamp with no answer refuses whole.
    Outside,
}

/// The set in hand, plus the bounds the marker shows: centre offset from the anchor and
/// half-extents, both from the pieces' own footprints at capture.
/// **A stamp caught in the capture box**, carried whole.
///
/// FVS-R-14: *tools act on the instance, never through it.* A box that catches a nurse station
/// takes the station, not its rows — copying expanded geometry would flatten the reference the
/// schema exists to protect, which `stamping_writes_a_reference_and_undo_takes_it_back` pins.
///
/// Its own list beside [`CloneSet::pieces`] rather than an enum sharing one, and that is forced
/// rather than chosen: [`CloneHost::InSet`] is an **index into `pieces`**, so interleaving a second
/// kind would silently repoint every host a captured set carries.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloneStamp {
    /// The [`bevy_grimoire_core::composition::Composition`] id this instances.
    pub of: String,
    /// From the set's anchor, in map metres — the same frame [`ClonePiece::offset`] uses.
    pub offset: (f32, f32),
    #[serde(default)]
    pub yaw: f32,
    /// Carried verbatim: an override is a statement about *this* instance, and a copy that dropped
    /// them would be a different nurse station wearing the same name.
    #[serde(default)]
    pub overrides: Vec<bevy_grimoire_core::composition::Override>,
    #[serde(default)]
    pub owned: bool,
    #[serde(default)]
    pub owned_because: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

/// **Serialisable, so the registry can be handed the set itself** (`composition_define`'s `set`): the set
/// in hand is not always the placements on the map, and an agent may build one with no map at all.
/// Offsets are from the set's centre; every field but the pieces' `descriptor` and `offset` defaults.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloneSet {
    pub pieces: Vec<ClonePiece>,
    /// Stamps the box caught. See [`CloneStamp`].
    #[serde(default)]
    pub stamps: Vec<CloneStamp>,
    #[serde(default)]
    pub centre_off: (f32, f32),
    #[serde(default)]
    pub half: (f32, f32),
    /// **How far the whole set has been turned**, degrees about +Y, applied about the anchor the
    /// pieces are offset from.
    ///
    /// Its own field rather than folded into each piece's yaw at the moment of turning: an offset
    /// rotated repeatedly accumulates float error, and a set turned four times by 90° has to come
    /// back to exactly where it started or "turn it round and put it back" quietly moves the group.
    /// Kept as one angle and applied once, at stamp and at preview.
    #[serde(default)]
    pub yaw: f32,
}

impl CloneSet {
    /// **Where a piece lands and which way it faces**, with the set's own turn folded in.
    ///
    /// Through `bevy_grimoire_core::composition::rotate_xz` — the function a *stamp* rotates by — rather
    /// than a local copy of the sign convention. Bevy's yaw turns +X toward −Z, and a second copy of
    /// that is how a set turned 90° would come out mirrored against a composition turned 90°.
    pub fn placed(&self, piece: &ClonePiece, target: (f32, f32)) -> ((f32, f32), f32) {
        let off = bevy_grimoire_core::composition::rotate_xz(piece.offset, self.yaw);
        (
            (target.0 + off.0, target.1 + off.1),
            bevy_grimoire_core::composition::add_yaw(piece.yaw, self.yaw),
        )
    }

    /// [`Self::placed`] for a carried stamp — the same turn, applied to a whole instance.
    pub fn placed_stamp(&self, st: &CloneStamp, target: (f32, f32)) -> ((f32, f32), f32) {
        let off = bevy_grimoire_core::composition::rotate_xz(st.offset, self.yaw);
        (
            (target.0 + off.0, target.1 + off.1),
            bevy_grimoire_core::composition::add_yaw(st.yaw, self.yaw),
        )
    }

    /// The turned set's axis-aligned footprint, for the marker that outlines it. A rotated rectangle
    /// is not a rectangle, so the box the author sees is the rotated one's **bounds** — which is what
    /// a box drawn on the ground can honestly show.
    pub fn bounds(&self, target: (f32, f32)) -> (f32, f32, f32, f32) {
        let c = bevy_grimoire_core::composition::rotate_xz(self.centre_off, self.yaw);
        let (cx, cz) = (target.0 + c.0, target.1 + c.1);
        let corners = [
            (-self.half.0, -self.half.1),
            (self.half.0, -self.half.1),
            (self.half.0, self.half.1),
            (-self.half.0, self.half.1),
        ];
        let (mut hx, mut hz) = (0.0f32, 0.0f32);
        for corner in corners {
            let r = bevy_grimoire_core::composition::rotate_xz(corner, self.yaw);
            hx = hx.max(r.0.abs());
            hz = hz.max(r.1.abs());
        }
        (cx - hx, cz - hz, cx + hx, cz + hz)
    }
}

/// The clone tool's state: the box being dragged, or the set in hand. Its own resource for the
/// reason [`RemovalDrag`] gives — written at drag rate, and `rebuild_palette` watches
/// `EditorState`.
#[derive(Resource, Default)]
pub struct CloneDrag {
    from: Option<(f32, f32)>,
    held: Option<CloneSet>,
}

/// **Arm a tool, or put it down** (UI-37, #231): the one body every front end of a tool runs.
///
/// A mode key and a toolbar button both mean "make this the tool", and before this they would have
/// been two bodies that could drift — the keys each had their own copy of "toggle against *my* variant,
/// note it, drop what the others held". Returns the sentence to show, so no caller invents its own
/// wording: `Esc` and a second press say the same thing.
///
/// **The toggle is against its own variant**, not against "any tool": pressing `X` while the move tool
/// is armed reaches removal rather than returning to placing, which is what an author means by pressing
/// a second tool's key.
pub fn arm_tool(state: &mut EditorState, tool: Tool, chord: &str) -> String {
    arm_tool_in(&mut state.tool, tool, chord)
}

/// **Arm a tool through the operation**, so the key and an agent change one thing (E-002, #129).
///
/// The view travels in `Ctx::view` and the operation's answer is written back; a refusal is said where a
/// refused key already is. Nothing here restates the arming rule — `tool_arm` calls the same
/// [`arm_tool_in`] this file does.
fn arm_through_operation(state: &mut EditorState, tool: Tool, chord: &str) -> String {
    let mut ctx = crate::ops::Ctx {
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        view: Some(crate::ops_view::View { tool: state.tool, ..Default::default() }),
        ..crate::ops::Ctx::default()
    };
    let params = serde_json::json!({ "tool": crate::ops_view::tool_name(tool) });
    let outcome = crate::ops::dispatch(&mut ctx, "tool_arm", &params);
    if let Some(view) = ctx.view {
        state.tool = view.tool;
    }
    match outcome {
        Ok(crate::ops::Applied::Done { outcome, .. }) => {
            if outcome.summary.is_empty() {
                crate::editor::arm_tool_in(&mut state.tool, tool, chord)
            } else {
                outcome.summary
            }
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
            state.status.problem(e.0);
            String::new()
        }
    }
}

/// **The arming rule itself**, on the value rather than on `EditorState`, so a view operation can arm a
/// tool without owning the editor's resources (E-002, #129). One copy of "press it again to put it back".
pub fn arm_tool_in(slot: &mut Tool, tool: Tool, chord: &str) -> String {
    let was = *slot;
    *slot = if was == tool { Tool::Place } else { tool };
    let on = *slot == tool;
    match (tool, on) {
        (Tool::Remove, true) => format!("removal mode: click a piece, or drag a box. {chord} or Esc to stop."),
        (Tool::Remove, false) => "removal mode off".to_owned(),
        (Tool::Move, true) => format!(
            "move mode: click a piece to pick it up, click again to put it down. {chord} or Esc to stop."
        ),
        (Tool::Move, false) => "move mode off".to_owned(),
        (Tool::Clone, true) => {
            "clone mode: drag a box to take a copy of what is inside, click to stamp it. Esc to stop.".to_owned()
        }
        (Tool::Clone, false) => "clone mode off".to_owned(),
        (Tool::Place, _) => "back to placing".to_owned(),
        (other, true) => other.label().to_owned(),
        (other, false) => format!("{} off", other.label()),
    }
}

/// **Which piece of a stack the piece-verbs mean** — `H`'s answer, held as `(placement id, the
/// snapped cell it was taken on)`.
///
/// A floor tile, a wall and its header legally share a cell (different layers pass the overlap
/// rule), and "the placement under the cursor" cannot name one of three — a nudge aimed at the
/// header moved the wall. The id, never an index, for `MoveDrag`'s reason; the cell, so the lock
/// lapses the moment the cursor walks away rather than following it around the map.
#[derive(Resource, Default)]
pub struct TargetLock(Option<(String, (f32, f32))>, bool);

impl TargetLock {
    /// The id of the placement the lock holds, if it holds one. The agent's `"selection"` frame
    /// target reads the lock through here, so "what is selected" has one answer.
    pub fn placement_id(&self) -> Option<&str> {
        self.0.as_ref().map(|(id, _)| id.as_str())
    }

    /// **Hold the lock on `id`**, as [`lock_on_placement`] does and for the same reason: the cursor is
    /// on the piece that was just Shift-clicked, but a held lock does not lapse when it walks away,
    /// and the selection the inspector reads is anchored on it. `at` is the piece's own position.
    pub(crate) fn hold(&mut self, id: &str, at: (f32, f32)) {
        self.0 = Some((id.to_owned(), at));
        self.1 = true;
    }
}

/// **Lock onto the placement `id` on the open map**, as a click on it would: the gold marker stands on
/// it and the piece-verbs mean it. `false` when the map has no such placement. The cell is the tile
/// cell its position snaps to, which is what a click's lock records. **A lock taken this way is held**
/// (the `bool` of [`TargetLock`]): a click's lock lapses when the cursor leaves its cell, and the
/// cursor is on the toast's Show button, not on the piece, so it would lapse on the next frame.
pub fn lock_on_placement(world: &mut World, id: &str) -> bool {
    let Some(at) = world
        .get_resource::<OpenMap>()
        .and_then(|open| open.map.placements.iter().find(|p| p.id == id).map(|p| p.at))
    else {
        return false;
    };
    let Some(project) = world.get_resource::<Project>() else {
        return false;
    };
    let cell = (snap_point(project, at.0, SnapLevel::Tile), snap_point(project, at.1, SnapLevel::Tile));
    let mut lock = world.resource_mut::<TargetLock>();
    lock.0 = Some((id.to_owned(), cell));
    lock.1 = true;
    true
}

/// The locked target's highlight — a third marker quad beside removal's red and clone's blue,
/// because three tools making three different promises must not share a colour.
///
/// File-private again: it was widened for a `badges::sense_subject` that read it while badges had a
/// third anchor on the piece in the world, and that anchor was removed — the piece-verbs' badges
/// live on the `UNDER` readout row now.
#[derive(Component)]
struct TargetTile;

impl CloneDrag {
    /// Whether a set is in hand — the one question `keys` asks (for `Esc`'s layering).
    fn holding(&self) -> bool {
        self.held.is_some()
    }

    /// **How many pieces are in hand**, for the status band (UI-34, #228): `Some(n)` while a set is
    /// held, `None` when nothing is. A read, not a second copy of the state — `held` stays private so
    /// the band cannot start deciding what a set is.
    pub fn held_pieces(&self) -> Option<usize> {
        self.held.as_ref().map(|set| set.pieces.len() + set.stamps.len())
    }
}

/// One reversible edit.
///
/// Only placements, and deliberately: the map's *size* and *name* are settings rather than edits, and
/// folding them into the same stack would mean Ctrl+Z sometimes resized the map when an author meant
/// to take back a crate. One kind of thing in the stacks.
///
/// # Closed under inversion, which is what makes redo one mechanism instead of two
///
/// Every variant's inverse is another variant of this same enum: undoing an `Added` produces a
/// `RemovedMany` holding what it took out, undoing that produces an `Added` again, and `Moved`,
/// `Turned` and `Pinned` each invert to themselves carrying the other value. So [`apply`] does the
/// work and *returns the inverse*, and undo and redo are the same function reading opposite stacks —
/// rather than a second body that has to be kept in step with the first.
#[derive(Clone)]
enum Undo {
    /// Remove the last `count` placements — the inverse of a place or a fill.
    Added { count: usize },
    /// Put back everything one drag took out. **Ascending by index**, which is what lets them go
    /// back in that order and each land where it came from — an earlier row returning first shifts
    /// the later ones into place. One entry for the whole rectangle, on the same argument the fill
    /// makes: a box the author drew once is one act to undo once.
    RemovedMany { items: Vec<(usize, Box<Placed>)> },
    /// Put a moved group back where it came from. **One entry for the whole group** — the piece and
    /// everything that was riding on it — on the same argument `RemovedMany` makes: one act the
    /// author performed is one act to take back. `bevy_grimoire_core::stack::Moved` records what changed and
    /// `restore_moved` is its inverse, so the undo cannot drift from the move.
    Moved { moved: bevy_grimoire_core::stack::Moved },
    /// Remove exactly these rows again — the inverse of [`Undo::RemovedMany`].
    ///
    /// **Not `Added { count }`.** That one removes the *last* `count`, which is right for a place or a
    /// fill because both append; but `RemovedMany` puts rows back at their original indices, which are
    /// not generally the tail. Redoing a delete through `Added` would have removed whatever happened to
    /// be at the end of the list instead.
    RemoveAt { indices: Vec<usize> },
    /// **Several reversals that are one act** — applied in order, inverted by reversing.
    ///
    /// `generate` needs it: undoing a `G` press must strip the solver rows AND put the removed sketch
    /// back, and two separate entries would make one keypress two undos. The group's inverse is the
    /// sub-inverses in reverse order — the standard composition rule, and what keeps the enum closed
    /// under inversion with no new mechanism.
    Group { ops: Vec<Undo> },
    /// **Remove the last `count` stamps** — the inverse of stamping a group.
    ///
    /// Its own list, not `Added`: stamps live in `map.stamps`, and a map that expands them is a
    /// different list from the one an author placed into by hand. Folding the two would make one
    /// Ctrl+Z sometimes take back a crate and sometimes a whole nurse station.
    Stamped { count: usize },
    /// Put stamps back at their original indices — the inverse of [`Undo::Stamped`], on exactly the
    /// argument [`Undo::RemovedMany`] makes about rows.
    UnstampedMany {
        items: Vec<(usize, Box<bevy_grimoire_core::composition::Stamped>)>,
    },
    /// **Take these exact stamps off the list** — the inverse of [`Undo::UnstampedMany`], and the
    /// stamp-side twin of [`Undo::RemoveAt`].
    ///
    /// `UnstampedMany` used to invert to [`Undo::Stamped`], which drains the **tail**. That is
    /// correct for the only thing that recorded it — `stamp_here` appends, so the stamps it put down
    /// *are* the tail — and wrong the moment a stamp is removed from the middle, which is what
    /// FVS-R-14's delete does: undo would put it back at index 3 and redo would then take the last
    /// one off instead. Indices, like every other paired removal in this file.
    UnstampAt { indices: Vec<usize> },
    /// **Put a stamp back where it was.** Its own inverse — a move is a move either way, so unlike
    /// the placement pair there is no second variant.
    ///
    /// By id rather than index, because [`Undo::UnstampAt`] can reorder the list between the move
    /// and the undo, and an index would then move somebody else.
    MovedStamp { id: String, from: (f32, f32) },
    /// **Take back, or put back, a pin**, by its operation-log line. Like [`Undo::Settings`], a pin
    /// goes through the registry, so its undo does too and Cmd+Z and the agent's undo share one
    /// history.
    ///
    /// `owned` is the pin this entry puts back, so the phrase reads `pin` or `unpin` the way the
    /// author will say it. `id` is the placement's id, not its row, for the reason on
    /// [`Undo::MovedStamp`]: the list can change between the pin and the undo.
    Pin {
        line: String,
        id: String,
        owned: bool,
    },
    /// **Take back, or put back, a settings change the size field made**, by its operation-log line.
    ///
    /// **Not a delta of its own.** A settings change goes through the registry and the log, so its
    /// undo does too: [`crate::ops::undo`] is the same function `grimoire_undo_operation` reaches, which
    /// keeps the person's Cmd+Z and the agent's undo **one history**. Without this entry the stack
    /// never heard of a resize, and Cmd+Z after one took back an older placement instead. The inverse
    /// is another `Settings` naming the line the undo wrote, so redo is the same arm.
    Settings {
        line: String,
        /// **The size this entry puts back**, captured when the change was recorded, so describing
        /// it never reads a file.
        bounds: (f32, f32, f32),
    },
    /// **Take back, or put back, a change another front end made through the registry**, by its
    /// operation-log line: an agent's turn, pin or size, or a turn the person made by key.
    ///
    /// **Why this entry exists (B-019).** An agent's change went to the log and never reached this
    /// stack, so Cmd+Z after an agent's turn took back the person's older change and left the agent's
    /// standing. Recording it here puts the agent's change in the same history. Its undo is
    /// [`crate::ops::undo`], the function `grimoire_undo_operation` reaches, so the two cannot disagree.
    /// `phrase` is the outcome's summary, captured when the change was made, so the INSPECTOR names it
    /// without reading the log.
    /// `agent` is `true` when an agent made the change, over BRP or the capture sequencer, so the
    /// history list can say who.
    Logged {
        line: String,
        phrase: String,
        agent: bool,
    },
}

/// **One row of the history list (E-018).** `position` is how many entries are applied when this row
/// is the one the editor stands on, so a row is a jump target. `agent` says who made the change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRow {
    pub position: usize,
    pub agent: bool,
    pub label: String,
}

impl Undo {
    /// **The short name of an entry for the history list (E-018).** Counts, not piece names: an entry
    /// further down names a map that no longer stands, so naming pieces here would name the wrong ones.
    /// It names the change the entry stands for, on either side of an undo; `phrase` names what Cmd+Z will do.
    fn kind(&self, redo: bool) -> String {
        let again = if redo { " again" } else { "" };
        // Undo side: the entry undoes the change, so the verb names the change it reverses. Redo side:
        // applying the entry is the change, so the verb names what applying it does. One change has one
        // label, whichever side of the undo it sits on.
        match self {
            Undo::Added { count } if redo => format!("remove {count}{again}"),
            Undo::Added { count } => format!("place {count}"),
            Undo::RemovedMany { items } if redo => format!("place {}{again}", items.len()),
            Undo::RemovedMany { items } => format!("remove {}", items.len()),
            Undo::RemoveAt { indices } if redo => format!("remove {}{again}", indices.len()),
            Undo::RemoveAt { indices } => format!("place {}", indices.len()),
            Undo::Moved { moved } => format!("move {}{again}", moved.was.len()),
            Undo::Group { ops } => format!("{} changes{again}", ops.len()),
            Undo::Stamped { count } if redo => format!("remove {count} stamps{again}"),
            Undo::Stamped { count } => format!("stamp {count}"),
            Undo::UnstampedMany { items } if redo => format!("stamp {}{again}", items.len()),
            Undo::UnstampedMany { items } => format!("remove {} stamps", items.len()),
            Undo::UnstampAt { indices } if redo => {
                format!("remove {} stamps{again}", indices.len())
            }
            Undo::UnstampAt { indices } => format!("stamp {}", indices.len()),
            Undo::MovedStamp { .. } => format!("move a stamp{again}"),
            Undo::Pin { .. } => format!("pin{again}"),
            Undo::Settings { .. } => format!("settings{again}"),
            Undo::Logged { phrase, .. } => phrase.clone(),
        }
    }

    /// **Whether an agent made this change (E-018).** A group is an agent's if any part of it is.
    fn by_agent(&self) -> bool {
        match self {
            Undo::Logged { agent, .. } => *agent,
            Undo::Group { ops } => ops.iter().any(Undo::by_agent),
            _ => false,
        }
    }

    /// **What applying this entry does, named in the author's words.** The INSPECTOR's UNDO and REDO
    /// rows and `grimoire/state` both show this phrase, so the two cannot disagree. It reads the map as
    /// it is now and no file, and names only pieces the map holds: an entry that can no longer act
    /// says `cannot: …` rather than naming a piece it will not touch.
    ///
    /// `redo` says which stack the entry sits on. A redo-stack entry is the inverse an undo produced,
    /// so the same change reads "turn X back" on the undo stack and "turn X again" on the redo stack.
    fn describe(&self, map: &bevy_grimoire_core::map::Map, redo: bool) -> String {
        clamp_phrase(self.phrase(map, redo))
    }

    /// The phrase before it is clamped. A group joins its parts from here, so [`join_parts`] can
    /// count what did not fit and keep every part's words whole.
    fn phrase(&self, map: &bevy_grimoire_core::map::Map, redo: bool) -> String {
        let name_at = |i: usize| {
            map.placements
                .get(i)
                .map_or("piece", |p| p.descriptor.as_str())
        };
        let gone = |i: usize| map.placements.get(i).is_none();
        // `back` on the undo stack, `again` on the redo stack. A pin takes neither word on the undo
        // stack: `pin crate` and `unpin crate` are what applying does, and `unpin crate back` is not
        // English.
        let back = if redo { " again" } else { " back" };
        let pin_again = if redo { " again" } else { "" };
        match self {
            Undo::Added { count } => {
                let names: Vec<&str> = map
                    .placements
                    .iter()
                    .rev()
                    .take(*count)
                    .map(|p| p.descriptor.as_str())
                    .collect();
                format!("remove {}", piece_count(&names))
            }
            Undo::RemovedMany { items } => {
                let names: Vec<&str> = items.iter().map(|(_, p)| p.descriptor.as_str()).collect();
                format!("restore {}", piece_count(&names))
            }
            Undo::Moved { moved } => {
                let names: Vec<&str> = moved.was.iter().map(|(i, _, _)| name_at(*i)).collect();
                format!("move {}{back}", piece_count(&names))
            }
            Undo::RemoveAt { indices } => {
                // Apply skips an index past the end, so the phrase names only what it will take out.
                let present: Vec<&str> = indices
                    .iter()
                    .filter(|i| !gone(**i))
                    .map(|i| name_at(*i))
                    .collect();
                match (present.is_empty(), indices.first()) {
                    (true, Some(first)) => format!("cannot: piece {first} is gone"),
                    (true, None) => "nothing".to_owned(),
                    (false, _) => format!("remove {}", piece_count(&present)),
                }
            }
            Undo::Pin { id, .. } if !map.placements.iter().any(|p| p.id == *id) => {
                format!("cannot: {id} is gone")
            }
            Undo::Pin { id, owned, .. } => {
                let verb = if *owned { "pin" } else { "unpin" };
                format!("{verb} {id}{pin_again}")
            }
            Undo::Group { ops } => {
                let parts: Vec<String> = ops.iter().map(|op| op.phrase(map, redo)).collect();
                if parts.is_empty() {
                    "nothing".to_owned()
                } else {
                    join_parts(&parts)
                }
            }
            Undo::Stamped { count } => format!("remove {}", stamps(*count)),
            Undo::UnstampedMany { items } => format!("restore {}", stamps(items.len())),
            Undo::UnstampAt { indices } => {
                // Apply skips an index past the end and refuses when none are left, so count
                // what it will take.
                let present = indices
                    .iter()
                    .filter(|i| stamp_is_present(map, **i))
                    .count();
                match (present, indices.first()) {
                    (0, Some(first)) => format!("cannot: stamp {first} is gone"),
                    (0, None) => "nothing".to_owned(),
                    (n, _) => format!("remove {}", stamps(n)),
                }
            }
            Undo::MovedStamp { id, .. } if !map.stamps.iter().any(|s| s.id == *id) => {
                format!("cannot: stamp {id} is gone")
            }
            Undo::MovedStamp { id, .. } => format!("move {id}{back}"),
            Undo::Settings {
                bounds: (x, y, z), ..
            } => {
                let to = if redo {
                    "map size to"
                } else {
                    "map size back to"
                };
                format!("{to} {x} x {y} x {z}")
            }
            // **The summary the change was made under**, so an agent's turn reads in the INSPECTOR the
            // way the person's own does.
            Undo::Logged { phrase, .. } => phrase.clone(),
        }
    }
}

/// **The noun of a phrase**: `8 fence` for a run of one name, `3 pieces` for a mix.
fn piece_count(names: &[&str]) -> String {
    match names {
        [] => "pieces".to_owned(),
        [first, rest @ ..] if rest.iter().all(|n| n == first) => {
            format!("{} {first}", names.len())
        }
        _ => format!("{} pieces", names.len()),
    }
}

fn stamps(n: usize) -> String {
    if n == 1 {
        "1 stamp".to_owned()
    } else {
        format!("{n} stamps")
    }
}

/// Whether a stamp index still names a stamp on the map. An undo entry can outlive the stamp it
/// pointed at, so the phrase checks before it promises a removal. This is an index check against
/// the map's own stamp list, not a count the census owns; the phrase only needs yes or no.
fn stamp_is_present(map: &bevy_grimoire_core::map::Map, index: usize) -> bool {
    index < map.stamps.len()
}

/// **Joins a group's parts in the order they apply, within the phrase budget.** A part is never cut
/// mid-phrase: the parts that fit are named, and the rest are counted as `, +N more`, so no part
/// disappears without a trace. A first part too long on its own is clamped by [`clamp_phrase`] like
/// any other phrase.
fn join_parts(parts: &[String]) -> String {
    let mut out = String::new();
    for (i, part) in parts.iter().enumerate() {
        let joined = if out.is_empty() {
            part.clone()
        } else {
            format!("{out}, {part}")
        };
        let left = parts.len() - i - 1;
        let tail = if left == 0 {
            0
        } else {
            format!(", +{left} more").chars().count()
        };
        if out.is_empty() || joined.chars().count() + tail <= PHRASE_MAX {
            out = joined;
        } else {
            return format!("{out}, +{} more", parts.len() - i);
        }
    }
    out
}

/// **The longest phrase a row shows.** The value column holds about 42 characters on one line, and
/// the ` (+N)` a stacked row adds fits in what is left.
const PHRASE_MAX: usize = 36;

fn clamp_phrase(phrase: String) -> String {
    if phrase.chars().count() <= PHRASE_MAX {
        return phrase;
    }
    let kept: String = phrase.chars().take(PHRASE_MAX - 1).collect();
    format!("{kept}…")
}

impl Default for EditorState {
    fn default() -> Self {
        EditorState {
            // Armed on the first piece at startup, as it has always been — an editor that opens with
            // nothing selected makes the first click do nothing and reads as broken.
            brush: Some(0),
            brush_yaw: 0.0,
            brush_tip: (0, 0),
            status: crate::chrome::Status::default(),
            hint: String::new(),
            next_id: 0,
            seed: 1,
            pinning: None,
            grouping: None,
            replacing: None,
            renaming: None,
            rule_seed: None,
            undo: Vec::new(),
            redo: Vec::new(),
            refused_undo: 0,
            refused_redo: 0,
            tool: Tool::default(),
            leaving: false,
            door_request: None,
            trust: crate::trusted::TrustState::default(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    fn label(self) -> &'static str {
        match self {
            Axis::X => "X",
            Axis::Y => "Y",
            Axis::Z => "Z",
        }
    }
    /// **The axis, as the operation names it.**
    ///
    /// **The editor's `Axis` and the operation's `Axis` are two types that mean the same thing**,
    /// and this is the one line that says so. The operation owns the vocabulary — an agent reads
    /// its schema — and the editor's own axis is the UI's half of the same three letters.
    fn op_axis(self) -> crate::ops::map_set_settings::Axis {
        match self {
            Axis::X => crate::ops::map_set_settings::Axis::X,
            Axis::Y => crate::ops::map_set_settings::Axis::Y,
            Axis::Z => crate::ops::map_set_settings::Axis::Z,
        }
    }
    fn get(self, b: (f32, f32, f32)) -> f32 {
        match self {
            Axis::X => b.0,
            Axis::Y => b.1,
            Axis::Z => b.2,
        }
    }
}

/// A map may not be smaller than this on any axis, metres. One, not zero: a zero-width map has no
/// floor to point at, and `fill` would refuse every cell in it for a reason that reads like a bug.
const MIN_BOUND: u32 = 1;
/// And no larger. 512 m at 0.5 m cells is past what one fill can cover and far past what an author
/// can see; a typo of one extra digit should be refused rather than resolved into a map nobody meant.
const MAX_BOUND: u32 = 512;

/// One clickable size field.
#[derive(Component, Clone, Copy)]
struct SizeField(Axis);

/// Which size field is being typed into, and the digits so far.
///
/// Its own resource for the reason [`RemovalDrag`] is: `rebuild_palette` watches `EditorState`, and a
/// keystroke landing there would tear down and rebuild all forty-one palette rows per character.
#[derive(Resource, Default)]
pub struct SizeEdit {
    active: Option<(Axis, String)>,
}

/// **The palette categories the author has folded away.**
///
/// A set of names rather than a per-row flag: the grouping is derived from the library on every
/// rebuild, so a flag stored on a row would be lost the moment the library changed.
///
/// **Its own resource, and that is what makes the palette cheap.** It was a field on
/// `EditorState`, which `rebuild_palette` watches — and it is the *only* thing on that resource a
/// row's content reads, so every status note and every repeat tick of the aim keys tore down all
/// forty-odd rows and respawned them with their thumbnails. Same argument [`RemovalDrag`] and
/// [`MoveDrag`] are separate for, from the other end: a watcher wants a resource that changes twice
/// per use, not one that changes constantly.
#[derive(Resource, Default)]
pub struct Folded(pub std::collections::HashSet<String>);

/// The value text of one axis row, refreshed as the size changes.
#[derive(Component, Clone, Copy)]
struct SizeReadout(Axis);

/// **The family settings field** (#96): one line, `<crate> <field> <value>`, whose Enter dispatches
/// `family_set` through [`set_family_field`].
#[derive(Component, Clone, Copy)]
struct FamilyField;

/// The text of the family field, refreshed as it is typed into.
#[derive(Component, Clone, Copy)]
struct FamilyReadout;

/// The family field's buffer while it is being typed into. Its own resource for [`SizeEdit`]'s reason:
/// a keystroke on `EditorState` would rebuild the palette.
#[derive(Resource, Default)]
pub struct FamilyEdit {
    active: Option<String>,
}

/// **The data field** (#86): one line, `<file> <path> <value>`, whose Enter dispatches `data_set`
/// through [`set_data_leaf`].
#[derive(Component, Clone, Copy)]
struct DataField;

/// The text of the data field, refreshed as it is typed into.
#[derive(Component, Clone, Copy)]
struct DataReadout;

/// **The terrain field** (#99): one line, `<brush> <x> <z> <radius_m> <strength>`, whose Enter
/// dispatches `terrain_stroke` through [`sculpt_terrain`].
#[derive(Component, Clone, Copy)]
struct TerrainField;

/// The text of the terrain field, refreshed as it is typed into.
#[derive(Component, Clone, Copy)]
struct TerrainReadout;

/// **The crowd field** (#107): one line, `<from_x> <from_z> <to_x> <to_z> <walkers>`, whose Enter
/// dispatches `crowd_preview` through [`run_crowd_preview`].
#[derive(Component, Clone, Copy)]
struct CrowdField;

/// The text of the crowd field, refreshed as it is typed into.
#[derive(Component, Clone, Copy)]
struct CrowdReadout;

/// **The Data panel's list** (#86): one row per leaf of every registered file. Rebuilt by
/// [`refresh_data_list`] when the leaves change on disk or by an edit.
#[derive(Component, Clone, Copy)]
struct DataList;

/// A row of [`DataList`]. Enter-equivalent: activating it opens the data field with `<file> <path> `
/// already typed, so the person types only the value.
#[derive(Component, Clone)]
struct DataLeafRow {
    file: String,
    path: String,
}

/// **The Plan panel's list** (E-069): the agent's plan for the open map, one numbered row per step
/// with its status, under a count of how many are done.
#[derive(Component, Clone, Copy)]
struct PlanList;

/// **The Notes panel's list** (#90): a header of two buttons, then one row per note.
#[derive(Component, Clone, Copy)]
struct NotesList;

/// **The Reload panel's list** (#203): whether a game is connected, then a line per file with news.
#[derive(Component, Clone, Copy)]
struct ReloadList;

#[derive(Component, Clone)]
struct NoteRow {
    id: String,
}

/// A lease button on a note row: which note, and which of `note_claim`, `note_renew`, `note_release`.
#[derive(Component, Clone)]
struct NoteLeaseButton {
    id: String,
    op: &'static str,
    /// What `note_ask`, `note_answer` and `note_resolve` carry as their text.
    text: &'static str,
}

#[derive(Component, Clone, Copy)]
struct NotesFilterButton;

#[derive(Component, Clone, Copy)]
struct NotesSendAll;

/// What the Notes list last drew: the filter and `(id, status, text, waiting, review, auto)` per row.
type NotesShown = (Option<&'static str>, Vec<(String, &'static str, String, bool, NoteReview, bool)>);

/// What a done note shows for review (#93): its summary, assumptions, the before and after captures
/// and the operations it ran. Empty for every other status.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct NoteReview {
    lines: Vec<String>,
    ops: Vec<String>,
    /// The before and after capture paths, drawn as pictures when the files decode.
    pictures: Vec<String>,
}

/// An operation a done note ran: clicking it frames the pieces that operation changed (#93).
#[derive(Component, Clone)]
struct NoteOpButton {
    op: String,
}

/// The status the Notes list is filtered to; `None` shows every note.
#[derive(Resource, Default)]
pub struct NotesFilter(Option<&'static str>);

/// The data field's buffer while it is being typed into; its own resource for [`FamilyEdit`]'s reason.
#[derive(Resource, Default)]
pub struct DataEdit {
    active: Option<String>,
}

/// The terrain field's buffer while it is being typed into; its own resource for [`FamilyEdit`]'s reason.
#[derive(Resource, Default)]
pub struct TerrainEdit {
    active: Option<String>,
}

/// The crowd field's buffer while it is being typed into.
#[derive(Resource, Default)]
pub struct CrowdEdit {
    active: Option<String>,
    /// The last run's heat overlay (#107): cell size in metres and `[x, z, trail, congestion]`
    /// cells in map space. A measurement kept for drawing; never saved. Empty draws nothing.
    heat: CrowdHeat,
}

/// What [`draw_crowd_heat`] draws.
#[derive(Default, Clone, Debug, PartialEq)]
pub struct CrowdHeat {
    pub cell: f32,
    pub cells: Vec<[f32; 4]>,
}

impl CrowdHeat {
    /// Read the `cell` and `heat` fields of a `crowd_preview` outcome's `after`.
    pub fn from_after(after: &serde_json::Value) -> CrowdHeat {
        let cell = after["cell"].as_f64().unwrap_or(0.0) as f32;
        let cells = after["heat"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .filter_map(|r| {
                        let v: Vec<f32> = r.as_array()?.iter().filter_map(|n| n.as_f64().map(|n| n as f32)).collect();
                        <[f32; 4]>::try_from(v).ok()
                    })
                    .collect()
            })
            .unwrap_or_default();
        CrowdHeat { cell, cells }
    }

    /// The colour of one cell: congestion runs from amber to red and wins over the trail, which is a
    /// faint blue, so a queue is never hidden under the path that led to it.
    pub fn colour(trail: f32, congestion: f32) -> Color {
        if congestion >= trail {
            // CHROME-OK: world ink, a heat ramp over the floor like the gizmo palettes, not panel chrome.
            Color::srgba(1.0, 0.85 - 0.75 * congestion, 0.05, 0.25 + 0.6 * congestion)
        } else {
            // CHROME-OK: world ink, the faint trail under the heat.
            Color::srgba(0.3, 0.6, 1.0, 0.1 + 0.4 * trail)
        }
    }
}

/// The node the palette is rebuilt into, so collapsing a category can redraw it.
///
/// **`pub` for the boxes that had none** (#275): a test, an agent and `tools/demo/record.py` all need
/// to ask "which box are these rows in", and there is no other answer — the palette and the inspector
/// are the same shape, so finding it by size found the wrong one three times. Naming the node costs a
/// keyword and settles it.
#[derive(Component)]
pub struct PaletteList;

/// A category header. Clicking it folds the group away.
#[derive(Component, Clone)]
struct CategoryHeader(String);

/// A palette row, carrying its library index so one observer can serve all of them.
///
/// Reflected so a BRP caller (`tools/ux/first_run_walk.py`) can ask which nodes are palette rows and
/// click one, whatever its words are.
#[derive(Component, Reflect, Clone, Copy)]
#[reflect(Component)]
pub struct PaletteRow(pub usize);

/// The Map palette's scrollable list, read for its geometry and written for its scroll position.
type PaletteListQuery<'w, 's> = Query<
    'w,
    's,
    (&'static ComputedNode, &'static UiGlobalTransform, &'static mut ScrollPosition),
    (With<PaletteList>, Without<PaletteRow>),
>;

/// **Scroll the palette so the armed brush is on screen** — `tiles::keep_selection_on_screen`'s
/// shape, for the Map's own list.
///
/// The palette never needed this while its only writer was a mouse click: you cannot click a row
/// you cannot see. The keyboard walk changed that — the arrows move `EditorState::brush` past the
/// fold, and the highlight left the screen while the list stood still. Reported from the keyboard,
/// 2026-08-14, mid-guide: *"if I arrow down and the scroll view, it just goes off the screen. The
/// scroll doesn't actually happen."*
///
/// One frame late for the sibling's reason: `rebuild_palette` watches the same change, and this
/// frame's geometry describes last frame's rows. Physical in, logical out — `ComputedNode` and
/// `UiGlobalTransform` are physical pixels, `ScrollPosition` is logical.
fn keep_palette_selection_on_screen(
    state: Res<EditorState>,
    rows: Query<(&PaletteRow, &ComputedNode, &UiGlobalTransform)>,
    mut lists: PaletteListQuery,
    mut follow: Local<crate::chrome::Follow<usize>>,
) {
    // **Keyed on the brush, not on `EditorState::is_changed`.** See `chrome::Follow` — the
    // resource is written most frames, so watching it re-armed this for ever and the scroll never
    // happened.
    if !follow.should_scroll(state.brush) {
        return;
    }
    let Some(brush) = state.brush else {
        return;
    };
    let Some((row_mid, row_half)) = rows
        .iter()
        .find(|(r, _, _)| r.0 == brush)
        .map(|(_, n, t)| (t.translation.y, n.size.y * 0.5))
    else {
        return;
    };
    for (list, list_tf, mut scroll) in &mut lists {
        let (at, max) = crate::chrome::scroll_bounds(list);
        if let Some(want) = crate::chrome::scroll_to_reveal(
            (row_mid, row_half),
            (list_tf.translation.y, list.size().y * 0.5),
            at,
            max,
            list.inverse_scale_factor,
        ) {
            scroll.0.y = want;
        }
    }
}

/// The Inspector's scrolling body, read for its geometry and written for its scroll position.
type InspectorBodyQuery<'w, 's> =
    Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform, &'static mut ScrollPosition), With<MapControlsPane>>;

/// **The note box is drawn where the person can read it** (#177): while a note is being typed, the
/// Inspector scrolls its receipt row (`Field::Last`, where the box's text, caret and `[auto]` are
/// drawn) back on screen.
///
/// The box has no field of its own: [`type_note`] writes the draft into the receipt. With the
/// Inspector scrolled down to NOTES, where the person watches the list a new note joins, that row
/// was off the top of the pane, so the note was typed blind. Measured recording #175 on sib.
///
/// **Keyed on what the row shows, not on the resource**, through `chrome::Follow`: the box opening,
/// each keystroke, Tab and a Shift-clicked piece re-arm it, and nothing else does. So a person who
/// scrolls back down to NOTES with the box open is left there until the next key, which brings the
/// text back the way a text editor brings back its caret.
fn keep_note_box_on_screen(
    note_box: Res<NoteBox>,
    rows: Query<(&Field, &ComputedNode, &UiGlobalTransform)>,
    mut bodies: InspectorBodyQuery,
    mut follow: Local<crate::chrome::Follow<(usize, bool, usize)>>,
) {
    let shown = note_box.draft.as_ref().map(|d| (d.text.len(), d.auto, d.pieces.len()));
    if !follow.should_scroll(shown) || shown.is_none() {
        return;
    }
    let Some((row_mid, row_half)) = rows
        .iter()
        .find(|(f, _, _)| **f == Field::Last)
        .map(|(_, n, t)| (t.translation.y, n.size.y * 0.5))
    else {
        return;
    };
    for (body, body_tf, mut scroll) in &mut bodies {
        let (at, max) = crate::chrome::scroll_bounds(body);
        if let Some(want) = crate::chrome::scroll_to_reveal(
            (row_mid, row_half),
            (body_tf.translation.y, body.size().y * 0.5),
            at,
            max,
            body.inverse_scale_factor,
        ) {
            scroll.0.y = want;
        }
    }
}

/// A spawned instance of a map placement, tagged with the id it came from.
#[derive(Component)]
pub struct Placement(pub String);

/// **A proposed piece's ghost**, so a preview can be taken down without touching the map's own rows.
///
/// Its own marker rather than `Ghost` alone, because the two have different lifetimes and different
/// owners: `Ghost` is the *look* (translucent, no shadow — see `fade_ghost`), and this is the
/// *bookkeeping* of a proposal that has not been committed. `sync_proposal_ghosts` despawns by this
/// and nothing else, so discarding a layout cannot take an armed brush's ghost with it.
#[derive(Component)]
pub struct ProposalGhost;

/// **The grid step the map is read on**, metres.
///
/// Shared by `generate` and `check_edges` on purpose: the learned grammar and the edge check must
/// agree about which two placements are neighbours, or the tool would report a fault between pieces
/// the solver does not think touch.
const CELL: f32 = grid::TILE;

/// How much of a generated region should close into rooms, and what that wish is worth — **as
/// shipped, and as the defaults of [`Enclosure`]** (E-005).
///
/// **These are defaults now, not the only answer.** A developer who wants more enclosed rooms edits
/// the map's `enclosure` block, or asks an agent to set it in the running editor; nothing here needs
/// a rebuild. The values stay because a map written before the setting existed has to mean something,
/// and the meaning it had is this one.
///
/// The wish being *soft* is what makes a fixed default safe: a region that cannot reach the target
/// returns its best arrangement and says so, rather than refusing.
///
/// The weight is what the wish is worth against everything else asked of the solve. It is the only
/// soft constraint in play — the per-cell texture is a preference and costs nothing — so any positive
/// value makes it decisive; this one is round and leaves room for a second wish to be weighed against
/// it later.
pub const ENCLOSURE_WISH: f32 = 0.25;
pub const ENCLOSURE_WEIGHT: u32 = 200;

/// The see-through preview of the armed brush.
///
/// **Public because a ghost is a ghost on every tab.** [`fade_ghost`] is the one thing that makes a
/// preview translucent, and it is keyed on this marker alone — so a tab that draws its own preview
/// (the tile assembler does) marks it with this rather than growing a second fading pass. Without the
/// marker the "ghost" renders solid, at full alpha, indistinguishable from a committed piece.
#[derive(Component)]
pub struct Ghost;

/// Which descriptor the live ghost is showing, so it is rebuilt only when the brush changes —
/// respawning a GLB every frame would thrash the asset server and never finish loading.
#[derive(Component)]
struct GhostOf(usize, bool);

#[derive(Component)]
struct Ghosted;

/// The readout block, so the whole thing can be found in one query.
#[derive(Component)]
struct StatusBlock;

/// **Lines the receipt row (`Field::Last`) keeps even when empty**, so a receipt arriving does not
/// push the blocks below it down. Two covers 165 of the 174 receipt sites (B-009).
const RECEIPT_LINES: f32 = 2.0;

/// The receipt row's label. It reads `LAST` while the receipt speaks and `HINT` while a hint does,
/// so the words in the slot always name what they are.
#[derive(Component)]
struct ReceiptLabel;

/// The live cost readout. In the chrome bar, in a reserved column — see [`COST_COL`], and
/// `the_triangle_count_does_not_shove_the_maps_name` for what happens without one. `pub` so that
/// test can drive it.
#[derive(Component)]
pub struct TriangleTotal;

/// **The label column of the readout block, in logical px**: `RELATIONS`, its longest single word, is
/// 9 glyphs of the 10 px label face (6 px each, 54 px), and the column is that and 4 px of air. Wider
/// than `COL_LABEL` by exactly what that word needs: a column the width of `COL_WIDE` costs the value
/// column the room the two-line receipt and hint slots are measured against.
const READOUT_LABEL_W: f32 = 58.0;

/// One labelled row of the readout. A field per line, rather than one string with separators in it,
/// because a separator is a thing the reader has to parse and a column is not.
#[derive(Component, Clone, Copy, PartialEq)]
enum Field {
    Name,
    /// **The open map's rule seed**, typed on `K` and committed through `rules_set` (E-081).
    Rule,
    Brush,
    Yaw,
    Map,
    /// **What Cmd+Z will take back**, named, so the author reads it before pressing the chord.
    ///
    /// The newest entry on the undo stack, read against the map as it is now. UNDO and REDO are two
    /// rows, not one, because the author compares them.
    Undo,
    /// **What Cmd+Shift+Z will put back**, named the same way. An empty history reads `nothing`.
    Redo,
    /// The last thing that happened — the only line that is prose. While a hint is set it takes this
    /// slot instead (B-013), labelled `HINT`, and the receipt returns when the hint clears.
    Last,
    /// **What the piece-verbs would act on**, and the chord that opens it for editing.
    ///
    /// The readout the panel was missing. `BRUSH` says what a click would *place*; nothing said what
    /// `H`, `R`/`T`, `Y`/`U`, `[`/`]`, `O` and `Cmd+2` would act *on* — all six read "the placement
    /// under the cursor", which the author could only learn by trying one. `Cmd+2` was reported as
    /// missing by someone looking straight at the map for it, which is what this row answers.
    ///
    /// Since 2026-08-21 it is also those verbs' badge *home*: `H`, `[`/`]`, `X` and `O` draw here
    /// while `K` is held (the turn cluster draws on [`Self::Yaw`]), so the row that names the
    /// subject is the row the chords stand on. See [`Field::control`].
    Under,
    /// **Where the map disagrees with the tokens the tiles declare.**
    ///
    /// See `bevy_grimoire_core::adjacency`. A standing readout rather than a message, because a fault is a
    /// state the map is in — it does not happen once and stop being true, so putting it on
    /// [`Self::Last`] would let the next action erase it.
    Edges,
    /// **The repeat a player will notice first** (#136), from `bevy_grimoire_core::repetition` under
    /// its default rules: the worst repeat's piece and how many pieces it has, then how many other
    /// looks repeat. The same answer `grimoire_state.repetition` gives an agent, so a person reads it
    /// too. A standing readout, like [`Self::Edges`], because it is a state the map is in.
    Repeats,
    /// **The intended relations the open map breaks** (#137), from `<map>.relations.ron` checked by
    /// `bevy_grimoire_core::relations::check`: how many are unmet and missing and the first broken one
    /// in words. The same answer `grimoire_state.relations` gives an agent, so a person reads it too.
    Relations,
}

impl Field {
    /// The rows that are badge homes — YAW carries the turn cluster, UNDER the piece-verbs. A
    /// mapping rather than a literal at each spawn, the `CellVerb::control()` shape, so the row
    /// loop stays one loop.
    fn control(self) -> Option<crate::keys::ControlId> {
        match self {
            Field::Yaw => Some(crate::keys::ControlId::Yaw),
            Field::Under => Some(crate::keys::ControlId::Under),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Field::Edges => "EDGES",
            Field::Name => "NAME",
            Field::Rule => "RULE",
            Field::Brush => "BRUSH",
            Field::Yaw => "YAW",
            Field::Map => "MAP",
            Field::Under => "UNDER CURSOR",
            Field::Undo => "UNDO",
            Field::Redo => "REDO",
            Field::Last => "LAST",
            Field::Repeats => "REPEATS",
            Field::Relations => "RELATIONS",
        }
    }
}

/// **What the note box holds while it is open** (#89): the area the note is about, the note being
/// edited (none for a new one) and the text typed so far.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteDraft {
    /// Inclusive cell rectangle `[min_x, min_z, max_x, max_z]`.
    pub area: [i32; 4],
    /// The id of the standing note being edited, or `None` for a new note.
    pub editing: Option<String>,
    pub text: String,
    /// Placement ids Shift-clicked as the note's subject. When any, the note is about these pieces
    /// and `area` is only the cells they span.
    pub pieces: Vec<String>,
    /// Tab in the box: "decide these yourself". The note is saved with `auto` (E-070), so its holder
    /// states what it assumed instead of asking about an open choice.
    pub auto: bool,
}

/// **The Note tool's state**: the corner a drag started at, and the box once the button comes up.
/// Its own resource for the reason [`RemovalDrag`] gives.
#[derive(Resource, Default)]
pub struct NoteBox {
    from: Option<(f32, f32)>,
    pub draft: Option<NoteDraft>,
}

/// The cell a map-space coordinate is in.
fn note_cell(v: f32) -> i32 {
    (v / grid::TILE).floor() as i32
}

/// **A piece becomes (part of) a note's subject**, the one way both the Note tool's Shift-click and a
/// piece menu's Note item say "this piece is what the note is about" (UI-36, #230). The box opens, or
/// stays open, on the pieces picked, and `area` is the cells they span. `toggle` is the Shift-click:
/// picking a piece already in the box takes it out; the menu's item only ever adds it.
///
/// A box holding a note being typed or edited is left as it is and says so: a second pick must not
/// throw away what the person has written.
pub(crate) fn note_pick_piece(
    open: &OpenMap,
    state: &mut EditorState,
    note_box: &mut NoteBox,
    id: String,
    toggle: bool,
) {
    if note_box.draft.as_ref().is_some_and(|d| d.editing.is_some() || d.pieces.is_empty()) {
        state.status.note("finish or cancel the open note first (Enter saves, Esc cancels)");
        return;
    }
    let mut draft = note_box.draft.take().unwrap_or(NoteDraft {
        area: [0; 4],
        editing: None,
        text: String::new(),
        pieces: Vec::new(),
        auto: false,
    });
    match draft.pieces.iter().position(|p| *p == id) {
        Some(k) if toggle => {
            draft.pieces.remove(k);
        }
        Some(_) => {}
        None => draft.pieces.push(id),
    }
    if draft.pieces.is_empty() {
        state.status.note("note: no pieces picked");
        return;
    }
    let mut area = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
    for p in open.map.placements.iter().filter(|p| draft.pieces.contains(&p.id)) {
        let (x, z) = (note_cell(p.at.0), note_cell(p.at.1));
        area = [area[0].min(x), area[1].min(z), area[2].max(x), area[3].max(z)];
    }
    draft.area = area;
    state.status.note(format!(
        "note on {} piece(s): Shift-click more, type it, Enter saves, Esc cancels",
        draft.pieces.len()
    ));
    note_box.draft = Some(draft);
    state.tool = Tool::Note;
}

/// **The Note tool**: press, drag, release over the ground. A drag is the note's area; a click on a
/// standing note's area opens that note, and a click on bare ground is a one-cell area.
#[allow(clippy::too_many_arguments)]
fn drive_note(
    mouse: Res<ButtonInput<MouseButton>>,
    // **The shared answer**, from `sense_over_ui` — see [`crate::view::OverUi`].
    over_ui: Res<crate::view::OverUi>,
    pointer: Res<crate::view::Pointer>,
    camera: Option<Single<(&Camera, &GlobalTransform), With<MainCamera>>>,
    open: Res<OpenMap>,
    project: Option<Res<Project>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<EditorState>,
    mut note_box: ResMut<NoteBox>,
) {
    // A box of Shift-clicked pieces stays open to take more pieces; any other box holds the keys.
    let collecting = note_box.draft.as_ref().is_some_and(|d| d.editing.is_none() && !d.pieces.is_empty());
    if state.tool != Tool::Note || (note_box.draft.is_some() && !collecting) {
        if note_box.from.is_some() {
            note_box.from = None;
        }
        return;
    }
    let Some(camera) = camera else {
        return;
    };
    let (cam, cam_tf) = *camera;
    let blocked = ui_blocks_gesture(over_ui.0, note_box.from.is_some());
    let hit = cursor_ground(pointer.0, cam, cam_tf).filter(|_| !blocked);
    let Some(hit) = hit else {
        if mouse.just_released(MouseButton::Left) {
            note_box.from = None;
        }
        return;
    };
    let at = open.map.to_map_space((hit.x, hit.z));
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    if shift && mouse.just_pressed(MouseButton::Left) {
        // Shift-click: a piece joins (or leaves) the note's subject; the box opens at the selection.
        match project.as_deref().and_then(|project| pick_at(project, &open, at)) {
            Some(i) => {
                let id = open.map.placements[i].id.clone();
                note_pick_piece(&open, &mut state, &mut note_box, id, true);
            }
            // **An empty pick is guidance, not silence.** It was a bare `return` inside the `if let`,
            // so a Shift-click on ground between the pieces did nothing at all — no box, no line —
            // and the words typed after it went to the status line while the next `Enter` took the
            // no-box branch and sent drafts (#269, measured while recording the note clip: the
            // receipt saying `Enter saves` had already gone). `build.rs` states the rule this
            // follows: *"an arrow that answers nothing at all is the defect"*. A note rather than a
            // problem, because nothing went wrong — a click beside a piece is a near miss, not a
            // refusal — and it names the gesture that does work, which is the rectangle.
            None => state.status.note(format!(
                "no piece there — Shift-click a piece, or drag a rectangle for {}",
                crate::keys::chord(Action::NoteTool)
            )),
        }
        return;
    }
    if collecting {
        return;
    }
    if mouse.just_pressed(MouseButton::Left) {
        note_box.from = Some(at);
    }
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(from) = note_box.from.take() else {
        return;
    };
    let (a, b) = ((note_cell(from.0), note_cell(from.1)), (note_cell(at.0), note_cell(at.1)));
    let clicked = (from.0 - at.0).abs() <= CLICK_EPS && (from.1 - at.1).abs() <= CLICK_EPS;
    if clicked {
        let notes = bevy_grimoire_core::notes::Notes::load(&bevy_grimoire_core::notes::path_for_map(&open.map_path))
            .unwrap_or_default();
        let standing = notes.notes.iter().find(|n| {
            n.stands()
                && n.site.as_ref().is_some_and(|s| {
                    let (x0, z0, x1, z1) = s.cells;
                    (x0..=x1).contains(&a.0) && (z0..=z1).contains(&a.1)
                })
        });
        if let Some(n) = standing {
            let cells = n.site.as_ref().map(|s| s.cells).unwrap_or((a.0, a.1, a.0, a.1));
            note_box.draft = Some(NoteDraft {
                area: [cells.0, cells.1, cells.2, cells.3],
                editing: Some(n.id.clone()),
                text: n.text.clone(),
                pieces: Vec::new(),
                auto: n.auto,
            });
            state.status.note(format!("note {}: edit, Enter saves, an emptied note is removed, Esc leaves it", n.id));
            return;
        }
    }
    let area = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)];
    note_box.draft = Some(NoteDraft { area, editing: None, text: String::new(), pieces: Vec::new(), auto: false });
    state.status.note(format!(
        "note on {}x{} cells: type it, Enter saves, Shift+Enter is a new line, Esc cancels",
        area[2] - area[0] + 1,
        area[3] - area[1] + 1
    ));
}

/// **The sign over every standing note, and the rectangle being dragged for a new one** (#89).
///
/// White for a draft (written, not sent), orange for an open note (an agent may work it); a
/// resolved note has no sign. Gizmos, for `draw_edge_faults`'s reason: the notes file is the truth and
/// a spawned marker would be a second copy to keep in step. The file is re-read only when its
/// modification time moves, so the frame pays one `metadata` call.
#[allow(clippy::too_many_arguments)]
fn draw_notes(
    open: Res<OpenMap>,
    state: Res<EditorState>,
    note_box: Res<NoteBox>,
    pointer: Res<crate::view::Pointer>,
    camera: Option<Single<(&Camera, &GlobalTransform), With<MainCamera>>>,
    view: Res<crate::notes_view::NotesView>,
    mut gizmos: Gizmos,
) {
    let (ox, oy, oz) = open.map.origin;
    let mut rect = |x0: f32, z0: f32, x1: f32, z1: f32, color: Color| {
        gizmos.rect(
            Isometry3d::new(
                Vec3::new(ox + (x0 + x1) * 0.5, oy + 0.03, oz + (z0 + z1) * 0.5),
                Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
            ),
            Vec2::new(x1 - x0, z1 - z0),
            color,
        );
    };
    for n in view.notes().notes.iter().filter(|n| n.stands()) {
        if let Some(site) = &n.site {
            let color = if n.is_draft() { Color::WHITE } else { ACCENT };
            let (x0, z0, x1, z1) = site.world;
            rect(x0, z0, x1, z1, color);
        }
    }
    if state.tool != Tool::Note {
        return;
    }
    let (Some(from), Some(camera)) = (note_box.from, camera) else {
        return;
    };
    let (cam, cam_tf) = *camera;
    let Some(hit) = cursor_ground(pointer.0, cam, cam_tf) else {
        return;
    };
    let at = open.map.to_map_space((hit.x, hit.z));
    let (a, b) = ((note_cell(from.0), note_cell(from.1)), (note_cell(at.0), note_cell(at.1)));
    let t = grid::TILE;
    rect(
        a.0.min(b.0) as f32 * t,
        a.1.min(b.1) as f32 * t,
        (a.0.max(b.0) + 1) as f32 * t,
        (a.1.max(b.1) + 1) as f32 * t,
        ACCENT,
    );
}

/// **The note box's keys.** While it is open it holds the keyboard ([`Fields::typing`]).
fn type_note(
    mut events: MessageReader<KeyboardInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    camera: Option<Single<&GlobalTransform, With<MainCamera>>>,
    mut project: Option<ResMut<Project>>,
    mut open: ResMut<OpenMap>,
    mut state: ResMut<EditorState>,
    mut note_box: ResMut<NoteBox>,
) {
    if note_box.draft.is_none() {
        // Enter with the tool armed and no box open sends every draft; anything else is drained,
        // so the key that armed the tool is not read as the box's first character.
        let enter = state.tool == Tool::Note
            && events.read().any(|e| e.state.is_pressed() && matches!(e.logical_key, Key::Enter));
        events.clear();
        if enter {
            send_notes(project.as_deref_mut(), &mut open, &mut state);
        }
        return;
    }
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }
        let Some(draft) = note_box.draft.as_mut() else {
            break;
        };
        match &event.logical_key {
            Key::Enter if shift => draft.text.push('\n'),
            Key::Enter => {
                let draft = note_box.draft.take().expect("checked above");
                let pose = camera.as_ref().map(|t| {
                    let (yaw, pitch, _) = t.rotation().to_euler(EulerRot::YXZ);
                    let p = t.translation();
                    [p.x, p.y, p.z, yaw.to_degrees(), pitch.to_degrees()]
                });
                match &draft.editing {
                    None => add_note(project.as_deref_mut(), &mut open, &mut state, &draft, pose),
                    Some(id) if draft.text.trim().is_empty() => {
                        remove_note(project.as_deref_mut(), &mut open, &mut state, id)
                    }
                    Some(id) => edit_note(project.as_deref_mut(), &mut open, &mut state, id, &draft.text),
                }
            }
            Key::Escape => {
                note_box.draft = None;
                state.status.note("note not saved");
            }
            Key::Backspace => {
                draft.text.pop();
            }
            Key::Tab if draft.editing.is_none() => draft.auto = !draft.auto,
            Key::Space => draft.text.push(' '),
            Key::Character(s) => draft.text.push_str(s),
            _ => {}
        }
        if let Some(d) = note_box.draft.as_ref() {
            let auto = if d.auto { " [auto: decides for itself]" } else { "" };
            state.status.note(format!("note: {}\u{258f}{auto} — Tab toggles auto, Enter saves, Esc cancels", d.text));
        }
    }
}

/// **The `note_add` arguments a note box's draft stands for.** `auto` is sent only when Tab turned it on.
pub fn note_add_params(draft: &NoteDraft, pose: Option<[f32; 5]>) -> serde_json::Value {
    let mut params = if draft.pieces.is_empty() {
        serde_json::json!({"cells": draft.area, "text": draft.text, "camera": pose})
    } else {
        serde_json::json!({"pieces": draft.pieces, "text": draft.text, "camera": pose})
    };
    if draft.auto {
        params["auto"] = serde_json::json!(true);
    }
    params
}

/// **Leave a note through the registry**, as an agent's `note_add` does. A refusal is a problem,
/// where the author looks.
fn add_note(
    project: Option<&mut Project>,
    open: &mut OpenMap,
    state: &mut EditorState,
    draft: &NoteDraft,
    pose: Option<[f32; 5]>,
) {
    dispatch_note(
        project,
        open,
        state,
        "note_add",
        note_add_params(draft, pose),
        "leave a note",
    );
}

/// **Change a standing note's text through the registry.**
fn edit_note(project: Option<&mut Project>, open: &mut OpenMap, state: &mut EditorState, id: &str, text: &str) {
    dispatch_note(project, open, state, "note_edit", serde_json::json!({"id": id, "text": text}), "edit a note");
}

/// **Send every draft through the registry**, as `note_send` does for an agent.
///
/// **Then start the project's notes agent** (#94), once per Send, when `agent.ron` names one. A launch
/// that fails is a problem on the status line; the notes stay sent either way.
pub fn send_notes(project: Option<&mut Project>, open: &mut OpenMap, state: &mut EditorState) {
    let root = project.as_deref().map(|p| p.root.clone());
    let sent = dispatch_note(project, open, state, "note_send", serde_json::json!({}), "send the drafts");
    let Some(root) = root.filter(|_| sent) else { return };
    use bevy_grimoire_core::note_agent::{AgentSettings, LaunchError};
    let trust = match crate::trusted::load(state.trust.file.as_deref()) {
        Ok(t) => t,
        Err(e) => return state.status.problem(e),
    };
    let launched = AgentSettings::load(&root).map_err(LaunchError::Failed).and_then(|s| s.launch(&root, &trust));
    match launched {
        Ok(Some(pid)) => state.status.note(format!("started the notes agent (pid {pid})")),
        Ok(None) => {}
        Err(LaunchError::Untrusted(spec)) => {
            // Nothing was started. The refusal is where a person looks, and the question is raised.
            state.status.problem(bevy_grimoire_core::trust::TrustStore::untrusted_sentence(&root, &spec));
            state.trust.pending = Some(crate::trusted::TrustRequest { root, spec });
        }
        Err(LaunchError::Failed(e)) => state.status.problem(e),
    }
}

/// **Hold, keep or give back a note through the registry** (#91): `op` is `note_claim`, `note_renew` or
/// `note_release`. The person holds as `person`.
fn lease_note(project: Option<&mut Project>, open: &mut OpenMap, state: &mut EditorState, op: &str, id: &str) {
    let params = if op == "note_release" {
        serde_json::json!({"id": id})
    } else {
        serde_json::json!({"id": id, "agent": "person"})
    };
    dispatch_note(project, open, state, op, params, "change a note's lease");
}

/// **Ask, answer or finish a note through the registry** (#92): `op` is `note_ask`, `note_answer` or
/// `note_resolve`. The person holds as `person`; `text` is the question, the answer or the summary.
fn talk_note(project: Option<&mut Project>, open: &mut OpenMap, state: &mut EditorState, op: &str, id: &str, text: &str) {
    let params = match op {
        "note_ask" => serde_json::json!({"id": id, "agent": "person", "question": text}),
        "note_answer" => serde_json::json!({"id": id, "answer": text}),
        "note_keep" => serde_json::json!({"id": id}),
        _ => serde_json::json!({"id": id, "agent": "person", "summary": text}),
    };
    dispatch_note(project, open, state, op, params, "talk on a note");
}

/// **Undo a done note's work as one step through the registry** (#93).
fn undo_note_work(project: Option<&mut Project>, open: &mut OpenMap, state: &mut EditorState, id: &str) {
    dispatch_note(project, open, state, "note_undo_work", serde_json::json!({"id": id}), "undo a note's work");
}

/// **Take a note off the map through the registry.**
fn remove_note(project: Option<&mut Project>, open: &mut OpenMap, state: &mut EditorState, id: &str) {
    dispatch_note(project, open, state, "note_remove", serde_json::json!({"id": id}), "remove a note");
}

/// The one dispatch the three note verbs share: build the context, dispatch, put the map back, and
/// put the change on the undo stack so Cmd+Z takes it back.
fn dispatch_note(
    project: Option<&mut Project>,
    open: &mut OpenMap,
    state: &mut EditorState,
    name: &str,
    params: serde_json::Value,
    phrase: &str,
) -> bool {
    let mut ctx = crate::ops::Ctx {
        view: None, trust_file: None,
        project: project.as_ref().map(|p| (**p).clone()),
        open: Some(std::mem::replace(
            open,
            OpenMap::new(bevy_grimoire_core::map::Map::default(), std::path::PathBuf::new()),
        )),
        source: crate::ops::Source::Human,
        root: None,
        session: crate::ops::session_id(),
        proposal: None,
    };
    let outcome = crate::ops::dispatch(&mut ctx, name, &params);
    if let Some(fresh) = ctx.open.take() {
        *open = fresh;
    }
    match outcome {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary);
            state.record(Undo::Logged { line: operation_id, phrase: phrase.to_owned(), agent: false });
            true
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
            state.status.raise(&format!("cannot {phrase}: "), e.problem());
            false
        }
    }
}

/// **Flip a group's envelope through the registry** as `composition_set_envelope`: anchored becomes
/// bounded (fitted to its members), bounded becomes anchored. Returns the receipt, or the refusal.
pub fn set_envelope(project: &mut Project, id: &str, to_bounded: bool) -> Result<String, String> {
    let mut ctx = crate::ops::Ctx {
        project: Some(project.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    let params = serde_json::json!({"id": id, "envelope": if to_bounded { "bounded" } else { "anchored" }});
    match crate::ops::dispatch(&mut ctx, "composition_set_envelope", &params) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => {
            if let Some(fresh) = ctx.project {
                project.compositions = fresh.compositions;
            }
            Ok(outcome.summary)
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

/// **Rename a composition through the registry** as `composition_rename` (E-002, #129): the COMPOSE
/// tab's `I` field comes here, so the naming rule, the refusals and the all-or-nothing write of the
/// compositions file and every stamping map are the operation's. The open map, when there is one,
/// takes the text its file now holds. Returns the receipt, or the refusal.
pub fn rename_composition(
    project: &mut Project,
    open: Option<&mut OpenMap>,
    from: &str,
    raw: &str,
) -> Result<String, String> {
    let mut ctx = crate::ops::Ctx {
        project: Some(project.clone()),
        open: open.as_deref().cloned(),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    let params = serde_json::json!({"composition": from, "name": raw});
    match crate::ops::dispatch(&mut ctx, "composition_rename", &params) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => {
            if let Some(fresh) = ctx.project {
                project.compositions = fresh.compositions;
            }
            if let (Some(open), Some(fresh)) = (open, ctx.open) {
                *open = fresh;
            }
            Ok(outcome.summary)
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(crate::screen::Screen::Editor), crate::terrain_view::warm_volume_pipeline);
        #[cfg(feature = "bevy_elements")]
        app.add_systems(OnEnter(crate::screen::Screen::Editor), crate::environment::warm_weathered_pipeline);
        #[cfg(feature = "bevy_elements")]
        app.init_resource::<crate::environment::ClockScrub>()
            .init_resource::<crate::environment::ClockRamp>()
            .add_systems(
            Update,
            (
                crate::environment::apply_environment.after(crate::family_host::Reloaded),
                // **After the settings, before the scrubber**: the walk moves the hour the file
                // asked for, and a held scrubber then wins, as it always has (#271).
                crate::environment::walk_the_clock
                    .after(crate::environment::apply_environment)
                    .before(crate::environment::scrub_clock),
                crate::environment::scrub_clock.after(crate::environment::apply_environment),
                crate::environment::dress_volume.after(crate::family_host::Reloaded).after(crate::terrain_view::sync_terrain_view),
            ),
        );
        #[cfg(all(feature = "bevy_flora", feature = "bevy_elements"))]
        app.add_systems(Update, crate::forest_season::follow_date.after(crate::environment::scrub_clock));
        #[cfg(feature = "bevy_flora")]
        app.add_plugins(crate::forest_view::ForestViewPlugin);
        #[cfg(feature = "bevy_lights")]
        app.add_plugins(crate::light_view::LightViewPlugin).add_observer(crate::ops_pool::on_pool_click);
        // **The audio panel (S-01):** `bevy_siren`'s own plugins, never `AudioPlugin` (the editor's
        // `DefaultPlugins` has it and a second add panics), plus this crate's panel state and its observers.
        #[cfg(feature = "bevy_siren")]
        app.add_plugins(bevy_siren::SirenPlugins)
            .init_resource::<crate::audio_panel::AudioPanel>()
            .add_observer(crate::audio_panel::on_audition)
            .add_systems(Update, crate::audio_panel::read_audio);
        #[cfg(feature = "bevy_elements")]
        app.add_observer(on_environment_click);
        // **The light map (#103):** shown by its button, re-baked when the map changes, drawn over the floor.
        #[cfg(feature = "bevy_light_grid")]
        app.init_resource::<crate::light_overlay::LightOverlay>()
            .add_observer(crate::light_overlay::on_light_map_click)
            .add_systems(
                Update,
                (
                    crate::light_overlay::refresh_light_overlay,
                    crate::light_overlay::draw_light_overlay
                        .after(crate::light_overlay::refresh_light_overlay)
                        .run_if(in_map_mode)
                        .run_if(resource_exists::<OpenMap>)
                        .run_if(resource_exists::<Project>),
                ),
            );
        app.add_plugins(crate::ui::EditorUiPlugin);
        app.add_plugins(crate::piece_menu::PieceMenuPlugin);
        app.add_plugins(crate::palette_menu::PaletteMenuPlugin);
        app.add_plugins(crate::command_palette::CommandPalettePlugin);
        app.add_plugins(crate::toolbar::ToolbarPlugin);
        app.add_plugins(crate::empty_state::EmptyStatePlugin);
        app.add_plugins(crate::add_meshes::AddMeshesPlugin);
        app.add_plugins(crate::menu_bar::MenuBarPlugin);
        app.add_plugins(crate::app_verbs::AppVerbsPlugin);
        app.add_plugins(crate::inspector_view::InspectorPlugin);
        app.add_plugins(crate::outliner::OutlinerPlugin);
        app.add_observer(drop_a_piece_on_the_map);
        app.add_plugins(crate::selection::SelectionPlugin);
        #[cfg(feature = "bevy_siren")]
        app.add_plugins(crate::audio_panel::AudioPanelPlugin);
        app.add_observer(on_play_click);
        app.register_type::<PaletteRow>()
            .register_type::<crate::chrome::ViewportSlot>();
        app.init_resource::<EditorState>()
            .init_resource::<MapFraming>()
            .init_resource::<RemovalDrag>()
            .init_resource::<NoteBox>()
            .init_resource::<NotesFilter>()
            .init_resource::<crate::notes_view::NotesView>()
            .init_resource::<crate::family_host::FamilySettings>()
            .add_systems(Update, (crate::family_host::report_problems, crate::family_host::reload_settings.in_set(crate::family_host::Reloaded)))
            // Registered in the same commit as `drive_move` and `keys` read it — a missing `ResMut`
            // panics its system in Bevy 0.19 rather than skipping it (`CLAUDE.md`).
            .init_resource::<MoveDrag>()
            .init_resource::<CloneDrag>()
            .init_resource::<TargetLock>()
            // Written by `redraw_stamps` and read by `pick_subject`'s callers, so this plugin owns
            // it — a missing `ResMut` panics its system in Bevy 0.19 rather than skipping it.
            .init_resource::<StampPicture>()
            // Read by `refresh_status`, so this plugin owns it — a missing `Res<T>` panics its
            // system in Bevy 0.19 rather than skipping it (`CLAUDE.md`).
            .init_resource::<UnderCursor>()
            // Read by `draw_map_grid` as a bare `Res<_>`, which panics its system in 0.19 if
            // nobody registered it.
            .init_resource::<Rung>()
            .init_resource::<Proposal>()
            // **A resource, not a field on `EditorState`.** `EditorState` is rebuilt on every door
            // crossing and the sync's `Local` is not, so a counter living there disagreed with the
            // thing comparing against it — see [`ProposalClock`].
            .init_resource::<ProposalClock>()
            .init_resource::<FineAnchor>()
            .init_resource::<PlaceDrag>()
            // Read by `drive_run`, so this plugin owns it — a missing `ResMut` panics its system in
            // Bevy 0.19 rather than skipping it (`CLAUDE.md`).
            .init_resource::<RunDrag>()
            // Read by `drive_run_ghost`, so this plugin owns it — a missing `ResMut` panics its
            // system in Bevy 0.19 rather than skipping it (`CLAUDE.md`).
            .init_resource::<RunGhostDrag>()
            // **This plugin reads it, so this plugin registers it** (CLAUDE.md's rule, and the
            // documented 0.19 trap: every run condition is evaluated, and `rebuild_palette`'s
            // `resource_changed::<ThumbGeneration>` panics if only ThumbsPlugin — which also inits
            // it, idempotently — happens to be absent from the app.
            .init_resource::<SizeEdit>()
            .init_resource::<FamilyEdit>()
            .init_resource::<DataEdit>()
            .init_resource::<TerrainEdit>()
            .init_resource::<CrowdEdit>()
            .init_resource::<crate::balloon::BalloonPreview>()
            .init_resource::<Folded>()
            .init_resource::<EdgeFaults>()
            // Shared by both tabs' lists, so it is registered once here rather than by whichever
            // plugin happens to build first.
            .init_resource::<crate::filter::Filters>();
        app.add_plugins(crate::snip::SnipPlugin);

        // **The filter box works on every door**, so it is registered above the gate below.
        //
        // It lived inside the gated block and stopped taking characters on the Tiles door the moment
        // the gate existed: `F` opened the box and nothing typed into it. Both list panels filter,
        // and this system takes only the key stream and `Filters` — nothing map-shaped — so the door
        // it runs on was never part of what it means.
        app.add_systems(Update,
                // **No door condition.** Both list panels filter, and this takes only the key
                // stream and `Filters` — nothing map-shaped. It was swept up when every system in
                // this file got the Maps-door gate, which killed the filter box on the Kit door for
                // the second time; the first was the `EditorPlugin` gate, and the fix is the same:
                // the door it runs on was never part of what it means.
                (
                    crate::filter::keys.in_set(keys::Phase::Text),
                    // **And the repaint, for the same reason — the third time this system has been
                    // swept up by that gate.**
                    //
                    // It sat in the Maps-door block below until 2026-08-20, so on the Kit door *no
                    // filter box has ever shown what was typed into it*: the text narrowed the list
                    // and the box went on saying `filter`, with no caret, on the two panels that do
                    // the most filtering. Found by looking at a captured frame — the tag block read
                    // `1 of 55` under a box that denied anything had been typed — which is where the
                    // last several defects here have been found and is not where a green suite
                    // looks.
                    //
                    // Like `keys` above it, this takes the key stream and `Filters` and nothing
                    // map-shaped, so the door it runs on was never part of what it means.
                    crate::filter::refresh,
                )
                    .run_if(in_state(crate::screen::Screen::Editor)),
            );
        // **The Map opens framed on the whole map, once** (backlog E-022). The rig's default is an
        // 18 m close-up of the origin, so a person who launched on a 1120 m town saw ground and had
        // to zoom out to find the map. It runs once, when the map and the viewport both exist.
        app.add_systems(
            Update,
            frame_map_on_open.run_if(in_state(crate::screen::Screen::Editor)),
        );
        app.add_systems(
            Update,
            frame_on_home.run_if(in_state(crate::screen::Screen::Editor)),
        );

        // **Resources always; systems only on the door that has a map** — as a run condition, not
        // a check here. See `Door::map_door_is_open` for what reading the door at build time cost.
        app.add_systems(
                Update,
                ((
                    // Before `sense_context`, which computes `Live` from `Filters::typing` —
                    // so the click that blurs is also the click that places.
                    crate::filter::blur_on_world_click
                        .in_set(keys::Phase::Sense)
                        .before(sense_context),
                    // Sensed before anything reads the cursor, so a click sees the anchor captured
                    // for its own press rather than one frame stale.
                    sense_fine_anchor.in_set(keys::Phase::Sense),
                ),)
                    .run_if(in_state(crate::screen::Screen::Editor))
                    .run_if(crate::tiles::Door::map_door_is_open),
            )
            .add_observer(crate::filter::on_click)
            // **The way back is not the Maps door's**, and gating it there was how the Kit and Rigs
            // doors ended up with no exit at all: the button every panel draws was never given its
            // observer, `Cmd+O` was handled inside a Maps-only system, and all three
            // `next.set(Screen::Project)` calls sat behind the same condition. See `leave_for_menu`.
            .add_systems(Update,
                (
                    wire_back_buttons,
                    back_to_the_menu.in_set(keys::Phase::Act),
                    // Ahead of every verb, because two of its three answers are also map verbs.
                    answer_the_leaving_prompt
                        .after(crate::confirm::ConfirmKeys)
                        .before(keys::Phase::Act),
                )
                    .run_if(in_state(crate::screen::Screen::Editor)),
            )
            // **The door switch, and it is on every door.** The key says *"the other door"*, so it
            // has to be live in both — and the crossing takes the project out of the world, which is
            // why it is an exclusive system.
            //
            // **In `PreUpdate`, and that is the whole design.**
            //
            // **The live test found why.** The editor's Map-door systems are gated by
            // `Door::map_door_is_open` — a **run condition on the tuple**, and Bevy evaluates a
            // *set's* conditions **once per schedule** (`schedule/executor/multi_threaded.rs`:
            // `evaluated_sets`). So a crossing that ran inside `Update` left that condition
            // evaluated against the *old* door, and four systems that take a `Res<OpenMap>` ran on
            // the Kit door — which has no map — and Bevy 0.19 **panics a system whose `Res<T>` is
            // missing** rather than skipping it.
            //
            // **`PreUpdate` runs before `StateTransition`, which runs before `Update`.** So the
            // crossing reads the key in `PreUpdate`, sets the pending state, and the transition —
            // the teardown and the rebuild — happens before any `Update` system evaluates its
            // conditions against the new door.
            .add_systems(
                PreUpdate,
                switch_door.run_if(in_state(crate::screen::Screen::Editor)),
            )
            // **The door switch's own answer**, and it is in `PreUpdate` for the same reason the
            // switch is: it crosses.
            .add_systems(
                PreUpdate,
                answer_the_switch_prompt.run_if(in_state(crate::screen::Screen::Editor)),
            )
            // **A door an agent asked for (`door_open` over BRP)** is crossed in the frame after the
            // request, when the editor is not asking a question.
            .add_systems(
                PreUpdate,
                apply_pending_door.run_if(in_state(crate::screen::Screen::Editor)),
            )
            .add_systems(
                OnEnter(crate::screen::Screen::Editor),
                (
                    arm_the_first_shown_piece,
                    crate::thumbs::setup,
                    spawn_panel,
                    spawn_palette_panel,
                    spawn_cost_readout,
                    spawn_removal_tile,
                    spawn_bounds_floor,
                    spawn_clone_tile,
                    spawn_target_tile,
                    spawn_existing,
                ).after(crate::chrome::FrameSystems)
                    .chain()
                    // The Map door's own furniture: a palette bound to a map, the bounds floor, the
                    // rows already placed. `spawn_existing` takes `Res<OpenMap>`, which no other
                    // door has.
                    .run_if(crate::tiles::Door::map_door_is_open),
            )
            .add_systems(
                Update,
                ((
                    // **The fields run last and the actions run first**, so no census action can
                    // fire on a keystroke a field has already swallowed. See `keys::Phase`.
                    rename_keys.in_set(keys::Phase::Text),
                    rule_seed_keys.in_set(keys::Phase::Text),
                    // Nested: the tuple is at Bevy 0.19's cap of 20. The two typed fields that ask
                    // a question about a piece — why it is pinned, what a group is called — ride
                    // together in `Phase::Text`.
                    (
                        pin_reason_keys.in_set(keys::Phase::Text),
                        group_name_keys.in_set(keys::Phase::Text),
                        // Before the fields, so the frame a tab changes on is the frame the prompt
                        // goes down, rather than one where it is invisible and still eating keys.
                        leaving_a_tab_puts_the_name_prompt_down.in_set(keys::Phase::Sense),
                    ),
                    (size_edit_keys, family_edit_keys, data_edit_keys, terrain_edit_keys, crowd_edit_keys, drop_reference).in_set(keys::Phase::Text),
                    // No `not_typing`, no `in_map_mode`: `keys::just_pressed` now refuses on both
                    // counts, and a run condition repeating it would be a second census. Dropping
                    // `in_map_mode` also makes `Cmd+S` and `Cmd+Z` work from every tab, which is what
                    // the census has always said they do (`Context::Global`) and what the guard
                    // quietly contradicted.
                    keys.in_set(keys::Phase::Act),
                    // These two stay gated: they read the MOUSE, which the census does not model, so
                    // context is not their guard. Mode is.
                    // **Nested, because this tuple is at Bevy 0.19's cap of 20.** The four
                    // placement tools are one idea — a click or a drag belongs to whichever is
                    // armed — and they read the MOUSE, which the census does not model, so mode is
                    // their guard.
                    (
                        (drive_place, drive_removal, drive_move, drive_clone, drive_run).run_if(not_typing),
                        // **Not behind `not_typing`** (#173): its own box would shut it off, and a box
                        // of Shift-clicked pieces takes more Shift-clicks.
                        drive_note.run_if(note_takes_clicks),
                    )
                        .run_if(in_map_mode),
                    // **Not behind `not_typing`**: it is the system that types, so it reads the keys
                    // exactly while the note box holds them.
                    type_note.run_if(in_map_mode),
                    draw_notes.run_if(in_map_mode),
                    // Nested for the same reason the pair below is: the tuple is at 20.
                    // `sense_under_cursor` is gated on the mode for `drive_target_marker`'s reason —
                    // it reads the MOUSE, which the census does not model, so context is not its
                    // guard. Both touch `TargetLock`, so they are named together.
                    (drive_target_marker, sense_under_cursor).run_if(in_map_mode),
                    hide_carried.run_if(in_map_mode),
                    // **Nested, because this tuple is at Bevy 0.19's cap of 20.** The pair is the
                    // two previews: one piece keyed by library index, and a whole set. Not folded
                    // into one system — that would be one system with two subjects and two
                    // lifetimes.
                    (
                        drive_ghost,
                        drive_clone_ghost,
                        drive_stamp_ghost,
                        drive_run_ghost,
                    )
                        .run_if(in_map_mode),
                    // **Not gated on the map.** The stamped rows are part of the map, so they stay
                    // drawn while an author is on Tiles or Compose looking at the group that made
                    // them — a world that empties out when you change tabs would read as the stamp
                    // having failed.
                    (redraw_stamps, redraw_runs),
                    // Nested: the tuple is at Bevy 0.19's cap of 20. `redraw_edited` is the Map
                    // side of a Tiles-tab write, so it rides with the other per-frame repaints.
                    // Nested: the tuple is at Bevy 0.19's cap of 20. `cycle_grid` is a census
                    // action, so it goes in `Act` with the rest of them.
                    // Nested: the tuple is at Bevy 0.19's cap of 20. `walk_palette` is a census
                    // action, so it goes in `Act` with the rest of them.
                    (
                        arm_palette_rows,
                        redraw_edited,
                        // **In `Act`, and not in the default set.** The watcher's `adopt_changes`
                        // raises the flag in `Phase::Sense`, and a system in no set has **no
                        // ordering** against that chain — Bevy may run it before or after. Here it
                        // ran before, every frame of the test, so the flag was always false when it
                        // looked. `Phase::Act` is chained after `Phase::Sense`, which is the order.
                        redraw_the_map.in_set(keys::Phase::Act),
                        cycle_grid.in_set(keys::Phase::Act),
                        walk_palette.in_set(keys::Phase::Act),
                        // `apply_proposal` after `keys`, so the `Enter` that accepts cannot be seen
                        // by the same frame's `Esc` peel — the two halves of one door must not both
                        // fire on one press.
                        apply_proposal.in_set(keys::Phase::Act).after(keys),
                        // **After `keys`, so a proposal is drawn on the frame it was made and taken
                        // down on the frame it was thrown away or kept** — a ghost that outlived its
                        // proposal would show a layout the map does not hold (B-046).
                        sync_proposal_ghosts
                            .in_set(keys::Phase::Act)
                            .after(keys)
                            // **After the commit door**, or the two race: `apply_proposal` takes
                            // the proposal and `sync` reads it, and with no order between them the
                            // sync can run first and leave `drawn` pointing at a proposal that is
                            // already gone — so the next `G` of equal generation goes undrawn.
                            .after(apply_proposal),
                        draw_proposal,
                    ),
                    (refresh_status, refresh_history),
                    // Nested — the tuple is at its twenty-system cap, and these are one rule: what
                    // the keys move must be on screen (the palette's row, the note box's text).
                    (
                        rebuild_palette.run_if(
                            // **`or_eager` on the last rung, because `run_once` holds a `Local`.**
                            // `bevy_ecs-0.19.0/src/schedule/condition.rs:690` is
                            // `pub fn run_once(mut has_run: Local<bool>) -> bool`, and `or_else`
                            // combines with `||` (same file, line 1564) — so on the very first
                            // frame, where `Project` was just inserted and answers true, `run_once`
                            // never ran and never spent its flag. It then fired on the first *later*
                            // frame where nothing else had changed, rebuilding the palette for
                            // nothing some frames after startup. `or_eager` (`|`, line 1586) runs it.
                            //
                            // The chain rungs above are stateless `resource_changed` conditions, so
                            // they keep `or_else` and its skip is free.
                            resource_exists_and_changed::<Project>
                                .or_else(resource_changed::<Folded>)
                                // **A change that only moved the keyboard does not change what the
                                // list shows** (#278, measured 2026-11-03). `on_row_click` blurs the
                                // filter after arming, and `blur` writes `Filters` — so with a plain
                                // `resource_changed` the click that armed a row also rebuilt every
                                // row, and the row it had just armed was despawned in the same frame.
                                // Measured: the first tap after typing cost exactly **1** rebuild and
                                // armed nothing, the second cost **0** and armed the piece.
                                .or_else(the_filter_text_changed)
                                // A newly created portrait handle has to be bound, and binding
                                // happens when the row is built. See `thumbs::ThumbGeneration` for
                                // why this is not `resource_changed::<Thumbnails>`.
                                .or_else(resource_changed::<crate::thumbs::ThumbGeneration>)
                                .or_eager(run_once),
                        ),
                        keep_palette_selection_on_screen.run_if(in_map_mode),
                        keep_note_box_on_screen.run_if(in_map_mode),
                    ),
                    (
                        refresh_size,
                        refresh_family,
                        refresh_data,
                        refresh_terrain,
                        refresh_crowd,
                        refresh_data_list,
                        refresh_notes_list,
                        refresh_scripts_list,
                        refresh_reload_list,
                        refresh_plan_list,
                    ),
                    refresh_triangle_total,
                    // Nested: the tuple is at 20. The wireframe edge and the slab inside it are
                    // one idea, and the slab only moves when the map's size does.
                    (
                        draw_bounds,
                        draw_map_grid,
                        crate::terrain_view::sync_terrain_view,
                        crate::notes_view::sync_notes_view,
                        fit_bounds_floor
                            .run_if(resource_exists_and_changed::<Project>.or_eager(run_once)),
                    ),
                    check_edges.run_if(resource_exists_and_changed::<Project>.or_eager(run_once)),
                    (draw_edge_faults, draw_crowd_heat, crate::balloon::show_balloon).run_if(in_map_mode),
                ),)
                    .run_if(in_state(crate::screen::Screen::Editor))
                    .run_if(crate::tiles::Door::map_door_is_open),
            )
            // **Paint follows the map, not the edit.** A piece's drawn paint can change when a
            // different piece is laid, so this runs on every map change and redraws only what went
            // stale. Gated like the rest of the Map door, because it takes `Res<OpenMap>`.
            .add_systems(
                Update,
                relevel_placements
                    .run_if(in_state(crate::screen::Screen::Editor))
                    .run_if(crate::tiles::Door::map_door_is_open),
            )
            // **The ghost pair is NOT map-door gated** — the map block above was swallowing it. A
            // tile's picked-but-not-dropped mesh (`StagedTile` + `Ghost`) is assembled on the Tiles
            // tab, so `fade_ghost` and `brighten_held` never ran under the kit door, and the ghost
            // rendered solid and shadow-casting: it read as "already added" instead of "awaiting
            // Enter". Both are unconditional editor rendering — they brush materials on entities
            // that carry their own markers, and `Ghost` only ever exists on a staged tile. What
            // stays map-gated is `redraw_stamps`, which repaints stamped rows of the MAP.
            .add_systems(
                Update,
                (fade_ghost, brighten_held).run_if(in_state(crate::screen::Screen::Editor)),
            )
            .add_observer(on_row_click)
            .add_observer(on_category_click)
            .add_observer(on_size_field_click)
            .add_observer(on_family_field_click)
            .add_observer(on_data_field_click)
            .add_observer(on_terrain_field_click)
            .add_observer(on_crowd_field_click)
            .add_observer(on_data_leaf_click)
            .add_observer(on_script_row_click)
            .add_observer(on_notes_click);
    }
}

// ── chrome ───────────────────────────────────────────────────────────────────────────────────────

/// **The triangle count, in the chrome bar with everything else that belongs to the window.**
///
/// It was a second absolute root at `right: MARGIN, top: MARGIN` on `PANEL_Z`, and it collided
/// twice. First it was painted over on Anim and painted over the Tiles candidate list's bottom row
/// (captured, 2026-08-17 audit), because z-order between two roots on one layer is spawn order.
/// Dodging left of the list collided with the 380 px controls panel the moment the window narrowed —
/// measured, not guessed. It was then moved to a top band no panel claimed, which held until the
/// frame gave that band a job: it landed on top of the chrome bar's `MAP · test`.
///
/// The frame is the answer to all three: it is a child in a row now, so nothing can overlap it and
/// nothing has to reason about what claims which strip of window. It carries `MapRoot` so
/// `tiles::apply_mode` hides it with the rest of the Map tab — a triangle count is a fact about the
/// map, not about the application.
fn spawn_cost_readout(mut commands: Commands, frame: Res<crate::chrome::Frame>) {
    commands.entity(frame.chrome_bar).with_children(|bar| {
        bar.spawn((
            crate::tiles::MapRoot,
            Node {
                padding: UiRect::axes(Val::Px(COST_PAD_X), Val::Px(crate::chrome::GAP_TIGHT)),
                // **A reserved column, and the number grows leftwards into it.**
                //
                // This is the last child of a row whose spacer pushes everything to the right end, so
                // every digit the count gained shoved `MAP · untitled_map` further left — reported
                // from the keyboard as the label "bouncing around a whole lot as those triangle
                // numbers change". A readout that moves its neighbour is a readout that makes the
                // neighbour unreadable, and the neighbour is the one thing on the bar that says which
                // map you are in.
                //
                // `min_width` rather than `width`, so a count past what [`COST_COL`] reserves pushes
                // out rather than clipping — the same choice, for the same reason, the key list's
                // chord column makes. `FlexEnd` is what makes the phrase hold still: the digits
                // extend into the slack instead of dragging `tris drawn` with them.
                min_width: Val::Px(COST_COL),
                justify_content: JustifyContent::FlexEnd,
                ..default()
            },
            // Nothing here is clickable, and a readout that eats clicks is a readout that steals
            // whatever is underneath it.
            Pickable::IGNORE,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                TextColor(DIM),
                crate::chrome::font(crate::chrome::text::BODY),
                TriangleTotal,
            ));
        });
    });
}

/// **Leave for the chooser — the whole decision, in one place.**
///
/// Three things ask for it: `Cmd+O`, the panel's `‹ kits & maps` button, and the last layer of the
/// Map tab's `Esc` peel. Three entry points to one path, the way `N` and `Enter` on a `+ new …` row
/// both start the same creation — and it is what keeps the unsaved-work refusal from being written
/// three times and drifting apart. A button that discarded work the key would have protected is the
/// exact failure this shape rules out.
///
/// # It always asks, and `Esc` is always the answer that cannot lose work
///
/// It used to leave immediately on a clean map. Asked for at the keyboard, 2026-08-16: *"escape
/// twice when nothing selected should prompt. Are you sure you want to quit? One more escape at that
/// prompt should quit."*
///
/// That request collides with the prompt already here, where **`Esc` means stay** — and both are
/// reachable by one gesture depending only on whether anything has been edited. The rule that makes
/// them one behaviour rather than two: **`Esc` is the answer that cannot lose work.** On a clean map
/// there is nothing to lose, so it goes; on a dirty one going would lose it, so it stays. Same key,
/// same promise, opposite action because the stakes are opposite — and no path through this door
/// discards anything on a key an author pressed by reflex.
///
/// **Every entry point asks**, including the deliberate chord. Confirming a `Cmd+O` costs one press;
/// having the chord and the reflex key mean different things costs an author their model of the
/// editor.
/// **`dirty`, not `&OpenMap`, because two of the three doors have no map.** The Kit and Rigs doors
/// carry no `OpenMap` at all (`args::Opened::insert_into` removes it), so a signature naming one
/// forced every caller — and therefore every route back to the menu — behind
/// `Door::map_door_is_open`. That left the Kit door with a `‹ kits & maps` button that lit on hover
/// and did nothing, a `Cmd+O` the shortcut overlay advertised and no system handled, and no exit but
/// closing the window. Nothing to lose is `false`, which is the clean-map question.
pub fn leave_for_menu(dirty: bool, state: &mut EditorState, confirm: &mut crate::confirm::Confirm) {
    state.leaving = true;
    // **One question, two keys, wherever it is asked** — see `crate::confirm`. This used to be a
    // line in the status bar answered by `S` save, `D` discard or `Esc` stay, while the chooser's
    // delete answered to `Y` and the labeller's overwrite answered to `Enter`. Three questions,
    // three vocabularies.
    //
    // **`D` discards, and it is the one way out that throws work away.** It was taken out with the
    // migration to `confirm` because `Cmd+O` and the back button already save and go
    // (`save_and_leave`), so a discard on this one route meant the same gesture kept work on two
    // paths out of three. It is back (#293) because without it a person who wants the file as it was
    // has no way to say so: leaving saved the edits whatever they thought of them. It is a key of its
    // own and never `Esc` or `N`, which stay on the map, so the answer a reflex gives still cannot
    // lose work, and `Cmd+O` and the back button still save.
    if dirty {
        confirm.ask_three(
            crate::confirm::Asked::LeaveMap,
            "Leave this map?",
            "It has unsaved edits. Leaving saves them first, unless you discard them.",
            "Save and leave",
            "Discard changes",
            KeyCode::KeyD,
            "Stay",
        );
    } else {
        confirm.ask(
            crate::confirm::Asked::LeaveMap,
            "Leave this map?",
            "Nothing is unsaved.",
            "Go",
            "Stay",
        );
    }
}

/// **Write what is open, then go — the one way the deliberate exits leave.**
///
/// # This replaced a confirmation, on purpose
///
/// [`leave_for_menu`] used to catch `Cmd+O` and the back button too, and its own doc argued for it:
/// *"Every entry point asks, including the deliberate chord. Confirming a `Cmd+O` costs one press;
/// having the chord and the reflex key mean different things costs an author their model of the
/// editor."* Reversed at the keyboard, 2026-08-18: *"make sure the Cmd+O button doesn't require a
/// second key press, it just autosaves and takes you back."*
///
/// The argument it was making has been answered rather than overruled. The prompt existed to stop
/// work being lost, and **saving loses nothing** — so there is no longer a question to ask. What is
/// left of the old rule still holds where it was actually about reflexes: `Esc` keeps its prompt,
/// because `Esc` is pressed by accident and a deliberate chord or a deliberate click is not. The
/// two keys still mean the same thing about *your work*; they differ only in how sure the editor is
/// that you meant to press them.
///
/// **A refused save keeps you here.** `Project::save` validates before it writes, so a map that
/// cannot be written is one this must not walk away from — leaving would silently discard exactly
/// the work the autosave was promising to keep. That is the one path out of this function that does
/// not reach the menu, and it is loud.
pub fn save_and_leave(
    open: Option<&mut OpenMap>,
    project: Option<&Project>,
    state: &mut EditorState,
    next: &mut NextState<crate::screen::Screen>,
) {
    // **Both `Option`, because two of the three doors have no map** — the Kit and Rigs doors carry
    // no `OpenMap` at all (`args::Opened::insert_into` removes it). Nothing open is nothing to
    // write, which is the same reason `leave_for_menu` takes a `dirty: bool` rather than a map.
    if let (Some(open), Some(project)) = (open, project)
        && open.dirty()
    {
        match open.save(project) {
            // Through `Project::save`, the same door `Cmd+S` uses: validate, then an atomic write.
            Ok(()) => state.status.note("saved".to_owned()),
            Err(e) => {
                state
                    .status
                    .problem(format!("NOT SAVED: {e} — staying on this map"));
                return;
            }
        }
    }
    next.set(crate::screen::Screen::Project);
}

/// **`Cmd+O` leaves this door for the chooser**, from whichever door is open.
///
/// Its own system rather than a branch in [`keys`], because `keys` is Maps-door furniture — it takes
/// `ResMut<OpenMap>` and `ResMut<Project>` and is registered behind `Door::map_door_is_open` — while
/// `Action::MainMenu` is `Context::Global` in the census and `chrome::shortcut_hint` prints it on
/// every panel. A globally advertised chord handled on one door in three is the dead key `keys.rs`
/// refuses to ship, and it was the only chord out of the Kit door.
///
/// Going back is a **state change** — `Screen::Project`, whose `OnExit(Editor)` tears the door down and
/// drops the project (`screen.rs`). It used to be a process exit carrying a code the parent compared
/// against, because the menu was another program; both screens are one application now.
///
/// It asks rather than leaving: [`leave_for_menu`] raises the unsaved-work question, and
/// [`answer_the_leaving_prompt`] is the one place that answers it.
fn back_to_the_menu(
    keyboard: Res<ButtonInput<KeyCode>>,
    live: Res<keys::Live>,
    open: Option<ResMut<OpenMap>>,
    project: Option<Res<Project>>,
    mut state: ResMut<EditorState>,
    mut next: ResMut<NextState<crate::screen::Screen>>,
) {
    // **An `Esc` prompt still owns the keyboard while it is up.** Answering it is
    // `answer_the_leaving_prompt`'s job, and a chord that reached past it would answer a question
    // the author is still looking at.
    if state.leaving {
        return;
    }
    if keys::just_pressed(&keyboard, *live, Action::MainMenu) {
        let mut open = open;
        save_and_leave(
            open.as_deref_mut(),
            project.as_deref(),
            &mut state,
            &mut next,
        );
    }
}

/// **Put the map back to what its file says**, for the leave question's `D`.
///
/// The file is read again rather than the editor's own record of it (`OpenMap::saved`), because the
/// question is "what is on disk", and a file an agent or the person edited by hand since the last
/// save is the answer to that. A map with no file yet is the empty one `OpenMap::open` makes, which is
/// what discarding a map that was never saved leaves. **A file that does not read is a refusal** and
/// the map is left as it was: a discard that cannot say what it is discarding to must not go.
fn discard_changes(open: &mut OpenMap, project: &Project) -> Result<(), String> {
    let reloaded = OpenMap::open(project, &open.saved().name)?;
    *open = reloaded;
    Ok(())
}

/// **Cross to the other door, keeping the project.**
///
/// **The same verb on both doors.** The key says *"the other door"* and never names a direction — a
/// key that meant "the Map door" would mean the opposite thing on the Map door, which is a mode
/// error wearing a chord. Braem & Egner's *Getting a grip on cognitive flexibility* (2018) is the
/// instruction: a switching cue should be *"a general readiness to switch between tasks rather
/// than to switch to one particular alternative task"*.
///
/// **It asks before it goes, exactly as the menu route does.** Leaving the Map door for the Kit
/// door leaves the map, and the map may hold unsaved edits — so the same question is raised, and
/// the same answer path saves it.
///
/// **A refusal is a refusal.** A crossing that cannot complete says so and **stays put**: the same
/// door, the same project, the same map, the same arm. See [`crate::screen::cross`].
fn switch_door(world: &mut World) {
    // **The key first, and it is read the way every other verb reads it.**
    let pressed = {
        let keyboard = world.resource::<ButtonInput<KeyCode>>();
        let live = *world.resource::<keys::Live>();
        keys::just_pressed(keyboard, live, Action::SwitchDoor)
    };
    if !pressed {
        return;
    }
    // **A prompt still owns the keyboard while it is up.**
    if world.resource::<EditorState>().leaving {
        return;
    }
    cross_to_the_other_door(world);
}

/// **Cross to the other door** — what `=` does and what a click on the strip's name of it does (#288),
/// so the two cannot disagree about which door is the other.
pub fn cross_to_the_other_door(world: &mut World) {
    let Some(door) = world.get_resource::<crate::tiles::Door>().copied() else {
        return;
    };
    let to = match door {
        crate::tiles::Door::Map => crate::tiles::Door::Kit,
        _ => crate::tiles::Door::Map,
    };
    // **The crossing is the operation's**, so the key and the agent take one path. `open_door` runs
    // `door_open`, asks the leaving question when the map is unsaved, and crosses.
    open_door(world, to, None);
}

/// **Answer the leaving question, and nothing else.**
///
/// The keys are `crate::confirm`'s — `Y` proceeds, `N` stops, `Esc` a synonym for `N`, and `D` is the
/// middle answer of a dirty map — so this only routes the answer. It used to own three of its own
/// (`S` save, `D` discard, `Esc` stay) while the chooser answered `Y` and the labeller answered
/// `Enter`; that spread is what `confirm`'s module note is about.
///
/// **`Y` saves and goes on a dirty map, and `D` goes without saving**: the map is read again from
/// its file first ([`discard_changes`]), so what the editor holds on the way out is the file's, not
/// the edit that was refused.
///
/// **Ordering makes the press that raises the question distinct from the press that answers it.**
/// This runs `.before(keys::Phase::Act)`, and the peel that calls `leave_for_menu` runs *in* `Act` —
/// so on the frame `Esc` raises the prompt, this has already run. The modal reads its own keys in
/// the same phase, so the raising press cannot also answer it.
fn answer_the_leaving_prompt(
    project: Option<Res<Project>>,
    open: Option<ResMut<OpenMap>>,
    mut state: ResMut<EditorState>,
    mut next: ResMut<NextState<crate::screen::Screen>>,
    mut confirm: ResMut<crate::confirm::Confirm>,
) {
    let Some(choice) = confirm.answer_choice(crate::confirm::Asked::LeaveMap) else {
        return;
    };
    state.leaving = false;
    if choice == crate::confirm::Choice::No {
        state.status.note("staying on this map".to_owned());
        return;
    }
    let mut open = open;
    if choice == crate::confirm::Choice::Alt
        && let (Some(open), Some(project)) = (open.as_deref_mut(), project.as_deref())
        && let Err(e) = discard_changes(open, project)
    {
        state.status.problem(format!("NOT DISCARDED: {e} — staying on this map"));
        return;
    }
    // **Saving is the going.** `save_and_leave` is the same door `Cmd+O` and the back button walk,
    // so a refused save keeps you here with the reason rather than leaving on a map that was never
    // written — the one path out of this that does not reach the menu. After a discard the map is the
    // file's, so there is nothing to write and it only goes.
    save_and_leave(
        open.as_deref_mut(),
        project.as_deref(),
        &mut state,
        &mut next,
    );
}

/// **Answer the door switch's own question** — and it goes to the **other door**, not the menu.
///
/// **A question of its own**, because the two answers go to different places: leaving goes to the
/// menu and crossing goes to the other door, and one question with two destinations would need
/// the answer to know which question it was answering.
///
/// **Saving is the going**, exactly as it is on the menu's route — so a refused save keeps the
/// author here with the reason, rather than crossing on a map that was never written.
fn answer_the_switch_prompt(world: &mut World) {
    let Some(agreed) = world
        .resource_mut::<crate::confirm::Confirm>()
        .answer(crate::confirm::Asked::SwitchDoor)
    else {
        return;
    };
    // **The request waits in `door_request` while the question is up**, so it is taken here whatever
    // the answer: a decline drops it, and an agreed save crosses to it.
    let request = world.resource_mut::<EditorState>().door_request.take();
    world.resource_mut::<EditorState>().leaving = false;
    if !agreed {
        world
            .resource_mut::<EditorState>()
            .status
            .note("staying on this map".to_owned());
        return;
    }
    // **Save first, and only cross if the save worked.** A refused save must not cross — the
    // author would arrive on the other door with their work still unsaved and now somewhere else.
    //
    // **The map and the project are taken out of the world for the save**, because `OpenMap::save`
    // wants the project and the world will not lend both at once — and `Project` is **not
    // `Clone`**, so it is moved rather than copied. Both go back, whatever the save says.
    let saved = match world.remove_resource::<OpenMap>() {
        Some(mut open) => {
            let project = world.remove_resource::<Project>();
            let result = match &project {
                Some(p) => open.save(p),
                None => Ok(()),
            };
            // **The project goes back first**, and the map second — so a failure leaves the editor
            // exactly as it was.
            if let Some(p) = project {
                world.insert_resource(p);
            }
            match result {
                Ok(()) => {
                    world.insert_resource(open);
                    true
                }
                Err(e) => {
                    // **The map goes back**, and the author stays on it with the reason.
                    world.insert_resource(open);
                    world
                        .resource_mut::<EditorState>()
                        .status
                        .problem(format!("NOT SAVED: {e} — staying on this map"));
                    false
                }
            }
        }
        None => true,
    };
    if !saved {
        return;
    }
    if let Some(request) = request {
        enter_door(world, request);
    }
}

/// **A door an operation asked for, waiting to be crossed to.**
///
/// `door_open` answers with a door and, optionally, a tab. The keys reach the crossing through
/// [`open_door`] at once; an agent's request over BRP is stored in [`EditorState::door_request`] and
/// crossed in the next frame by [`apply_pending_door`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DoorRequest {
    pub door: crate::tiles::Door,
    pub tab: Option<crate::tiles::Mode>,
}

/// **The words for a door that cannot be crossed to yet.** The keys and the agent get the same
/// sentence, so a person and an agent read the same reason.
pub const LEAVING_QUESTION_REFUSAL: &str = "the editor is asking the person a question first (\"Leave this map?\"). The person answers it, then the door can be asked for again.";
/// **A door is already waiting.** The editor crosses to it first; a second request would replace it.
pub const DOOR_ALREADY_WAITING_REFUSAL: &str = "another door is already waiting to be crossed to. The editor crosses to it first, then this door can be asked for.";

/// **Why a door may not be asked for right now, or `None` when it may.** `asking` is whether any
/// question is on screen, the leaving question or another.
pub fn door_refusal(state: &EditorState, asking: bool) -> Option<&'static str> {
    if asking || state.leaving {
        Some(LEAVING_QUESTION_REFUSAL)
    } else if state.door_request.is_some() {
        Some(DOOR_ALREADY_WAITING_REFUSAL)
    } else {
        None
    }
}

/// **Take the door an operation answered with.** A request the editor cannot take is refused with its
/// reason, and nothing is stored over the request already waiting.
pub fn request_door(state: &mut EditorState, after: Option<&serde_json::Value>) {
    if let Some(why) = door_refusal(state, false) {
        state.status.problem(why.to_owned());
        return;
    }
    let Some(after) = after else {
        return;
    };
    let door = after
        .get("door")
        .and_then(serde_json::Value::as_str)
        .and_then(|d| {
            crate::tiles::Door::ALL
                .into_iter()
                .find(|x| x.label().eq_ignore_ascii_case(d))
        });
    let tab = after
        .get("tab")
        .and_then(serde_json::Value::as_str)
        .and_then(|t| {
            crate::tiles::Mode::ALL
                .iter()
                .copied()
                .find(|m| m.label().eq_ignore_ascii_case(t))
        });
    if let Some(door) = door {
        state.door_request = Some(DoorRequest { door, tab });
    }
}

/// **Open a door through the registered `door_open` operation.** The keys and the tab shortcuts call
/// this, so a key and an agent get the same answer about what may be opened.
pub fn open_door(world: &mut World, door: crate::tiles::Door, tab: Option<crate::tiles::Mode>) {
    let params = serde_json::json!({
        "door": door.label().to_lowercase(),
        "tab": tab.map(|m| m.label().to_lowercase()),
    });
    // **Copies, not the world's own.** A door press changes no document. Taking the resources out and
    // putting them back marked both changed, so every watcher of the map saw an edit on each tab press.
    let project = world.get_resource::<Project>().cloned();
    let open = world.get_resource::<OpenMap>().cloned();
    let mut ctx = crate::ops::Ctx {
        view: None, trust_file: None,
        project,
        open,
        source: crate::ops::Source::Human,
        root: None,
        session: crate::ops::session_id(),
        proposal: None,
    };
    let applied = crate::ops::dispatch(&mut ctx, "door_open", &params);
    match applied {
        Ok(crate::ops::Applied::Done { outcome, .. }) => {
            request_door(
                &mut world.resource_mut::<EditorState>(),
                outcome.after.as_ref(),
            );
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
            world.resource_mut::<EditorState>().status.problem(e.0);
        }
    }
    apply_pending_door(world);
}

/// **Carry out the door an agent asked for**, when the editor is not asking a question.
fn apply_pending_door(world: &mut World) {
    // **A question on screen owns the frame (gap 6).** A crossing taken under any question acts on an
    // answer the person has not given, so the stored door waits until the question is answered.
    let asking = world
        .get_resource::<crate::confirm::Confirm>()
        .is_some_and(|c| c.is_open());
    let pending = world
        .get_resource::<EditorState>()
        .is_some_and(|s| s.door_request.is_some() && !s.leaving);
    if pending && !asking {
        apply_door_request(world);
    }
}

/// **Cross to the requested door, or ask the leaving question first** when the map is unsaved.
fn apply_door_request(world: &mut World) {
    let Some(request) = world.resource_mut::<EditorState>().door_request.take() else {
        return;
    };
    let Some(current) = world.get_resource::<crate::tiles::Door>().copied() else {
        return;
    };
    let dirty = world.get_resource::<OpenMap>().is_some_and(|o| o.dirty());
    if request.door != current && dirty {
        // **The request waits for the answer**, and the leaving question owns the next frame.
        world.resource_mut::<EditorState>().door_request = Some(request);
        world.resource_mut::<EditorState>().leaving = true;
        world.resource_mut::<crate::confirm::Confirm>().ask(
            crate::confirm::Asked::SwitchDoor,
            "Leave this map?",
            "It has unsaved edits. Leaving saves them first.",
            "Save and go",
            "Stay",
        );
        return;
    }
    enter_door(world, request);
}

/// **Cross to a door and enter the tab it was asked for.** A refused crossing stops here, so a tab is
/// never entered on a door the author did not reach.
fn enter_door(world: &mut World, request: DoorRequest) {
    // **The receipt is carried across a crossing (cycle 8, gap 7).** The crossing resets
    // `EditorState`, which erased the receipt of whoever asked for the door: an agent's `door_open`
    // changed the window's door with no word of who did it. It is posted again once the tab is
    // entered, so it stays the last word the person reads. Problems are not carried: the journal holds
    // them already. A tab switch with no crossing still clears the note, as it always has.
    let receipt = world.resource::<EditorState>().status.note_text().to_owned();
    let mut crossed = false;
    if world.get_resource::<crate::tiles::Door>().copied() != Some(request.door) {
        match crate::screen::cross(world, request.door) {
            Ok(()) => {
                crossed = true;
                world
                    .resource_mut::<EditorState>()
                    .status
                    .note(format!("the {} door", request.door.label().to_lowercase()));
            }
            Err(e) => {
                world
                    .resource_mut::<EditorState>()
                    .status
                    .problem(format!("cannot cross: {e}"));
                return;
            }
        }
    }
    if let Some(tab) = request.tab {
        use bevy::ecs::system::RunSystemOnce;
        if let Err(e) = world.run_system_once_with(crate::tiles::go_to_tab, tab) {
            world
                .resource_mut::<EditorState>()
                .status
                .problem(format!("cannot enter that tab: {e}"));
        }
    }
    if crossed && !receipt.is_empty() {
        world.resource_mut::<EditorState>().status.note(receipt);
    }
}

/// **Give every back button its click, wherever a panel put one.**
///
/// Four tabs each build their own furniture, so the button is spawned in four places; hanging the
/// observer here rather than at each spawn site means a fifth tab gets a working button by calling
/// `chrome::back_button` and nothing else. `Added` fires once per entity, so this is a no-op on
/// every frame after the panels are built.
fn wire_back_buttons(
    mut commands: Commands,
    fresh: Query<Entity, Added<crate::chrome::BackButton>>,
) {
    for e in &fresh {
        commands.entity(e).observe(back_button_clicked);
    }
}

/// Hang the click on whichever panel spawned the button. An observer rather than a polling system,
/// so it costs nothing on the frames nobody clicks.
fn back_button_clicked(
    _: On<bevy::picking::events::PointerClick>,
    open: Option<ResMut<OpenMap>>,
    project: Option<Res<Project>>,
    mut state: ResMut<EditorState>,
    mut next: ResMut<NextState<crate::screen::Screen>>,
) {
    // **The same door the chord uses.** A click on `‹ kits & maps` is as deliberate as `Cmd+O`, so
    // it saves and goes rather than asking — see [`save_and_leave`] for why the question went away.
    let mut open = open;
    save_and_leave(
        open.as_deref_mut(),
        project.as_deref(),
        &mut state,
        &mut next,
    );
}

/// **The Map's left panel scrolls, and this is the pane that does it.** See [`spawn_panel`] for why
/// it exists at all: this was the one panel in the editor with no scroll area anywhere in it.
///
/// `pub` because a test has to name it: ten scroll areas exist in the editor, and "the leftmost one" is
/// a guess that picks a different pane on another tab (#272).
#[derive(Component)]
pub struct MapControlsPane;

/// Build the panel's fixed furniture. The palette itself is `rebuild_palette`'s, which is why neither
/// the project nor the thumbnails are read here — they were parameters that had stopped being used.
fn spawn_panel(mut commands: Commands, frame: Res<crate::chrome::Frame>) {
    let root = crate::chrome::panel_root(
        &mut commands,
        &frame,
        crate::chrome::Side::Left,
        crate::chrome::CONTROLS_W,
        // **`true`, where the Map alone passed `false`.** It was the only content-sized panel in
        // the editor, which is half of why it could not scroll — a `max_height` inside a panel that
        // is not pinned to the bottom is never reached, the same failure the palette was fixed out
        // of. One left width and every panel scrollable is the 2026-09-03 decision (F15, D8).
        true,
        false,
        // What a screen reader calls this panel (UI-30, #224): the heading it draws.
        "INSPECTOR",
    )
    .insert(crate::tiles::MapRoot)
    .id();

    commands.entity(root).with_children(|p| {
        // **`INSPECTOR`, not `GRIMOIRE MAPPER`.**
        //
        // This slot held the application's own name while every other panel in the editor named a
        // job — the 2026-09-03 audit's F12, answered by D9: a panel is named for what it holds,
        // **left = the thing being inspected, right = the list**. The window says which application
        // this is twice already (its title and the chrome bar), so the name here was the one word
        // on screen that told the author nothing about what was under it.
        //
        // `title` rather than `list_heading`, which is the other half of the same rule — `title`
        // for every left dock, `list_heading` for every right dock. The argument is written out
        // once, at `tiles::spawn_tiles_panel`.
        crate::chrome::title(p, "INSPECTOR");

        // **The body scrolls, through the shared builder.**
        //
        // Nothing in this panel could scroll, so the readout, the map-size fields and anything ever
        // added below them simply ran off the bottom of the window. `chrome::scroll_list` is the
        // only way to get one — it carries the `ScrollArea` the wheel handler reads and the bar
        // with its own right gutter, which is what `nobody_spawns_their_own_scroll_container`
        // exists to enforce.
        //
        // FOLLOW-OK: a readout and a field block, not a list. Nothing in here is walked by the
        // arrows, so there is no selection to keep on screen — the scroll exists for HEIGHT. The
        // Map's one walked list is `PaletteList`, in the panel across the screen, and that one has
        // `keep_palette_selection_on_screen`. The one thing here that does move under the keys is
        // the note box's text in LAST, and `keep_note_box_on_screen` scrolls that row back (#177).
        crate::chrome::scroll_list(p, MapControlsPane).with_children(|p| {
            // The readout, in the same two columns as the keys so the whole panel shares one left
            // edge.
            p.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    // `GAP_TIGHT`, where this was a bare 1: the rows of one block belong to each
                    // other, and 1 px is not a step on any scale. The margin that used to sit above
                    // this block is gone with it — `chrome::title` carries the gap below itself now.
                    row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                    ..default()
                },
                StatusBlock,
                crate::notice::CopyPane(&[crate::tiles::Mode::Map]),
                // The Map's own text pane — where `Cmd+C` copies from, and so where its badge goes.
                crate::chrome::Control(crate::keys::ControlId::Detail),
            ))
            .with_children(|s| {
                for field in [
                    Field::Name,
                    // **The rule seed (E-081) sits under the name**, because both are one value the
                    // author types about the whole map: what it is called, and what generates it.
                    Field::Rule,
                    Field::Brush,
                    Field::Yaw,
                    Field::Map,
                    Field::Under,
                    // **The hint is the receipt's second line** (B-013), not a row of its own. The
                    // receipt's row already reserves two lines, so a hint takes a line that is
                    // there and nothing below it moves while no hint shows.
                    Field::Last,
                    // **After the receipt, not above it.** The receipt carries its own LAST label now,
                    // so it no longer needs the row above to lend it one. The pair still reads as what
                    // happened (the receipt) and then what Cmd+Z / Cmd+Shift+Z would do about it.
                    Field::Undo,
                    Field::Redo,
                    Field::Edges,
                    // **Under EDGES**: both are standing readouts of the map as a whole.
                    Field::Repeats,
                    Field::Relations,
                ] {
                    // **The history list sits under REDO and above EDGES (E-018).** Its rows are
                    // fixed in number, so the block keeps one height whatever the history holds.
                    if field == Field::Edges {
                        history_block(s);
                    }
                    let mut spawned = s.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        // The receipt starts under the row above it, not floating mid-box when it
                        // is one line inside its two-line room.
                        align_items: match field {
                            Field::Last => AlignItems::FlexStart,
                            _ => AlignItems::Center,
                        },
                        // **The receipt row keeps its room when it is empty** (B-009). It was zero
                        // tall until a note arrived and then grew to the note, so every receipt
                        // moved MAP SIZE, the size fields and the composition list down two lines
                        // and back, often under the pointer that had just typed into a field.
                        // Two lines, because 165 of the editor's 174 receipt sites are 80
                        // characters or fewer and the value column holds about 42 per line. A
                        // longer receipt still grows rather than being cut short: the words matter
                        // more than the stillness.
                        // Rounded up to a whole pixel: the text's measured box is rounded up, and
                        // an exact reserve laid out one pixel short of a two-line receipt.
                        min_height: match field {
                            Field::Last => {
                                Val::Px((RECEIPT_LINES * crate::chrome::BODY_LINE_H).ceil())
                            }
                            _ => Val::Auto,
                        },
                        ..default()
                    });
                    if let Some(id) = field.control() {
                        spawned.insert(crate::chrome::Control(id));
                    }
                    spawned.with_children(|row| {
                        if field == Field::Last {
                            // **The hint takes the receipt's slot while it speaks** (B-013). The label
                            // is the one that changes with it: `LAST` for a receipt, `HINT` for a hint.
                            crate::chrome::row_label_with(
                                row,
                                READOUT_LABEL_W,
                                field.label(),
                                ReceiptLabel,
                            );
                            row.spawn(Node {
                                flex_direction: FlexDirection::Column,
                                flex_grow: 1.0,
                                min_width: Val::Px(0.0),
                                ..default()
                            })
                            .with_children(|col| {
                                crate::chrome::row_value(col, "", TEXT, Field::Last);
                            });
                        } else {
                            // `READOUT_LABEL_W` holds the longest single word here (`RELATIONS`), and
                            // `UNDER CURSOR` stands on two lines inside it, so every value starts at
                            // the same place and none touches its label (#302).
                            if field == Field::Under {
                                crate::chrome::row_label_wrapping(row, READOUT_LABEL_W, field.label());
                            } else {
                                crate::chrome::row_label(row, READOUT_LABEL_W, field.label());
                            }
                            crate::chrome::row_value(row, "", TEXT, field);
                        }
                    });
                }
            });

            // **Map size.** Stated, adjustable, and drawn in the world — an edge nothing shows is an
            // edge nobody believes. It is also what gives the flood fill somewhere to stop.
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            })
            .with_children(|s| {
                // A real `section`, not a hand-rolled 10 px twin — the block's separation now comes
                // from the heading's own margins (the 2026-08-17 type-role decision).
                crate::chrome::section(s, "MAP SIZE  (m)");
                for axis in [Axis::X, Axis::Y, Axis::Z] {
                    s.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(crate::chrome::GAP_TIGHT),
                        ..default()
                    })
                    .with_children(|row| {
                        // `COL_TIGHT`: the label is a single axis letter, which is exactly what that
                        // column is for. It was a bare 14.
                        crate::chrome::row_label(row, crate::chrome::COL_TIGHT, axis.label());
                        // **A field, not a pair of nudges.** Stepping to 48 m from 32 was four
                        // clicks and no way to say the number; the author knows the size they want,
                        // so the control should let them state it. Click to focus, type digits,
                        // Enter to keep.
                        //
                        // `chrome::NUM_FIELD_W`, where this was a bare 56 against the Tiles tab's
                        // bare 62 — one idea, two numbers, one per file.
                        crate::chrome::text_field(
                            row,
                            Val::Px(crate::chrome::NUM_FIELD_W),
                            SizeField(axis),
                            crate::chrome::text::BODY,
                            (String::new(), TEXT),
                            SizeReadout(axis),
                        );
                    });
                }
            });

            // **Family settings (#96).** One line: `<crate> <field> <value>`, e.g.
            // `bevy_elements sun.hour 6.5`. Enter writes it through `family_set`.
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            })
            .with_children(|s| {
                crate::chrome::section(s, "FAMILY SETTINGS");
                crate::chrome::text_field(
                    s,
                    Val::Percent(100.0),
                    FamilyField,
                    crate::chrome::text::BODY,
                    ("crate field value".to_owned(), TEXT),
                    FamilyReadout,
                );
            });

            // **Play (#97):** start the project's game on the open map at the camera, and stop it.
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            })
            .with_children(crate::ops_play::spawn_section);

            // **Environment (#98):** clock scrubber and forced weather, for a project that uses `bevy_elements`.
            #[cfg(feature = "bevy_elements")]
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            })
            .with_children(crate::environment::spawn_section);

            // **Lights (#103):** the pool's budget from where the camera looks.
            #[cfg(feature = "bevy_lights")]
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            })
            .with_children(crate::ops_pool::spawn_section);

            // **Audio (S-01):** `bevy_siren`'s buses and cues, with an audition and its log.
            #[cfg(feature = "bevy_siren")]
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            })
            .with_children(crate::audio_panel::spawn_section);

            // **Light map (#103):** show or hide `bevy_light_grid`'s field over the open map.
            #[cfg(feature = "bevy_light_grid")]
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            })
            .with_children(crate::light_overlay::spawn_section);

            // **Terrain (#99).** One line: `<brush> <x> <z> <radius_m> <strength>`, e.g.
            // `raise 12 8 6 2`. Enter sculpts the open map's ground through `terrain_stroke`.
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            })
            .with_children(|s| {
                crate::chrome::section(s, "TERRAIN");
                crate::chrome::text_field(
                    s,
                    Val::Percent(100.0),
                    TerrainField,
                    crate::chrome::text::BODY,
                    ("brush x z radius strength".to_owned(), TEXT),
                    TerrainReadout,
                );
            });

            // **Crowd (#107).** One line: `<from_x> <from_z> <to_x> <to_z> <walkers>`, e.g. `-8 -8 8 8 40`.
            // Enter walks that many people between two 2 m squares round the open map's solid pieces.
            #[cfg(all(feature = "bevy_orca", feature = "bevy_stigmergy"))]
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            })
            .with_children(|s| {
                crate::chrome::section(s, "CROWD");
                crate::chrome::text_field(
                    s,
                    Val::Percent(100.0),
                    CrowdField,
                    crate::chrome::text::BODY,
                    ("from_x from_z to_x to_z walkers".to_owned(), TEXT),
                    CrowdReadout,
                );
            });

            // **Data files (#86).** One line: `<file> <path> <value>`, e.g. `tuning/mob speed 2.5`.
            // Enter writes it through `data_set`.
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            })
            .with_children(|s| {
                crate::chrome::section(s, "DATA");
                crate::chrome::text_field(
                    s,
                    Val::Percent(100.0),
                    DataField,
                    crate::chrome::text::BODY,
                    ("file path value".to_owned(), TEXT),
                    DataReadout,
                );
                s.spawn((
                    DataList,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                        ..default()
                    },
                ));
            });

            // **Scripts (#105).** Every trigger of the open map; a row opens the terrain line to edit it.
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            })
            .with_children(|s| {
                crate::chrome::section(s, "SCRIPTS");
                s.spawn((
                    ScriptsList,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                        ..default()
                    },
                ));
            });

            // **Notes (#90).** Every note of the open map, newest first; a row flies to it.
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            })
            .with_children(|s| {
                crate::chrome::section(s, "NOTES");
                s.spawn((
                    NotesList,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                        ..default()
                    },
                ));
            });

            // **Reload (#203).** What became of each file's latest edit: whether a game is connected, and
            // for every file a game or the editor has news on, applied, refused (with why), pending or
            // needs restart, and how long ago.
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            })
            .with_children(|s| {
                crate::chrome::section(s, "RELOAD");
                s.spawn((
                    ReloadList,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                        ..default()
                    },
                ));
            });

            // **Plan (E-069).** What the agent means to do on this map and how far it has got: the
            // `.plan.ron` that `plan_set` writes, read like the notes are.
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            })
            .with_children(|s| {
                crate::chrome::section(s, "PLAN");
                s.spawn((
                    PlanList,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                        ..default()
                    },
                ));
            });

            // **The composition picker** — the Map door's own way to arm a composition.
            //
            // It is here, in the INSPECTOR panel, because the INSPECTOR is the panel that says
            // *what is in hand* — the brush, the yaw, the thing under the cursor — and the arm is
            // exactly that: what the next click will put down. Before this, the arm was reachable
            // only from the COMPOSE tab, which is on the Kit door, while the click that stamps is
            // on the Map door; the two never met. See `compose::picker` for the measurement.
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            })
            .with_children(|s| {
                crate::chrome::section(s, "COMPOSITIONS");
                // **No scroll list of its own.** The panel's own body is already a scroll area, and
                // a scroll inside a scroll is two wheels fighting over one gesture — the author
                // scrolls the picker and the panel does not move, which reads as a stuck panel.
                // The picker's rows are part of the panel's flow, and the panel scrolls them.
                //
                // **No `ControlId` of its own, and that is the ratchet's doing.** The panel's
                // status block already claims `ControlId::Detail`, and
                // `every_control_the_census_homes_a_verb_at_is_on_screen` fails on *two* visible
                // nodes claiming one id — so a second `Detail` here would take the badge away from
                // the block that has it. The picker's verbs are the panel's verbs; one control, one
                // badge, one answer to "what can I do here".
                s.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                        ..default()
                    },
                    crate::compose::picker::PickerBody,
                ));
            });
        });
    });
}

/// **The palette, in its own panel against the right edge.**
///
/// It used to be the last child of the panel above, which meant it was handed whatever vertical space
/// the keys, the status block and the map-size rows had not already taken. At `UiScale(1.2)` on a
/// 1280×802 logical screen that is 668 UI px of budget against ~447 px of everything else — so the
/// list rendered *two* rows and the rest ran off the bottom of the screen. The `max_height` it carried
/// was never reached and never did anything.
///
/// A panel pinned to BOTH insets cannot have that failure: `top` and `bottom` together give the node a
/// height taken from the viewport rather than from its content, so the list has a bottom to stop at
/// and the scroll area is bounded by construction. Nothing above it can push it off-screen, because
/// there is no longer anything above it.
///
/// Right edge rather than a second column beside the keys: the left panel keeps its width, and the map
/// stays visible in the band between them, which is the thing being authored.
fn spawn_palette_panel(
    mut commands: Commands,
    frame: Res<crate::chrome::Frame>,
    // **A list heading's ink, from the editor's skin** (UI-02, #210).
    skin: Option<Res<bevy_gooey::GooeySkin>>,
) {
    let heading = crate::chrome::ListHeadingInk::of(skin.as_deref());
    crate::chrome::panel_root(
        &mut commands,
        &frame,
        crate::chrome::Side::Right,
        LIST_W,
        // Pinned top AND bottom — the whole point of this panel.
        true,
        false,
        // What a screen reader calls this panel (UI-30, #224): the heading it draws.
        "PLACE",
    )
    // Tagged like the panel above so `tiles::apply_mode` hides it with the rest of the map tab — it
    // iterates every `MapRoot`, so a second one needs no change there. Without this the palette would
    // sit over the tiles tab offering pieces that tab cannot place.
    .insert(crate::tiles::MapRoot)
    .with_children(|p| {
        crate::chrome::list_heading_in(p, "PLACE", heading);
        crate::filter::spawn(p, crate::filter::Pane::Palette, None);
        crate::chrome::scroll_list(p, (PaletteList, crate::chrome::Control(crate::keys::ControlId::Palette)));
    });
}

/// What the Map palette is built from, read as one param so the palette systems stay within Bevy's
/// parameter ceiling. `palette_indices` and `palette_categories` take these four resources.
#[derive(bevy::ecs::system::SystemParam)]
struct PaletteSource<'w> {
    project: Res<'w, Project>,
    open: Res<'w, OpenMap>,
    filters: Res<'w, crate::filter::Filters>,
    fold: Res<'w, Folded>,
}

/// **Did the filter's *text* change, or only which box has the keyboard?** (#278)
///
/// `on_row_click` blurs the box after arming, and `blur` writes `Filters`. With a plain
/// `resource_changed` that write rebuilt every row — so the click that armed a row also despawned it.
/// Measured: the first tap after typing cost exactly **1** rebuild and armed nothing; the second cost
/// **0** and armed the piece. Comparing the texts instead means a click that only moves the keyboard
/// leaves the rows standing, which is what `arm_palette_rows` was built to do.
fn the_filter_text_changed(filters: Res<crate::filter::Filters>, mut last: Local<Option<FilterTexts>>) -> bool {
    if !filters.is_changed() {
        return false;
    }
    let now = crate::filter::filter_texts(&filters);
    if last.as_ref() == Some(&now) {
        return false;
    }
    *last = Some(now);
    true
}


/// **A map opens with the palette's first row armed** (#289).
///
/// `EditorState` arms library row 0 so the first click places something, but the palette groups
/// rows by kind, so row 0 can be drawn anywhere: the bedroom's sat half-way down, and
/// [`keep_palette_selection_on_screen`] scrolled the list there on open. Arming the row drawn first
/// keeps the first click useful and opens the list at its top.
fn arm_the_first_shown_piece(palette: PaletteSource, mut state: ResMut<EditorState>) {
    let PaletteSource {
        project,
        open,
        filters,
        fold,
    } = palette;
    state.brush = palette_indices(&project, &open, &fold, &filters).first().copied();
}

/// **The palette, grouped.**
///
/// Forty-one rows is a list you scroll past; grouped by what a thing IS it becomes a handful of
/// headings you can skip. The grouping is the `kind` axis of the project's own vocabulary rather than
/// a second taxonomy invented for the panel — `docs/ui.md` §1.2 names over-informing as the failure
/// mode, and two competing category systems is the version of that which also makes people wrong.
///
/// Categories appear in VOCABULARY order and never in frequency or recency order. Samp 2011, via
/// §3.5: a menu's cost is paid at first sight, so fix positions permanently — a palette that
/// reshuffles as a map fills is one nobody builds a memory of.
fn rebuild_palette(
    mut commands: Commands,
    palette: PaletteSource,
    thumbs: Option<Res<crate::thumbs::Thumbnails>>,
    // Read for one fact only — which row is armed — so the row spawns already selected rather than
    // waiting a frame for [`arm_palette_rows`]. It is deliberately NOT in the run condition above:
    // arming a brush must not respawn a hundred and eighty rows.
    state: Res<EditorState>,
    lists: Query<Entity, With<PaletteList>>,
    order: Option<ResMut<crate::thumbs::ThumbOrder>>,
) {
    let PaletteSource {
        project,
        open,
        filters,
        fold,
    } = palette;
    // **The booth bakes what the palette shows, in the order it shows it**, so the rows an author is
    // looking at fill in before the ones under a folded heading. Written here and only here: this
    // runs when the palette is rebuilt, which is exactly when that order can have changed.
    if let Some(mut order) = order {
        order.0 = palette_indices(&project, &open, &fold, &filters)
            .into_iter()
            .filter_map(|ix| project.library.descriptors.get(ix).map(|d| d.id.clone()))
            .collect();
    }
    for list in &lists {
        commands.entity(list).despawn_related::<Children>();
        commands.entity(list).with_children(|p| {
            // **A library with no pieces says so, and where to get some** (#301): the same sentence and button as the map's own empty state.
            if project.library.descriptors.is_empty() {
                p.spawn(bevy_gooey::label(crate::add_meshes::NO_PIECES));
                p.spawn((bevy_gooey::button(crate::add_meshes::ACTION), crate::add_meshes::AddMeshesButton));
            }
            for (category, members) in palette_categories(&project, &open, &filters) {
                let folded = fold.0.contains(&category);
                p.spawn((
                    // It keeps `HEADER_BG` because it is a signpost over rows rather than one of
                    // them, and carries `RowRest` so the five states still come from
                    // `chrome::style_list_rows`.
                    // CHROME-OK: a group band, not a list row.
                    UiButton,
                    Hovered::default(),
                    CategoryHeader(category.clone()),
                    // The band's rest fill, carried so `chrome::style_list_rows` gives it the same
                    // five states every row and chip has. It sensed `Hovered` and nothing repainted
                    // it.
                    crate::chrome::RowRest(HEADER_BG),
                    Node {
                        width: Val::Percent(100.0),
                        padding: CHIP_PAD,
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(crate::chrome::GAP_ROW),
                        margin: UiRect::top(Val::Px(crate::chrome::GAP_ROW)),
                        ..default()
                    },
                    BackgroundColor(HEADER_BG),
                ))
                .with_children(|row| {
                    // A caret, so folded and unfolded differ by shape and not only by what is below
                    // them — an encoding that survives being glanced at.
                    row.spawn((
                        Node {
                            width: Val::Px(crate::chrome::COL_TIGHT),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        Text::new(if folded { ">" } else { "v" }),
                        TextColor(LABEL),
                        crate::chrome::font(crate::chrome::text::LABEL),
                    ));
                    row.spawn((
                        Node {
                            flex_grow: 1.0,
                            // CHROME-OK: zero, not a spacing step — see `tiles::count_cell`.
                            min_width: Val::Px(0.0),
                            ..default()
                        },
                        Text::new(category.to_uppercase()),
                        TextColor(LABEL),
                        crate::chrome::font(crate::chrome::text::LABEL),
                    ));
                    // The count in a non-wrapping column of its own, through the one builder — F6
                    // was this same defect on the Kit door's pack headings.
                    crate::tiles::count_cell(row, format!("{}", members.len()), LABEL);
                });

                if folded {
                    continue;
                }
                for ix in members {
                    let Some(d) = project.library.descriptors.get(ix) else {
                        continue;
                    };
                    // **`chrome::list_row`, where this was a hand-rolled `UiButton` at
                    // `axes(8, 4)`.**
                    //
                    // It carried no `RowRest`, so the shared repainter could not see it and a
                    // private `style_rows` restated the whole rest/hover/selected priority one file
                    // away from the four other lists that take it from `chrome` — the 2026-09-03
                    // audit's F11. `style_rows` is deleted; the fill, the hover, the press and the
                    // accent selection rail all come from the builder now, and what is left of it
                    // is [`arm_palette_rows`], which says only *which* row is armed.
                    //
                    // `align_items` is the one thing this row wants that a list row does not
                    // assume: a fixed-height thumbnail beside auto-height text has to be centred,
                    // and it is set through `entry` rather than a second `Node`, because a bundle
                    // carrying two of the same component panics at spawn in Bevy 0.19.
                    let mut palette_row =
                        crate::chrome::list_row(p, state.brush == Some(ix), PaletteRow(ix));
                    palette_row
                        .entry::<Node>()
                        .and_modify(|mut n| n.align_items = AlignItems::Center);
                    // **The id and the count's unit, where a person can find them** (#289): the row
                    // reads as words, and two kits may each have a `wall` that both read "Wall".
                    let tris = project.triangles.get(ix).copied().unwrap_or(0);
                    palette_row.insert(bevy_gooey::Tooltip::new(palette_tooltip(&d.id, tris)));
                    // **A library piece can be carried to the map** (UI-52, #238): the row is a drag source whose
                    // payload is the descriptor's id, and a drop on the world places it ([`drop_a_piece_on_the_map`]).
                    // A press that does not move six pixels is still the click that arms the brush.
                    palette_row.insert(bevy_gooey::DragPayload(PieceDrag(d.id.clone())));
                    palette_row.with_children(|row| {
                            let mut slot = row.spawn((
                                Node {
                                    width: Val::Px(crate::chrome::THUMB_SLOT),
                                    height: Val::Px(crate::chrome::THUMB_SLOT),
                                    margin: UiRect::right(Val::Px(crate::chrome::GAP_ROW)),
                                    flex_shrink: 0.0,
                                    ..default()
                                },
                                BackgroundColor(SLOT_BG),
                            ));
                            // **By id, not by index.** The library grows on Accept and shrinks on
                            // remove, and an index into a portrait list that was sized at startup
                            // showed a blank tile in the first case and the neighbour's picture in
                            // the second.
                            if let Some(image) = thumbs
                                .as_ref()
                                .zip(project.library.descriptors.get(ix))
                                .and_then(|(t, d)| t.image(&d.id))
                            {
                                slot.insert(ImageNode::new(image));
                            }
                            row.spawn((
                                Node {
                                    flex_grow: 1.0,
                                    // See `tiles::count_cell`: the name wraps inside its own column
                                    // rather than pushing the cost out of the row.
                                    // CHROME-OK: zero is not a spacing step.
                                    min_width: Val::Px(0.0),
                                    overflow: Overflow::clip_x(),
                                    ..default()
                                },
                                Text::new(bevy_grimoire_core::naming::display_name(&d.id)),
                                TextColor(TEXT),
                                crate::chrome::font(crate::chrome::text::BODY),
                                // **A word longer than the column breaks inside itself** (#289): with
                                // the default word-boundary break, `toilet_roll_holder` was one word
                                // the row could not wrap, and it drew over the count beside it.
                                TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
                            ));
                            crate::tiles::count_cell(row, brief_count(tris), cost_tint(tris));
                    });
                }
            }
        });
    }
}

/// The library grouped by its primary `kind`, in vocabulary order.
///
/// A descriptor's FIRST kind token is its category. Most carry exactly one — the converter maps the
/// old `category` field straight across — and for the few that carry several, the first is the one
/// the author wrote first, which is a better guess at "what this mainly is" than any rule this
/// function could invent.
/// **What the palette shows, filtered, in the order it shows it.**
///
/// The one answer to that question, asked by the panel that draws the rows and by the arrows that
/// walk them. Two copies of this rule would drift the moment a filter changed — `brush_at` is the
/// precedent, and its note records what that drift cost: the ghost and the commit sat on lattices
/// half a tile apart because four call sites were threaded by hand.
///
/// Folding is **not** applied here: the panel needs a folded category's member count to print it,
/// and only the walk needs it gone. [`palette_indices`] is where that happens.
fn palette_categories(project: &Project,
    open: &OpenMap,
    filters: &crate::filter::Filters,
) -> Vec<(String, Vec<usize>)> {
    let mut out = categories(project);
    // **The map's kit selection, folded in with the text filter.** One `retain`, because both answer
    // the same question — "is this row offered right now" — and two passes over one list would be
    // two places to forget. `Project::palette_namespaces` has already folded in whatever the map
    // uses, so nothing on screen can vanish from under a placement.
    let offered = open.palette_namespaces(project);
    for (_, members) in out.iter_mut() {
        // **The filter narrows; it never reorders.** Rows that survive keep the positions they had,
        // so what an author learned about where a piece sits is still true — Samp 2011, via
        // `docs/ui.md` §3.5.
        members.retain(|ix| {
            project
                .library
                .descriptors
                .get(*ix)
                .is_some_and(|d| {
                    filters.keeps(crate::filter::Pane::Palette, &d.id)
                        // **Which kit DEFINES it**, not what its id spells. Reading the namespace
                        // out of the id made this inert for every flat library — see
                        // `Project::kit_of`. A piece no bound kit defines is offered rather than
                        // hidden: it can only have come from somewhere this filter does not model,
                        // and hiding it would be this control deciding something it was not given.
                        && project
                            .kit_of(&d.id)
                            .is_none_or(|k| offered.contains(k))
                })
        });
    }
    // A heading with nothing under it is a heading about nothing.
    out.retain(|(_, members)| !members.is_empty());
    out
}

/// **Every palette row an author can reach right now**, flattened in screen order.
///
/// What the arrows walk. A folded category's members are not on screen, so stepping onto one would
/// arm a brush whose row cannot be seen — the selection and the picture would disagree, which is the
/// same class of fault `keep_*_selection_visible` exists to prevent one tab over.
pub fn palette_indices(
    project: &Project,
    open: &OpenMap,
    fold: &Folded,
    filters: &crate::filter::Filters,
) -> Vec<usize> {
    palette_categories(project, open, filters)
        .into_iter()
        .filter(|(category, _)| !fold.0.contains(category))
        .flat_map(|(_, members)| members)
        .collect()
}

/// The palette's headings, in the order they are drawn, empty ones left out. Public so a test can
/// read what the panel will say without finding it in the UI tree.
pub fn palette_headings(project: &Project) -> Vec<String> {
    categories(project).into_iter().map(|(name, _)| name).collect()
}

/// **The palette's groups, in drawn order.** Headed by kind alone while the project binds one kit,
/// and by `kit · kind` once it binds two or more: a project that mixes a first-party kit with an
/// abstract tile set would otherwise show `building-module` and `landmark` with both kits' pieces
/// interleaved under them. Kits go in `kits.ron` bind order, and within one the vocabulary's own
/// kind order, then `unsorted` last. A piece no bound kit defines sorts under an `unsorted` kit,
/// last of all.
fn categories(project: &Project) -> Vec<(String, Vec<usize>)> {
    let by_kit = project.kits.len() >= 2;
    let kits: Vec<&str> = if by_kit {
        project
            .kits
            .iter()
            .map(|k| k.namespace.as_str())
            .chain(std::iter::once(UNSORTED))
            .collect()
    } else {
        vec![""]
    };
    let heading = |kit: &str, kind: &str| {
        if by_kit {
            format!("{kit} · {kind}")
        } else {
            kind.to_owned()
        }
    };
    let mut out: Vec<(String, Vec<usize>)> = Vec::new();
    for kit in &kits {
        for kind in project.vocab.kind.names() {
            out.push((heading(kit, kind), Vec::new()));
        }
        // A home for anything untagged, LAST — it is a to-do list, not a category, and putting it
        // first would give the least useful group the most valuable position.
        out.push((heading(kit, UNSORTED), Vec::new()));
    }
    let at: std::collections::HashMap<String, usize> = out
        .iter()
        .enumerate()
        .map(|(i, (name, _))| (name.clone(), i))
        .collect();

    for (ix, d) in project.library.descriptors.iter().enumerate() {
        let kit = if by_kit {
            project.kit_of(&d.id).unwrap_or(UNSORTED)
        } else {
            ""
        };
        let kind = d.kind.first().map(String::as_str).unwrap_or(UNSORTED);
        // A token the vocabulary does not have cannot reach here — `Library::resolve` refuses it at
        // load — but grouping it visibly beats dropping the row from the palette entirely.
        let slot = at
            .get(&heading(kit, kind))
            .or_else(|| at.get(&heading(kit, UNSORTED)));
        if let Some(&slot) = slot {
            out[slot].1.push(ix);
        }
    }
    // Empty categories are noise: the vocabulary has sixteen and a project may use six.
    out.retain(|(_, members)| !members.is_empty());
    out
}

/// Fold or unfold a category.
fn on_category_click(
    activate: On<Activate>,
    headers: Query<&CategoryHeader>,
    fold: Option<ResMut<Folded>>,
) {
    // **`Option`, because this is a GLOBAL observer** — it fires for any `Activate` anywhere in
    // the application, and in Bevy 0.19 a missing resource panics at param validation rather
    // than skipping. See [`on_cell_verb`] for the whole argument; the ratchet is
    // `every_resource_says_what_a_door_does_to_it.rs`.
    let Some(mut fold) = fold else {
        return;
    };
    let Ok(header) = headers.get(activate.entity) else {
        return;
    };
    if !fold.0.remove(&header.0) {
        fold.0.insert(header.0.clone());
    }
}

/// **Choose what to place, from the keyboard.**
///
/// The Map tab had no way to do this. `EditorState::brush` had a single writer — [`on_row_click`],
/// below — so arming a brush meant reaching for the pointer, on the tab the code calls *"the job"*
/// and in a tool whose brief was *"this should be done by the keyboard, as key strokes are faster."*
///
/// It walks [`palette_indices`], which is what the panel draws, so the selection can never step onto
/// a row that is filtered out or folded away. Held arrows repeat at the shared
/// [`keys::REPEAT_SECS`] cadence, and `Shift` is the same five-row stride every other list here uses.
fn walk_palette(
    keyboard: Res<ButtonInput<KeyCode>>,
    live: Res<keys::Live>,
    time: Res<Time>,
    mut repeat: ResMut<keys::Repeat>,
    mut state: ResMut<EditorState>,
    palette: PaletteSource,
) {
    let PaletteSource {
        project,
        open,
        filters,
        fold,
    } = palette;
    let dt = time.delta_secs();
    // One `repeating` call per direction, never two OR'd together: `Repeat` carries a single
    // countdown, so asking it about two actions in one frame has the second reset the first's
    // cadence. The Meshes tab paid for this already.
    let down = keys::repeating(&keyboard, *live, keys::Action::PaletteNext, &mut repeat, dt);
    let up = keys::repeating(&keyboard, *live, keys::Action::PalettePrev, &mut repeat, dt);
    if !down && !up {
        return;
    }

    let visible = palette_indices(&project, &open, &fold, &filters);
    if visible.is_empty() {
        // A refusal, not silence: an empty palette after a filter looks exactly like a dead key.
        state
            .status
            .problem("no palette rows to walk — the filter hides them all".to_owned());
        return;
    }

    let stride = if keys::shift_held(&keyboard) { 5 } else { 1 };
    let at = state
        .brush
        .and_then(|b| visible.iter().position(|ix| *ix == b));
    let want = match at {
        Some(at) if down => at + stride,
        Some(at) => at.saturating_sub(stride),
        // **Nothing armed, or armed to something not on screen** — an arrow then means "start from
        // the end of the list I am moving towards", which is where the eye already is. Same rule the
        // candidate list follows when its selection has been filtered away.
        None if down => 0,
        None => visible.len().saturating_sub(1),
    };
    let Some(&next) = visible.get(want.min(visible.len().saturating_sub(1))) else {
        return;
    };
    // **The walk is the `brush_arm` operation, the same one a click is** (gap 1). It used to set the
    // brush and the tool itself, so the log never heard of it. `arm_piece` dispatches the arm as the
    // person, applies its answer, and returns to placing, exactly as a click does.
    let Some(id) = project.library.descriptors.get(next).map(|d| d.id.clone()) else {
        return;
    };
    arm_piece(&mut state, &project, Some(&*open), &id);
}

/// One observer for the whole palette. `Activate` carries the entity, so the index lives on the row
/// as a component and there is no macro-unrolled observer per row.
/// **Every resource here is an `Option`, and that is not defensive — it is required.**
///
/// This is a **global** observer: it fires for any `Activate` anywhere in the application, and it
/// takes the Map door's resources. That was survivable while `chrome::list_row` was only ever called
/// by editor panels. The moment the **menu** adopted the shared row vocabulary it spawned rows that
/// carry `Button` too, and the first click on one panicked the whole application — `Res<Project>`
/// does not exist on `Screen::Project`, and in Bevy 0.19 a missing `Res<T>` panics rather than skipping
/// (`CLAUDE.md`; `bevy_ecs/error/handler.rs:130`).
///
/// The row query already answers "is this mine" — a menu row has no `PaletteRow` — so the guard was
/// there and simply ran too late, after parameter validation. Found by adopting the vocabulary,
/// which is the sort of thing sharing a builder is supposed to surface.
fn on_row_click(
    activate: On<Activate>,
    rows: Query<&PaletteRow>,
    project: Option<Res<Project>>,
    state: Option<ResMut<EditorState>>,
    filters: Option<ResMut<crate::filter::Filters>>,
    open: Option<Res<crate::project::OpenMap>>,
) {
    let Ok(row) = rows.get(activate.entity) else {
        return;
    };
    let (Some(project), Some(mut state), Some(mut filters)) = (project, state, filters) else {
        return;
    };
    // **And it takes the keyboard back from the filter box.** `drive_place` is gated on `not_typing`,
    // so leaving the box focused made *the most natural way to find a piece* — type a few letters —
    // the way to break placing it, with no message. Clicking the row you filtered for blurs it, as a
    // click on the world already does. Only when the box has it: `blur` is a write, a write marks
    // `Filters` changed, and a changed `Filters` respawns every row, so a click on an unfocused palette
    // used to take the row it was on (and a context menu's target) away.
    if filters.typing() {
        filters.blur();
    }
    let Some(d) = project.library.descriptors.get(row.0) else {
        return;
    };
    // **The click is the `brush_arm` operation**, the one an agent calls. See [`arm_piece`].
    arm_piece(&mut state, &project, open.as_deref(), &d.id);
}

/// **Arm a piece from the library**, through the operation `brush_arm` — the one an agent calls.
///
/// **One path for both front ends.** The palette click used to set the brush itself, so the editor
/// and an agent could hold different pieces with no record of either. Now the click dispatches the
/// operation, and both front ends apply its answer through [`apply_armed`].
///
/// **Arming returns to placing.** Picking something to place is an unambiguous statement that you are
/// done deleting or moving, and a tool is otherwise only escapable by a key — which a borderless
/// fullscreen window can have taken from it before Bevy ever sees it. A refusal puts the tool back and
/// says why in `Status::problem`.
/// **The open map is what the palette filters by**, so the arm refuses a piece the map's bash hides
/// (gap 5). `None` only when no map is open, and then there is no bash to honour.
pub fn arm_piece(
    state: &mut EditorState,
    project: &Project,
    open: Option<&crate::project::OpenMap>,
    descriptor: &str,
) {
    let was = std::mem::take(&mut state.tool);
    let mut ctx = crate::ops::Ctx {
        view: None, trust_file: None,
        project: Some(project.clone()),
        open: open.cloned(),
        source: crate::ops::Source::Human,
        root: None,
        session: crate::ops::session_id(),
        proposal: None,
    };
    let params = serde_json::json!({"descriptor": descriptor});
    match crate::ops::dispatch(&mut ctx, "brush_arm", &params) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => {
            apply_armed(state, project, outcome.after.as_ref());
            state.status.note(if was == Tool::Place {
                format!("{descriptor} armed")
            } else {
                format!("{descriptor} armed — {} off", was.label())
            });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
            state.tool = was;
            state.status.problem(e.0);
        }
    }
}

/// **The brush an armed outcome names.** The library holds the piece by its index, so the id is
/// resolved against the library here, once, for both front ends. Both call this with the operation's
/// `after`: a name arms that piece, `null` puts the brush down (Esc's answer), and an outcome that
/// names neither changes nothing.
pub fn apply_armed(state: &mut EditorState, project: &Project, after: Option<&serde_json::Value>) {
    match after.and_then(|a| a.get("armed")) {
        Some(serde_json::Value::Null) => state.brush = None,
        Some(armed) => {
            let Some(id) = armed.as_str() else {
                return;
            };
            if let Some(ix) = project.library.descriptors.iter().position(|d| d.id == id) {
                state.brush = Some(ix);
                state.tool = Tool::Place;
            }
        }
        None => {}
    }
}

/// **Arm a composition, or put it down if it is already armed**, through the operation
/// `composition_arm` — the one an agent calls (E-002). The picker's `Enter`, a picker row's click
/// and the COMPOSE tab's `Enter` all come here, so the three and an agent have one answer to "what
/// is armed". The caller writes the receipt from `armed`; `Err` is the refusal.
pub fn arm_composition(
    project: &Project,
    armed: &mut crate::compose::Armed,
    id: &str,
) -> Result<(), String> {
    let mut ctx = crate::ops::Ctx {
        view: None, trust_file: None,
        project: Some(project.clone()),
        open: None,
        source: crate::ops::Source::Human,
        root: None,
        session: crate::ops::session_id(),
        proposal: None,
    };
    let params = if armed.is(id) {
        serde_json::json!({"composition": null})
    } else {
        serde_json::json!({"composition": id})
    };
    match crate::ops::dispatch(&mut ctx, "composition_arm", &params) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => {
            apply_composition_armed(armed, outcome.after.as_ref());
            Ok(())
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

/// **The composition an armed outcome names.** Both front ends call this with `composition_arm`'s
/// `after`: a name arms it, `null` puts it down, and an outcome that names neither changes nothing.
pub fn apply_composition_armed(armed: &mut crate::compose::Armed, after: Option<&serde_json::Value>) {
    match after.and_then(|a| a.get("armed")) {
        Some(serde_json::Value::Null) => armed.0 = None,
        Some(serde_json::Value::String(id)) => armed.0 = Some(id.clone()),
        _ => {}
    }
}

/// **The Meshes shelf's selection, as an operation answers it (E-044).** A click and `piece_select`
/// both land here, so the answer names the piece and the shelf holds it as a click holds it: through
/// `focus_on`, which keeps the highlight and the focused pack in step.
pub fn apply_selected(import: &mut crate::tiles::ImportState, after: Option<&serde_json::Value>) {
    let Some(id) = after
        .and_then(|a| a.get("selected"))
        .and_then(serde_json::Value::as_str)
    else {
        return;
    };
    import.focus_on(&crate::tiles::EditTarget::Library(id.to_string()));
}

/// **A click on a shelf row selects that piece (E-044).** It is the same operation an agent calls,
/// so a click and an agent cannot disagree about what selecting a piece does. The note names the
/// removal chord, because the row is where a person goes to remove a piece.
pub fn select_piece(import: &mut crate::tiles::ImportState, project: &Project, id: &str) {
    let mut ctx = crate::ops::Ctx {
        view: None, trust_file: None,
        project: Some(project.clone()),
        open: None,
        source: crate::ops::Source::Human,
        root: None,
        session: crate::ops::session_id(),
        proposal: None,
    };
    let params = serde_json::json!({"id": id});
    match crate::ops::dispatch(&mut ctx, "piece_select", &params) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => {
            apply_selected(import, outcome.after.as_ref());
            import.status.note(format!(
                "`{id}` selected — {} removes it",
                keys::chord(Action::RemoveTile)
            ));
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => import.status.problem(e.0),
    }
}

/// **Set one library piece's label axis through the operation** (E-042). The chip calls this for a
/// piece in the library, so a person's light goes to the operation log and is undone with the rest.
/// `Source::Human` stamps the label as a person's decision. Returns `true` when the write landed,
/// so the caller records the change only then; a refusal goes to the status row.
pub fn set_label(
    import: &mut crate::tiles::ImportState,
    project: &mut crate::project::Project,
    id: &str,
    axis: &str,
    tokens: &[String],
) -> bool {
    let mut ctx = crate::ops::Ctx {
        view: None, trust_file: None,
        project: Some(project.clone()),
        open: None,
        source: crate::ops::Source::Human,
        root: None,
        session: crate::ops::session_id(),
        proposal: None,
    };
    let params = serde_json::json!({"id": id, "axis": axis, "tokens": tokens});
    match crate::ops::dispatch(&mut ctx, "label_set", &params) {
        Ok(crate::ops::Applied::Done { .. }) => {
            if let Some(fresh) = ctx.project {
                adopt_library(project, &fresh);
            }
            true
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
            import.status.problem(e.0);
            false
        }
    }
}

/// **Set one library piece's mount through the operation** (E-002, #129 slice 6). `M` on the Meshes
/// tab and the mount chip call this for a piece in the library, so a person's mount is the change an
/// agent's `piece_set_mount` makes, with the person's source. Returns `true` when the write landed,
/// so the caller records the change only then; a refusal goes to the status row.
pub fn set_mount(
    import: &mut crate::tiles::ImportState,
    project: &mut crate::project::Project,
    id: &str,
    mount: &bevy_grimoire_core::descriptor::Mount,
) -> bool {
    let mut ctx = crate::ops::Ctx {
        view: None, trust_file: None,
        project: Some(project.clone()),
        open: None,
        source: crate::ops::Source::Human,
        root: None,
        session: crate::ops::session_id(),
        proposal: None,
    };
    let params = serde_json::json!({"id": id, "mount": mount});
    match crate::ops::dispatch(&mut ctx, "piece_set_mount", &params) {
        Ok(crate::ops::Applied::Done { .. }) => {
            if let Some(fresh) = ctx.project {
                adopt_library(project, &fresh);
            }
            true
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
            import.status.problem(e.0);
            false
        }
    }
}

/// **Describe one library piece through the operation** (E-002, #129 slice 17). Enter in the Meshes
/// tab's description field calls this for a piece in the library, so a person's description is the
/// change an agent's `piece_set_note` makes, with the person's source. Returns `true` when the write
/// landed, so the caller records the change only then; a refusal goes to the status row.
pub fn set_note(
    import: &mut crate::tiles::ImportState,
    project: &mut crate::project::Project,
    id: &str,
    note: &str,
) -> bool {
    dispatch_piece_field(import, project, "piece_set_note", serde_json::json!({"id": id, "note": note}))
}

/// **Resize one library piece through the operation** (E-002, #129 slice 18). Enter in the Meshes tab's
/// SIZE field calls this for a piece in the library, so a person's resize is the change an agent's
/// `piece_set_width` makes, with the person's source. Returns `true` when the write landed, so the
/// caller records the change only then; a refusal goes to the status row.
pub fn set_width(
    import: &mut crate::tiles::ImportState,
    project: &mut crate::project::Project,
    id: &str,
    width: f32,
) -> bool {
    dispatch_piece_field(import, project, "piece_set_width", serde_json::json!({"id": id, "width": width}))
}

fn dispatch_piece_field(
    import: &mut crate::tiles::ImportState,
    project: &mut crate::project::Project,
    operation: &str,
    params: serde_json::Value,
) -> bool {
    let mut ctx = crate::ops::Ctx {
        view: None, trust_file: None,
        project: Some(project.clone()),
        open: None,
        source: crate::ops::Source::Human,
        root: None,
        session: crate::ops::session_id(),
        proposal: None,
    };
    match crate::ops::dispatch(&mut ctx, operation, &params) {
        Ok(crate::ops::Applied::Done { .. }) => {
            if let Some(fresh) = ctx.project {
                adopt_library(project, &fresh);
            }
            true
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
            import.status.problem(e.0);
            false
        }
    }
}

/// **Leave a pack out of the kit, or bring it back, through the operation** (E-002, #129 slice 7).
/// `Shift+R` on the Meshes tab calls this, so a person's exclusion is the change an agent's
/// `pack_set_excluded` makes, with the person's source, and its log line is one Cmd+Z on the Meshes
/// tab. Returns `true` when the write landed; a refusal goes to the status row.
pub fn set_pack_excluded(
    import: &mut crate::tiles::ImportState,
    project: &mut crate::project::Project,
    pack: &str,
    excluded: bool,
) -> bool {
    let mut ctx = crate::ops::Ctx {
        view: None, trust_file: None,
        project: Some(project.clone()),
        open: None,
        source: crate::ops::Source::Human,
        root: None,
        session: crate::ops::session_id(),
        proposal: None,
    };
    let params = serde_json::json!({"pack": pack, "excluded": excluded});
    match crate::ops::dispatch(&mut ctx, "pack_set_excluded", &params) {
        Ok(crate::ops::Applied::Done {
            outcome,
            operation_id,
            ..
        }) => {
            if let Some(fresh) = ctx.project {
                adopt_library(project, &fresh);
            }
            import.record_logged(operation_id, outcome.summary.clone());
            import.status.note(outcome.summary);
            true
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
            import.status.problem(format!("not excluded: {}", e.0));
            false
        }
    }
}

/// **Take one piece out of the library through the operation** (E-002, #129 slice 8). `Delete` on the
/// Meshes tab's library shelf calls this, so a person's removal is the change an agent's
/// `piece_remove` makes, with the person's source, and its log line is one Cmd+Z on the Meshes tab.
/// Returns the outcome's `after` (`id`, `removed`, `mesh`) when the write landed, so the caller can
/// move the shelf with [`crate::tiles::follow_piece_removal`]; a refusal goes to the status row.
pub fn remove_piece(
    import: &mut crate::tiles::ImportState,
    project: &mut crate::project::Project,
    id: &str,
) -> Option<serde_json::Value> {
    let mut ctx = crate::ops::Ctx {
        view: None, trust_file: None,
        project: Some(project.clone()),
        open: None,
        source: crate::ops::Source::Human,
        root: None,
        session: crate::ops::session_id(),
        proposal: None,
    };
    let params = serde_json::json!({"id": id});
    match crate::ops::dispatch(&mut ctx, "piece_remove", &params) {
        Ok(crate::ops::Applied::Done {
            outcome,
            operation_id,
            ..
        }) => {
            if let Some(fresh) = ctx.project {
                adopt_library(project, &fresh);
            }
            import.record_logged(operation_id, outcome.summary);
            outcome.after
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
            // About the piece, so the toast's Show selects it on the shelf.
            import.status.problem_about(format!("not sent back: {}", e.0), id);
            None
        }
    }
}

/// **Turn one library piece's mesh through the operation** (E-002, #129 slice 9). `N`, `O` and `P` on
/// the Meshes tab, the three rotate chips and the labeler's righting call this, so a person's turn is
/// the change an agent's `piece_turn` makes, with the person's source, and its log line is one Cmd+Z on
/// the Meshes tab. Returns `true` when the turn landed, so the labeler chains its re-photograph only
/// then; a refusal goes to the status row.
pub fn turn_piece(
    import: &mut crate::tiles::ImportState,
    project: &mut crate::project::Project,
    id: &str,
    axis: crate::tiles::RotateAxis,
    quarters: u8,
    force: bool,
) -> bool {
    let mut ctx = crate::ops::Ctx {
        view: None, trust_file: None,
        project: Some(project.clone()),
        open: None,
        source: crate::ops::Source::Human,
        root: None,
        session: crate::ops::session_id(),
        proposal: None,
    };
    let axis_name = match axis {
        crate::tiles::RotateAxis::X => "x",
        crate::tiles::RotateAxis::Y => "y",
        crate::tiles::RotateAxis::Z => "z",
    };
    let params = serde_json::json!({"id": id, "axis": axis_name, "quarters": quarters, "force": force});
    match crate::ops::dispatch(&mut ctx, "piece_turn", &params) {
        Ok(crate::ops::Applied::Done {
            outcome,
            operation_id,
            ..
        }) => {
            if let Some(fresh) = ctx.project {
                adopt_library(project, &fresh);
            }
            import.record_logged(operation_id, outcome.summary.clone());
            import.status.note(outcome.summary);
            true
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
            import.status.problem(e.0);
            false
        }
    }
}

/// **Edit cells of one library piece's lattice through the operation** (E-002, #129 slice 10). `Z`, `X`
/// and `V` on the Meshes tab, the grid's chips and its headers call this, so a person's edit is the
/// change an agent's `piece_set_cells` makes, with the person's source, and its log line is one Cmd+Z on
/// the Meshes tab. `change` is the verb's own field (`{"solid": true}`, `{"edge": "wall"}` or
/// `{"clear": true}`). Returns `true` when the write landed; a refusal goes to the status row.
pub fn set_cells(
    import: &mut crate::tiles::ImportState,
    project: &mut crate::project::Project,
    id: &str,
    cells: &[(u32, u32, u32)],
    change: serde_json::Value,
) -> bool {
    let mut ctx = crate::ops::Ctx {
        view: None, trust_file: None,
        project: Some(project.clone()),
        open: None,
        source: crate::ops::Source::Human,
        root: None,
        session: crate::ops::session_id(),
        proposal: None,
    };
    let mut params = serde_json::json!({"id": id, "cells": cells});
    if let (Some(p), Some(c)) = (params.as_object_mut(), change.as_object()) {
        p.extend(c.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    match crate::ops::dispatch(&mut ctx, "piece_set_cells", &params) {
        Ok(crate::ops::Applied::Done {
            outcome,
            operation_id,
            ..
        }) => {
            if let Some(fresh) = ctx.project {
                adopt_library(project, &fresh);
            }
            import.record_logged(operation_id, outcome.summary.clone());
            import.status.note(outcome.summary);
            true
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
            import.status.problem(e.0);
            false
        }
    }
}

/// **Add a token to the vocabulary through the operation** (E-002, #129 slice 11). The vocabulary
/// prompt's Enter calls this, so a person's addition is the change an agent's `vocab_add_token`
/// makes, with the person's source, and its log line names it. On success the project's vocabulary and
/// masks are the fresh ones and the receipt is returned; a refusal is returned as the prompt's problem.
pub fn add_vocab_token(
    project: &mut crate::project::Project,
    axis: &str,
    name: &str,
    note: &str,
) -> Result<String, String> {
    let mut ctx = crate::ops::Ctx {
        view: None, trust_file: None,
        project: Some(project.clone()),
        open: None,
        source: crate::ops::Source::Human,
        root: None,
        session: crate::ops::session_id(),
        proposal: None,
    };
    let params = serde_json::json!({"axis": axis, "name": name, "note": note});
    match crate::ops::dispatch(&mut ctx, "vocab_add_token", &params) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => {
            if let Some(fresh) = ctx.project {
                project.vocab = fresh.vocab.clone();
                project.masks = fresh.masks.clone();
            }
            Ok(outcome.summary)
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

/// **Adopt a rig's measured gait values through the operation** (E-002, #129 slice 13). The Anim tab's
/// Enter calls this, so a person's adoption is the change an agent's `rig_adopt_measured` makes, with
/// the person's source, and its log line names it. `skip_slots` is the tab's transient exclude. The
/// receipt is returned, or the refusal as the tab's problem; the tab reads the manifest back itself.
pub fn adopt_rig_measured(
    project: &crate::project::Project,
    rig: &str,
    skip_slots: &[usize],
) -> Result<String, String> {
    let mut ctx = crate::ops::Ctx {
        view: None, trust_file: None,
        project: Some(project.clone()),
        open: None,
        source: crate::ops::Source::Human,
        root: None,
        session: crate::ops::session_id(),
        proposal: None,
    };
    let params = serde_json::json!({"rig": rig, "skip_slots": skip_slots});
    match crate::ops::dispatch(&mut ctx, "rig_adopt_measured", &params) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => Ok(outcome.summary),
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

/// **Carry the library a label operation changed onto the live project.** Only the fields a
/// library write touches move: the measured library and the layers the Map draws from it, and the
/// kit's policy (a pack's exclusion). A clone is not the live state, so without this the write would
/// reach the file and not the screen.
pub fn adopt_library(live: &mut crate::project::Project, fresh: &crate::project::Project) {
    live.measured = fresh.measured.clone();
    live.library = fresh.library.clone();
    live.masks = fresh.masks.clone();
    live.kits = fresh.kits.clone();
    live.triangles = fresh.triangles.clone();
    live.touched = fresh.touched.clone();
    live.policy = fresh.policy.clone();
}

/// Grow or shrink one axis. Clamped at one cell rather than at zero: a map has to enclose something,
/// and `Map::validate` refuses a non-positive extent — better to stop at the floor than to write a
/// map the save will then reject.
fn on_size_field_click(
    activate: On<Activate>,
    fields: Query<&SizeField>,
    edit: Option<ResMut<SizeEdit>>,
    state: Option<ResMut<EditorState>>,
) {
    // **`Option`, because this is a GLOBAL observer** — it fires for any `Activate` anywhere in
    // the application, and in Bevy 0.19 a missing resource panics at param validation rather
    // than skipping. See [`on_cell_verb`] for the whole argument; the ratchet is
    // `every_resource_says_what_a_door_does_to_it.rs`.
    let Some(mut edit) = edit else {
        return;
    };
    let Some(mut state) = state else {
        return;
    };
    let Ok(field) = fields.get(activate.entity) else {
        return;
    };
    // **Starts empty, and says so.** Seeding it with the current number meant the first digit typed
    // appended to it — 32 became 328 — and it looked like it had worked. `rename_keys` records the
    // same trap and the same answer: a field with no selection model starts blank, and the value it
    // is replacing is still on screen until Enter.
    edit.active = Some((field.0, String::new()));
    state.status.note(format!(
        "{} size: type a whole number of metres, Enter to keep it, Esc to leave it alone",
        field.0.label()
    ));
}

/// Digits, and nothing else.
///
/// Filtered at the keystroke rather than validated at commit: a field that accepts `4.5` and then
/// refuses it has taught the author it was allowed. This one simply never shows a character that
/// cannot be part of the answer.
fn size_edit_keys(
    mut events: MessageReader<KeyboardInput>,
    mut edit: ResMut<SizeEdit>,

    mut open: ResMut<OpenMap>,
    project: Option<ResMut<Project>>,
    mut state: ResMut<EditorState>,
) {
    if edit.active.is_none() {
        return;
    }
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }
        match &event.logical_key {
            Key::Enter => {
                let Some((axis, raw)) = edit.active.take() else {
                    return;
                };
                if raw.is_empty() {
                    state
                        .status
                        .note("nothing typed; the size is unchanged".to_owned());
                    return;
                }
                // **Metres, not whole metres.** The old +/- nudges stepped Y by 0.5 m; the field that
                // replaced them took `u32`, so a ceiling saved at 3.5 m could be read on screen and
                // never typed back — the author's only way to change it snapped it to 3 or 4. Bounds
                // are `f32` in the schema and always were.
                let Ok(want) = raw.parse::<f32>() else {
                    state
                        .status
                        .problem(format!("`{raw}` is not a number of metres"));
                    return;
                };
                if !want.is_finite() || !(MIN_BOUND as f32..=MAX_BOUND as f32).contains(&want) {
                    state.status.problem(format!(
                        "a map axis runs {MIN_BOUND}..{MAX_BOUND} m; `{raw}` is outside it"
                    ));
                    return;
                }
                // **The size before this change**, so the entry on the undo stack can name what Cmd+Z
                // puts back.
                let was_bounds = open.map.bounds;
                // **Through the registry, and not around it.** The size field and an agent's
                // `map_set_settings` call are **one operation**, so they cannot drift: the same
                // validation, the same write to the file, the same line in the operation log, and
                // the same refusal text. **A UI path that changed the map's bounds without
                // dispatching the operation would be a second way to do one thing** — and the
                // brief's rule is that a UI path which changes editor state without dispatching a
                // registered operation is a must-fix bug.
                //
                // **The map is *taken*, not copied** — the operation edits it, and a copy would
                // leave the editor's own map behind the change. **And it comes back either way**,
                // so a refusal cannot leave the editor holding an empty map.
                let mut taken_open = Some(std::mem::replace(
                    &mut *open,
                    OpenMap::new(bevy_grimoire_core::map::Map::default(), std::path::PathBuf::new()),
                ));
                let mut ctx = crate::ops::Ctx {
        view: None, trust_file: None,
                    project: project.as_deref().cloned(),
                    open: taken_open.take(),
                    source: crate::ops::Source::Human,
                    root: None,
                    session: crate::ops::session_id(),
                    proposal: None,
                };
                let outcome = crate::ops::dispatch(
                    &mut ctx,
                    "map_set_settings",
                    &serde_json::json!({
                        "axis": axis.op_axis(),
                        "axis_value": want,
                    }),
                );
                if let Some(fresh) = ctx.open.take() {
                    *open = fresh;
                }
                match outcome {
                    Ok(crate::ops::Applied::Done {
                        outcome,
                        operation_id,
                        ..
                    }) => {
                        state.status.note(outcome.summary);
                        // **On the undo stack, so Cmd+Z takes the resize back** and not an older
                        // edit underneath it.
                        state.record(Undo::Settings {
                            line: operation_id,
                            bounds: was_bounds,
                        });
                    }
                    Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
                        state.status.problem(e.0);
                    }
                }
            }
            Key::Escape => {
                edit.active = None;
                state.status.note("size unchanged".to_owned());
            }
            Key::Backspace => {
                if let Some((_, raw)) = edit.active.as_mut() {
                    raw.pop();
                }
            }
            Key::Character(s) => {
                if let Some((_, raw)) = edit.active.as_mut() {
                    // Digits and one decimal point — room for `MAX_BOUND`'s three digits plus
                    // `.5`, and at most one point, so the buffer cannot grow into something the
                    // parse has to refuse later.
                    let point = s == "." && !raw.contains('.');
                    let digit = s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty();
                    if (digit || point) && raw.len() < 5 {
                        raw.push_str(s);
                    }
                }
            }
            _ => {}
        }
    }
}

fn on_family_field_click(
    activate: On<Activate>,
    fields: Query<&FamilyField>,
    edit: Option<ResMut<FamilyEdit>>,
    state: Option<ResMut<EditorState>>,
) {
    // `Option`, because this is a GLOBAL observer: see [`on_size_field_click`].
    let (Some(mut edit), Some(mut state)) = (edit, state) else {
        return;
    };
    if fields.get(activate.entity).is_err() {
        return;
    }
    edit.active = Some(String::new());
    state.status.note(
        "family setting: `<crate> <field> <value>`, Enter to write it, Esc to leave it alone".to_owned(),
    );
}

#[cfg(feature = "bevy_elements")]
/// **Environment buttons** (#98): the clock buttons set [`crate::environment::ClockScrub`] (no file, no
/// undo); the weather buttons dispatch `family_set`, so each is one undo step.
fn on_environment_click(
    activate: On<Activate>,
    actions: Query<&crate::environment::EnvAction>,
    project: Option<Res<Project>>,
    scrub: Option<ResMut<crate::environment::ClockScrub>>,
    state: Option<ResMut<EditorState>>,
) {
    // `Option`, because this is a GLOBAL observer: see [`on_size_field_click`].
    let (Some(mut scrub), Some(mut state)) = (scrub, state) else {
        return;
    };
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    use crate::environment::EnvAction;
    match *action {
        EnvAction::Scrub(hour) => scrub.set(hour),
        EnvAction::ReleaseClock => scrub.release(),
        other => {
            let params = other.family_params().expect("weather actions write the file");
            let mut ctx = crate::ops::Ctx {
                project: project.as_deref().cloned(),
                source: crate::ops::Source::Human,
                session: crate::ops::session_id(),
                ..crate::ops::Ctx::default()
            };
            match crate::ops::dispatch(&mut ctx, "family_set", &params) {
                Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
                    state.status.note(outcome.summary.clone());
                    state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
                }
                Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
            }
        }
    }
}

/// **Play and Stop** (#97): dispatch `play` at the editor camera's position, or `play_stop`.
fn on_play_click(
    activate: On<Activate>,
    buttons: Query<&crate::ops_play::PlayButton>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    camera: Query<&Transform, With<MainCamera>>,
    state: Option<ResMut<EditorState>>,
) {
    // `Option`, because this is a GLOBAL observer: see [`on_size_field_click`].
    let Some(mut state) = state else {
        return;
    };
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let mut ctx = crate::ops::Ctx {
        project: project.as_deref().cloned(),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        trust_file: state.trust.file.clone(),
        ..crate::ops::Ctx::default()
    };
    let result = match button {
        crate::ops_play::PlayButton::Play => {
            // A game no person has allowed is not started: ask first. The operation refuses the same way
            // for an agent, which has no one to ask but the person looking at the journal.
            if let Some(root) = ctx.root_or_project() {
                match crate::ops_play::untrusted_request(&ctx, &root) {
                    Ok(Some(request)) => {
                        state.status.problem(bevy_grimoire_core::trust::TrustStore::untrusted_sentence(&request.root, &request.spec));
                        state.trust.pending = Some(request);
                        return;
                    }
                    Ok(None) => {}
                    Err(e) => return state.status.problem(e),
                }
            }
            let params = serde_json::json!({
                "map": open.as_deref().map(|o| o.map.name.clone()),
                "camera": camera.iter().next().map(|t| t.translation.to_array()),
            });
            play_game(&mut ctx, params)
        }
        crate::ops_play::PlayButton::Stop => stop_game(&mut ctx),
    };
    match result {
        Ok(summary) => state.status.note(summary),
        Err(e) => state.status.problem(e),
    }
}

fn play_game(ctx: &mut crate::ops::Ctx, params: serde_json::Value) -> Result<String, String> {
    match crate::ops::dispatch(ctx, "play", &params) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => Ok(outcome.summary),
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

fn stop_game(ctx: &mut crate::ops::Ctx) -> Result<String, String> {
    match crate::ops::dispatch(ctx, "play_stop", &serde_json::json!({})) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => Ok(outcome.summary),
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

fn family_edit_keys(
    mut events: MessageReader<KeyboardInput>,
    mut edit: ResMut<FamilyEdit>,
    project: Option<ResMut<Project>>,
    mut open: Option<ResMut<OpenMap>>,
    mut state: ResMut<EditorState>,
    mut balloon: ResMut<crate::balloon::BalloonPreview>,
) {
    if edit.active.is_none() {
        return;
    }
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }
        match &event.logical_key {
            Key::Enter => {
                let raw = edit.active.take().unwrap_or_default();
                if raw.trim_start().starts_with("dialogue ") {
                    preview_dialogue(project.as_deref(), open.as_deref().map(|o| &o.map), &mut state, &mut balloon, &raw);
                } else if raw.trim_start().starts_with('@') {
                    set_piece_family(project.as_deref(), open.as_deref_mut(), &mut state, &raw);
                } else {
                    set_family_field(project.as_deref(), &mut state, &raw);
                }
                return;
            }
            Key::Escape => {
                edit.active = None;
                state.status.note("family setting unchanged".to_owned());
                return;
            }
            Key::Backspace => {
                if let Some(raw) = edit.active.as_mut() {
                    raw.pop();
                }
            }
            Key::Space => {
                if let Some(raw) = edit.active.as_mut() {
                    raw.push(' ');
                }
            }
            Key::Character(c) => {
                if let Some(raw) = edit.active.as_mut() {
                    raw.push_str(c);
                }
            }
            _ => {}
        }
    }
}

/// **Commit the typed line through the registry** as `family_set`: the first word is the crate, the
/// second the dot-pathed field, and the rest one RON value.
fn set_family_field(project: Option<&Project>, state: &mut EditorState, raw: &str) {
    let mut words = raw.trim().splitn(3, ' ');
    let (Some(krate), Some(field), Some(value)) = (words.next(), words.next(), words.next()) else {
        state.status.problem(format!("`{}` is not `<crate> <field> <value>`, so nothing was written", raw.trim()));
        return;
    };
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    let params = serde_json::json!({"crate": krate, "field": field, "value": value});
    match crate::ops::dispatch(&mut ctx, "family_set", &params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **Walk a conversation through the registry** as `dialogue_preview`: `dialogue <conversation> [option numbers]`,
/// and say its lines in the status line.
#[cfg(feature = "bevy_speech_bubbles")]
fn preview_dialogue(
    project: Option<&Project>,
    map: Option<&bevy_grimoire_core::map::Map>,
    state: &mut EditorState,
    balloon: &mut crate::balloon::BalloonPreview,
    raw: &str,
) {
    let mut words = raw.split_whitespace().skip(1);
    let Some(name) = words.next() else {
        state.status.problem("`dialogue` needs a conversation name: `dialogue <conversation> [option numbers]`".to_owned());
        return;
    };
    let Ok(choices) = words.map(str::parse::<usize>).collect::<Result<Vec<_>, _>>() else {
        state.status.problem(format!("`{}` has an option that is not a number, so nothing was walked", raw.trim()));
        return;
    };
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    match crate::ops::dispatch(&mut ctx, "dialogue_preview", &serde_json::json!({"conversation": name, "choices": choices})) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => {
            if let Some(after) = outcome.after.as_ref() {
                *balloon = crate::balloon::BalloonPreview::from_after(after, map, name);
            }
            let said = outcome.after.as_ref().and_then(|a| a["lines"].as_array()).map(|l| {
                l.iter().map(|x| format!("{}: {}", x["speaker"].as_str().unwrap_or(""), x["text"].as_str().unwrap_or(""))).collect::<Vec<_>>().join(" / ")
            });
            state.status.note(format!("{} — {}", outcome.summary, said.unwrap_or_default()));
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

#[cfg(not(feature = "bevy_speech_bubbles"))]
fn preview_dialogue(
    _project: Option<&Project>,
    _map: Option<&bevy_grimoire_core::map::Map>,
    state: &mut EditorState,
    _balloon: &mut crate::balloon::BalloonPreview,
    _raw: &str,
) {
    state.status.problem("this editor was built without bevy_speech_bubbles, so there is no dialogue to preview".to_owned());
}

/// **Commit `@<id> <crate> <value>` through the registry** as `placement_set_family`: the piece
/// named by the id carries the crate's own settings as one RON value; no value takes them off.
fn set_piece_family(project: Option<&Project>, open: Option<&mut OpenMap>, state: &mut EditorState, raw: &str) {
    let mut words = raw.trim().trim_start_matches('@').splitn(3, ' ');
    let (Some(id), Some(krate)) = (words.next(), words.next()) else {
        state.status.problem(format!("`{}` is not `@<id> <crate> <value>`, so nothing was written", raw.trim()));
        return;
    };
    let (Some(project), Some(open)) = (project, open) else {
        state.status.problem("no map is open, so there is no piece to set".to_owned());
        return;
    };
    let mut ctx = crate::ops::Ctx {
        project: Some(project.clone()),
        open: Some(std::mem::replace(open, OpenMap::new(bevy_grimoire_core::map::Map::default(), std::path::PathBuf::new()))),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    let mut params = serde_json::json!({"id": id, "crate": krate});
    if let Some(value) = words.next().filter(|v| !v.trim().is_empty()) {
        params["value"] = serde_json::Value::String(value.to_owned());
    }
    let outcome = crate::ops::dispatch(&mut ctx, "placement_set_family", &params);
    if let Some(fresh) = ctx.open.take() {
        *open = fresh;
    }
    match outcome {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

fn refresh_family(
    edit: Res<FamilyEdit>,
    mut readouts: Query<(&mut Text, &mut TextColor), With<FamilyReadout>>,
    mut fields: Query<(&Hovered, &mut BackgroundColor), With<FamilyField>>,
) {
    for (mut text, mut colour) in &mut readouts {
        let (want, want_colour) = crate::chrome::field_text(
            edit.active.as_deref(),
            ("crate field value".to_owned(), TEXT),
        );
        if text.0 != want {
            text.0 = want;
        }
        if colour.0 != want_colour {
            colour.0 = want_colour;
        }
    }
    for (hovered, mut bg) in &mut fields {
        let want = if edit.active.is_some() {
            FOCUS_BG
        } else if hovered.0 {
            ROW_HOVER
        } else {
            ROW_BG
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

fn on_terrain_field_click(
    activate: On<Activate>,
    fields: Query<&TerrainField>,
    edit: Option<ResMut<TerrainEdit>>,
    state: Option<ResMut<EditorState>>,
) {
    // `Option`, because this is a GLOBAL observer: see [`on_size_field_click`].
    let (Some(mut edit), Some(mut state)) = (edit, state) else {
        return;
    };
    if fields.get(activate.entity).is_err() {
        return;
    }
    edit.active = Some(String::new());
    state.status.note(
        "terrain stroke: `<raise|lower|smooth|flatten:H|noise:SEED:SCALE> <x> <z> <radius_m> <strength>`, Enter to sculpt, Esc to leave it".to_owned(),
    );
}

fn terrain_edit_keys(
    mut events: MessageReader<KeyboardInput>,
    mut edit: ResMut<TerrainEdit>,
    project: Option<ResMut<Project>>,
    mut open: Option<ResMut<OpenMap>>,
    mut state: ResMut<EditorState>,
    due: Option<ResMut<crate::watch::Due>>,
) {
    if edit.active.is_none() {
        return;
    }
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }
        match &event.logical_key {
            Key::Enter => {
                let raw = edit.active.take().unwrap_or_default();
                if cfg!(feature = "bevy_carnage") && raw.trim_start().starts_with("destructible ") {
                    #[cfg(feature = "bevy_carnage")]
                    mark_destructible(project.as_deref(), &mut state, &raw);
                } else if cfg!(feature = "bevy_carnage") && raw.trim_start().starts_with("hit ") {
                    #[cfg(feature = "bevy_carnage")]
                    hit_destructible(project.as_deref(), &mut state, &raw);
                } else if cfg!(feature = "bevy_elements") && raw.trim_start().starts_with("preset ") {
                    #[cfg(feature = "bevy_elements")]
                    preset_terrain(project.as_deref(), open.as_deref(), &mut state, &raw);
                } else if raw.trim() == "water run" {
                    run_water(project.as_deref(), open.as_deref(), &mut state);
                } else if raw.trim_start().starts_with("volume ") {
                    edit_volume(project.as_deref(), open.as_deref(), &mut state, &raw);
                } else if raw.trim_start().starts_with("water ") {
                    place_water(project.as_deref(), open.as_deref(), &mut state, &raw);
                } else if raw.trim_start().starts_with("import ") {
                    import_terrain(project.as_deref(), open.as_deref(), &mut state, &raw);
                } else if raw.trim_start().starts_with("forest ") {
                    scatter_forest(project.as_deref(), open.as_deref(), &mut state, &raw);
                } else if raw.trim_start().starts_with("tree ") {
                    edit_tree(project.as_deref(), open.as_deref(), &mut state, &raw);
                } else if raw.trim() == "plan clear" {
                    clear_plan(project.as_deref(), open.as_deref(), &mut state);
                } else if raw.trim() == "relations solve" {
                    solve_relations(project.as_deref(), open.as_mut(), due, &mut state);
                } else if raw.trim() == "repeats vary" {
                    vary_repeats(project.as_deref(), open.as_mut(), due, &mut state);
                } else if raw.trim_start().starts_with("people run") {
                    run_people(project.as_deref(), open.as_deref(), &mut state, &raw);
                } else if raw.trim_start().starts_with("trigger ") {
                    set_trigger(project.as_deref(), open.as_deref(), &mut state, &raw);
                } else if raw.trim_start().starts_with("ref ") {
                    set_reference(project.as_deref(), open.as_deref(), &mut state, &raw);
                } else if raw.trim_start().starts_with("person ") {
                    set_person(project.as_deref(), open.as_deref(), &mut state, &raw);
                } else {
                    sculpt_terrain(project.as_deref(), open.as_deref(), &mut state, &raw);
                }
                return;
            }
            Key::Escape => {
                edit.active = None;
                state.status.note("terrain unchanged".to_owned());
                return;
            }
            Key::Backspace => {
                if let Some(raw) = edit.active.as_mut() {
                    raw.pop();
                }
            }
            Key::Space => {
                if let Some(raw) = edit.active.as_mut() {
                    raw.push(' ');
                }
            }
            Key::Character(c) => {
                if let Some(raw) = edit.active.as_mut() {
                    raw.push_str(c);
                }
            }
            _ => {}
        }
    }
}

/// **Commit the typed line through the registry** as `terrain_stroke`: a brush, one point, a radius and
/// a strength. A map with no terrain gets a flat 64 m field centred on the origin first.
fn sculpt_terrain(project: Option<&Project>, open: Option<&OpenMap>, state: &mut EditorState, raw: &str) {
    let words: Vec<&str> = raw.split_whitespace().collect();
    let nums: Option<Vec<f32>> = words.get(1..).filter(|w| w.len() == 4).and_then(|w| w.iter().map(|n| n.parse().ok()).collect());
    let (Some(open), Some(nums), Some(brush)) = (open, nums, words.first().and_then(|b| terrain_brush(b))) else {
        state.status.problem(format!(
            "`{}` is not `<brush> <x> <z> <radius_m> <strength>` with a brush of raise, lower, smooth, flatten:H or noise:SEED:SCALE, so nothing was changed",
            raw.trim()
        ));
        return;
    };
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        open: Some(open.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    let params = serde_json::json!({
        "map": open.map.name,
        "stroke": {"brush": brush, "radius_m": nums[2], "strength": nums[3], "points": [[nums[0], nums[1]]]},
        "create": {"origin": [-32.0, -32.0], "cell_m": 1.0, "width": 65, "depth": 65, "height_m": 0.0},
    });
    match crate::ops::dispatch(&mut ctx, "terrain_stroke", &params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **Author a person through the registry** as `person_set`, from the terrain line
/// `person <name> remove`, or `person <name> [<x> <z>] [home=<id>] [work=<id>] [faction=<id>] [tie=<name>[:<affinity>[:<trust>]]]`.
/// **Add or remove a reference image through the registry** as `reference_set`, from the terrain line
/// `ref <path> [caption]` or `ref <id> remove`.
fn set_reference(project: Option<&Project>, open: Option<&OpenMap>, state: &mut EditorState, raw: &str) {
    let words: Vec<&str> = raw.split_whitespace().collect();
    let (Some(open), Some(first)) = (open, words.get(1)) else {
        state.status.problem(format!("`{}` is not `ref <path> [caption]` or `ref <id> remove`, so nothing was changed", raw.trim()));
        return;
    };
    let params = if words.get(2) == Some(&"remove") && words.len() == 3 {
        serde_json::json!({"remove": first})
    } else {
        serde_json::json!({"path": first, "caption": words[2..].join(" ")})
    };
    dispatch_reference(project, open, state, &params);
}

fn dispatch_reference(project: Option<&Project>, open: &OpenMap, state: &mut EditorState, params: &serde_json::Value) {
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        open: Some(open.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    match crate::ops::dispatch(&mut ctx, "reference_set", params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **An image dropped on the window becomes a reference of the open map** (#124, E-075), through
/// the same `reference_set` as the `ref` line and an agent's `apply`: one log line, one undo.
///
/// A file already inside the project is referenced where it is. One from outside is copied into the
/// project's `references/` first, because a reference is project data and must travel with it; a
/// different file already holding that name is refused by name rather than overwritten. Undo takes
/// the reference back and leaves the copied file, which is the person's file now.
fn drop_reference(
    mut drops: MessageReader<bevy::window::FileDragAndDrop>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    mut state: ResMut<EditorState>,
) {
    for drop in drops.read() {
        let bevy::window::FileDragAndDrop::DroppedFile { path_buf, .. } = drop else {
            continue;
        };
        // A mesh is for the library (`add_meshes`), not a reference.
        if crate::add_meshes::is_mesh_file(path_buf) {
            continue;
        }
        let (Some(project), Some(open)) = (project.as_deref(), open.as_deref()) else {
            state.status.problem(format!("{} was dropped with no map open, so it is not a reference", path_buf.display()));
            continue;
        };
        match reference_path_for_drop(&project.root, path_buf) {
            Ok(rel) => dispatch_reference(Some(project), open, &mut state, &serde_json::json!({"path": rel})),
            Err(e) => state.status.problem(e),
        }
    }
}

/// **The project-relative path a dropped image is referenced by**, copying it into `references/`
/// when it lies outside the project.
fn reference_path_for_drop(root: &std::path::Path, dropped: &std::path::Path) -> Result<String, String> {
    let shown = dropped.display();
    let ext = dropped.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif") {
        return Err(format!("{shown} is not a png, jpg, jpeg, webp or gif image, so it is not a reference"));
    }
    let canon_root = root.canonicalize().map_err(|e| format!("the project folder {} cannot be read ({e}), so {shown} is not a reference", root.display()))?;
    let canon = dropped.canonicalize().map_err(|e| format!("{shown} cannot be read ({e}), so it is not a reference"))?;
    let relative = |p: &std::path::Path| p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
    if let Ok(inside) = canon.strip_prefix(&canon_root) {
        return Ok(relative(inside));
    }
    let name = canon.file_name().ok_or_else(|| format!("{shown} has no file name, so it is not a reference"))?;
    let into = std::path::Path::new("references").join(name);
    let dest = canon_root.join(&into);
    let bytes = std::fs::read(&canon).map_err(|e| format!("{shown} cannot be read ({e}), so it is not a reference"))?;
    match std::fs::read(&dest) {
        Ok(there) if there == bytes => {}
        Ok(_) => return Err(format!("{} already holds a different picture, so {shown} was not copied over it", into.display())),
        Err(_) => {
            std::fs::create_dir_all(canon_root.join("references")).map_err(|e| format!("references/ cannot be made ({e}), so {shown} is not a reference"))?;
            bevy_grimoire_core::atomic_file::write(&dest, &bytes).map_err(|e| format!("{shown} cannot be copied into {} ({e}), so it is not a reference", into.display()))?;
        }
    }
    Ok(relative(&into))
}

fn set_person(project: Option<&Project>, open: Option<&OpenMap>, state: &mut EditorState, raw: &str) {
    let words: Vec<&str> = raw.split_whitespace().collect();
    let (Some(open), Some(name)) = (open, words.get(1)) else {
        state.status.problem(format!("`{}` is not `person <name> [<x> <z>] [home=<id>] [work=<id>] [faction=<id>]`, so nothing was changed", raw.trim()));
        return;
    };
    let mut params = serde_json::json!({"map": open.map.name, "name": name});
    let mut rest = &words[2..];
    if rest == ["remove"] {
        params["remove"] = true.into();
        rest = &[];
    }
    if let [x, z, tail @ ..] = rest
        && let (Ok(x), Ok(z)) = (x.parse::<f32>(), z.parse::<f32>())
    {
        params["at"] = serde_json::json!([x, z]);
        rest = tail;
    }
    for w in rest {
        match w.split_once('=') {
            Some((k @ ("home" | "work" | "faction"), v)) => params[k] = (*v).into(),
            Some(("tie", v)) => {
                let mut f = v.split(':');
                let with = f.next().unwrap_or_default();
                let rest: Vec<&str> = f.collect();
                let nums: Vec<Option<f32>> = rest.iter().map(|n| n.parse().ok()).collect();
                params["tie"] = if rest == ["remove"] {
                    serde_json::json!({"with": with, "remove": true})
                } else if rest.len() <= 2 && nums.iter().all(Option::is_some) {
                    serde_json::json!({"with": with, "affinity": nums.first().copied().flatten(), "trust": nums.get(1).copied().flatten()})
                } else {
                    state.status.problem(format!("`{w}` is not `tie=<name>[:<affinity>[:<trust>]]` or `tie=<name>:remove`, so nothing was changed"));
                    return;
                };
            }
            _ => {
                state.status.problem(format!("`{w}` is not a person setting (home=<id>, work=<id>, faction=<id>), so nothing was changed"));
                return;
            }
        }
    }
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        open: Some(open.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    match crate::ops::dispatch(&mut ctx, "person_set", &params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **Edit a tree file through the registry** as `tree_edit`: `tree <name> [<preset>] [<field.path>=<value> ...]`,
/// one write, so one undo step. A value is JSON where it parses (`trunk.radius=0.6`), a string otherwise.
fn edit_tree(project: Option<&Project>, open: Option<&OpenMap>, state: &mut EditorState, raw: &str) {
    let mut words = raw.split_whitespace().skip(1);
    let Some(name) = words.next() else {
        state.status.problem("`tree` needs `tree <name> [<preset>] [<field.path>=<value> ...]`, so nothing was written");
        return;
    };
    let mut params = serde_json::json!({"name": name});
    let mut set = serde_json::Map::new();
    for w in words {
        match w.split_once('=') {
            Some((path, v)) => {
                set.insert(path.to_owned(), serde_json::from_str(v).unwrap_or_else(|_| serde_json::Value::String(v.to_owned())));
            }
            None => params["preset"] = serde_json::Value::String(w.to_owned()),
        }
    }
    params["set"] = serde_json::Value::Object(set);
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        open: open.cloned(),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    match crate::ops::dispatch(&mut ctx, "tree_edit", &params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **Plant the typed line through the registry** as `forest_scatter`: `forest <x> <z> <radius_m>
/// <spacing_m> <seed> <species>`, one point, so one undo step.
fn scatter_forest(project: Option<&Project>, open: Option<&OpenMap>, state: &mut EditorState, raw: &str) {
    let words: Vec<&str> = raw.split_whitespace().collect();
    let nums: Option<Vec<f32>> = words.get(1..5).and_then(|w| w.iter().map(|n| n.parse().ok()).collect());
    let seed: Option<i32> = words.get(5).and_then(|n| n.parse().ok());
    let (Some(open), Some(nums), Some(seed), Some(species)) = (open, nums, seed, words.get(6)) else {
        state.status.problem(format!(
            "`{}` is not `forest <x> <z> <radius_m> <spacing_m> <seed> <species>`, so nothing was planted",
            raw.trim()
        ));
        return;
    };
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        open: Some(open.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    let params = serde_json::json!({
        "map": open.map.name,
        "plant": {"points": [[nums[0], nums[1]]], "radius_m": nums[2], "spacing_m": nums[3], "seed": seed,
                  "species": [{"name": species, "weight": 1.0}]},
    });
    match crate::ops::dispatch(&mut ctx, "forest_scatter", &params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **Import a 16-bit height map through the registry** as `terrain_import`, from the terrain line
/// `import <file> <cell_m> <low_m> <high_m>`; the field's first cell is the origin.
fn import_terrain(project: Option<&Project>, open: Option<&OpenMap>, state: &mut EditorState, raw: &str) {
    let words: Vec<&str> = raw.split_whitespace().collect();
    let nums: Option<Vec<f32>> = words.get(2..).filter(|w| w.len() == 3).and_then(|w| w.iter().map(|n| n.parse().ok()).collect());
    let (Some(open), Some(nums)) = (open, nums) else {
        state.status.problem(format!("`{}` is not `import <file> <cell_m> <low_m> <high_m>`, so nothing was changed", raw.trim()));
        return;
    };
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        open: Some(open.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    let params = serde_json::json!({"map": open.map.name, "file": words[1], "cell_m": nums[0], "range_m": [nums[1], nums[2]]});
    match crate::ops::dispatch(&mut ctx, "terrain_import", &params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **Bake a `bevy_elements` preset into the map's terrain through the registry** as `terrain_preset`, from
/// the terrain line `preset <hills|mountains|dunes|plains> <seed> <cell_m> <cells_x> <cells_z>`; the
/// preset replaces the map's terrain and one undo takes it back, the same bake an agent's `apply` makes.
#[cfg(feature = "bevy_elements")]
pub fn preset_terrain(project: Option<&Project>, open: Option<&OpenMap>, state: &mut EditorState, raw: &str) {
    let words: Vec<&str> = raw.split_whitespace().collect();
    let parsed = (|| {
        let [_, preset, seed, cell_m, w, d] = words.as_slice() else { return None };
        Some((*preset, seed.parse::<u64>().ok()?, cell_m.parse::<f32>().ok()?, w.parse::<usize>().ok()?, d.parse::<usize>().ok()?))
    })();
    let (Some(open), Some((preset, seed, cell_m, w, d))) = (open, parsed) else {
        state.status.problem(format!(
            "`{}` is not `preset <hills|mountains|dunes|plains> <seed> <cell_m> <cells_x> <cells_z>`, so nothing was changed",
            raw.trim()
        ));
        return;
    };
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        open: Some(open.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    let params = serde_json::json!({"map": open.map.name, "preset": preset, "seed": seed, "cell_m": cell_m, "size": [w, d]});
    match crate::ops::dispatch(&mut ctx, "terrain_preset", &params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **Mark a piece destructible through the registry** as `destructible_set`, from the terrain line
/// `destructible <piece> on|off`.
#[cfg(feature = "bevy_carnage")]
fn mark_destructible(project: Option<&Project>, state: &mut EditorState, raw: &str) {
    let words: Vec<&str> = raw.split_whitespace().collect();
    let (Some(piece), Some(on)) = (words.get(1), words.get(2).and_then(|w| match *w { "on" => Some(true), "off" => Some(false), _ => None })) else {
        state.status.problem(format!("`{}` is not `destructible <piece> on|off`, so nothing was changed", raw.trim()));
        return;
    };
    let mut ctx = crate::ops::Ctx { project: project.cloned(), source: crate::ops::Source::Human, session: crate::ops::session_id(), ..crate::ops::Ctx::default() };
    match crate::ops::dispatch(&mut ctx, "destructible_set", &serde_json::json!({"piece": piece, "on": on})) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **Preview a hit through the registry** as `destructible_hit`, from the terrain line
/// `hit <piece> <x> <y> <z> <radius_m>`; it changes nothing.
#[cfg(feature = "bevy_carnage")]
fn hit_destructible(project: Option<&Project>, state: &mut EditorState, raw: &str) {
    let words: Vec<&str> = raw.split_whitespace().collect();
    let nums: Option<Vec<f32>> = words.get(2..).filter(|w| w.len() == 4).and_then(|w| w.iter().map(|n| n.parse().ok()).collect());
    let (Some(piece), Some(n)) = (words.get(1), nums) else {
        state.status.problem(format!("`{}` is not `hit <piece> <x> <y> <z> <radius_m>`, so no hit was previewed", raw.trim()));
        return;
    };
    let mut ctx = crate::ops::Ctx { project: project.cloned(), source: crate::ops::Source::Human, session: crate::ops::session_id(), ..crate::ops::Ctx::default() };
    match crate::ops::dispatch(&mut ctx, "destructible_hit", &serde_json::json!({"piece": piece, "point": [n[0], n[1], n[2]], "radius": n[3]})) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => state.status.note(outcome.summary),
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **Place water through the registry** as `water_place`, from the terrain line
/// `water spring <x> <z> <rate_m3_s> <radius_m>`, `water sink <x> <z> <rate_m3_s> <radius_m>` or
/// `water lake <x> <z> <radius_m> <level_m>`; `water clear` removes the map's water.
fn place_water(project: Option<&Project>, open: Option<&OpenMap>, state: &mut EditorState, raw: &str) {
    let words: Vec<&str> = raw.split_whitespace().collect();
    let nums: Option<Vec<f32>> = words.get(2..).filter(|w| w.len() == 4).and_then(|w| w.iter().map(|n| n.parse().ok()).collect());
    let Some(open) = open else {
        state.status.problem("no map is open, so there is no water to place".to_owned());
        return;
    };
    let place = match (words.get(1).copied(), nums) {
        (Some("clear"), _) if words.len() == 2 => None,
        (Some("spring"), Some(n)) => Some(serde_json::json!({"Spring": {"x": n[0], "z": n[1], "rate_m3_s": n[2], "radius_m": n[3]}})),
        (Some("sink"), Some(n)) => Some(serde_json::json!({"Sink": {"x": n[0], "z": n[1], "rate_m3_s": n[2], "radius_m": n[3]}})),
        (Some("lake"), Some(n)) => Some(serde_json::json!({"Lake": {"x": n[0], "z": n[1], "radius_m": n[2], "level_m": n[3]}})),
        _ => {
            state.status.problem(format!(
                "`{}` is not `water spring|sink <x> <z> <rate_m3_s> <radius_m>`, `water lake <x> <z> <radius_m> <level_m>` or `water clear`, so nothing was changed",
                raw.trim()
            ));
            return;
        }
    };
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        open: Some(open.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    let params = match place {
        Some(p) => serde_json::json!({"map": open.map.name, "place": p}),
        None => serde_json::json!({"map": open.map.name, "clear": true}),
    };
    match crate::ops::dispatch(&mut ctx, "water_place", &params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **Add or carve volume through the registry** as `volume_edit`, from the terrain line
/// `volume add|carve sphere <x> <y> <z> <radius_m> [smooth_m]`, `... box <x> <y> <z> <hx> <hy> <hz> [smooth_m]`,
/// `... capsule <ax> <ay> <az> <bx> <by> <bz> <radius_m> [smooth_m]`; `volume clear` removes the map's edits.
fn edit_volume(project: Option<&Project>, open: Option<&OpenMap>, state: &mut EditorState, raw: &str) {
    let Some(open) = open else {
        state.status.problem("no map is open, so there is no volume to edit".to_owned());
        return;
    };
    let words: Vec<&str> = raw.split_whitespace().collect();
    let nums: Vec<f32> = words.iter().skip(3).map_while(|n| n.parse().ok()).collect();
    let (arity, shape) = match words.get(2).copied() {
        Some("sphere") => (4, nums.get(..4).map(|n| serde_json::json!({"Sphere": {"center": [n[0], n[1], n[2]], "radius_m": n[3]}}))),
        Some("box") => (6, nums.get(..6).map(|n| serde_json::json!({"Box": {"center": [n[0], n[1], n[2]], "half_m": [n[3], n[4], n[5]]}}))),
        Some("capsule") => (7, nums.get(..7).map(|n| serde_json::json!({"Capsule": {"a": [n[0], n[1], n[2]], "b": [n[3], n[4], n[5]], "radius_m": n[6]}}))),
        _ => (0, None),
    };
    let mode = words.get(1).copied().filter(|m| matches!(*m, "add" | "carve")).map(|m| if m == "add" { "Add" } else { "Carve" });
    let params = if words.len() == 2 && words[1] == "clear" {
        serde_json::json!({"map": open.map.name, "clear": true})
    } else if let (Some(shape), Some(mode), true) = (shape, mode, nums.len() <= arity + 1 && words.len() == 3 + nums.len()) {
        serde_json::json!({"map": open.map.name, "edit": {"shape": shape, "mode": mode, "smooth_m": nums.get(arity).copied().unwrap_or(0.0)}})
    } else {
        state.status.problem(format!(
            "`{}` is not `volume add|carve sphere|box|capsule <numbers> [smooth_m]` or `volume clear`, so nothing was changed",
            raw.trim()
        ));
        return;
    };
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        open: Some(open.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    match crate::ops::dispatch(&mut ctx, "volume_edit", &params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **Run the map's water and keep it** through the registry as `water_run`, from the terrain line
/// `water run`. One undo takes the kept water back.
fn run_water(project: Option<&Project>, open: Option<&OpenMap>, state: &mut EditorState) {
    let Some(open) = open else {
        state.status.problem("no map is open, so there is no water to run".to_owned());
        return;
    };
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        open: Some(open.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    match crate::ops::dispatch(&mut ctx, "water_run", &serde_json::json!({"map": open.map.name})) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **Author a trigger through the registry** as `trigger_set`, from the terrain line
/// `trigger <id> remove`, or `trigger <id> | <on> | <if> | <then> [| people=<a,b>] [| faction=<id>] [| area=<x0,z0,x1,z1>]`
/// (`<on>` is a bevy_society event as RON, `<if>` may be empty). A script that does not compile is refused with its line.
fn set_trigger(project: Option<&Project>, open: Option<&OpenMap>, state: &mut EditorState, raw: &str) {
    let Some(open) = open else {
        state.status.problem("no map is open, so there is nowhere to keep a trigger".to_owned());
        return;
    };
    let mut parts = raw.trim_start().trim_start_matches("trigger").split(" | ").map(str::trim);
    let head = parts.next().unwrap_or_default();
    let mut params = serde_json::json!({"map": open.map.name});
    if let Some(id) = head.strip_suffix(" remove") {
        params["id"] = id.trim().into();
        params["remove"] = true.into();
    } else {
        let (Some(on), Some(cond), Some(then)) = (parts.next(), parts.next(), parts.next()) else {
            state.status.problem("`trigger <id> | <on> | <if> | <then>` has four parts, so nothing was changed".to_owned());
            return;
        };
        params["id"] = head.into();
        params["on"] = on.into();
        params["if"] = cond.into();
        params["then"] = then.into();
        let mut bind = serde_json::Map::new();
        for w in parts {
            match w.split_once('=') {
                Some(("people", v)) => {
                    bind.insert("people".into(), v.split(',').map(str::trim).collect::<Vec<_>>().into());
                }
                Some(("faction", v)) => {
                    bind.insert("faction".into(), v.into());
                }
                Some(("area", v)) => {
                    let n: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    if n.len() != 4 {
                        state.status.problem(format!("`{w}` is not `area=<x0,z0,x1,z1>`, so nothing was changed"));
                        return;
                    }
                    bind.insert("area".into(), n.into());
                }
                _ => {
                    state.status.problem(format!("`{w}` is not a binding (people=, faction=, area=), so nothing was changed"));
                    return;
                }
            }
        }
        if !bind.is_empty() {
            params["bind"] = bind.into();
        }
    }
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        open: Some(open.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    match crate::ops::dispatch(&mut ctx, "trigger_set", &params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **Run the map's people forward** through the registry as `people_run`, from the terrain line
/// `people run [seed] [days]`. It only reads, so there is nothing to undo; the status says what the day was.
fn run_people(project: Option<&Project>, open: Option<&OpenMap>, state: &mut EditorState, raw: &str) {
    let Some(open) = open else {
        state.status.problem("no map is open, so there is nobody to run".to_owned());
        return;
    };
    let words: Vec<&str> = raw.split_whitespace().collect();
    let mut params = serde_json::json!({"map": open.map.name});
    match (words.get(2).map(|w| w.parse::<u64>()), words.get(3).map(|w| w.parse::<f32>())) {
        (Some(Err(_)), _) | (_, Some(Err(_))) => {
            state.status.problem(format!("`{}` is not `people run [seed] [days]`, so nothing was run", raw.trim()));
            return;
        }
        (seed, days) => {
            if let Some(Ok(s)) = seed {
                params["seed"] = s.into();
            }
            if let Some(Ok(d)) = days {
                params["days"] = d.into();
            }
        }
    }
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        open: Some(open.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    match crate::ops::dispatch(&mut ctx, "people_run", &params) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => state.status.note(outcome.summary),
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **Empty the agent's plan** through the registry as `plan_set`, from the terrain line `plan clear`.
/// Undo puts the plan back, as it does for an agent's own replacement.
fn clear_plan(project: Option<&Project>, open: Option<&OpenMap>, state: &mut EditorState) {
    let Some(open) = open else {
        state.status.problem("no map is open, so there is no plan to clear".to_owned());
        return;
    };
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        open: Some(open.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    match crate::ops::dispatch(&mut ctx, "plan_set", &serde_json::json!({"items": []})) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// **Mend the open map's broken relations** through the registry as `relations_solve`, from the
/// terrain line `relations solve` (#137). The moved rows are redrawn, and one Cmd+Z puts them back.
fn solve_relations(
    project: Option<&Project>,
    open: Option<&mut ResMut<OpenMap>>,
    due: Option<ResMut<crate::watch::Due>>,
    state: &mut EditorState,
) {
    rewrite_rows("relations_solve", "there are no relations to mend", project, open, due, state);
}

/// **Turn the clutter copies that repeat visibly** through the registry as `repetition_vary`, from
/// the terrain line `repeats vary` (#143). The turned rows are redrawn, and one Cmd+Z puts them back.
fn vary_repeats(
    project: Option<&Project>,
    open: Option<&mut ResMut<OpenMap>>,
    due: Option<ResMut<crate::watch::Due>>,
    state: &mut EditorState,
) {
    rewrite_rows("repetition_vary", "there are no repeats to turn", project, open, due, state);
}

/// **Dispatch an operation that rewrites some of the open map's rows and takes no parameters**
/// (`relations_solve`, `repetition_vary`), then redraw what it changed and record its undo.
fn rewrite_rows(
    operation: &str,
    nothing: &str,
    project: Option<&Project>,
    open: Option<&mut ResMut<OpenMap>>,
    due: Option<ResMut<crate::watch::Due>>,
    state: &mut EditorState,
) {
    let (Some(project), Some(open)) = (project, open) else {
        state.status.problem(format!("no map is open, so {nothing}"));
        return;
    };
    // **A refusal or a run with nothing to change is not an edit**, so the map goes in and out of
    // the operation without change detection, and is marked changed only when rows changed.
    let held = open.bypass_change_detection();
    let mut ctx = crate::ops::Ctx {
        project: Some(project.clone()),
        open: Some(std::mem::replace(held, OpenMap::new(bevy_grimoire_core::map::Map::default(), std::path::PathBuf::new()))),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    let outcome = crate::ops::dispatch(&mut ctx, operation, &serde_json::json!({}));
    if let Some(fresh) = ctx.open.take() {
        *held = fresh;
    }
    match outcome {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            if !outcome.saved {
                return;
            }
            // **The changed pieces are drawn where the map now has them**, by the same full redraw a
            // map replaced from disk gets.
            open.set_changed();
            if let Some(mut due) = due {
                due.map_replaced = true;
            }
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// A brush word as the stroke's JSON spells it.
fn terrain_brush(word: &str) -> Option<serde_json::Value> {
    let mut parts = word.split(':');
    Some(match (parts.next()?, parts.next(), parts.next()) {
        ("raise", None, _) => serde_json::json!("Raise"),
        ("lower", None, _) => serde_json::json!("Lower"),
        ("smooth", None, _) => serde_json::json!("Smooth"),
        ("flatten", Some(h), None) => serde_json::json!({"Flatten": h.parse::<f32>().ok()?}),
        ("noise", Some(seed), Some(scale)) => {
            serde_json::json!({"Noise": {"seed": seed.parse::<u32>().ok()?, "scale_m": scale.parse::<f32>().ok()?}})
        }
        _ => return None,
    })
}

fn on_crowd_field_click(
    activate: On<Activate>,
    fields: Query<&CrowdField>,
    edit: Option<ResMut<CrowdEdit>>,
    state: Option<ResMut<EditorState>>,
) {
    // `Option`, because this is a GLOBAL observer: see [`on_size_field_click`].
    let (Some(mut edit), Some(mut state)) = (edit, state) else {
        return;
    };
    if fields.get(activate.entity).is_err() {
        return;
    }
    edit.active = Some(String::new());
    state.status.note("crowd: `<from_x> <from_z> <to_x> <to_z> <walkers>`, Enter to run, Esc to leave it".to_owned());
}

fn crowd_edit_keys(
    mut events: MessageReader<KeyboardInput>,
    mut edit: ResMut<CrowdEdit>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    mut state: ResMut<EditorState>,
) {
    if edit.active.is_none() {
        return;
    }
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }
        match &event.logical_key {
            Key::Enter => {
                let raw = edit.active.take().unwrap_or_default();
                if let Some(heat) = run_crowd_preview(project.as_deref(), open.as_deref(), &mut state, &raw) {
                    edit.heat = heat;
                }
                return;
            }
            Key::Escape => {
                edit.active = None;
                state.status.note("crowd preview left".to_owned());
                return;
            }
            Key::Backspace => {
                if let Some(raw) = edit.active.as_mut() {
                    raw.pop();
                }
            }
            Key::Space => {
                if let Some(raw) = edit.active.as_mut() {
                    raw.push(' ');
                }
            }
            Key::Character(c) => {
                if let Some(raw) = edit.active.as_mut() {
                    raw.push_str(c);
                }
            }
            _ => {}
        }
    }
}

/// The two 2 m squares and the head count of a typed crowd line, as `crowd_preview`'s parameters.
fn crowd_params(raw: &str) -> Option<serde_json::Value> {
    let words: Vec<&str> = raw.split_whitespace().collect();
    if words.len() != 5 {
        return None;
    }
    let n: Vec<f32> = words[..4].iter().map(|w| w.parse().ok()).collect::<Option<_>>()?;
    let walkers: usize = words[4].parse().ok()?;
    let square = |x: f32, z: f32| [x - 1.0, z - 1.0, x + 1.0, z + 1.0];
    Some(serde_json::json!({"from": square(n[0], n[1]), "to": square(n[2], n[3]), "walkers": walkers}))
}

/// **Run the typed line through the registry** as `crowd_preview` and say what it measured.
fn run_crowd_preview(project: Option<&Project>, open: Option<&OpenMap>, state: &mut EditorState, raw: &str) -> Option<CrowdHeat> {
    let Some(params) = crowd_params(raw) else {
        state.status.problem(format!("`{}` is not `<from_x> <from_z> <to_x> <to_z> <walkers>`, so no crowd was run", raw.trim()));
        return None;
    };
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        open: open.cloned(),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    match crate::ops::dispatch(&mut ctx, "crowd_preview", &params) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => {
            state.status.note(outcome.summary);
            outcome.after.as_ref().map(CrowdHeat::from_after)
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => {
            state.status.problem(e.0);
            None
        }
    }
}

/// **Draw the crowd preview's heat over the floor** (#107): one flat square per cell the run wore a
/// path through or queued in. Gizmos, like `draw_edge_faults`: it is a statement about the run, not
/// a thing in the map, and an empty [`CrowdHeat`] draws nothing.
fn draw_crowd_heat(edit: Res<CrowdEdit>, open: Res<OpenMap>, project: Res<Project>, mut gizmos: Gizmos) {
    let heat = &edit.heat;
    if heat.cells.is_empty() || heat.cell <= 0.0 {
        return;
    }
    let y = open.map.origin.1 - ground_drop(&project) + BOUNDS_FILL_CLEARANCE + 0.02;
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    for [x, z, trail, congestion] in &heat.cells {
        // Nested outlines read as a filled square: gizmos have no fill.
        for k in [0.9, 0.6, 0.3] {
            gizmos.rect(
                Isometry3d::new(Vec3::new(open.map.origin.0 + x, y, open.map.origin.2 + z), flat),
                Vec2::splat(heat.cell * k),
                CrowdHeat::colour(*trail, *congestion),
            );
        }
    }
}

fn refresh_crowd(
    edit: Res<CrowdEdit>,
    mut readouts: Query<(&mut Text, &mut TextColor), With<CrowdReadout>>,
    mut fields: Query<(&Hovered, &mut BackgroundColor), With<CrowdField>>,
) {
    for (mut text, mut colour) in &mut readouts {
        let (want, want_colour) =
            crate::chrome::field_text(edit.active.as_deref(), ("from_x from_z to_x to_z walkers".to_owned(), TEXT));
        if text.0 != want {
            text.0 = want;
        }
        if colour.0 != want_colour {
            colour.0 = want_colour;
        }
    }
    for (hovered, mut bg) in &mut fields {
        let want = if edit.active.is_some() {
            FOCUS_BG
        } else if hovered.0 {
            ROW_HOVER
        } else {
            ROW_BG
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

fn refresh_terrain(
    edit: Res<TerrainEdit>,
    mut readouts: Query<(&mut Text, &mut TextColor), With<TerrainReadout>>,
    mut fields: Query<(&Hovered, &mut BackgroundColor), With<TerrainField>>,
) {
    for (mut text, mut colour) in &mut readouts {
        let (want, want_colour) =
            crate::chrome::field_text(edit.active.as_deref(), ("brush x z radius strength".to_owned(), TEXT));
        if text.0 != want {
            text.0 = want;
        }
        if colour.0 != want_colour {
            colour.0 = want_colour;
        }
    }
    for (hovered, mut bg) in &mut fields {
        let want = if edit.active.is_some() {
            FOCUS_BG
        } else if hovered.0 {
            ROW_HOVER
        } else {
            ROW_BG
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

fn on_data_field_click(
    activate: On<Activate>,
    fields: Query<&DataField>,
    edit: Option<ResMut<DataEdit>>,
    state: Option<ResMut<EditorState>>,
) {
    // `Option`, because this is a GLOBAL observer: see [`on_size_field_click`].
    let (Some(mut edit), Some(mut state)) = (edit, state) else {
        return;
    };
    if fields.get(activate.entity).is_err() {
        return;
    }
    edit.active = Some(String::new());
    state.status.note(
        "data setting: `<file> <path> <value>`, Enter to write it, Esc to leave it alone".to_owned(),
    );
}

fn on_data_leaf_click(
    activate: On<Activate>,
    rows: Query<&DataLeafRow>,
    edit: Option<ResMut<DataEdit>>,
    state: Option<ResMut<EditorState>>,
) {
    // `Option`, because this is a GLOBAL observer: see [`on_size_field_click`].
    let (Some(mut edit), Some(mut state)) = (edit, state) else {
        return;
    };
    let Ok(row) = rows.get(activate.entity) else {
        return;
    };
    edit.active = Some(format!("{} {} ", row.file, row.path));
    state.status.note("Enter writes the value, Esc leaves it alone".to_owned());
}

/// A row of [`ScriptsList`]: one trigger of the open map, carrying the terrain line that rewrites it.
#[derive(Component, Clone)]
struct ScriptRow {
    line: String,
}

/// **The Scripts panel's list** (#105): one row per trigger in `scripts/<map>.ron`.
#[derive(Component, Clone, Copy)]
struct ScriptsList;

/// A row opens the terrain line with that trigger already typed, so a person edits it and presses Enter
/// (`trigger_set`, refused with the Rhai line when it does not compile). Global observer: see [`on_size_field_click`].
fn on_script_row_click(
    activate: On<Activate>,
    rows: Query<&ScriptRow>,
    edit: Option<ResMut<TerrainEdit>>,
    state: Option<ResMut<EditorState>>,
) {
    let (Some(mut edit), Some(mut state)) = (edit, state) else {
        return;
    };
    let Ok(row) = rows.get(activate.entity) else {
        return;
    };
    edit.active = Some(row.line.clone());
    state.status.note("Enter writes the trigger (`trigger <id> remove` deletes it), Esc leaves it alone".to_owned());
}

/// Keep [`ScriptsList`] equal to the open map's saved triggers: read twice a second, rows respawned only when they change.
fn refresh_scripts_list(
    mut commands: Commands,
    time: Res<Time>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    lists: Query<Entity, With<ScriptsList>>,
    mut shown: Local<Option<Vec<crate::ops_scripts::Trigger>>>,
    mut since: Local<f32>,
) {
    *since += time.delta_secs();
    if shown.is_some() && *since < 0.5 {
        return;
    }
    *since = 0.0;
    let rows = match (project, open) {
        (Some(p), Some(o)) => crate::ops_scripts::scripts_of(&p, &o.map.name).triggers,
        _ => Vec::new(),
    };
    if shown.as_ref() == Some(&rows) {
        return;
    }
    for list in &lists {
        commands.entity(list).despawn_related::<Children>();
        commands.entity(list).with_children(|p| {
            if rows.is_empty() {
                p.spawn((
                    Text::new("no triggers on this map"),
                    TextColor(DIM),
                    crate::chrome::font(crate::chrome::text::LABEL),
                ));
            }
            for t in &rows {
                crate::chrome::list_row(p, false, ScriptRow { line: crate::ops_scripts::trigger_line(t) })
                    .with_children(|row| {
                        let who = if !t.bind.people.is_empty() {
                            format!(" · {}", t.bind.people.join(", "))
                        } else if let Some(f) = &t.bind.faction {
                            format!(" · {f}")
                        } else if t.bind.area.is_some() {
                            " · area".to_owned()
                        } else {
                            String::new()
                        };
                        row.spawn((
                            Text::new(format!("{} · {}{}", t.id, t.on, who)),
                            TextColor(TEXT),
                            crate::chrome::font(crate::chrome::text::BODY),
                        ));
                    });
            }
        });
    }
    *shown = Some(rows);
}

/// Keep [`DataList`] equal to the registered files' leaves. The files are read twice a second, not
/// every frame, and the rows are respawned only when what they say changed.
fn refresh_data_list(
    mut commands: Commands,
    time: Res<Time>,
    project: Option<Res<Project>>,
    lists: Query<Entity, With<DataList>>,
    mut shown: Local<Option<Vec<crate::ops_data::LeafRow>>>,
    mut since: Local<f32>,
) {
    *since += time.delta_secs();
    if shown.is_some() && *since < 0.5 {
        return;
    }
    *since = 0.0;
    let rows = project.map(|p| crate::ops_data::listing(&p.grimoire_dir)).unwrap_or_default();
    if shown.as_ref() == Some(&rows) {
        return;
    }
    for list in &lists {
        commands.entity(list).despawn_related::<Children>();
        commands.entity(list).with_children(|p| {
            if rows.is_empty() {
                p.spawn((
                    Text::new("no data files registered (data.ron)"),
                    TextColor(DIM),
                    crate::chrome::font(crate::chrome::text::LABEL),
                ));
            }
            for r in &rows {
                crate::chrome::list_row(
                    p,
                    false,
                    DataLeafRow { file: r.file.clone(), path: r.path.clone() },
                )
                .with_children(|row| {
                    row.spawn((
                        Text::new(format!("{} {} = {}", r.file, r.path, r.value)),
                        TextColor(TEXT),
                        crate::chrome::font(crate::chrome::text::BODY),
                    ));
                });
            }
        });
    }
    *shown = Some(rows);
}

/// **The Notes panel's actions** (#90): a row flies the camera to its note, the filter button
/// cycles the status shown, Send all drafts runs `note_send`. Global observer: see [`on_size_field_click`].
#[allow(clippy::too_many_arguments)]
fn on_notes_click(
    activate: On<Activate>,
    rows: Query<&NoteRow>,
    filters: Query<(), With<NotesFilterButton>>,
    sends: Query<(), With<NotesSendAll>>,
    leases: Query<&NoteLeaseButton>,
    op_buttons: Query<&NoteOpButton>,
    filter: Option<ResMut<NotesFilter>>,
    project: Option<ResMut<Project>>,
    open: Option<ResMut<OpenMap>>,
    state: Option<ResMut<EditorState>>,
    rig: Option<ResMut<crate::view::Rig>>,
    cameras: Query<&Camera, With<MainCamera>>,
) {
    let (Some(mut filter), Some(mut open), Some(mut state)) = (filter, open, state) else {
        return;
    };
    let e = activate.entity;
    if filters.get(e).is_ok() {
        let all = bevy_grimoire_core::notes::STATUSES;
        filter.0 = match filter.0 {
            None => Some(all[0]),
            Some(s) => all.iter().position(|a| *a == s).and_then(|i| all.get(i + 1)).copied(),
        };
    } else if sends.get(e).is_ok() {
        let mut project = project;
        send_notes(project.as_deref_mut(), &mut open, &mut state);
    } else if let Ok(button) = op_buttons.get(e) {
        let (Some(project), Some(mut rig)) = (project, rig) else {
            return;
        };
        let entries = crate::ops::read_entries(&crate::ops::oplog_path(&project));
        let ids: Vec<String> = entries
            .iter()
            .find(|x| x.operation_id == button.op)
            .map(|x| crate::ops_notes::footprint(x, &open.map).placements)
            .unwrap_or_default()
            .into_iter()
            .filter(|id| open.map.placements.iter().any(|p| &p.id == id))
            .collect();
        let Some(aspect) = cameras.single().ok().and_then(aspect_of) else {
            state.status.problem("the Map camera has no viewport yet, so there is nothing to frame against".to_owned());
            return;
        };
        if ids.is_empty() {
            state.status.problem(format!("operation {} left no piece on the map to show", button.op));
            return;
        }
        match resolve_frame(&FrameTarget::Placements(ids.clone()), &project, &open, None) {
            Ok(frame) => {
                apply_frame(&mut rig, &frame, aspect, &open);
                state.status.note(format!("framed the {} piece(s) operation {} changed", ids.len(), button.op));
            }
            Err(why) => state.status.problem(why),
        }
    } else if let Ok(lease) = leases.get(e) {
        let mut project = project;
        if lease.op == "note_undo_work" {
            undo_note_work(project.as_deref_mut(), &mut open, &mut state, &lease.id);
        } else if matches!(lease.op, "note_ask" | "note_answer" | "note_resolve" | "note_keep") {
            talk_note(project.as_deref_mut(), &mut open, &mut state, lease.op, &lease.id, lease.text);
        } else {
            lease_note(project.as_deref_mut(), &mut open, &mut state, lease.op, &lease.id);
        }
    } else if let Ok(row) = rows.get(e) {
        let (Some(project), Some(mut rig)) = (project, rig) else {
            return;
        };
        let notes = bevy_grimoire_core::notes::Notes::load(&bevy_grimoire_core::notes::path_for_map(&open.map_path));
        let Some(note) = notes.ok().and_then(|n| n.get(&row.id).cloned()) else {
            state.status.problem(format!("note {} is no longer on the map", row.id));
            return;
        };
        let target = match note.frame_rect() {
            Some(r) => FrameTarget::Rect(r),
            None => FrameTarget::Placements(
                note.targets
                    .iter()
                    .filter_map(|t| match t {
                        bevy_grimoire_core::notes::Target::Placement(p) => Some(p.clone()),
                        _ => None,
                    })
                    .collect(),
            ),
        };
        let Some(aspect) = cameras.single().ok().and_then(aspect_of) else {
            state.status.problem("the Map camera has no viewport yet, so there is nothing to frame against".to_owned());
            return;
        };
        match resolve_frame(&target, &project, &open, None) {
            Ok(frame) => {
                apply_frame(&mut rig, &frame, aspect, &open);
                state.status.note(format!("framed note {} ({})", note.id, note.status()));
            }
            Err(why) => state.status.problem(why),
        }
    }
}

/// What [`refresh_reload_list`] last drew, and how long ago it looked.
#[derive(Default)]
struct ReloadShown {
    lines: Vec<(String, crate::reload_files::Tone)>,
    since: f32,
    drawn: bool,
}

/// Keep [`ReloadList`] equal to what became of each file's latest edit (#203, H6): a header that says
/// whether a game has answered, then a line per file with news, drawn from the same
/// [`crate::reload_files`] rows `grimoire_state.reload_files` reports. The project is looked at twice a
/// second; the lines are rebuilt only when their text changes, which includes the age moving on a
/// second.
#[allow(clippy::too_many_arguments)]
fn refresh_reload_list(
    mut commands: Commands,
    time: Res<Time>,
    project: Option<Res<Project>>,
    game: Option<Res<crate::live::LiveGame>>,
    reloads: Option<Res<crate::live::EditorReloads>>,
    live: Option<Res<crate::live::LiveWrite>>,
    lists: Query<Entity, With<ReloadList>>,
    mut local: Local<ReloadShown>,
) {
    use crate::reload_files::{self, Tone};
    let ReloadShown { lines: shown, since, drawn } = &mut *local;
    *since += time.delta_secs();
    if *drawn && *since < 0.5 {
        return;
    }
    *since = 0.0;
    let Some(project) = project else { return };
    if lists.is_empty() {
        return;
    }
    let verdict = game.as_deref().and_then(|g| g.verdict.as_ref());
    let editor = reloads.as_deref().map_or(&[][..], |r| r.kinds());
    let files = reload_files::files(&project.project_dir, &project.root, verdict, editor);
    let now = reload_files::panel_lines(&files, verdict, live.is_some(), reload_files::now_ms());
    if *drawn && *shown == now {
        return;
    }
    for list in &lists {
        commands.entity(list).despawn_related::<Children>();
        commands.entity(list).with_children(|p| {
            for (line, tone) in &now {
                let colour = match tone {
                    Tone::Dim => DIM,
                    Tone::Text => TEXT,
                    Tone::Accent => ACCENT,
                    Tone::Danger => DANGER,
                };
                let size = if *tone == Tone::Dim { crate::chrome::text::LABEL } else { crate::chrome::text::BODY };
                p.spawn((Text::new(line.clone()), TextColor(colour), crate::chrome::font(size)));
            }
        });
    }
    *shown = now;
    *drawn = true;
}

/// Keep [`PlanList`] equal to the open map's plan: a count, then `<n> [<status>] <text>` per step in
/// the agent's order, or "no plan". Read twice a second, like the notes, so a `plan_set`, its undo
/// and a hand edit of the file all show; rebuilt only when what it says changed.
fn refresh_plan_list(
    mut commands: Commands,
    time: Res<Time>,
    open: Option<Res<OpenMap>>,
    lists: Query<Entity, With<PlanList>>,
    mut local: Local<(Option<bevy_grimoire_core::plan::Plan>, f32)>,
) {
    use bevy_grimoire_core::plan::{self, Plan, Status};
    let (shown, since) = &mut *local;
    *since += time.delta_secs();
    if shown.is_some() && *since < 0.5 {
        return;
    }
    *since = 0.0;
    let now = open.and_then(|o| Plan::load(&plan::path_for_map(&o.map_path)).ok()).unwrap_or_default();
    if shown.as_ref() == Some(&now) {
        return;
    }
    for list in &lists {
        commands.entity(list).despawn_related::<Children>();
        commands.entity(list).with_children(|p| {
            if now.items.is_empty() {
                p.spawn((Text::new("no plan"), TextColor(DIM), crate::chrome::font(crate::chrome::text::LABEL)));
                return;
            }
            let (_, _, done, _) = now.counts();
            p.spawn((
                Text::new(format!("plan: {done} of {} done", now.items.len())),
                TextColor(DIM),
                crate::chrome::font(crate::chrome::text::LABEL),
            ));
            for (at, item) in now.items.iter().enumerate() {
                let (status, colour) = match item.status {
                    Status::Pending => ("pending", TEXT),
                    Status::Doing => ("doing", ACCENT),
                    Status::Done => ("done", DIM),
                    Status::Skipped => ("skipped", DIM),
                };
                p.spawn((
                    Text::new(format!("{} [{status}] {}", at + 1, item.text)),
                    TextColor(colour),
                    crate::chrome::font(crate::chrome::text::BODY),
                ));
            }
        });
    }
    *shown = Some(now);
}

/// Keep [`NotesList`] equal to the open map's notes under the current filter: newest first, every
/// status, resolved notes included. Read twice a second; rebuilt only when what it says changed.
fn refresh_notes_list(
    mut commands: Commands,
    time: Res<Time>,
    view: Res<crate::notes_view::NotesView>,
    filter: Res<NotesFilter>,
    lists: Query<Entity, With<NotesList>>,
    mut local: Local<(Option<NotesShown>, f32)>,
    mut images: ResMut<Assets<Image>>,
) {
    let (shown, since) = &mut *local;
    *since += time.delta_secs();
    if shown.is_some() && *since < 0.5 && !filter.is_changed() {
        return;
    }
    *since = 0.0;
    let all = view.notes();
    let rows: Vec<(String, &'static str, String, bool, NoteReview, bool)> = all
        .listing(filter.0)
        .iter()
        .map(|n| {
            let waiting = n.pending_question();
            let text = match (n.holder(), waiting) {
                (Some(by), Some(q)) => format!("{} (held by {by}; waiting for you: {q})", n.text),
                (Some(by), None) => format!("{} (held by {by})", n.text),
                (None, _) => n.text.clone(),
            };
            let review = match &n.state {
                bevy_grimoire_core::notes::State::Resolved { ops, summary, assumptions, captures } => {
                    let mut lines = vec![format!("summary: {summary}")];
                    lines.extend(assumptions.iter().map(|a| format!("assumed: {a}")));
                    if let [before, after] = captures.as_slice() {
                        lines.push(format!("before: {before}"));
                        lines.push(format!("after: {after}"));
                    }
                    let pictures = if captures.len() == 2 { captures.clone() } else { Vec::new() };
                    NoteReview { lines, ops: ops.clone(), pictures }
                }
                _ => NoteReview::default(),
            };
            (n.id.clone(), n.status(), text, waiting.is_some(), review, n.auto)
        })
        .collect();
    let key = (filter.0, rows);
    if shown.as_ref() == Some(&key) {
        return;
    }
    let (_, rows) = &key;
    for list in &lists {
        commands.entity(list).despawn_related::<Children>();
        commands.entity(list).with_children(|p| {
            p.spawn(Node { column_gap: Val::Px(crate::chrome::GAP_TIGHT), ..default() }).with_children(|h| {
                crate::chrome::button(
                    h,
                    NotesFilterButton,
                    &format!("Show: {}", filter.0.unwrap_or("all")),
                    crate::chrome::Severity::Plain,
                );
                crate::chrome::button(h, NotesSendAll, "Send all drafts", crate::chrome::Severity::Primary);
            });
            if rows.is_empty() {
                p.spawn((
                    Text::new("no notes"),
                    TextColor(DIM),
                    crate::chrome::font(crate::chrome::text::LABEL),
                ));
            }
            for (id, status, text, waiting, review, auto) in rows {
                crate::chrome::list_row(p, false, NoteRow { id: id.clone() }).with_children(|row| {
                    row.spawn((
                        Text::new(format!("{id} [{status}] {text}")),
                        TextColor(TEXT),
                        crate::chrome::font(crate::chrome::text::BODY),
                    ));
                    // **An auto note says so on its row** (E-070): the agent decides its minor
                    // choices itself instead of asking. A label, not a button: Tab in the note box
                    // and `note_add`'s `auto` are where it is set.
                    if *auto {
                        row.spawn((
                            Node {
                                padding: crate::chrome::CHIP_PAD,
                                border: UiRect::all(Val::Px(crate::chrome::EDGE_W)),
                                border_radius: BorderRadius::all(Val::Px(crate::chrome::RADIUS_ROW)),
                                ..default()
                            },
                            BorderColor::all(crate::chrome::SUGGEST),
                        ))
                        .with_children(|chip| {
                            chip.spawn((
                                Text::new("auto"),
                                TextColor(crate::chrome::SUGGEST),
                                crate::chrome::font(crate::chrome::text::LABEL),
                            ));
                        });
                    }
                });
                for line in &review.lines {
                    p.spawn((Text::new(line.clone()), TextColor(DIM), crate::chrome::font(crate::chrome::text::LABEL)));
                }
                let loaded: Vec<Handle<Image>> = review
                    .pictures
                    .iter()
                    .filter_map(|path| {
                        let bytes = std::fs::read(path).ok()?;
                        let image = Image::from_buffer(
                            &bytes,
                            bevy::image::ImageType::Extension("png"),
                            bevy::image::CompressedImageFormats::NONE,
                            true,
                            bevy::image::ImageSampler::default(),
                            bevy::asset::RenderAssetUsages::default(),
                        )
                        .ok()?;
                        Some(images.add(image))
                    })
                    .collect();
                if !loaded.is_empty() {
                    p.spawn(Node { column_gap: Val::Px(crate::chrome::GAP_TIGHT), ..default() }).with_children(|h| {
                        for handle in loaded {
                            // CHROME-OK: a fixed 4:3 review thumbnail, not spacing
                            h.spawn((Node { width: Val::Px(160.0), height: Val::Px(120.0), ..default() }, ImageNode::new(handle)));
                        }
                    });
                }
                if !review.ops.is_empty() {
                    p.spawn(Node { column_gap: Val::Px(crate::chrome::GAP_TIGHT), flex_wrap: FlexWrap::Wrap, ..default() }).with_children(|h| {
                        for op in &review.ops {
                            crate::chrome::button(h, NoteOpButton { op: op.clone() }, op, crate::chrome::Severity::Plain);
                        }
                    });
                }
                let ops: &[(&'static str, &str, &'static str)] = match (*status, *waiting) {
                    ("sent", _) => &[("note_claim", "Claim", "")],
                    ("claimed", true) => &[("note_answer", "Yes", "yes"), ("note_answer", "No", "no"), ("note_release", "Release", "")],
                    ("claimed", false) => &[
                        ("note_renew", "Renew", ""),
                        ("note_release", "Release", ""),
                        ("note_ask", "Ask", "Is this what you meant me to do?"),
                        ("note_resolve", "Done", "finished by hand"),
                    ],
                    ("done", _) => &[("note_keep", "Keep", ""), ("note_undo_work", "Undo this note", "")],
                    _ => &[],
                };
                if !ops.is_empty() {
                    p.spawn(Node { column_gap: Val::Px(crate::chrome::GAP_TIGHT), ..default() }).with_children(|h| {
                        for (op, label, text) in ops {
                            crate::chrome::button(h, NoteLeaseButton { id: id.clone(), op, text }, label, crate::chrome::Severity::Plain);
                        }
                    });
                }
            }
        });
    }
    *shown = Some(key);
}

fn data_edit_keys(
    mut events: MessageReader<KeyboardInput>,
    mut edit: ResMut<DataEdit>,
    project: Option<ResMut<Project>>,
    mut state: ResMut<EditorState>,
) {
    if edit.active.is_none() {
        return;
    }
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }
        match &event.logical_key {
            Key::Enter => {
                let raw = edit.active.take().unwrap_or_default();
                set_data_leaf(project.as_deref(), &mut state, &raw);
                return;
            }
            Key::Escape => {
                edit.active = None;
                state.status.note("data setting unchanged".to_owned());
                return;
            }
            Key::Backspace => {
                if let Some(raw) = edit.active.as_mut() {
                    raw.pop();
                }
            }
            Key::Space => {
                if let Some(raw) = edit.active.as_mut() {
                    raw.push(' ');
                }
            }
            Key::Character(c) => {
                if let Some(raw) = edit.active.as_mut() {
                    raw.push_str(c);
                }
            }
            _ => {}
        }
    }
}

/// **Commit the typed line through the registry** as `data_set`: the first word is the registered file,
/// the second the leaf path, and the rest one JSON scalar (`2.5`, `true`, `"name"`).
fn set_data_leaf(project: Option<&Project>, state: &mut EditorState, raw: &str) {
    let mut words = raw.trim().splitn(3, ' ');
    let (Some(krate), Some(field), Some(value)) = (words.next(), words.next(), words.next()) else {
        state.status.problem(format!("`{}` is not `<file> <path> <value>`, so nothing was written", raw.trim()));
        return;
    };
    let mut ctx = crate::ops::Ctx {
        project: project.cloned(),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        ..crate::ops::Ctx::default()
    };
    let value = serde_json::from_str::<serde_json::Value>(value)
        .unwrap_or_else(|_| serde_json::Value::String(value.to_owned()));
    let params = serde_json::json!({"file": krate, "path": field, "value": value});
    match crate::ops::dispatch(&mut ctx, "data_set", &params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
            state.status.note(outcome.summary.clone());
            state.record(Undo::Logged { line: operation_id, phrase: outcome.summary, agent: false });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

fn refresh_data(
    edit: Res<DataEdit>,
    mut readouts: Query<(&mut Text, &mut TextColor), With<DataReadout>>,
    mut fields: Query<(&Hovered, &mut BackgroundColor), With<DataField>>,
) {
    for (mut text, mut colour) in &mut readouts {
        let (want, want_colour) = crate::chrome::field_text(
            edit.active.as_deref(),
            ("file path value".to_owned(), TEXT),
        );
        if text.0 != want {
            text.0 = want;
        }
        if colour.0 != want_colour {
            colour.0 = want_colour;
        }
    }
    for (hovered, mut bg) in &mut fields {
        let want = if edit.active.is_some() {
            FOCUS_BG
        } else if hovered.0 {
            ROW_HOVER
        } else {
            ROW_BG
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

fn refresh_size(
    open: Res<OpenMap>,
    edit: Res<SizeEdit>,
    mut readouts: Query<(&SizeReadout, &mut Text, &mut TextColor)>,
    mut fields: Query<(&SizeField, &Hovered, &mut BackgroundColor)>,
) {
    for (readout, mut text, mut colour) in &mut readouts {
        // While a field is being typed into it shows what has been typed, with a caret — so the
        // number on screen is always the number Enter would commit, never the one being replaced.
        let editing = match &edit.active {
            Some((axis, raw)) if *axis == readout.0 => Some(raw.as_str()),
            _ => None,
        };
        let v = readout.0.get(open.map.bounds);
        // Whole metres now that the field only accepts them. A map loaded with a fractional
        // bound still reads truthfully rather than being silently rounded on screen.
        let idle = if (v - v.round()).abs() < 1e-3 {
            format!("{v:.0}")
        } else {
            format!("{v:.1}")
        };
        let (want, want_colour) = crate::chrome::field_text(editing, (idle, TEXT));
        if text.0 != want {
            text.0 = want;
        }
        if colour.0 != want_colour {
            colour.0 = want_colour;
        }
    }

    for (field, hovered, mut bg) in &mut fields {
        let focused = matches!(&edit.active, Some((axis, _)) if *axis == field.0);
        // Focus beats hover: while a field is being typed into, the pointer resting on it is not
        // news. Hover is `chrome::ROW_HOVER`'s signifier — this is clickable, and not yet chosen.
        let want = if focused {
            FOCUS_BG
        } else if hovered.0 {
            ROW_HOVER
        } else {
            ROW_BG
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// Draw the map's extent as a wireframe box.
///
/// Gizmos rather than a mesh: the bounds are a statement about the map, not a thing in it, and a real
/// box would be pickable, shadow-casting, and something a click could land on.
/// **Every place the map disagrees with the tiles' declared edge tokens.**
///
/// Recomputed only when the project changes — it is O(placements^2) in the worst case and the answer
/// cannot move while nothing has been placed, turned or removed.
#[derive(Resource, Default)]
pub struct EdgeFaults(pub Vec<bevy_grimoire_core::adjacency::Fault>);

fn check_edges(project: Res<Project>,
    open: Res<OpenMap>, mut faults: ResMut<EdgeFaults>) {
    // The **layered** library, because that is what the map places — and this project's divisions,
    // because a face's length is derived from a piece's size and how finely the project divides.
    faults.0 =
        bevy_grimoire_core::adjacency::faults(&open.map, &project.library, project.lattice.face_bands);
}

/// Outline both halves of every fault, so the sentence in the panel has something to point at.
///
/// A gizmo rather than a spawned marker: it is derived from a resource that is already recomputed on
/// change, so there is nothing to keep in step and nothing to clean up.
fn draw_edge_faults(
    faults: Res<EdgeFaults>,
    project: Res<Project>,
    open: Res<OpenMap>,
    heights: Query<(&Placement, &Transform)>,
    mut gizmos: Gizmos,
) {
    if faults.0.is_empty() {
        return;
    }
    for fault in &faults.0 {
        for id in [fault.a.as_str(), fault.b.as_str()] {
            if id.is_empty() {
                continue;
            }
            let Some((_, tf)) = heights.iter().find(|(p, _)| p.0 == id) else {
                continue;
            };
            let footprint = open
                .map
                .placements
                .iter()
                .find(|p| p.id == id)
                .and_then(|p| descriptor_of(&project.library, p))
                .and_then(|d| bevy_grimoire_core::descriptor::placed_footprint(&d))
                .unwrap_or((CELL, CELL));
            gizmos.rect(
                Isometry3d::new(
                    tf.translation + Vec3::Y * 0.02,
                    Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
                ),
                Vec2::new(footprint.0, footprint.1),
                FAULT_LINE,
            );
        }
    }
}

/// The colour a disagreeing face is outlined in — the palette's refusal colour, because a fault is
/// the map and the tokens contradicting each other and that is a thing to fix, not a warning.
const FAULT_LINE: Color = DANGER;

/// **How far apart the drawn grid's lines are, in metres.**
///
/// **The rung the map places on, latched.**
///
/// # This used to be a drawn grid that nothing landed on
///
/// It was `GridSpacing(f32)`, cycled by `J` through `[0.5, 1.0, 2.0, 4.0]` metres — *a view setting*,
/// explicitly not the lattice. The lattice was reached by a **held modifier** instead: bare
/// `SnapLevel::Tile`, `Shift` one rung finer, `{MOD}+Shift` two. So the map ran two grid mechanisms
/// at once, and the one with a key on it was the one that decided nothing:
///
/// | | what `J` did | what you landed on |
/// |---|---|---|
/// | Map | drew a 0.5/1/2/4 m grid | a held modifier |
/// | Tiles | **latched the rung** | the latched rung |
///
/// Two tabs, one key, two meanings — and 2 m and 4 m were lines no piece could ever sit on. This is
/// the most likely single cause of *"I can't predict where the piece will go"*, and it is the open
/// half of FVS-R-19, whose own note says it plainly: *"Holding Shift is right for one nudge and
/// wrong for a dressing session."*
///
/// Now there is one ladder. `J` steps it, the drawn grid *is* it, and `Shift` stays what it always
/// meant — one rung finer, for a single nudge — with [`grid::SnapLevel::finer`] saturating at the
/// bottom so the modifier can never be the largest movement available.
///
/// **Latched is safe here because it is drawn**, which is Bier's snap-dragging argument and the same
/// one `build::Build::rung` makes one tab over.
///
/// Not persisted: it is about this session's hand, not about the map or the kit, and neither file is
/// a place to keep it.
#[derive(Resource, Default)]
pub struct Rung(pub SnapLevel);

/// **A generated layout that has not been written to the map.**
///
/// See [`keys::Stance::Proposed`] for why this is a phase rather than a flag, and [`propose`] for
/// the measured reason it exists at all.
///
/// It holds only what the solver produced. What the layout would *clear* is decided when it is
/// taken, against the live map, because an index captured now and used later is a bug waiting for
/// the first author who places a piece while a proposal is up.
///
/// **`PartialEq` because a proposal is compared, not just drawn**: `layout_propose` answers a test
/// with two layouts and the question "did the same seed lay the same thing" has to be answerable.
#[derive(Clone, Debug, PartialEq)]
pub struct Proposed {
    /// True when the source was `Composed`, which lays stamps rather than placements — and therefore
    /// clears the unpinned stamps rather than the unpinned placements.
    pub composes: bool,
    pub placements: Vec<Placed>,
    pub stamps: Vec<bevy_grimoire_core::composition::Stamped>,
    /// The edge of one proposed cell in metres — what [`draw_proposal`] sizes each outline to. The
    /// editor's own step ([`CELL`]) unless the proposal came from the project's block rules, which
    /// state their own (`BlockRules::cell`), so the preview is drawn on the grid the game builds.
    pub cell: f32,
    /// True for a proposal the editor may show but never keep: the game lays these blocks out
    /// itself, so writing them into the map would make a second, stale copy of its answer.
    /// [`apply_proposal`] refuses one and leaves it standing.
    pub preview_only: bool,
    /// **How varied this layout is (E-080)**, or `None` when it cannot be scored.
    ///
    /// `range::measure` needs three things the solver had and `Proposed` used to drop: the collapsed
    /// grid, its size, and the interfaces that say which prototype presents a wall on which face.
    /// `Solved` returns all three — its own doc says *"`crate::range` measures a solved grid"* — so
    /// this is four values carried one function further, not a second measurement.
    ///
    /// **`None` for a source with no interfaces.** A grammar learned from placements derives no wall
    /// faces, so nothing can be classified. `0.0` would be a *meaningful* enclosure value ("no floor
    /// is enclosed"), which is why "unmeasurable" must not be spelled the same way.
    pub variety: Option<bevy_grimoire_core::range::Measured>,
    /// **How many cells the measurement covered**, `0` when there is no measurement.
    ///
    /// `Measured` reports fractions and counts of *regions*, not of cells, so a reader given only
    /// `enclosure: 0.0` cannot tell a walled-in empty map from a 16×16 solve that enclosed nothing.
    /// Carried beside the measurement rather than inside it, because `range` has no reason to report
    /// a number its own callers already know.
    pub cells: usize,
    /// **Which generate made this**, monotonic for the life of the process.
    ///
    /// The preview's own identity, and the reason it is a number rather than something derived from
    /// the rows: pressing `G` twice in a row is ordinary, and a key built from the layout would make
    /// the second proposal of the same length go undrawn. See [`sync_proposal_ghosts`].
    ///
    /// **Not `state.seed`.** That is a door-scoped field (`EditorState` is rebuilt on every door
    /// crossing, `seed: 1`), and the sync's `Local` survives the reset — so the two disagreed after
    /// a crossing and a proposal could be mistaken for the one already drawn. [`ProposalClock`] is a
    /// resource, so it outlives the door exactly as the `Local` does.
    pub generation: u64,
    /// **The part of the map this layout was laid in** (#192), or `None` for the whole map. Keeping
    /// the layout replaces only the unpinned rows in it; see [`Proposed::replaces_placement`].
    pub area: Option<bevy_grimoire_core::grammar::Area>,
}

impl Proposed {
    /// **Does keeping this layout replace `row`?** An unpinned placement, and with an area only one
    /// in a cell the area lays ([`bevy_grimoire_core::grammar::Area::holds`]); the author's pinned
    /// pieces and everything outside the area stay.
    pub fn replaces_placement(&self, map: &bevy_grimoire_core::map::Map, row: &Placed) -> bool {
        !row.owned && self.area.is_none_or(|a| a.holds(map, self.cell, row.at))
    }

    /// [`Proposed::replaces_placement`] for a stamp.
    pub fn replaces_stamp(&self, map: &bevy_grimoire_core::map::Map, row: &bevy_grimoire_core::composition::Stamped) -> bool {
        !row.owned && self.area.is_none_or(|a| a.holds(map, self.cell, row.at))
    }
}

/// **A counter that never resets**, for telling one proposal from the next.
///
/// Its own resource rather than a field on `EditorState`, because `EditorState` is rebuilt every time
/// a door is crossed and the thing it is compared against ([`sync_proposal_ghosts`]'s `Local`) is
/// not. A counter with a different lifetime from its reader is a counter that will eventually lie.
#[derive(Resource, Default)]
pub struct ProposalClock(pub u64);

impl ProposalClock {
    /// The next proposal's number. Wrapping is fine: two proposals 2^64 apart are not a case.
    pub fn tick(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(1);
        self.0
    }
}

/// The proposal in hand, or `None`. `None` is a real state: it is what makes the Map's key list show
/// the region-fills instead of the door.
#[derive(Resource, Default)]
pub struct Proposal(pub Option<Proposed>);

/// `J`: the next rung, wrapping. Says the pitch in metres, because a grid an author cannot name is
/// one they cannot ask for again.
fn cycle_grid(
    keyboard: Res<ButtonInput<KeyCode>>,
    live: Res<keys::Live>,
    project: Res<Project>,
    mut rung: ResMut<Rung>,
    mut state: ResMut<EditorState>,
) {
    if !keys::just_pressed(&keyboard, *live, Action::CycleGrid) {
        return;
    }
    // **The key dispatches the operation an agent calls** (E-002, #129). The rung is view state, so it
    // travels in `Ctx::view` rather than being written here: one place decides what a cycle means, and
    // `the_same_view_change_by_key_and_by_mcp_is_one_operation` holds the two front ends together.
    let mut ctx = crate::ops::Ctx {
        project: Some(project.clone()),
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        view: Some(crate::ops_view::View { rung: rung.0, ..Default::default() }),
        ..crate::ops::Ctx::default()
    };
    let outcome = crate::ops::dispatch(&mut ctx, "view_cycle_grid", &serde_json::json!({}));
    if let Some(view) = ctx.view {
        rung.0 = view.rung;
    }
    let divisor = project.lattice.snap_divisor;
    match outcome {
        Ok(crate::ops::Applied::Done { outcome, .. }) => {
            state.status.note(format!(
                "{} — Shift nudges at {:.3} m",
                outcome.summary,
                rung.0.finer().pitch(divisor),
            ));
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// The slab that shows how far the map goes. See [`BOUNDS_FILL`].
#[derive(Component)]
pub struct BoundsFloor;

/// Spawned once, at whatever size; [`fit_bounds_floor`] gives it the map's.
fn spawn_bounds_floor(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        BoundsFloor,
        // A unit rectangle, scaled to the bounds below. `Rectangle` is authored in the XY plane and
        // laid flat by a quarter turn about X — `spawn_removal_tile`'s shape, for the same reason.
        Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: BOUNDS_FILL,
            // **Unlit, and opaque.** Unlit because a reference surface that got brighter where a
            // key light happens to fall would be reporting the lighting rig rather than the bounds.
            // Opaque because it is ground, not a marker — the three translucent tints in this file
            // all mean "look at what is under me", and this has nothing under it.
            unlit: true,
            ..default()
        })),
        Transform::default(),
    ));
}

/// **Follow `Map::bounds`.** Cheap, and gated on the project changing, so resizing the map in the
/// size fields moves the slab with the wireframe rather than leaving the two disagreeing.
fn fit_bounds_floor(project: Res<Project>,
    open: Res<OpenMap>, mut slab: Query<&mut Transform, With<BoundsFloor>>) {
    // Map space, centred on zero — the origin goes back on here, exactly as `draw_bounds` does.
    let (min_x, min_z, max_x, max_z) = open.map.floor_rect();
    let (w, _h, d) = open.map.bounds;
    for mut tf in &mut slab {
        *tf = Transform::from_xyz(
            open.map.origin.0 + (min_x + max_x) * 0.5,
            open.map.origin.1 - ground_drop(&project) - BOUNDS_FLOOR_BELOW_GROUND,
            open.map.origin.2 + (min_z + max_z) * 0.5,
        )
        .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
        .with_scale(Vec3::new(w, d, 1.0));
    }
}

/// **The map's cells, and only over the map.**
///
/// Replaces `bevy::dev_tools::infinite_grid`, which was wrong twice. It drew at 1.0 m while [`SNAP`]
/// is 0.5, so one square was two cells and a 1 m piece looked like it filled one — and it drew
/// *forever*, so the ground beyond `Map::bounds` looked exactly as buildable as the ground inside
/// them. Both were invisible until `BOUNDS_FILL` made the ground legible.
///
/// `Gizmos::grid` is bounded by construction — a cell count, not a plane — so "stop at the edge"
/// costs nothing and cannot drift from `draw_bounds`, which reads the same rectangle.
fn draw_map_grid(
    project: Res<Project>,
    open: Res<OpenMap>,
    keyboard: Res<ButtonInput<KeyCode>>,
    rung: Res<Rung>,
    settings: Option<Res<crate::settings::EditorSettings>>,
    mut gizmos: Gizmos,
) {
    // **The settings screen's two grid keys** (UI-71): `show_grid` turns the lines off and
    // `grid_strength` makes them fainter. Without the resource (a test that adds no settings) the grid is as it was.
    let (show, strength) = settings.map_or((true, 1.0), |s| (s.show_grid, s.grid_strength));
    if !show {
        return;
    }
    let (min_x, min_z, max_x, max_z) = open.map.floor_rect();
    let (w, _h, d) = open.map.bounds;
    // **The grid IS the lattice you will land on. There is no longer a second answer.**
    //
    // `pcgbook-ch11`: *"a human designer may become frustrated or confused if the computer
    // consistently acts as though it is not following the model that the human designer has in her
    // head."* This drew the held rung already — the half that was right — and fell back to the
    // author's cycled `GridSpacing` when no modifier was down, which could be 2 m or 4 m: lines no
    // piece could ever sit on. One expression now, so the picture cannot disagree with the commit.
    //
    // `pitch` divides `TILE` by a divisor clamped at 2, so it cannot return zero and ask the gizmo
    // for every cell there is.
    let level = snap_level(&keyboard, rung.0);
    let step = level.pitch(project.lattice.snap_divisor);

    // **One grid, at the live rung, over the whole map.**
    //
    // It used to draw two: the tile rung everywhere plus a bounded fine window at the camera focus
    // — the major/minor idiom, and defensible on paper. Reported from the keyboard 2026-08-14 and
    // decided there: *"there's, like, a big grid that covers the whole map, and then the small grid
    // that appears inside."* Two grids on one floor read as two answers to "where can this land",
    // and the window's edge reads as a boundary that is not one. The Tiles tab already draws only
    // the stops its arrows can reach; this is the same promise for the Map.
    //
    // **The known cost, stated because it was measured:** at the finest rung over the default 32 m
    // map this is 288 lines each way, which is dense. The window existed to avoid exactly that
    // wash. If the finest rung reads as a solid field in use, the fix is the rung's extent or its
    // colour — not a second grid.
    // The height every line sits at: on the backdrop, not above it. A cell an author has already
    // floored does not need a line drawn through it, and a line that survived the floor would be
    // drawing the grid on top of the map rather than under it.
    let y = open.map.origin.1 - ground_drop(&project) + BOUNDS_FILL_CLEARANCE * 0.5;
    // `grid` draws in the isometry's XY plane; a quarter turn about X lays it on the ground, the
    // same correction `spawn_bounds_floor` makes for its rectangle.
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);

    gizmos.grid(
        Isometry3d::new(
            Vec3::new(
                open.map.origin.0 + (min_x + max_x) * 0.5,
                y,
                open.map.origin.2 + (min_z + max_z) * 0.5,
            ),
            flat,
        ),
        // At least one cell each way: a map smaller than a cell still has a floor to draw.
        UVec2::new(
            ((w / step).round() as u32).max(1),
            ((d / step).round() as u32).max(1),
        ),
        Vec2::splat(step),
        GRID_LINE.with_alpha(GRID_LINE.alpha() * strength),
    );
}

/// **The map's envelope in world space** — centre, then size.
///
/// Extracted so the wireframe an author sees and the rect a badge must not cover are computed from
/// one expression. `badges::sense_world_ink` projects this to screen and calls it ink; if the two
/// ever disagreed, the overlay would carefully avoid a box that is not where the box is.
pub fn map_bounds(open: &OpenMap) -> (Vec3, Vec3) {
    // `floor_rect` is the floor PLAN — map space, centred on zero. Drawing happens in the world, so
    // the origin goes back on here.
    let (min_x, min_z, max_x, max_z) = open.map.floor_rect();
    let (floor, ceiling) = open.map.height_span();
    let (w, h, d) = open.map.bounds;
    (
        Vec3::new(
            open.map.origin.0 + (min_x + max_x) * 0.5,
            (floor + ceiling) * 0.5,
            open.map.origin.2 + (min_z + max_z) * 0.5,
        ),
        Vec3::new(w, h, d),
    )
}

/// **What a frame is asked to look at**: the whole map, the locked placement, placements by id, or a
/// rectangle. Every target resolves through [`resolve_frame`], so a name means one thing.
pub enum FrameTarget {
    Map,
    Selection,
    Placements(Vec<String>),
    /// A map-space rectangle `[x0, z0, x1, z1]`, in metres.
    Rect([f32; 4]),
}

/// **What a frame resolved to**: the box to see, in world metres, and how many placements lie in it.
pub struct Framing {
    /// Named the way the receipt and the reply both say it.
    pub what: String,
    pub centre: Vec3,
    pub size: Vec3,
}

/// **One placement's drawn box**: the axis-aligned box round its eight corners, carried through the same
/// yaw, tip and seat the editor draws it with (`bevy_grimoire::spawn_descriptor`). `None` for an unmeasured
/// piece, which has no box to frame. `ys` is the resolved heights, indexed as the placements are.
fn placed_box(project: &Project, open: &OpenMap, ys: &[f32], i: usize) -> Option<(Vec3, Vec3)> {
    let p = open.map.placements.get(i)?;
    let d = descriptor_of(&project.library, p)?;
    let (w, h, depth) = bevy_grimoire_core::descriptor::tipped_extents(&d, p.tip)?;
    let base = bevy_grimoire::origin_of(
        p.at,
        open.map.origin,
        *ys.get(i)? + bevy_grimoire::tip_seat(&d, p.tip),
    );
    let turn = Quat::from_rotation_y(bevy_grimoire::draw_yaw(&d, p.yaw).to_radians())
        * bevy_grimoire::tip_quat(p.tip);
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for sx in [-0.5, 0.5] {
        for sz in [-0.5, 0.5] {
            for sy in [0.0, 1.0] {
                let corner = base + turn * Vec3::new(sx * w, sy * h, sz * depth);
                lo = lo.min(corner);
                hi = hi.max(corner);
            }
        }
    }
    Some(((lo + hi) * 0.5, hi - lo))
}

/// The one box round a set of boxes: their union, as a centre and a size. `None` for an empty set.
fn enclose(boxes: impl IntoIterator<Item = (Vec3, Vec3)>) -> Option<(Vec3, Vec3)> {
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    let mut any = false;
    for (c, s) in boxes {
        any = true;
        lo = lo.min(c - s * 0.5);
        hi = hi.max(c + s * 0.5);
    }
    any.then(|| ((lo + hi) * 0.5, hi - lo))
}

/// **One placement's drawn box, by index.** Framing one piece and the resolver both read a box this way.
pub fn placement_bounds(project: &Project, open: &OpenMap, index: usize) -> Option<(Vec3, Vec3)> {
    let ys = heights(project, open).ok()?;
    placed_box(project, open, &ys, index)
}

/// The box round the named placements, each by its id. A name the map does not hold is refused by name.
fn named_box(
    project: &Project,
    open: &OpenMap,
    ys: &[f32],
    ids: &[String],
) -> Result<(Vec3, Vec3), String> {
    let mut boxes = Vec::with_capacity(ids.len());
    for id in ids {
        let i = open
            .map
            .placements
            .iter()
            .position(|p| &p.id == id)
            .ok_or_else(|| format!("no placement `{id}` on this map"))?;
        let b = placed_box(project, open, ys, i)
            .ok_or_else(|| format!("`{id}` has no measured size, so there is no box to frame"))?;
        boxes.push(b);
    }
    enclose(boxes).ok_or_else(|| "no placements to frame".to_string())
}

/// **Resolve a frame target to the box it means.** The one place a name is read, so no caller can read
/// a target differently. `lock` is the placement the target lock holds, which is the selection's
/// *primary* (`selection::Selection`): the frame target frames that one piece, not the set beside it.
/// A refusal says what to change, and nothing is
/// framed on one.
pub fn resolve_frame(
    target: &FrameTarget,
    project: &Project,
    open: &OpenMap,
    lock: Option<&str>,
) -> Result<Framing, String> {
    // **The stack is resolved by the targets that stand pieces on something, and not before.**
    // A map's own box is its floor plan and its height span, so no placement's Y can move it — and
    // resolving heights up front turned a map with one bad stack into a refusal, which left the Map
    // on `Rig::default()`'s 18 m close-up of the origin (B-018). The three targets that read
    // placements resolve the stack and refuse exactly as they did.
    let (what, centre, size) = match target {
        FrameTarget::Map => {
            let (c, s) = map_bounds(open);
            ("the whole map".to_string(), c, s)
        }
        FrameTarget::Selection => {
            let ys = heights(project, open)?;
            let id = lock.ok_or(
                "nothing is selected: click a placement on the Map to lock it, or name placements or a rectangle",
            )?;
            let (c, s) = named_box(project, open, &ys, &[id.to_string()])?;
            (format!("the selected placement `{id}`"), c, s)
        }
        FrameTarget::Placements(ids) => {
            if ids.is_empty() {
                return Err("no placements named: give at least one placement id".to_string());
            }
            let ys = heights(project, open)?;
            let (c, s) = named_box(project, open, &ys, ids)?;
            let names = ids
                .iter()
                .map(|id| format!("`{id}`"))
                .collect::<Vec<_>>()
                .join(", ");
            let noun = if ids.len() == 1 {
                "placement"
            } else {
                "placements"
            };
            (format!("{noun} {names}"), c, s)
        }
        FrameTarget::Rect(rect) => {
            let [x0, z0, x1, z1] = *rect;
            if rect.iter().any(|v| !v.is_finite()) || x1 <= x0 || z1 <= z0 {
                return Err(format!(
                    "a rectangle needs finite numbers with x0 < x1 and z0 < z1, in map metres; got [{x0}, {z0}, {x1}, {z1}]"
                ));
            }
            let ys = heights(project, open)?;
            let o = open.map.origin;
            // A rectangle is ground: its footprint is the rectangle, and its height is what stands inside
            // it. A tall piece elsewhere on the map must not raise the frame (cycle-3 item 1).
            let ground = Vec3::new(o.0 + (x0 + x1) * 0.5, o.1, o.2 + (z0 + z1) * 0.5);
            let plan = Vec3::new(x1 - x0, 0.0, z1 - z0);
            let inside = (0..open.map.placements.len())
                .filter_map(|i| placed_box(project, open, &ys, i))
                .filter(|(c, s)| {
                    c.x + s.x * 0.5 > o.0 + x0
                        && c.x - s.x * 0.5 < o.0 + x1
                        && c.z + s.z * 0.5 > o.2 + z0
                        && c.z - s.z * 0.5 < o.2 + z1
                });
            let (centre, size) = match enclose(inside) {
                Some((c, s)) => (
                    Vec3::new(ground.x, c.y, ground.z),
                    Vec3::new(plan.x, s.y, plan.z),
                ),
                None => (ground, plan),
            };
            (
                format!("the rectangle [{x0}, {z0}, {x1}, {z1}] m"),
                centre,
                size,
            )
        }
    };
    Ok(Framing { what, centre, size })
}

/// **The aspect of a camera's viewport**, width over height, or `None` before the viewport has a size.
pub fn aspect_of(camera: &Camera) -> Option<f32> {
    let size = camera.logical_viewport_size()?;
    (size.y > 0.0).then(|| size.x / size.y)
}

/// **Frame the Map camera on a resolved box.** The one place a frame is written to the rig, so every caller
/// frames the same way. Camera state only: the map does not change, so this is
/// not an operation and nothing is logged or undone.
pub fn apply_frame(
    rig: &mut crate::view::Rig,
    framing: &Framing,
    aspect: f32,
    open: &OpenMap,
) -> (Vec3, f32) {
    let (focus, height) = crate::view::frame_box(
        rig,
        framing.centre,
        framing.size,
        aspect,
        crate::view::max_zoom(Some(open), aspect),
    );
    rig.focus = focus;
    rig.height = height;
    (focus, height)
}

/// **Whether the Map has been framed for this door** (backlog E-022). A door resource, so a crossing
/// or a leave resets it with the rest of the door's state (`screen::door_state!`) and the next Map
/// opens framed. `done` makes the framing one-shot: after it, the person's own camera moves are never
/// overwritten. `last_size` is the viewport size seen on the previous update.
#[derive(Resource, Default)]
pub struct MapFraming {
    done: bool,
    last_size: Option<Vec2>,
}

/// **Frame the whole map once, when the Map opens.** A map that cannot be framed is a refusal and goes
/// in a problem, not a silent retry on every frame.
fn frame_map_on_open(
    mut framing: ResMut<MapFraming>,
    mut state: ResMut<EditorState>,
    mut rig: ResMut<crate::view::Rig>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    cameras: Query<&Camera, With<MainCamera>>,
) {
    if framing.done {
        return;
    }
    let (Some(project), Some(open)) = (project, open) else {
        return;
    };
    let Ok(camera) = cameras.single() else {
        return;
    };
    // **Wait for the layout.** The first viewport size is the window's guess, not the laid-out one, and
    // the Map is framed against the aspect it will be drawn at. So the frame waits until the size has
    // stopped changing across two updates.
    let size = camera.logical_viewport_size();
    if size != framing.last_size {
        framing.last_size = size;
        return;
    }
    let Some(aspect) = aspect_of(camera) else {
        return;
    };
    framing.done = true;
    match resolve_frame(&FrameTarget::Map, &project, &open, None) {
        Ok(frame) => {
            apply_frame(&mut rig, &frame, aspect, &open);
            // **The frame does not need the stack, and the author still does.** A map's own box is
            // its floor plan and its height span, so a bad stack no longer refuses the frame (B-018)
            // — but the reason it is bad is the one thing the author has to be told, and it used to
            // arrive only as this refusal. Asking the same `heights` the draw asks costs one walk
            // and keeps the sentence.
            if let Err(e) = heights(&project, &open) {
                state.status.problem(e);
            }
        }
        Err(e) => state.status.problem(e),
    }
}

/// **Home frames the view: the locked placement if there is one, otherwise the whole map.** The same
/// `resolve_frame` + `apply_frame` as `grimoire/frame_view`, so the key and the agent's method cannot
/// disagree about what the map or the selection is. A refusal goes in a problem.
#[allow(clippy::too_many_arguments)]
fn frame_on_home(
    keyboard: Res<ButtonInput<KeyCode>>,
    live: Res<keys::Live>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    lock: Option<Res<TargetLock>>,
    mut rig: ResMut<crate::view::Rig>,
    cameras: Query<&Camera, With<MainCamera>>,
    mut state: ResMut<EditorState>,
) {
    if !keys::just_pressed(&keyboard, *live, keys::Action::FrameView) {
        return;
    }
    let (Some(project), Some(open)) = (project, open) else {
        return;
    };
    let locked = lock
        .as_deref()
        .and_then(|l| l.placement_id().map(str::to_owned));
    let target = if locked.is_some() {
        FrameTarget::Selection
    } else {
        FrameTarget::Map
    };
    let frame = match resolve_frame(&target, &project, &open, locked.as_deref()) {
        Ok(frame) => frame,
        Err(why) => {
            state.status.problem(why);
            return;
        }
    };
    let Some(aspect) = cameras.single().ok().and_then(aspect_of) else {
        state.status.problem(
            "the Map camera has no viewport yet, so there is nothing to frame against".to_owned(),
        );
        return;
    };
    apply_frame(&mut rig, &frame, aspect, &open);
    state.status.note(format!(
        "framed {} ({:.0} x {:.0} m)",
        frame.what, frame.size.x, frame.size.z,
    ));
}

fn draw_bounds(
    keyboard: Res<ButtonInput<KeyCode>>,
    live: Res<crate::keys::Live>,
    open: Res<OpenMap>,
    mut gizmos: Gizmos,
) {
    let (centre, size) = map_bounds(&open);
    // Faded while the shortcut key is held, the same as the Tiles envelope — the two are the same
    // claim about the world, so they step back together (`chrome::WORLD_HELD`).
    let held = crate::keys::pressed(&keyboard, *live, crate::keys::Action::Shortcuts);
    // `cube`, not `cuboid` — 0.19 spells it `Gizmos::cube` and takes a transform whose SCALE is
    // the box's size (`bevy_gizmos-0.19.0/src/gizmos.rs:637`).
    gizmos.cube(
        Transform::from_translation(centre).with_scale(size),
        crate::chrome::stepped_back(BOUNDS_LINE, held),
    );
}

/// **Which palette row is armed**, and all that is left of `style_rows`.
///
/// `style_rows` restated `chrome::style_list_rows`'s entire rest / hover / selected priority
/// privately, in the one list that never adopted the shared row (the 2026-09-03 audit's F11). The
/// priority belongs to `chrome`, which now also owns pressed and disabled; what a tab still knows
/// and `chrome` cannot is *which* row is the one being acted on, and this says only that.
///
/// It has to be a system rather than a value baked in at spawn, because `rebuild_palette` runs on
/// `Project` / `Folded` / `Filters` and deliberately not on `EditorState` — arming a brush must not
/// respawn a hundred and eighty rows, which is the same reason `SizeEdit` and `RemovalDrag` were
/// split out of `EditorState` in the first place.
///
/// **All three carriers together.** `chrome::row_shape` sets the fill, the `RowSelected` flag and
/// the rail's colour as one decision, and a fill that disagreed with the rail is exactly what
/// `RowSelected` exists to make impossible. Compares before writing: all three are change-detected,
/// and this runs every frame over every row in the list.
fn arm_palette_rows(
    state: Res<EditorState>,
    mut rows: Query<(
        &PaletteRow,
        &mut crate::chrome::RowRest,
        &mut crate::chrome::RowSelected,
        &mut BorderColor,
    )>,
) {
    for (row, mut rest, mut selected, mut border) in &mut rows {
        let armed = state.brush == Some(row.0);
        if selected.0 != armed {
            selected.0 = armed;
        }
        let fill = if armed { ROW_SELECTED } else { ROW_BG };
        if rest.0 != fill {
            rest.0 = fill;
        }
        let edge = if armed { ACCENT } else { Color::NONE };
        if border.left != edge {
            *border = BorderColor::all(edge);
        }
    }
}

/// **One line of the UNDO or REDO row**: the newest entry's name, with ` (+N)` for the entries
/// stacked behind it, or a dim `nothing` when the stack is empty.
fn stack_row(next: Option<String>, depth: usize) -> (String, Color) {
    match next {
        Some(phrase) if depth > 1 => (format!("{phrase} (+{})", depth - 1), TEXT),
        Some(phrase) => (phrase, TEXT),
        None => ("nothing".to_owned(), DIM),
    }
}

/// **How many history rows show on each side of the current position (E-018).** The list is
/// `2 * HISTORY_EACH_WAY + 1` rows, always, so it never changes height and the rows below stay put.
const HISTORY_EACH_WAY: isize = 3;

/// **One row of the history list (E-018 option C).** `offset` is the row's distance from the
/// current position, fixed at spawn. `refresh_history` writes the rest each time the history
/// changes. A click jumps to `position` as one undo group (`jump`). The keyboard route to the same
/// row is `|offset|` presses of Cmd+Z (rows behind) or Cmd+Shift+Z (rows ahead); see
/// `docs/guide/building-maps.md`.
#[derive(Component)]
pub struct HistorySlot {
    pub offset: isize,
    /// The history position this row jumps to; `None` when the row is spare.
    pub position: Option<usize>,
    /// The row for the position the stacks are at now. A click on it does nothing.
    pub current: bool,
}

/// The text of one history row.
#[derive(Component)]
struct HistoryText;

/// **The history list under REDO (E-018).** Rows run from the redo side at the top to the undo side
/// at the bottom.
fn history_block(s: &mut ChildSpawnerCommands) {
    for offset in (-HISTORY_EACH_WAY..=HISTORY_EACH_WAY).rev() {
        // **Exactly one line, clipped (E-018).** A label that wrapped grew its row, and every row
        // below it, MAP SIZE included, moved down: `a_receipt_does_not_move_the_panel` caught it.
        // So the row is one line tall, its text does not wrap, and what runs past the panel is
        // clipped rather than pushing anything. The wrapper holds the row's own top and bottom inset
        // as well as its line, or the line's far edge is cut.
        s.spawn(Node {
            height: Val::Px((crate::chrome::BODY_LINE_H + 2.0 * crate::chrome::GAP_TIGHT).ceil()),
            overflow: bevy::ui::Overflow::clip(),
            ..default()
        })
        .with_children(|wrap| {
            let mut row = crate::chrome::quiet_row(
                wrap,
                false,
                HistorySlot {
                    offset,
                    position: None,
                    current: false,
                },
            );
            row.observe(history_row_clicked);
            // A spare or current row is not a pointer target: `refresh_history` sets this on each change.
            row.insert(bevy::picking::Pickable::default());
            row.with_children(|line| {
                line.spawn((
                    Text::new(""),
                    TextColor(TEXT),
                    crate::chrome::font(crate::chrome::text::BODY),
                    bevy::text::TextLayout::new(
                        bevy::text::Justify::Left,
                        bevy::text::LineBreak::NoWrap,
                    ),
                    HistoryText,
                    // Ignored, so the pointer hits the row and not its text (see `refresh_history`).
                    bevy::picking::Pickable::IGNORE,
                ));
            });
        });
    }
}

/// What a history row needs to jump: the same resources the keyboard's undo system takes.
#[derive(bevy::ecs::system::SystemParam)]
struct HistoryJump<'w, 's> {
    commands: Commands<'w, 's>,
    assets: Res<'w, AssetServer>,
    project: ResMut<'w, Project>,
    open: ResMut<'w, OpenMap>,
    state: ResMut<'w, EditorState>,
    placed: Query<'w, 's, (Entity, &'static Placement)>,
    /// **A drawn piece's own transform**, which is how the sweep tells a piece standing where the
    /// map wants it from one that is not (B-048).
    transforms: Query<'w, 's, &'static Transform>,
}

/// **A click on a history row jumps there, as one group (E-018).** Refused while the leaving
/// question is up, as the undo chord is: a jump edits the map the question is asking about.
fn history_row_clicked(
    click: On<bevy::picking::events::PointerClick>,
    slots: Query<&HistorySlot>,
    mut jump_to: HistoryJump,
) {
    let Ok(slot) = slots.get(click.entity) else {
        return;
    };
    if jump_to.state.leaving || slot.current {
        return;
    }
    let Some(position) = slot.position else {
        return;
    };
    let j = &mut jump_to;
    jump(
        &mut j.commands,
        &j.assets,
        &mut j.project,
        &mut j.open,
        &mut j.state,
        &j.placed,
        &j.transforms,
        position,
    );
}

/// **The history list's text (E-018).** Runs on the same change as the UNDO and REDO rows. Each row
/// reads the history as it stands now, so a click means the row's current position.
fn refresh_history(
    state: Res<EditorState>,
    open: Res<OpenMap>,
    mut slots: Query<(
        &mut HistorySlot,
        &Children,
        &mut bevy::picking::Pickable,
        &mut Visibility,
    )>,
    mut texts: Query<(&mut Text, &mut TextColor), With<HistoryText>>,
) {
    if !state.is_changed() && !open.is_changed() {
        return;
    }
    let depth = state.undo_depth();
    // **The window, not the whole list** (UI-53): only the rows on screen are labelled, so a session
    // with 5,000 entries costs seven labels a refresh rather than five thousand. The window is the
    // same `2 * HISTORY_EACH_WAY + 1` the block spawns, around the current position.
    let rows = state.history_window(
        (depth as isize - HISTORY_EACH_WAY).max(1) as usize,
        (depth as isize + HISTORY_EACH_WAY).max(1) as usize,
    );
    let first = rows.first().map_or(0, |r| r.position);
    for (mut slot, children, mut pickable, mut visibility) in &mut slots {
        let position = usize::try_from(depth as isize + slot.offset)
            .ok()
            .filter(|&p| p <= rows.len());
        slot.position = position;
        // **A spare row draws nothing, and still holds its height** (#302). The list is always
        // `2 * HISTORY_EACH_WAY + 1` rows tall so nothing below it moves as the history grows; a
        // row with no entry behind it was a blank bar. `Hidden`, not `Display::None`, keeps the room.
        let shown = if position.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != shown {
            *visibility = shown;
        }
        // **A spare or current row is not a pointer target** (the click observer refuses it), so it
        // must not light up on hover either. `Hovered` is read from the pointer's hit, and an ignored
        // entity is never hit, so the row goes non-pickable here and stays pickable when it is live.
        let target = slot.position.is_some() && !slot.current;
        let pick = if target {
            bevy::picking::Pickable::default()
        } else {
            bevy::picking::Pickable::IGNORE
        };
        if *pickable != pick {
            *pickable = pick;
        }
        let (want, tint) = match position {
            None => (String::new(), TEXT),
            Some(p) => {
                let (what, who) = if p == 0 {
                    ("before the first change".to_owned(), "")
                } else {
                    match rows.get(p.saturating_sub(first)) {
                        Some(row) => (row.label.clone(), if row.agent { "the Familiar" } else { "you" }),
                        None => (String::new(), ""),
                    }
                };
                let now = if slot.current { "now: " } else { "" };
                let who = if who.is_empty() {
                    String::new()
                } else {
                    format!("  \u{b7} {who}")
                };
                let tint = if p > depth { DIM } else { TEXT };
                (format!("{now}{what}{who}"), tint)
            }
        };
        for child in children.iter() {
            if let Ok((mut text, mut colour)) = texts.get_mut(child) {
                if text.0 != want {
                    text.0 = want.clone();
                }
                colour.0 = tint;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
/// **The status line's ink, from the editor's skin** (UI-02, #210).
///
/// The status band is the line an author reads without looking for it, and it was drawn from six
/// `chrome.rs` constants, so a project's skin file could not reach it.
///
/// **Each falls back to its constant**, the seam `chrome::RowFills` established: a harness with no
/// skin draws the shipped palette, which is what every protected test already reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatusInk {
    /// A field's ordinary value.
    pub read: Color,
    /// A value that is present but quiet.
    pub quiet: Color,
    /// A live answer, and an unsaved map — the two things that are happening *now*.
    pub accent: Color,
    /// A fault.
    pub danger: Color,
}

impl StatusInk {
    /// The skin's ink where it has one, the shipped constants where it does not.
    pub fn of(skin: Option<&bevy_gooey::GooeySkin>) -> StatusInk {
        let Some(skin) = skin else {
            return StatusInk { read: TEXT, quiet: DIM, accent: ACCENT, danger: DANGER };
        };
        StatusInk { read: skin.text, quiet: skin.text_muted, accent: skin.accent, danger: skin.danger }
    }
}

/// The three resources the status band reads, bundled so adding the skin did not push
/// `refresh_status` past clippy's argument limit (UI-02, #210).
#[derive(bevy::ecs::system::SystemParam)]
struct StatusReads<'w> {
    project: Res<'w, Project>,
    faults: Res<'w, EdgeFaults>,
    /// **The status line's ink, from the editor's skin** (UI-02, #210).
    skin: Option<Res<'w, bevy_gooey::GooeySkin>>,
}

fn refresh_status(
    StatusReads { project, faults, skin }: StatusReads<'_>,
    open: Res<OpenMap>,
    state: Res<EditorState>,
    under: Res<UnderCursor>,
    // **When the relations file was last read**, on the wall clock: a file on disk changes on it, and
    // virtual time clamps a long frame to 250 ms and can be paused.
    mut relations_read_at: Local<Option<std::time::Instant>>,
    mut fields: Query<(&Field, &mut Text, &mut TextColor), Without<ReceiptLabel>>,
    mut labels: Query<&mut Text, With<ReceiptLabel>>,
) {
    let StatusInk { read, quiet, accent, danger } = StatusInk::of(skin.as_deref());
    // `none` covers both "nothing armed" and "the armed index no longer exists", which read the same
    // to an author and want the same word.
    // A piece is named for people: `lamp_tall` is a file's name, `Lamp tall` is what the palette says.
    let brush = state
        .brush
        .and_then(|ix| project.library.descriptors.get(ix))
        .map_or_else(|| "none".to_owned(), |d| bevy_grimoire_core::naming::display_name(&d.id));

    // **The label follows the words in the slot** (B-013): `HINT` while a hint speaks, `LAST` otherwise.
    let label = if state.hint.is_empty() { "LAST" } else { "HINT" };
    for mut text in &mut labels {
        if text.0 != label {
            text.0 = label.to_owned();
        }
    }

    for (field, mut text, mut colour) in &mut fields {
        // **The undo rows are only read when the history or the map changed**, and once when they
        // spawn (their text starts empty). Recomputing two strings every frame is the cost this avoids.
        if matches!(field, Field::Undo | Field::Redo)
            && !text.0.is_empty()
            && !state.is_changed()
            && !open.is_changed()
        {
            continue;
        }
        // **The repeats are read only when the map or the library changed**: a pairwise pass over
        // every placement is too much to spend on a frame that changed nothing.
        if *field == Field::Repeats
            && !text.0.is_empty()
            && !open.is_changed()
            && !project.is_changed()
        {
            continue;
        }
        // **The relations file is re-read when the map changed, and about once a second**, so an
        // edit to it shows with no restart and no frame pays for a disk read it did not need.
        if *field == Field::Relations {
            let now = std::time::Instant::now();
            let fresh = relations_read_at.is_some_and(|at| now.duration_since(at).as_secs_f32() < 1.0);
            if !text.0.is_empty() && !open.is_changed() && fresh {
                continue;
            }
            *relations_read_at = Some(now);
        }
        let (want, tint) = match field {
            // Silent when the map agrees with itself, which is the common case and the one a standing
            // line must not shout about. `docs/ui.md` §1.2: more information helps, noise does not.
            Field::Edges => match faults.0.len() {
                0 => ("ok".to_owned(), quiet),
                1 => (faults.0[0].message.clone(), danger),
                n => (format!("{n} — {}", faults.0[0].message), danger),
            },
            // While renaming, the field shows what the name WILL be, not what was typed — that is
            // what "forced" means, and seeing `site_67` appear as you type "Site 67" teaches the rule
            // without anyone having to read it.
            Field::Name => match &state.renaming {
                // A caret, so an empty field reads as "waiting for you" rather than as the name
                // having been wiped.
                Some(raw) => (
                    format!("{}_", bevy_grimoire_core::naming::to_snake_case(raw)),
                    accent,
                ),
                None => (open.map.name.clone(), read),
            },
            Field::Rule => match &state.rule_seed {
                // A caret, so an empty field reads as "waiting for you".
                Some(raw) => (format!("{raw}_"), accent),
                None => (
                    rule_seed_of(Some(&project), &open.map.name),
                    read,
                ),
            },
            Field::Brush => (brush.clone(), read),
            // ACCENT while something is under the cursor, because it is a live answer that changes
            // as the mouse moves — the same colour the name field uses while it is being typed.
            Field::Under => (under.0.clone(), accent),
            // A caret while typing, so an empty field reads as "waiting for you" rather than as
            // nothing happening — the rename field's rule. Forced to snake_case as it is typed, so
            // the naming rule teaches itself.
            Field::Yaw => (format!("{} deg", state.brush_yaw), read),
            Field::Map => (
                // Counted by `bevy_grimoire_core::census`, never here — see
                // `tests/census_is_the_one_counter.rs` for why a rendered count is the one that
                // drifts. The stamp's rows are counted **inside** the total, because the total is
                // what the consumer has — the game spawns the group's rows too, so a number that
                // left them out would disagree with the world the author is authoring.
                {
                    // **The count the consumer will have**, not the count of hand-authored rows.
                    //
                    // A stamp is a reference, so the rows it stands for are derived — and a panel
                    // saying "728 placed" beside a game holding 746 is a panel lying. The
                    // census expands, so this is the same number `project_data::load` produces.
                    //
                    // **A map whose stamps do not expand says so**, rather than showing a number
                    // that quietly omits a house.
                    match bevy_grimoire_core::census::of_map(
                        &open.map,
                        &project.compositions.compositions,
                        &project.library,
                    ) {
                        Ok(counted) => {
                            // **The total first, then where it came from.**
                            //
                            // "745 placed (727 hand, 18 group(s))" reads as if the 18 were on top
                            // of the 745, which they are not — the 745 already counts them. So the
                            // breakdown says what is *included*, and the author can add the two
                            // numbers and get the number they read.
                            //
                            // **Short, because this row is one line.** The inspector's rows are
                            // labels and values on a single line each, and a value that wraps
                            // pushes the rows below it out of the panel — the "MAP" label sat
                            // beside two lines of value, which reads as two rows and is neither.
                            // Measured at the narrowest panel: the value column holds about 40
                            // characters before it wraps, so the breakdown has to fit in that.
                            //
                            // The number of **references** is not here. It is in the log line the
                            // map prints when it opens, and the reference count is not what the
                            // author is reading this row for — the row count is.
                            let stamps = match counted.stamps {
                                0 => String::new(),
                                _n => format!(
                                    " ({} hand, {} group(s))",
                                    counted.placements, counted.stamped
                                ),
                            };
                            let unsaved = if open.dirty() { ", unsaved" } else { "" };
                            format!("{} placed{stamps}{unsaved}", counted.total())
                        }
                        Err(e) => format!("cannot count this map: {e}"),
                    }
                },
                if open.dirty() { accent } else { read },
            ),
            // **What the next Cmd+Z and Cmd+Shift+Z will do**, named from the two stacks. See
            // [`stack_row`].
            Field::Undo => stack_row(
                state.undo_next(&open.map).map(|phrase| named_receipt(&phrase, &project.library)),
                state.undo_depth(),
            ),
            Field::Redo => stack_row(
                state.redo_next(&open.map).map(|phrase| named_receipt(&phrase, &project.library)),
                state.redo_depth(),
            ),
            // **The hint takes the receipt's slot while it speaks** (B-013), in the refusal colour: a
            // refusal has to read differently from a success, or "NOT SAVED" scrolls past as if it were a
            // receipt. The receipt comes back when the hint clears.
            Field::Last if !state.hint.is_empty() => (state.hint.clone(), accent),
            // **The receipt only, and never coloured by guessing.** This read
            // `status.starts_with("NOT SAVED")`, which made exactly one refusal red and left every
            // other one — `stamp refused:`, `is not a number of metres`, every `cannot draw it:` —
            // in the same grey as `stamped 4 piece(s)`. Refusals go to the block above now, and the
            // write site is what decides, so there is nothing left here to be wrong about.
            Field::Last => (named_receipt(&plain_receipt(state.status.note_text()), &project.library), quiet),
            // **The worst repeat first, then how many other looks repeat**, short enough for one line.
            // Dim when nothing repeats, the EDGES rule: a standing line that is fine stays quiet.
            // Under the project's `repetition.ron` (#139), the rules `grimoire_state` reports under; a
            // file that cannot be read says so in the refusal colour rather than falling back.
            Field::Repeats => {
                use bevy_grimoire_core::repetition::{repeats, FILE};
                match crate::ops_brp::load_repetition(&project.project_dir) {
                    Err(_) => (format!("{FILE} unreadable"), accent),
                    Ok(rules) => {
                        let all = repeats(&open.map.placements, &project.library, &rules);
                        match all.split_first() {
                            None => ("none".to_owned(), quiet),
                            Some((worst, rest)) => {
                                let more = match rest.len() {
                                    0 => String::new(),
                                    n => format!(", {n} more"),
                                };
                                (format!("{} \u{d7}{}{more}", worst.descriptor, worst.pieces), read)
                            }
                        }
                    }
                }
            }
            // **How many are broken, then the first broken one in words.** Dim when every relation
            // holds or there is no file, the EDGES rule; a file that does not read is named.
            Field::Relations => relations_readout(&open),
        };
        if text.0 != want {
            text.0 = want;
        }
        if colour.0 != tint {
            colour.0 = tint;
        }
    }
}

/// **A receipt that names a piece the way the palette does** (#302): `placed lamp_tall at (2.3, 1.3)`
/// becomes `placed lamp tall at (2.3, 1.3)`, and ``added `bath` `` becomes `added Bath`.
///
/// A piece's id is a file-safe name (`lamp_tall`), which the palette already shows as `Lamp tall`. Two
/// spellings are met in receipts and each is made what a person says:
///
/// - **A name in backticks** (a quoted id: ``added `bath` ``) loses its backticks and reads as the
///   palette spells it, because the quoting is how the code marks an id and the palette's name is what the
///   person sees on screen. Only a lone word is meant (letters, digits, `_`, `/` and `-`): a quoted file
///   (`kits.ron`) or a chord (`Ctrl+Z`) is not a name and keeps its quotes.
/// - **A name run into a sentence** (`placed lamp_tall`) is spelled lower case and spaced. Only a name the
///   library knows is touched, so a word that merely has an underscore in it (a file or a key) stays as it
///   is, and a receipt with no underscore costs nothing.
pub fn named_receipt(text: &str, library: &bevy_grimoire_core::library::Library) -> String {
    let text = unquote_names(text);
    if !text.contains('_') {
        return text;
    }
    let mut named: Vec<&str> = library
        .descriptors
        .iter()
        .filter_map(|d| d.id.rsplit('/').next())
        .filter(|name| name.contains('_') && text.contains(name))
        .collect();
    // The longest first, so a name that holds another is spelled out whole.
    named.sort_by_key(|name| std::cmp::Reverse(name.len()));
    named.dedup();
    let mut out = text.clone();
    for name in named {
        out = out.replace(name, &bevy_grimoire_core::naming::display_name(name).to_lowercase());
    }
    out
}

/// A quoted lone word is a name: it reads as the palette spells it, with no backticks (see [`named_receipt`]).
fn unquote_names(text: &str) -> String {
    // An odd number of backticks is not a quoting this can read: leave the text as it is.
    if !text.matches('`').count().is_multiple_of(2) {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    for (at, part) in text.split('`').enumerate() {
        let quoted = at % 2 == 1;
        let is_name = quoted && !part.is_empty() && part.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '/' | '-'));
        if is_name {
            out.push_str(&bevy_grimoire_core::naming::display_name(part));
        } else if quoted {
            out.push('`');
            out.push_str(part);
            out.push('`');
        } else {
            out.push_str(part);
        }
    }
    out
}

/// **A receipt, as a person reads it** (#302): `placed lamp_tall@1 at (2.31, 1.31)` becomes `placed
/// lamp_tall at (2.3, 1.3)`.
///
/// The `@1` is which of several pieces of one kind the operation meant, which an agent's log needs and a
/// person cannot see on the map; and a position to a hundredth of a metre is a precision no one placed
/// anything to. Only the readout is changed: the receipt itself, and the agent's log, keep every digit.
/// A coordinate pair (`(x, y)`) is rounded to a tenth; every other number is left exactly as it is.
pub fn plain_receipt(text: &str) -> String {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let chars: Vec<char> = text.chars().collect();
    // The `@N` of an instance, only where it follows a name and is the whole of its number.
    let mut without: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '@' && i > 0 && word(chars[i - 1]) {
            let digits = chars[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
            if digits > 0 && !chars.get(i + 1 + digits).copied().is_some_and(word) {
                i += 1 + digits;
                continue;
            }
        }
        without.push(chars[i]);
        i += 1;
    }
    // A pair of decimals in brackets, rounded to a tenth.
    let number = |from: usize| -> Option<(f32, usize)> {
        let mut end = from;
        if without.get(end) == Some(&'-') {
            end += 1;
        }
        let whole = without[end..].iter().take_while(|c| c.is_ascii_digit()).count();
        if whole == 0 {
            return None;
        }
        end += whole;
        if without.get(end) != Some(&'.') {
            return None;
        }
        let fraction = without[end + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
        if fraction == 0 {
            return None;
        }
        end += 1 + fraction;
        let value: String = without[from..end].iter().collect();
        value.parse().ok().map(|v| (v, end))
    };
    let mut out = String::with_capacity(without.len());
    let mut i = 0;
    while i < without.len() {
        if without[i] == '('
            && let Some((x, after_x)) = number(i + 1)
            && without.get(after_x) == Some(&',')
            && without.get(after_x + 1) == Some(&' ')
            && let Some((y, after_y)) = number(after_x + 2)
            && without.get(after_y) == Some(&')')
        {
            out.push_str(&format!("({x:.1}, {y:.1})"));
            i = after_y + 1;
            continue;
        }
        out.push(without[i]);
        i += 1;
    }
    out
}

/// **The `RELATIONS` row's words and tint** for the open map: `none` with no relations file, `N met`
/// when every relation holds, else `U unmet, M missing: <the first broken relation>`, and the file's
/// name when it does not read. The same check `grimoire_state.relations` runs (#137).
fn relations_readout(open: &OpenMap) -> (String, Color) {
    use bevy_grimoire_core::relations::{check_map, path_for_map, Kind, Outcome, Relations};
    let path = path_for_map(&open.map_path);
    let unreadable = || {
        let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        (format!("{name} does not read"), DANGER)
    };
    let relations = match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return ("none".to_owned(), DIM),
        Err(_) => return unreadable(),
        Ok(text) => match Relations::parse(&text) {
            Ok(r) => r,
            Err(_) => return unreadable(),
        },
    };
    if relations.relations.is_empty() {
        return ("none".to_owned(), DIM);
    }
    let outcomes = check_map(&open.map.placements, &relations);
    let unmet = outcomes.iter().filter(|o| matches!(o, Outcome::Unmet { .. })).count();
    let missing = outcomes.iter().filter(|o| matches!(o, Outcome::Missing { .. })).count();
    if unmet + missing == 0 {
        return (format!("{} met", outcomes.len()), DIM);
    }
    // The first unmet hard relation, else the first unmet one, else the first one naming a piece
    // the map does not have.
    let pairs = || relations.relations.iter().zip(&outcomes);
    let first = pairs()
        .find(|(r, o)| r.hard && matches!(o, Outcome::Unmet { .. }))
        .or_else(|| pairs().find(|(_, o)| matches!(o, Outcome::Unmet { .. })))
        .or_else(|| pairs().find(|(_, o)| matches!(o, Outcome::Missing { .. })));
    let words = match first {
        Some((_, Outcome::Missing { id })) => format!("{id} is not on the map"),
        Some((r, outcome)) => {
            let of = &r.of;
            let what = match &r.kind {
                Kind::Faces(to) => format!("{of} faces {to}"),
                Kind::Near { to, .. } => format!("{of} near {to}"),
                Kind::Apart { from, .. } => format!("{of} apart from {from}"),
                Kind::InFrontOf { of: other, .. } => format!("{of} in front of {other}"),
                Kind::Behind { of: other, .. } => format!("{of} behind {other}"),
                Kind::LeftOf { of: other, .. } => format!("{of} left of {other}"),
                Kind::RightOf { of: other, .. } => format!("{of} right of {other}"),
                Kind::ClearInFront { .. } => format!("{of} clear in front"),
                Kind::VisibleFrom { from, .. } => format!("{of} seen from {from}"),
                Kind::Along { path, .. } => format!("{of} along {}", path.join(" → ")),
                Kind::ReachableFrom { from, .. } => format!("{of} reachable from {from}"),
                Kind::AlignedWith(to) => format!("{of} aligned with {to}"),
                Kind::Between { a, b, .. } => format!("{of} between {a} and {b}"),
                Kind::MirrorOf { of: other, about, .. } => format!("{of} mirror of {other} about {about}"),
                Kind::BacksOnto { of: other, .. } => format!("{of} backs onto {other}"),
                Kind::Inside { of: other, .. } => format!("{of} inside {other}"),
                Kind::Balances { with, about, .. } => format!("{of} balances {} about {about}", with.join(", ")),
            };
            let what = if r.hard { format!("{what} (hard)") } else { what };
            match outcome {
                Outcome::Unmet { by: Some(by), .. } => format!("{what} ({by} in the way)"),
                _ => what,
            }
        }
        None => String::new(),
    };
    let mut counts = Vec::new();
    if unmet > 0 {
        counts.push(format!("{unmet} unmet"));
    }
    if missing > 0 {
        counts.push(format!("{missing} missing"));
    }
    (format!("{}: {words}", counts.join(", ")), DANGER)
}

/// **Every text field that can be taking raw keys, in one parameter.**
///
/// This is [`crate::keys::Context::Typing`] in the census's terms — the context that overlaps every
/// other one and suppresses all of them.
///
/// **One list, and now one type.** There were two (`not_typing` and `tiles.rs`'s
/// `not_renaming_candidate`), each knowing about the fields of its own tab, and a filter box would
/// have made three: a system gated on the wrong one fires while you type, which is how `2` in a text
/// box lands you on another tab. Then [`not_typing`] and [`sense_context`] each grew their own copy
/// of the list, which is the same drift one level up — and the second copy hit Bevy's sixteen-parameter
/// limit the moment the journal had to be asked as well. One bundle, read by both.
///
/// The buffers stay with their fields — a name, a pin's reason, an axis, a candidate id and a filter
/// hold different things and commit differently. That is several kinds of state, not one fact written
/// several times, so this reads them rather than owning them. The fields are `pub` because
/// [`sense_context`] also asks `build` what is in hand.
///
/// **The filter boxes are deliberately absent.** A filter box is a text field, but it is the one that
/// keeps its own list walking — see [`keys::Holder::Filter`] — so the caller folds `Filters` in
/// itself and says what it bought.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Fields<'w> {
    pub state: Res<'w, EditorState>,
    pub size: Res<'w, SizeEdit>,
    /// **The family settings field (#96).**
    pub family: Res<'w, FamilyEdit>,
    /// **The data field (#86).**
    pub data: Res<'w, DataEdit>,
    /// **The terrain field (#99).**
    pub terrain: Res<'w, TerrainEdit>,
    /// **The crowd field (#107).**
    pub crowd: Res<'w, CrowdEdit>,
    pub import: Res<'w, crate::tiles::ImportState>,
    pub cell: Res<'w, crate::tiles::CellEdit>,
    pub note: Res<'w, crate::tiles::NoteEdit>,
    /// **The note box (#89).** Registered here so no other key fires while it is open.
    pub note_box: Res<'w, NoteBox>,
    pub width: Res<'w, crate::tiles::ScaleEdit>,
    pub height: Res<'w, crate::tiles::HeightEdit>,
    /// The tile name prompt, added when naming became explicit (2026-08-15). Leaving it out is not
    /// cosmetic: the `Enter` that confirms a name is also `BuildDrop`, so without it the same
    /// keypress named the tile and dropped a piece into it.
    pub build: Res<'w, crate::build::Build>,
    /// The vocabulary prompt, added with the tag block's `+` chip (2026-08-22). While it is open,
    /// typing a name containing `n` must not rotate the mesh — the guard this list exists to give.
    pub token: Res<'w, crate::token_prompt::TokenPrompt>,
    /// The COMPOSE tab's rename prompt, added 2026-10-21. **The same reason as `build` above, and
    /// the same one the `grouping` line records**: the `Enter` that confirms a name is also
    /// `ComposeArm`, so without this line the same keypress named the composition *and* armed it —
    /// and the arm's message overwrote the rename's receipt, so the author was told the composition
    /// was armed and never heard that every map that stamped it follows.
    pub compose: Res<'w, crate::compose::ComposeState>,
    /// The snip's one-line note, added with E-014. Without this line, typing a note would also
    /// fire every action its letters are bound to.
    pub snip: Res<'w, crate::snip::SnipEdit>,
}

/// **The chords the settings gave to commands** (UI-72b, #265), as [`sense_context`] hands them to the
/// dispatcher in [`keys::Live`]. The table is rebuilt only when the settings changed: a command's id
/// names its action, and a chord the dispatcher then answers to instead of the census's.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Rebinding<'w, 's> {
    /// What the user and the project rebound; absent in a test that builds no settings.
    settings: Option<Res<'w, crate::settings::EditorSettings>>,
    table: Local<'s, keys::Rebound>,
}

impl Rebinding<'_, '_> {
    fn table(&mut self) -> keys::Rebound {
        if self.settings.as_ref().is_some_and(|s| s.is_changed()) {
            *self.table = keys::Rebound::of(self.settings.iter().flat_map(|s| {
                s.keymap.iter().filter_map(|(id, chord)| Some((crate::commands::by_id(id)?.action, *chord)))
            }));
        }
        *self.table
    }
}

/// What the tabs can be holding or waiting on. Read as one param by [`sense_context`], which folds
/// these into the `Stance` the keyboard census answers from.
#[derive(bevy::ecs::system::SystemParam)]
pub struct InHand<'w> {
    move_drag: Res<'w, MoveDrag>,
    clone_drag: Res<'w, CloneDrag>,
    proposal: Res<'w, Proposal>,
    derived: Res<'w, crate::tiles::DerivedEdges>,
    picker: Res<'w, crate::compose::picker::Picker>,
}

impl Fields<'_> {
    /// Is one of them taking keys right now? Adding a field is adding a line, in
    /// [`Self::typing_beside_the_note_box`] and nowhere else.
    pub fn typing(&self) -> bool {
        self.typing_beside_the_note_box() || self.note_box.draft.is_some()
    }

    /// **Every field but the note box** (#173). The box holds the keys like any field, but a box of
    /// Shift-clicked pieces also takes more Shift-clicks, so the Note tool's mouse guard
    /// ([`note_takes_clicks`]) must not count it.
    fn typing_beside_the_note_box(&self) -> bool {
        self.state.renaming.is_some()
            // **The rule seed field (E-081).** Found by this iteration's own review: without this
            // line, typing `1234` into the seed sent `1` through `Map`'s actions and nothing
            // visible happened, but a seed containing a letter-ish key would have fired that verb
            // — `R` turns, `N` renames, `K` reopens this very field. The list is the ONE place a
            // field is registered, and a field missing from it is a field whose keys act twice.
            || self.state.rule_seed.is_some()
            || self.state.pinning.is_some()
            // **`grouping` was missing here**, so naming a captured composition also dispatched Map
            // actions for every letter typed — `corner` fired aim, turn, rename-map and turn-view.
            || self.state.grouping.is_some()
            || self.size.active.is_some()
            || self.family.active.is_some()
            || self.data.active.is_some()
            || self.terrain.active.is_some()
            || self.crowd.active.is_some()
            || self.import.renaming.is_some()
            || self.cell.typing()
            || self.note.typing()
            || self.width.typing()
            || self.height.typing()
            || self.build.naming.is_some()
            || self.token.open.is_some()
            || self.compose.renaming.is_some()
            || self.snip.pending.is_some()
    }
}

/// While a name is being typed, every other key belongs to the name.
///
/// `Option<Res<_>>` is not needed here because every resource in [`Fields`] is `init_resource`d by
/// this same plugin — but every condition in a `.run_if(..)` chain IS evaluated in Bevy 0.19, so a
/// condition reading a resource some *other* plugin owns must take the option. Worth stating next to
/// the one place that legitimately does not.
/// **Whether a command palette holds the keys**: one that is open, or coming back. A palette on its way out
/// (gooey's `Leaving`, its card fading) has given them back, so the command it ran is not refused as
/// "typing" and the keys pressed during its fade reach the tab. The component outlives the close by the
/// length of the exit, which is why the marker alone is not the answer.
fn palette_is_open(palette: &Query<Option<&bevy_gooey::Leaving>, With<bevy_gooey::CommandPalette>>) -> bool {
    palette.iter().any(|leaving| leaving.is_none_or(bevy_gooey::Leaving::returning))
}

pub fn not_typing(
    fields: Fields,
    filters: Res<crate::filter::Filters>,
    // The command palette's field is gooey's own, so it is asked of its marker (see `sense_context`).
    palette: Query<Option<&bevy_gooey::Leaving>, With<bevy_gooey::CommandPalette>>,
) -> bool {
    !fields.typing() && !filters.typing() && !palette_is_open(&palette)
}

/// **The Note tool's mouse guard** (#173): [`not_typing`], except that the note box does not count.
/// `drive_note` decides which open box takes a click (a box of Shift-clicked pieces takes more; any
/// other holds the keys and ignores the mouse). Behind `not_typing`, the first Shift-click opened the
/// box and the box then shut `drive_note` off, so every later Shift-click was dropped.
pub fn note_takes_clicks(fields: Fields, filters: Res<crate::filter::Filters>) -> bool {
    !fields.typing_beside_the_note_box() && !filters.typing()
}

/// **Decide who owns the keyboard, once, before anything reads a key.**
///
/// Runs in [`keys::Phase::Sense`], which is ordered before both the action systems and the text
/// fields. That ordering is not tidiness — see [`keys::Phase`] for the defect it fixes, in which a
/// text field cleared its own flag mid-frame and the run condition guarding `Enter` re-evaluated to
/// true behind it, importing six descriptors into `library.ron` by accident.
///
/// This reads exactly the fields [`not_typing`] reads, and for the same reason: the buffers stay with
/// their fields, so adding a field is adding a line *here* — the one list — rather than a new guard
/// somebody else has to remember to consult.
// Eight: one more than clippy's budget. The other seven are what the census reads, and the keymap is
// one more fact about who owns which key; folding it into one of the bundles would hide it from the
// ratchet tests that read system signatures.
#[allow(clippy::too_many_arguments)]
pub fn sense_context(
    mode: Res<crate::tiles::Mode>,
    fields: Fields,
    filters: Res<crate::filter::Filters>,
    held: InHand,
    mut live: ResMut<keys::Live>,
    // The panel, so the journal can be told it owns the keyboard. A `Query` rather than a resource
    // because `Display::None` on the node *is* the open/closed state — `notice::toggle_journal`
    // writes exactly that, and a second flag beside it would be a second answer.
    journal: Query<&Node, With<crate::chrome::JournalPanel>>,
    // **The command palette's field takes the keys** (UI-32). It is gooey's own text field, which none
    // of [`Fields`] knows about, so "a palette is open" is asked of the palette's marker: without
    // it, typing `undo` in the palette would also press U, N, D and O on the tab behind it.
    palette: Query<Option<&bevy_gooey::Leaving>, With<bevy_gooey::CommandPalette>>,
    // **A report owns the keys while it is open** (`report.rs`). Its overlay root is the state, so
    // the question has one answer and it is the drawn thing: without this, pointing at a panel to
    // report on it also pressed every key on the tab behind it.
    report: Query<(), With<crate::report::ReportOpen>>,
    mut rebinding: Rebinding,
    // **A question that owns the keyboard** (today the trust question only, see [`crate::confirm::Asked::owns_keyboard`]):
    // the key that answers it (`Y`, `P`, `N`) is not also a verb on the tab behind it. Measured 2026-11-02:
    // `N` to decline opened the map-rename field and `Y` tipped the brush, because nothing here knew a
    // question was up. `Option`, because the menu screen has no editor state of its own.
    confirm: Option<Res<crate::confirm::Confirm>>,
    // **The terminal owns the keys while it has them** (`terminal_pane::TerminalFocus`). Folded into
    // the same `typing` answer a text field gives, deliberately rather than given a clause of its own:
    // `Live(Context::Typing, ..)` is the one gate every binding already respects, and a second swallow
    // beside it would be two answers to "who gets this key" — the failure `keys.rs`'s header refuses.
    // A shell that typed `d` and moved the camera would be unusable, and this one line is the whole
    // defence against it.
    terminal: Option<Res<crate::terminal_pane::TerminalFocus>>,
) {
    let InHand {
        move_drag,
        clone_drag,
        proposal,
        derived,
        picker,
    } = held;
    // **The filter box is a text field with a named exemption, not a context of its own.** Every
    // other field here makes the whole census quiet ([`keys::Context::Typing`]); a filter box narrows
    // a list that must keep walking while it holds the keys, so it also sets [`keys::Holder::Filter`]
    // below. It used to be a `Context`, which threw the tab away and fired every tab's walk on one
    // arrow — see [`keys::Holder`].
    let filter_typing = filters.typing();
    let typing = fields.typing()
        || palette_is_open(&palette)
        || confirm.as_deref().is_some_and(|c| c.owns_keyboard())
        || !report.is_empty()
        // **The terminal is a text field as far as the census is concerned.** It takes raw keys the
        // way a filter box does, and it is the one field whose whole purpose is that a command
        // typed into it is not also a command in the editor.
        || terminal.as_deref().is_some_and(crate::terminal_pane::TerminalFocus::holds);
    // **What is in hand, asked of the tab that can be holding something.** Two tabs can, and they hold
    // different things: the assembler has a piece taken with `Space`, the map has one picked up in
    // Move or a set captured in Clone. Both answer the same question — *do the arrows move a thing, or
    // walk a list* — so both fold into one `Stance` rather than into two tab-shaped flags, which is
    // what lets `keys::rows` render the answer without knowing which tab asked.
    let stance = match mode.context() {
        // **Holding means there is something the arrows would move — nothing else.**
        //
        // Twice now this was keyed on the wrong fact, and both were reported from the keyboard. First
        // on `Build::placing`, which only `Space` set: `Enter` brings a piece in without it, so the
        // arrows went dead over a member sitting focused in the tile. Then on a drop *setting*
        // `placing`, which fixed that and broke the other end — removing the last member left
        // `placing` true over an empty tile, so the arrows went on trying to move a piece that was
        // no longer there instead of walking the library, and the next `Enter` re-dropped the same
        // mesh. Measured: two captures of the "second" drop, byte-identical.
        //
        // **Both, and neither alone was enough** — each was tried and each broke the other end.
        //
        // `placing` is the *intent*: `Space` takes a piece, a drop leaves you adjusting what you
        // dropped, `Esc` puts you back on the library list. `focused` is the *subject*: is there a
        // member for the arrows to act on. Holding on intent alone left the arrows moving a piece
        // that had just been deleted, so the next `Enter` re-dropped the same mesh — measured live,
        // two captures byte-identical. Holding on the subject alone made it permanent: with anything
        // in the tile the library could never be walked again, so every second drop was a repeat.
        //
        // The pair says what the arrows are for: there is a piece, and you are placing it.
        // **The Tiles page outranks a held piece**, and cannot honestly collide with it: `TileOpen`
        // and `PageEnter` are bound at `Browsing`, so the page's keys are what a held piece cannot
        // swallow — and opening from the page clears `placing` and `browsing` together.
        keys::Context::Tiles if fields.build.browsing.is_some() => keys::Stance::Browsing,
        keys::Context::Tiles if fields.build.placing && crate::build::focused(&fields.build) => {
            keys::Stance::Holding
        }
        // **A proposal outranks a held piece**, because it is the more consequential thing waiting
        // for an answer — and because the two cannot honestly be true at once anyway: `generate_from`
        // is `Stance::Idle`, so nothing can be in hand at the moment a proposal is made.
        keys::Context::Map if proposal.0.is_some() => keys::Stance::Proposed,
        // The same phase, one tab over: edge tokens read off a mesh are a proposal in exactly
        // the sense a generated layout is, and they take the same door. See
        // `tiles::DerivedEdges`.
        keys::Context::Meshes if derived.0.is_some() => keys::Stance::Proposed,
        keys::Context::Map if move_drag.held.is_some() || clone_drag.held.is_some() => {
            keys::Stance::Holding
        }
        // **The composition picker has the arrows.** It is asked *after* the proposal, because a
        // proposal is the door's own state and it owns `Enter`; the picker is the author's, and the
        // author can always `Esc` out of a proposal first.
        keys::Context::Map if picker.active => keys::Stance::Picking,
        _ => keys::Stance::Idle,
    };
    // **Journal over filter over tab**, because that is the drawing order. The tab is carried in
    // every case now; only the holder changes — that is the whole of the fix, and it is why
    // `keys::Context::Filter` no longer exists.
    let holder = if journal.iter().any(|n| n.display != Display::None) {
        keys::Holder::Journal
    } else if filter_typing {
        keys::Holder::Filter
    } else {
        keys::Holder::Tab
    };
    // A filter box **is** a text field, so it sets `typing` as well: letters go to the box, not to
    // the verbs. What `Holder::Filter` buys is the exemption for that pane's own list walk, and
    // `keys::allowed` consults the tab through the row's own context — so the walk that stays live is
    // the one belonging to the tab the box is on, and no other.
    let mut want = keys::live(mode.context(), typing || filter_typing, stance, holder);
    want.3 = rebinding.table();
    // Written through the change detector only when it actually moves, so `Live` staying put does not
    // wake every `resource_changed` reader in the editor on every frame.
    if *live != want {
        *live = want;
    }
}

/// Placing belongs to map mode. Without this, `F` in import mode would flood the map with whatever
/// the palette last had armed, which is a surprising amount of work to undo.
///
/// **`Option<Res<..>>`, because `Mode` is a door's resource now.** `args::Opened::remove_from` drops
/// it on the way out of a door, and **every run condition is evaluated** — a bare `Res<Mode>` here
/// panicked on the first `Update` after `Esc` took the author back to the menu, even though the
/// `in_state(Editor)` condition beside it had already answered false. Same shape as
/// `Door::map_door_is_open`, and for the same reason.
pub(crate) fn in_map_mode(mode: Option<Res<crate::tiles::Mode>>) -> bool {
    mode.is_some_and(|m| *m == crate::tiles::Mode::Map)
}

/// **The rule seed the project states for this map**, or `none` when no rule names it.
///
/// The field shows this while it is not being typed into, so the author reads the number a generate
/// would actually use rather than a blank.
fn rule_seed_of(project: Option<&Project>, map: &str) -> String {
    match project.and_then(|p| p.rules.blocks.iter().find(|b| b.example == map)) {
        Some(rule) => rule.seed.to_string(),
        None => "none".to_owned(),
    }
}

/// **Type the open map's rule seed, and commit it through `rules_set` (E-081).**
///
/// The person's half of the operation: `K` opens a one-number field, Enter dispatches `rules_set` with
/// the same parameters an agent sends, Esc leaves it alone. Without this a rule was reachable by an
/// agent and by nobody at the keyboard, which is the parity the brief calls a must-fix bug.
///
/// **`events.clear()` before the early return**, the same invariant `rename_keys` documents: the
/// reader only advances when `read()` is called, so without it the `K` that opened the field is read
/// as its first character next frame.
fn rule_seed_keys(
    mut events: MessageReader<KeyboardInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    live: Res<keys::Live>,
    mut project: Option<ResMut<Project>>,
    mut open: ResMut<OpenMap>,
    mut state: ResMut<EditorState>,
) {
    if state.rule_seed.is_none() {
        if keys::just_pressed(&keyboard, *live, Action::SetRule) {
            // **Refused before the field opens**, so a project with no rule for this map says so
            // instead of offering a field whose Enter would be refused.
            let named = project
                .as_deref()
                .is_some_and(|p| p.rules.blocks.iter().any(|b| b.example == open.map.name));
            if !named {
                state.status.problem(format!(
                    "no rule is learned from `{}`, so there is no seed to set. rules.ron names the \
                     maps that have one.",
                    open.map.name
                ));
            } else {
                state.rule_seed = Some(String::new());
                state.status.note(format!(
                    "type a seed for the rule on `{}` — Enter to set it, Esc to leave it alone",
                    open.map.name
                ));
            }
        }
        events.clear();
        return;
    }
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }
        match &event.logical_key {
            Key::Enter => {
                let raw = state.rule_seed.take().unwrap_or_default();
                set_rule_seed(project.as_deref_mut(), &mut open, &mut state, &raw);
            }
            Key::Escape => {
                state.rule_seed = None;
                state.status.note("seed unchanged".to_owned());
            }
            Key::Backspace => {
                if let Some(raw) = state.rule_seed.as_mut() {
                    raw.pop();
                }
            }
            Key::Character(c) => {
                if let Some(raw) = state.rule_seed.as_mut() {
                    for ch in c.chars().filter(char::is_ascii_digit) {
                        raw.push(ch);
                    }
                }
            }
            _ => {}
        }
    }
}

/// **Commit the typed seed through the registry**, the way the size field and the rename do.
fn set_rule_seed(
    project: Option<&mut Project>,
    open: &mut OpenMap,
    state: &mut EditorState,
    raw: &str,
) {
    let Ok(seed) = raw.parse::<u64>() else {
        state
            .status
            .problem(format!("`{raw}` is not a whole number, so the seed is unchanged"));
        return;
    };
    let mut ctx = crate::ops::Ctx {
        view: None, trust_file: None,
        project: project.as_ref().map(|p| (**p).clone()),
        open: Some(std::mem::replace(
            open,
            OpenMap::new(
                bevy_grimoire_core::map::Map::default(),
                std::path::PathBuf::new(),
            ),
        )),
        source: crate::ops::Source::Human,
        root: None,
        session: crate::ops::session_id(),
        proposal: None,
    };
    let params = serde_json::json!({"example": open.map.name, "seed": seed});
    let outcome = crate::ops::dispatch(&mut ctx, "rules_set", &params);
    if let Some(fresh) = ctx.open.take() {
        *open = fresh;
    }
    if let Some(project) = project {
        carry_changes(project, &mut ctx);
    }
    match outcome {
        Ok(crate::ops::Applied::Done {
            outcome,
            operation_id,
            ..
        }) => {
            state.status.note(format!("{}; seed {seed}", outcome.summary));
            // **On the undo stack**, so Cmd+Z puts the old seed back and writes it.
            state.record(Undo::Logged {
                line: operation_id,
                phrase: format!("seed {seed}"),
                agent: false,
            });
        }
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
    }
}

/// Type a name. Snake case is applied to what is shown and to what is committed, so the illegal state
/// is never reachable rather than merely being rejected at the end.
fn rename_keys(
    mut events: MessageReader<KeyboardInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    live: Res<keys::Live>,
    mut project: Option<ResMut<Project>>,

    mut open: ResMut<OpenMap>,
    mut state: ResMut<EditorState>,
) {
    if state.renaming.is_none() {
        // `N` starts a rename. The comment here used to claim reading from the buffered events was
        // enough to stop a keypress both starting the rename and being typed into it; it is not —
        // the reader's cursor only advances when `read()` is called, and this branch returns without
        // calling it. `events.clear()` below is what actually holds the invariant.
        if keys::just_pressed(&keyboard, *live, Action::RenameMap) {
            // **Empty, not seeded with the current name.** Seeding it meant the first keystroke
            // appended, so renaming `site_67_hub` to `galley_deck` produced
            // `site_67_hubgalley_deck` — and it looked like it had worked, because the panel showed a
            // name growing as expected. A real rename field starts with the old name SELECTED so
            // typing replaces it; there is no selection model here, so the honest equivalent is to
            // start blank. The old name is still on screen the moment you press Esc.
            state.renaming = Some(String::new());
            state.status.note(format!(
                "type a new name for `{}` — Enter to keep it, Esc to leave it alone",
                open.map.name
            ));
        }
        // Drain before leaving, so the `N` that opened the field is not read as its first character
        // next frame. Same invariant as `tiles::cell_keys`.
        events.clear();
        return;
    }

    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }
        match &event.logical_key {
            Key::Enter => {
                let raw = state.renaming.take().unwrap_or_default();
                rename_map(project.as_deref_mut(), &mut open, &mut state, &raw);
            }
            Key::Escape => {
                state.renaming = None;
                state.status.note("name unchanged".to_owned());
            }
            Key::Backspace => {
                if let Some(raw) = state.renaming.as_mut() {
                    raw.pop();
                }
            }
            Key::Space => {
                if let Some(raw) = state.renaming.as_mut() {
                    raw.push(' ');
                }
            }
            Key::Character(s) => {
                if let Some(raw) = state.renaming.as_mut() {
                    raw.push_str(s);
                }
            }
            _ => {}
        }
    }
}

/// **Triangles actually drawn this frame**, bottom right.
///
/// Visible entities, not resident ones — the same distinction `perf_probe` draws in the game, and for
/// the same reason: a culled mesh costs memory, not frame time, and mixing them is how you end up
/// optimising the wrong thing. It is here because a flood fill can lay down 1,400 pieces in one
/// keystroke, and the moment to notice that costs 40 million triangles is while it is happening.
///
/// **A mesh the render world took is not counted** (#193): `bevy_elements`' stars, moon and lightning are
/// `RENDER_WORLD`-only, so after their first frame the main world holds no indices for them and
/// `Mesh::indices()` panics. Asking with `try_indices_option` skips them instead of aborting the editor.
fn refresh_triangle_total(
    drawn: Query<(&ViewVisibility, &Mesh3d)>,
    meshes: Res<Assets<Mesh>>,
    settings: Option<Res<crate::settings::EditorSettings>>,
    mut readout: Query<(&mut Text, &mut TextColor), With<TriangleTotal>>,
) {
    // **Behind View → Show statistics** (#302). A count the author did not ask for is the first thing
    // a newcomer reads at the top of the window, and it says nothing they can act on; so while it is
    // off the readout says nothing and the sum over every visible mesh is not taken.
    if !settings.is_some_and(|s| s.show_statistics) {
        for (mut text, _) in &mut readout {
            if !text.0.is_empty() {
                text.0.clear();
            }
        }
        return;
    }
    let total: usize = drawn
        .iter()
        .filter(|(v, _)| v.get())
        .filter_map(|(_, m)| meshes.get(&m.0))
        .filter_map(|m| match m.try_indices_option() {
            Ok(Some(i)) => Some(i.len() / 3),
            Ok(None) => Some(m.count_vertices() / 3),
            Err(_) => None,
        })
        .sum();

    let want = format!("{} tris drawn", with_thousands(total));
    for (mut text, mut colour) in &mut readout {
        if text.0 != want {
            text.0 = want.clone();
        }
        let tint = if total > HEAVY_SCENE {
            DANGER
        } else if total > BUSY_SCENE {
            ACCENT
        } else {
            DIM
        };
        if colour.0 != tint {
            colour.0 = tint;
        }
    }
}

/// A triangle count at a glance: `1.5k`, `70k`, `4.2M`. Exact numbers belong in the total at the
/// bottom; a palette row needs an order of magnitude.
fn brief_count(n: usize) -> String {
    match n {
        0 => String::new(),
        n if n < 1_000 => format!("{n}"),
        n if n < 100_000 => format!("{:.1}k", n as f32 / 1_000.0),
        n if n < 1_000_000 => format!("{}k", n / 1_000),
        n => format!("{:.1}M", n as f32 / 1_000_000.0),
    }
}

/// **What a palette row's tooltip says**: the piece's id, as operations and files name it, and its
/// triangle count with its unit, which the row's own column draws bare (#289).
fn palette_tooltip(id: &str, triangles: usize) -> String {
    if triangles == 0 {
        return id.to_owned();
    }
    format!("{id} · {} triangles", with_thousands(triangles))
}

/// Group a number for reading. The total is meant to be compared against itself over time, so the
/// digits matter.
///
/// `pub` because [`COST_COL`] is sized for the widest string this can produce, and the test that
/// checks the sizing has to ask *this* function rather than restate the format — a second copy of it
/// would agree with the constant while disagreeing with the readout.
pub fn with_thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A piece worth a second look before filling a room with it, and one worth a first look.
///
/// Calibrated on the shipped kit rather than picked: its median is 1,526 triangles and its
/// second-densest piece is 7,996, so 20k is well clear of everything except the one genuine outlier.
fn cost_tint(triangles: usize) -> Color {
    if triangles > bevy_grimoire_core::import::BUSY_TRIANGLES {
        DANGER
    } else if triangles > 5_000 {
        ACCENT
    } else {
        LABEL
    }
}

// ── placing ──────────────────────────────────────────────────────────────────────────────────────

/// **Snap a bare point** — an anchor, not a piece, so there is no footprint and no phase.
///
/// `snap_corner` with a zero span is exactly rounding to the pitch, which is what the clone tools
/// want: a captured set carries its own layout and only its anchor lands on the lattice.
fn snap_point(project: &Project, v: f32, level: SnapLevel) -> f32 {
    grid::snap_corner(v, 0.0, level.pitch(project.lattice.snap_divisor))
}

/// **The footprint a brush will occupy, tipped onto its side and then turned by its own yaw.**
///
/// Unmeasured pieces answer `(0, 0)`, which makes [`grid::snap_corner`] fall back to snapping the
/// centre — the honest answer when nothing knows how big the thing is, and the same "refuse to invent
/// a box" rule `compose::member_footprint` applies. A **tipped** piece with no measured height is
/// unmeasured by that same rule: once it is lying down its height is one of its floor dimensions, so
/// not knowing it means not knowing the box. See `descriptor::tipped_footprint`, which is where the
/// upright case is kept from ever consulting a height it does not need.
///
/// The two turns compose in the order they are drawn — tip in the piece's own frame first, then yaw
/// about +Y — which is the order [`bevy_grimoire_core::map::Placed::tip`] documents.
pub fn brush_span(d: &bevy_grimoire_core::descriptor::Descriptor, yaw: f32, tip: (u8, u8)) -> (f32, f32) {
    match bevy_grimoire_core::descriptor::tipped_footprint(d, tip) {
        Some(fp) => crate::compose::turned_footprint(fp, yaw),
        None => (0.0, 0.0),
    }
}

/// **The class of host this piece has to be mounted on**, or `None` if it stands on its own.
///
/// One function rather than each caller asking `stack::needs_surface` and remembering to ask
/// `stack::needs_face` too. That is not a hypothetical tidiness: `fill::flood` asked only about
/// surfaces, so the day `Mount::OnFace` arrived a face-mounted brush became flood-fillable and wrote
/// one unresolvable placement per cell — the same shape as the measured **4,089 invisible lamps**,
/// resurrected by a second mount kind the guard had never heard of. A third one must not be able to
/// do it again, so "does this need a host" has exactly one answer here.
///
/// **`OnSurfaceOrFloor` is deliberately not one of them**, and this is the one place that distinction
/// has to be written down. A brush that *may* take a surface can be laid on open floor — that is the
/// whole of its mount — so a flood or a box fill with one is legal and seats every piece on the deck.
/// Asking `stack::may_take_surface` here instead would refuse the gesture the feature exists for.
///
/// [`bevy_grimoire_core::stack`] keeps the two apart deliberately — they answer different questions and a
/// caller that wants the datum needs to know which — so this composes them rather than replacing
/// them.
pub fn mount_class(d: &bevy_grimoire_core::descriptor::Descriptor) -> Option<&str> {
    match (
        bevy_grimoire_core::stack::needs_surface(d),
        bevy_grimoire_core::stack::needs_face(d),
    ) {
        (Some(class), _) => Some(class),
        (None, Some((class, _))) => Some(class),
        (None, None) => None,
    }
}

/// **Which rung a stamp may use, and how big it is.**
///
/// A `Bounded` composition is the solver's own unit, so it is pinned to [`SnapLevel::Tile`] whatever
/// the modifiers say — see the call site for the mixed-initiative argument. An `Anchored` group claims
/// no tile and is furniture by `from_compositions`'s own rule, so it rides the ladder like any prop.
pub fn stamp_snap(
    project: &Project,
    armed: &crate::compose::Armed,
    keyboard: &ButtonInput<KeyCode>,
    rung: SnapLevel,
) -> (SnapLevel, (f32, f32)) {
    let armed = armed.id().and_then(|id| {
        project
            .compositions
            .compositions
            .iter()
            .find(|c| c.id == id)
    });
    match armed.map(|c| &c.envelope) {
        Some(bevy_grimoire_core::composition::Envelope::Bounded { size }) => {
            (SnapLevel::Tile, (size.0, size.2))
        }
        _ => (snap_level(keyboard, rung), (0.0, 0.0)),
    }
}

/// The footprint of whatever the move tool is carrying, so a dragged piece snaps by its own corner.
///
/// `(0, 0)` when nothing is held or the piece is unmeasured — centre-snapping, as above.
pub fn held_span(project: &Project,
    open: &OpenMap, drag: &MoveDrag) -> (f32, f32) {
    let Some(Held::Piece(id)) = drag.held.as_ref() else {
        return (0.0, 0.0);
    };
    let Some(p) = open.map.placements.iter().find(|p| &p.id == id) else {
        return (0.0, 0.0);
    };
    match descriptor_of(&project.library, p) {
        Some(d) => brush_span(&d, p.yaw, p.tip),
        None => (0.0, 0.0),
    }
}

/// **Which rung of the placement ladder is being asked for** — the latched one, or finer.
///
/// Extracted as a pure decision so a test can ask it directly — the projection needs a viewport and
/// this does not. Free placement is [`keys::alt_held`] and is not on this ladder at all.
///
/// `Shift` still means exactly *one rung finer than where you are*, which is what it always claimed
/// to mean; what changed is that "where you are" is now [`Rung`] rather than always the tile. The
/// modified pair saturates through [`grid::SnapLevel::finer`], so from the finest rung both
/// modifiers are no-ops rather than wrapping back to the whole tile.
pub fn snap_level(keyboard: &ButtonInput<KeyCode>, rung: SnapLevel) -> SnapLevel {
    match (keys::mod_held(keyboard), keys::shift_held(keyboard)) {
        (true, true) => rung.finer().finer(),
        (_, true) => rung.finer(),
        _ => rung,
    }
}

/// A world ground point as a **map-space** `at`, snapped to the authoring grid unless `free`.
///
/// The conversion has to happen somewhere, and here is the only somewhere: `cursor_ground` answers in
/// world metres and `Placed::at` is in map space. They agree only for a map at the origin — which is
/// every map the editor has ever authored, so writing world coordinates straight into `at` looked
/// right for as long as nobody moved a map.
///
/// # Holding the platform modifier places freely
///
/// The grid is what makes pieces meet, so it is the default and stays the default. But a prop that
/// wants to sit at an angle against a wall, or a bit of clutter that should not read as laid out on
/// a grid, has nowhere to go on a half-metre lattice — and the alternative was hand-editing the
/// map's RON afterwards.
///
/// Only X and Z: they are the axes the grid quantises and the axes a cursor on the ground plane can
/// express. **Y is not freed** — it comes from the piece's `mount` through `stack::datum`, which is
/// what puts a lamp on a table rather than through it, and there is no second mouse axis to say
/// otherwise with.
/// # Free placement stays inside the cell it started in
///
/// Fine control is for nudging a piece **within** the cell an author already chose, not for sliding it
/// across the map with the snap off. Unbounded, a small hand movement while the modifier was down
/// walked the piece a cell or two over and the author had to notice and undo it — the grid's whole job
/// is to stop that, and holding the modifier used to switch the job off entirely rather than turn it
/// down.
///
/// So the cell under the cursor is captured the moment the modifier goes down ([`FineAnchor`]) and the
/// free position is clamped to it: anywhere inside that half-metre, nowhere outside it. Release and
/// press again over a different cell to nudge a different one. An assist, not a restriction — every
/// position that was reachable is still reachable, in two gestures instead of one.
pub fn map_at(project: &Project,
    open: &OpenMap,
    hit: Vec3,
    free: bool,
    anchor: &FineAnchor,
    level: SnapLevel,
    span: (f32, f32),
) -> (f32, f32) {
    let (x, z) = open.map.to_map_space((hit.x, hit.z));
    match (free, anchor.cell) {
        // `snap` rounds to the nearest multiple of `SNAP`, so the cell around a snapped point reaches
        // half a step either side of it.
        (true, Some((cx, cz))) => {
            let half = level.pitch(project.lattice.snap_divisor) * 0.5;
            (x.clamp(cx - half, cx + half), z.clamp(cz - half, cz + half))
        }
        // The modifier went down off the ground plane, so there is no cell to hold to. Free, as it
        // was — refusing to place at all would be a worse answer than the one the author asked for.
        (true, None) => (x, z),
        // **The piece's minimum corner lands on the lattice, not its centre.** `cell_centre` is
        // `min + (c + 0.5) * TILE`, so the phase differs per footprint; snapping the centre put a 1 m
        // tile on whole metres, spanning two solver cells, and `to_cell` then floored it into one it
        // half covered. See `grid::SnapLevel`.
        (false, _) => {
            let pitch = level.pitch(project.lattice.snap_divisor);
            (
                // **Corner here, centre in the tile assembler, and that is not a disagreement.**
                //
                // The map's origin is a cell CORNER: cell k spans [k, k+1], so a 1 m piece fills a
                // cell only when its near edge is on the lattice, and three careless aims abut only
                // for the same reason. A tile's members are positioned relative to the tile's own
                // ANCHOR, which is its centre, so there the lattice has to run through zero.
                //
                // Both mean "aligned to the cell"; they are measured from different zeros. Switching
                // this one to `snap_centre` put every map tile on a cell corner and left gaps
                // between them — `a_tile_click_lands_filling_a_cell` and
                // `three_tiles_aimed_carelessly_still_abut` said so immediately.
                grid::snap_corner(x, span.0, pitch),
                grid::snap_corner(z, span.1, pitch),
            )
        }
    }
}

/// **Where a brush of this descriptor, at this yaw, lands — asked once.**
///
/// The preview and the commit have to agree, and the module's opening line says so: *"a preview drawn
/// somewhere the piece will *not* end up is worse than no preview, because it is a promise the game
/// then breaks."* They did not. `drive_ghost` passed a hardcoded `(0.0, 0.0)` span where `drive_place`
/// passed the real one, and a zero span makes [`grid::snap_corner`] degenerate to rounding the
/// **centre** — so the two sat on lattices half a tile apart for any piece an odd number of cells
/// across, and 50 mm apart across a wall.
///
/// The commit that introduced corner-snapping (`a10cadf`) threaded a real span into `drive_place`,
/// both `drive_move` branches and `drive_stamp_ghost`, and missed the brush ghost. Four call sites
/// that must agree, updated by hand, is the shape of defect that comes back — so this is the one
/// expression, and the disagreement is now unrepresentable rather than merely fixed.
///
/// **Yaw is the authored one, never [`bevy_grimoire::draw_yaw`].** `draw_yaw` adds the mesh's `front`
/// correction for *rendering*; snapping by it would put preview and commit back on different lattices
/// for exactly the pieces that carry one.
pub fn brush_at(
    project: &Project,
    open: &OpenMap,
    keyboard: &ButtonInput<KeyCode>,
    rung: SnapLevel,
    anchor: &FineAnchor,
    hit: Vec3,
    piece: Pose<'_>,
) -> (f32, f32) {
    map_at(project, open,
        hit,
        keys::alt_held(keyboard),
        anchor,
        snap_level(keyboard, rung),
        brush_span(piece.d, piece.yaw, piece.tip),
    )
}

/// A piece as it is being laid: its descriptor, its authored yaw, and its tip. [`brush_span`] reads
/// all three together, so they travel together.
pub struct Pose<'a> {
    /// The descriptor being laid.
    pub d: &'a bevy_grimoire_core::descriptor::Descriptor,
    /// The authored yaw, in degrees.
    pub yaw: f32,
    /// The authored tip.
    pub tip: (u8, u8),
}

/// The rectangle being dragged out to fill, in map space. Its own resource on the same argument
/// [`RemovalDrag`] makes: written every frame of a drag, and `rebuild_palette` watches
/// [`EditorState`].
#[derive(Resource, Default)]
pub struct PlaceDrag {
    /// Where the button went down, or `None` while only hovering.
    from: Option<(f32, f32)>,
}

/// **The line being dragged out to run a piece along**, in map space.
///
/// Its own resource on the same argument [`PlaceDrag`] makes: written every frame of a drag, and
/// `rebuild_palette` watches [`EditorState`] rather than this.
#[derive(Resource, Default)]
pub struct RunDrag {
    /// Where the button went down, or `None` while only hovering.
    from: Option<(f32, f32)>,
    /// **An end of a standing run the press landed on**: its id, and `true` when it is the `to` end.
    /// A release moves that end through `run_set`; the drag draws no new run.
    grab: Option<(String, bool)>,
}

/// The three map resources a placement writes: the project, the open map, and the editor's state.
/// They always travel together, so a placement helper takes them as one.
struct MapEdit<'a> {
    project: &'a mut Project,
    open: &'a mut OpenMap,
    state: &'a mut EditorState,
}

/// Fill a dragged box with the brush, as one act.
///
/// Split out of [`drive_place`] so the release path reads as the two things it chooses between —
/// place one, or fill a box — rather than as one function with a second half.
fn box_fill_between(
    commands: &mut Commands,
    assets: &AssetServer,
    placed: &Query<(Entity, &Placement)>,
    edit: MapEdit,
    brush: &bevy_grimoire_core::descriptor::Descriptor,
    corners: ((f32, f32), (f32, f32)),
    pitch: f32,
) {
    let MapEdit { project, open, state } = edit;
    let mut n = state.next_id;
    let short = short_id(&brush.id).to_owned();
    let filled = match crate::fill::box_fill(
        &open.map,
        brush,
        corners,
        state.brush_yaw,
        state.brush_tip,
        pitch,
        || {
            n += 1;
            format!("{short}@{n}")
        },
    ) {
        Ok(f) => f,
        // **A refusal is a `problem`, not a receipt.** The `Status` type's own doc says where a
        // refusal goes: *"the editor could not do what was asked, or did only part of it… a refusal
        // handed back by `bevy_grimoire_core`"*. This one is handed back by the core, and it is about
        // a piece that cannot be laid at all — *"mounts to a `support`, so it cannot be box filled"*.
        //
        // **The comment this replaces said the opposite** — *"a refusal is the answer, not a
        // failure"* — and it was wrong, because it read the rule from the wrong half of the
        // contract. The half that says *an answer* is for *"clicking bare ground with the remove
        // tool armed, dragging a box round nothing"*: a no-op the author can see was a no-op. **A
        // fill that could not be laid is not a no-op** — the author asked for a floor and did not
        // get one.
        //
        // **And the difference is not cosmetic.** A `problem` reaches the toast and the journal; a
        // `note` is one dim row, and it is erased by the next receipt and by a tab switch. NN/g's
        // Error-Message Guidelines: *"the very worst error messages are those that don't exist."*
        Err(e) => {
            state.status.problem(e);
            return;
        }
    };

    // **What the fill lands on is replaced, as one click is.** Every new row's blockers, against the
    // map as it was before the fill, as one set. A blocker with the brush's own descriptor stays:
    // `box_fill` already skipped the cells that hold the brush, so one that still overlaps a new row
    // is a neighbour the author laid off-lattice, not something this fill is repainting. The pure
    // `box_fill` is unchanged and knows nothing of this; replacement is the editor's.
    let mut doomed: Vec<usize> = filled
        .placements
        .iter()
        .flat_map(|row| {
            bevy_grimoire_core::stack::blockers(
                &open.map,
                &project.library,
                brush,
                row.at,
                row.yaw,
                row.tip,
                None,
            )
        })
        .filter(|&i| open.map.placements.get(i).is_some_and(|p| p.descriptor != brush.id))
        .collect();
    doomed.sort_unstable();
    doomed.dedup();
    if let Err(e) = riders_refuse(&open.map, &doomed) {
        // Refused whole: a fill that stopped at the first host would leave a half-painted box.
        //
        // **And it is a `problem`**, for the same reason as the refusal above: the editor could
        // not do what was asked, and the author needs to know *before* they look at the map and
        // wonder why the box is short.
        state.status.problem(e);
        return;
    }
    state.next_id = n;
    let removed = take_rows(commands, placed, &mut open.map, &doomed);

    let count = filled.placements.len();
    let first = open.map.placements.len();
    open.map.placements.extend(filled.placements);
    // Into the map first, drawn second: how high a piece sits is a question about the finished map.
    spawn_range(commands, assets, project, &*open, state, first);
    // **One entry for the whole box**, the rule `RemovedMany` states: one act the author performed is
    // one act to take back.
    let replaced = removed.len();
    state.record(record_added(count, removed));
    // **A truncated fill is a partial result, so it is a problem and not a receipt.** The two
    // branches say different kinds of thing: one reports a box that filled, the other reports a box
    // that stopped early and looks exactly like the first on screen.
    let over = if replaced == 0 {
        String::new()
    } else {
        format!(", replacing {replaced}")
    };
    if filled.truncated {
        state.status.problem(format!(
            "filled {count}{over} — stopped at the {} cell cap. {}",
            crate::fill::MAX_CELLS,
            crate::fill::CAP_ROUTE
        ));
    } else {
        state.status.note(format!("filled {count}{over}"));
    }
}

/// **Open the map's size field for one axis, the way a click on it does** — by triggering the
/// same `Activate` the button sends, so the field's own observer runs. `axis` is `'x'`, `'y'` or
/// `'z'`. What is typed next goes through the real `KeyboardInput` path. Panics when the field is
/// not on screen, because a test that silently typed into nothing would pass for the wrong reason.
pub fn size_field_for_test(world: &mut World, axis: char) {
    let want = match axis {
        'x' => Axis::X,
        'y' => Axis::Y,
        'z' => Axis::Z,
        other => panic!("size_field_for_test: `{other}` is not an axis"),
    };
    let mut fields = world.query::<(Entity, &SizeField)>();
    let Some(entity) = fields
        .iter(world)
        .find(|(_, f)| f.0 == want)
        .map(|(e, _)| e)
    else {
        panic!(
            "size_field_for_test: no {} size field is on screen",
            want.label()
        )
    };
    world.trigger(Activate { entity });
    world.flush();
}

/// **Click one palette row, the way a person's press does** — E-041's gap.
///
/// Fires the same `Activate` the button sends, so [`on_row_click`]'s observer runs, which is
/// [`arm_piece`] → `brush_arm`. Nothing in the suite could reach a row until this existed: the
/// brush's other route under test is the keyboard's (`walk_palette`), a different function with its
/// own writes, and the closest a test got was calling `arm_piece` itself — which skips the guard
/// that makes a click on a *menu* row safe, the filter blur, and the row's own `PaletteRow` index.
///
/// `descriptor` is the library id, resolved against the library the way the palette names it, so a
/// test states what it clicked rather than which row number it happened to be.
///
/// Panics when the row is not on screen, because a test that silently clicked nothing would pass
/// for the wrong reason — [`size_field_for_test`] and `tiles::click_tag_chip_for_test` make the same
/// argument.
pub fn click_palette_row_for_test(world: &mut World, descriptor: &str) {
    let ix = world
        .get_resource::<Project>()
        .and_then(|p| p.library.descriptors.iter().position(|d| d.id == descriptor))
        .unwrap_or_else(|| {
            panic!("click_palette_row_for_test: no descriptor `{descriptor}` in the library")
        });
    let mut rows = world.query::<(Entity, &PaletteRow)>();
    let Some(entity) = rows
        .iter(world)
        .find(|(_, r)| r.0 == ix)
        .map(|(e, _)| e)
    else {
        panic!(
            "click_palette_row_for_test: `{descriptor}` has no row on screen — the palette is \
             folded, filtered, or the map's bash leaves its kit out"
        )
    };
    world.trigger(Activate { entity });
    world.flush();
}

/// **A run of `descriptor` between two points**, reachable from `tests/headless.rs`.
///
/// The release path of the run tool, without the mouse — the same split `fill_for_test` makes. The
/// mouse path is not drivable headless, and a test that re-implemented the body would pass while
/// the real one was broken.
pub fn run_for_test(world: &mut World, descriptor: &str, from: (f32, f32), to: (f32, f32)) {
    use bevy::ecs::system::RunSystemOnce;
    let descriptor = descriptor.to_owned();
    world
        .run_system_once(
            move |mut commands: Commands,
                  assets: Res<AssetServer>,
                  mut project: ResMut<Project>,
                  mut open: ResMut<OpenMap>,
                  mut state: ResMut<EditorState>| {
                let Some(d) = project.library.get(&descriptor).cloned() else {
                    panic!("run_for_test: no descriptor `{descriptor}` in the library");
                };
                state.brush_yaw = 0.0;
                state.brush_tip = (0, 0);
                run_between(
                    &mut commands,
                    &assets,
                    MapEdit { project: &mut project, open: &mut open, state: &mut state },
                    &d,
                    from,
                    to,
                );
            },
        )
        .unwrap_or_else(|e| panic!("run_for_test: {e}"));
}

/// The clone tool's marker quad — [`spawn_removal_tile`]'s twin in the clone tint, and spawned
/// once for the same leak-shaped reason.
fn spawn_clone_tile(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        CloneTile,
        Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: CLONE_TINT,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
    ));
}

/// **The run tool** — [`drive_place`]'s gesture with a different payload.
///
/// The gesture is the fill's gesture: press, drag, release, with the same [`CLICK_EPS`] that tells
/// a click from a drag and the same `Alt` modifier that means "place one, exactly here". The
/// difference is what the drag draws: a **line of repeated pieces** rather than a **grid of them**.
///
/// **The line is the piece's own pitch, not the project's lattice.** That is the whole reason the
/// tool exists: Sanctuary's fence posts stand 2.032 m apart, and the lattice's rungs are 1.000,
/// 0.333 and 0.111 m, so a fill lays them 3.000, 2.333 or 2.111 m apart — never 2.032, and 8 cm
/// out at best.
///
/// Preview and commit live in one system for the reason `drive_place` gives: the line drawn IS the
/// line committed, and computing it twice is how a preview comes to disagree with what it does.
#[allow(clippy::too_many_arguments)]
fn drive_run(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    // Held, the platform modifier drops the grid snap — see `map_at`.
    keyboard: Res<ButtonInput<KeyCode>>,
    assets: Res<AssetServer>,
    claim: Claim,
    camera: Option<Single<(&Camera, &GlobalTransform), With<MainCamera>>>,
    mut project: ResMut<Project>,
    mut open: ResMut<OpenMap>,
    mut state: ResMut<EditorState>,
    mut drag: ResMut<RunDrag>,
    mut ghost_drag: ResMut<RunGhostDrag>,
    mut gizmos: Gizmos,
    placed: Query<(Entity, &Placement)>,
    mut due: ResMut<crate::watch::Due>,
) {
    if state.tool != Tool::Run {
        // Read, never write, unless something happened — `rebuild_palette` watches `EditorState`
        // and `drag` is deliberately not on it, but the same discipline applies.
        if drag.from.is_some() {
            drag.from = None;
        }
        ghost_drag.line = None;
        return;
    }
    let Some(camera) = camera else {
        return;
    };
    let (cam, cam_tf) = *camera;
    // **A snip or a report owns the pointer while either is open**: a press one takes starts no run.
    let snipping = claim.snip.claims_pointer() || claim.report.claims_pointer();
    let Some(hit) = cursor_ground(claim.pointer.0, cam, cam_tf).filter(|_| !snipping) else {
        // **A release ends the drag wherever it happens** — the defect `drive_removal` records.
        if mouse.just_released(MouseButton::Left) {
            drag.from = None;
        }
        return;
    };
    // **A drag that started on the world owns the pointer until it is let go** — the same rule
    // `drive_place` states, and for the same reason: a fill box dragged across the palette used to
    // tear the gesture in half.
    if ui_blocks_gesture(claim.over_ui.0, drag.from.is_some()) {
        return;
    }

    // **Nothing armed places nothing** — and says so, because `F` with nothing armed used to be
    // indistinguishable from `F` not being bound.
    let Some(d) = state
        .brush
        .and_then(|ix| project.library.descriptors.get(ix))
        .cloned()
    else {
        if drag.from.is_some() {
            drag.from = None;
        }
        if mouse.just_released(MouseButton::Left) {
            state
                .status
                .note("nothing armed to run with — pick a piece from the palette".to_owned());
        }
        return;
    };

    // **The line's ends are the author's own points, not the lattice's.** A run's posts stand at the
    // piece's pitch, so snapping the ends to the project's lattice would put the first post a
    // third of a metre off where the author pointed. The ends are the cursor, in map space.
    let at = open.map.to_map_space((hit.x, hit.z));
    if mouse.just_pressed(MouseButton::Left) {
        drag.from = Some(at);
        drag.grab = run_end_near(&open.map, at);
    }

    // **The line, while it is being dragged.** A gizmo, not a quad: the run is a *line* and a
    // rectangle would draw a shape the tool does not make.
    //
    // **And the posts it would lay**, on their own ghost — because a line is the *instruction* to
    // lay, not the landing. The author has to see the fence before they let go.
    if let Some(from) = drag.from {
        let dragging = (from.0 - at.0).abs() > CLICK_EPS || (from.1 - at.1).abs() > CLICK_EPS;
        ghost_drag.line = dragging.then_some((from, at)).filter(|_| drag.grab.is_none());
        if dragging {
            gizmos.line(
                Vec3::new(
                    open.map.origin.0 + from.0,
                    open.map.origin.1 + MARKER_LIFT,
                    open.map.origin.2 + from.1,
                ),
                Vec3::new(
                    open.map.origin.0 + at.0,
                    open.map.origin.1 + MARKER_LIFT,
                    open.map.origin.2 + at.1,
                ),
                ACCENT,
            );
        }
    }

    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(from) = drag.from.take() else {
        return;
    };
    // **The ghost goes with the drag** — the posts are about to be real, and a ghost of them would
    // be a second copy.
    ghost_drag.line = None;
    // **A drag that began on a standing run's end moves that end**, and a click there changes nothing:
    // the run's rows follow because they are derived from it.
    if let Some((id, is_to)) = drag.grab.take() {
        if (from.0 - at.0).abs() > CLICK_EPS || (from.1 - at.1).abs() > CLICK_EPS {
            set_run(Some(&mut project), &mut open, &mut state, &id, is_to, at);
        }
        return;
    }
    // **A click places one; a drag runs.** The same threshold the removal tool uses, so a hand that
    // moved a hair while clicking is still a click in both.
    //
    // **Never while the modifier is held** — the same rule `drive_place` states, and for the same
    // reason: holding it means "place one, exactly here".
    let free = keys::alt_held(&keyboard);
    if free || ((from.0 - at.0).abs() <= CLICK_EPS && (from.1 - at.1).abs() <= CLICK_EPS) {
        // **A click in run mode places one piece**, which is what the author meant by a click and
        // what the mode's own name promises not to swallow.
        place_one(
            &mut commands,
            &assets,
            &placed,
            &project,
            &mut open,
            &mut state,
            &d,
            at,
            free,
        );
        return;
    }
    // **The platform modifier turns the rows along the line into a run** (#115), one way.
    if keys::mod_held(&keyboard) {
        if rows_to_run_along(Some(&mut project), &mut open, &mut state, &d, from, at) {
            due.map_replaced = true;
        }
        return;
    }
    // **Shift lays the line as a run** (#115): one `RunPlaced` the map holds, whose rows follow when an
    // end moves. Without it the line lays ordinary rows, as it always has.
    if keys::shift_held(&keyboard) {
        add_run(Some(&mut project), &mut open, &mut state, &d, from, at);
        return;
    }
    run_between(
        &mut commands,
        &assets,
        MapEdit { project: &mut project, open: &mut open, state: &mut state },
        &d,
        from,
        at,
    );
}

/// **The run tool's posts, laid along a dragged line** — one act.
///
/// The fourth placement tool, and the one the other three could not be. A flood lays a *grid*: one
/// piece, one yaw, one pitch, over a region. A run lays a *line*: the piece repeated along it at
/// **its own pitch**, which is not the project's lattice.
///
/// The drag is the fill's drag — press, drag, release — and the line is the one the author drew.
/// The corners are not drawn: a corner is a *turn*, and a turn is the same piece turned 90°. So a
/// run of a straight line carries no corners, and the corner rule belongs to the group that the
/// author makes of four runs.
fn run_between(
    commands: &mut Commands,
    assets: &AssetServer,
    edit: MapEdit,
    brush: &bevy_grimoire_core::descriptor::Descriptor,
    from: (f32, f32),
    to: (f32, f32),
) {
    let MapEdit { project, open, state } = edit;
    let run = bevy_grimoire_core::run::Run {
        descriptor: brush.id.clone(),
        pitch: None,
        from,
        to,
        corners: Vec::new(),
        gaps: Vec::new(),
    };
    let posts = match bevy_grimoire_core::run::lay(&run, &project.library) {
        Ok(p) => p,
        // **A refusal is the answer, not a failure** — the same call `flood_from_cursor` makes,
        // and for the same reason: a fence with a hole in it is worse than no fence.
        Err(e) => {
            state.status.problem(e);
            return;
        }
    };

    // **The rows, as ordinary placements.** A run's posts are rows like any other, so the map file
    // gains rows and nothing else, the game's loader needs no change, and the inspector's count
    // already includes them.
    let mut n = state.next_id;
    let short = short_id(&brush.id).to_owned();
    let first = open.map.placements.len();
    for post in &posts {
        n += 1;
        open.map.placements.push(Placed {
            id: format!("{short}@{n}"),
            descriptor: brush.id.clone(),
            at: post.at,
            yaw: post.yaw,
            ..Placed::default()
        });
    }
    state.next_id = n;

    // Into the map first, drawn second: how high a piece sits is a question about the finished map.
    spawn_range(commands, assets, project, &*open, state, first);
    // **One entry for the whole run**, the rule `RemovedMany` states: one act the author performed
    // is one act to take back.
    state.record(record_added(posts.len(), Vec::new()));
    state.status.note(format!("ran {} piece(s) along the line", posts.len()));
}

/// **Lay the dragged line as a run the map holds** (`run_add`): a row for the whole line, not for
/// each post. The posts are derived by `Map::expanded` and drawn by `redraw_runs`.
fn add_run(
    project: Option<&mut Project>,
    open: &mut OpenMap,
    state: &mut EditorState,
    brush: &bevy_grimoire_core::descriptor::Descriptor,
    from: (f32, f32),
    to: (f32, f32),
) {
    // **One row in the map, not the posts it lays** (#115): the run is stored as the run, its rows are
    // derived by `Map::expanded`, and `redraw_runs` draws them. A refusal (a piece with no
    // footprint, a line shorter than a post) is the answer, the same call `flood_from_cursor` makes:
    // a fence with a hole in it is worse than no fence.
    state.next_id += 1;
    let id = format!("{}_run@{}", short_id(&brush.id), state.next_id);
    let run = bevy_grimoire_core::run::Run {
        descriptor: brush.id.clone(),
        pitch: None,
        from,
        to,
        corners: Vec::new(),
        gaps: Vec::new(),
    };
    dispatch_note(
        project,
        open,
        state,
        "run_add",
        serde_json::json!({"id": id, "run": run}),
        "run a piece along a line",
    );
}

/// **Replace the standing rows of `brush` along the dragged line by the run that lays them**
/// (`placements_to_run`). Rows within `RUN_GRAB` of the line, in path order. `true` when the map
/// changed, so the caller redraws the pieces; a refusal names the first row that does not fit.
fn rows_to_run_along(
    project: Option<&mut Project>,
    open: &mut OpenMap,
    state: &mut EditorState,
    brush: &bevy_grimoire_core::descriptor::Descriptor,
    from: (f32, f32),
    to: (f32, f32),
) -> bool {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let len2 = dx * dx + dy * dy;
    if len2 <= 0.0 {
        return false;
    }
    let mut hits: Vec<(f32, String)> = open
        .map
        .placements
        .iter()
        .filter(|p| p.descriptor == brush.id)
        .filter_map(|p| {
            let t = (((p.at.0 - from.0) * dx + (p.at.1 - from.1) * dy) / len2).clamp(0.0, 1.0);
            let off = (p.at.0 - (from.0 + t * dx)).hypot(p.at.1 - (from.1 + t * dy));
            (off <= RUN_GRAB).then(|| (t, p.id.clone()))
        })
        .collect();
    if hits.is_empty() {
        state.status.problem(format!("no `{}` rows along that line to turn into a run", brush.id));
        return false;
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0));
    state.next_id += 1;
    let id = format!("{}_run@{}", short_id(&brush.id), state.next_id);
    let rows: Vec<String> = hits.into_iter().map(|(_, id)| id).collect();
    let before = open.map.runs.len();
    dispatch_note(project, open, state, "placements_to_run", serde_json::json!({"id": id, "rows": rows}), "turn rows into a run");
    open.map.runs.len() != before
}

/// **Move one end of a standing run through the registry** (`run_set`); its rows follow.
fn set_run(
    project: Option<&mut Project>,
    open: &mut OpenMap,
    state: &mut EditorState,
    id: &str,
    is_to: bool,
    at: (f32, f32),
) {
    let params = if is_to {
        serde_json::json!({"id": id, "to": at})
    } else {
        serde_json::json!({"id": id, "from": at})
    };
    dispatch_note(project, open, state, "run_set", params, "move the end of a run");
}

/// **Take a run off the map whole through the registry** (`run_remove`), with every row it lays.
fn remove_run(project: Option<&mut Project>, open: &mut OpenMap, state: &mut EditorState, id: &str) {
    dispatch_note(project, open, state, "run_remove", serde_json::json!({"id": id}), "remove a run");
}

/// How near a press must be to an end of a standing run to take it (metres, map space).
const RUN_GRAB: f32 = 0.5;

/// **The end of a standing run within reach of `at`**: its id, and `true` for the `to` end.
fn run_end_near(map: &bevy_grimoire_core::map::Map, at: (f32, f32)) -> Option<(String, bool)> {
    let near = |p: (f32, f32)| (p.0 - at.0).hypot(p.1 - at.1) <= RUN_GRAB;
    map.runs.iter().find_map(|r| {
        if near(r.run.from) {
            Some((r.id.clone(), false))
        } else if near(r.run.to) {
            Some((r.id.clone(), true))
        } else {
            None
        }
    })
}

/// **The run whose post covers `probe`**, for the removal tool's click. Laid on the click, never per
/// frame; a run that does not lay has no posts to point at.
fn pick_run(project: &Project, map: &bevy_grimoire_core::map::Map, probe: (f32, f32)) -> Option<String> {
    map.runs.iter().find_map(|r| {
        let d = project.library.descriptors.iter().find(|d| d.id == r.run.descriptor)?;
        let posts = bevy_grimoire_core::run::lay(&r.run, &project.library).ok()?;
        posts
            .iter()
            .any(|p| bevy_grimoire_core::stack::covers(d, p.at, p.yaw, probe))
            .then(|| r.id.clone())
    })
}

/// **The clone tool** — [`drive_removal`]'s shape with the opposite payload: the box takes a COPY
/// of everything whose centre it contains, and every later click stamps the whole set at the
/// cursor, fresh ids and all.
///
/// Preview and commit in one system for the reason `drive_removal` gives: the rectangle drawn IS
/// the rectangle captured, because they are the same numbers.
#[allow(clippy::too_many_arguments)]
fn drive_clone(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    rung: Res<Rung>,
    over_ui: Res<crate::view::OverUi>,
    pointer: Res<crate::view::Pointer>,
    snip: Res<crate::snip::SnipEdit>,
    report: Res<crate::report::ReportEdit>,
    camera: Option<Single<(&Camera, &GlobalTransform), With<MainCamera>>>,
    mut marker: Query<(&mut Transform, &mut Visibility), With<CloneTile>>,
    mut drag: ResMut<CloneDrag>,
    mut project: ResMut<Project>,
    mut open: ResMut<OpenMap>,
    mut state: ResMut<EditorState>,
    picture: Res<StampPicture>,
) {
    // Read, never write, unless something happened — `rebuild_palette` watches `state`.
    if state.tool != Tool::Clone {
        if drag.from.is_some() || drag.held.is_some() {
            drag.from = None;
            drag.held = None;
        }
        for (_, mut vis) in &mut marker {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
        return;
    }

    let Some(camera) = camera else {
        return;
    };
    let (cam, cam_tf) = *camera;
    let snipping = snip.claims_pointer() || report.claims_pointer();
    let hit = cursor_ground(pointer.0, cam, cam_tf)
        .filter(|_| !snipping && !over_ui.0);
    let Some(hit) = hit else {
        // A release off the world ends the drag — the defect `drive_removal` records.
        if mouse.just_released(MouseButton::Left) {
            drag.from = None;
        }
        for (_, mut vis) in &mut marker {
            *vis = Visibility::Hidden;
        }
        return;
    };
    let at = open.map.to_map_space((hit.x, hit.z));

    if mouse.just_pressed(MouseButton::Left) && drag.held.is_none() {
        drag.from = Some(at);
    }

    // What the marker shows: the held set's bounds riding the (snapped) cursor, else the box being
    // dragged out.
    let rect = if let Some(set) = &drag.held {
        Some(set.bounds((
            snap_point(&project, at.0, snap_level(&keyboard, rung.0)),
            snap_point(&project, at.1, snap_level(&keyboard, rung.0)),
        )))
    } else {
        drag.from.map(|from| {
            (
                from.0.min(at.0),
                from.1.min(at.1),
                from.0.max(at.0),
                from.1.max(at.1),
            )
        })
    };
    for (mut tf, mut vis) in &mut marker {
        match rect {
            Some((x0, z0, x1, z1)) => {
                *vis = Visibility::Visible;
                *tf = Transform::from_xyz(
                    open.map.origin.0 + (x0 + x1) * 0.5,
                    open.map.origin.1 + MARKER_LIFT,
                    open.map.origin.2 + (z0 + z1) * 0.5,
                )
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::new(
                    (x1 - x0).max(0.05),
                    (z1 - z0).max(0.05),
                    1.0,
                ));
            }
            None => *vis = Visibility::Hidden,
        }
    }

    if !mouse.just_released(MouseButton::Left) {
        return;
    }

    // **A set in hand stamps on every click.** The drag that took it is long finished, so the
    // release IS the gesture.
    if drag.held.is_some() {
        let Some(set) = drag.held.take() else {
            return;
        };
        let anchor_at = (
            snap_point(&project, at.0, snap_level(&keyboard, rung.0)),
            snap_point(&project, at.1, snap_level(&keyboard, rung.0)),
        );
        stamp_set(
            &mut commands,
            &assets,
            &mut project, &mut open,
            &mut state,
            &set,
            anchor_at,
        );
        // Back in hand whatever happened: a refusal's fix is usually "two cells to the left".
        drag.held = Some(set);
        return;
    }

    let Some(from) = drag.from.take() else {
        return;
    };
    if (from.0 - at.0).abs() <= CLICK_EPS && (from.1 - at.1).abs() <= CLICK_EPS {
        state
            .status
            .note("drag a box to take a copy of what is inside it".to_owned());
        return;
    }

    // **Capture.** Centre-in-box, the same containment the removal box applies, so the two box
    // gestures agree about what a rectangle contains.
    let (x0, z0) = (from.0.min(at.0), from.1.min(at.1));
    let (x1, z1) = (from.0.max(at.0), from.1.max(at.1));
    let caught: Vec<usize> = open
        .map
        .placements
        .iter()
        .enumerate()
        .filter(|(_, p)| p.at.0 >= x0 && p.at.0 <= x1 && p.at.1 >= z0 && p.at.1 <= z1)
        .map(|(i, _)| i)
        .collect();
    // **Stamps the box catches come whole**, on the rule the removal box already follows and for the
    // stronger reason: a box clipping the corner of a nurse station cannot take a corner of it, so
    // the choice is the instance or nothing. Caught by any drawn row's centre, which is the same
    // test the placements above use.
    let caught_stamps: Vec<usize> = {
        let mut v: Vec<usize> = picture
            .rows
            .iter()
            .filter(|r| r.at.0 >= x0 && r.at.0 <= x1 && r.at.1 >= z0 && r.at.1 <= z1)
            .filter_map(|r| open.map.stamps.iter().position(|s| s.id == r.stamp))
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    if caught.is_empty() && caught_stamps.is_empty() {
        state.status.note("nothing in that box to clone".to_owned());
        return;
    }

    // The anchor is the snapped centroid, so the set rides centred under the hand and stamps land
    // on the grid while every internal offset stays exact.
    //
    // **Over both lists**, or a box holding only stamps divides by zero — and a set anchored on the
    // placements alone would ride off-centre the moment the two kinds are mixed.
    let mut sum = (0.0f32, 0.0f32);
    let mut n = 0.0f32;
    for &i in &caught {
        sum.0 += open.map.placements[i].at.0;
        sum.1 += open.map.placements[i].at.1;
        n += 1.0;
    }
    for &i in &caught_stamps {
        sum.0 += open.map.stamps[i].at.0;
        sum.1 += open.map.stamps[i].at.1;
        n += 1.0;
    }
    // The captured set's own anchor sits on the tile lattice: a group is placed as a group.
    let anchor = (
        snap_point(&project, sum.0 / n, SnapLevel::Tile),
        snap_point(&project, sum.1 / n, SnapLevel::Tile),
    );

    let (pieces, [mut bx0, mut bz0, mut bx1, mut bz1]) =
        capture_pieces(&project, &open.map, &caught, anchor);
    // The stamps, carried whole. Their bounds come from the rows the picture already knows about,
    // so the marker claims what the stamp actually covers rather than a point at its anchor.
    let mut stamps = Vec::with_capacity(caught_stamps.len());
    for &i in &caught_stamps {
        let st = &open.map.stamps[i];
        let offset = (st.at.0 - anchor.0, st.at.1 - anchor.1);
        for row in picture.rows.iter().filter(|r| r.stamp == st.id) {
            let (w, depth) = descriptor_with(&project.library, &row.descriptor, row.patch.as_ref())
                .map(|d| crate::fill::cell_extents(&d, row.yaw, row.tip))
                .unwrap_or((crate::fill::MIN_CELL, crate::fill::MIN_CELL));
            let ro = (row.at.0 - anchor.0, row.at.1 - anchor.1);
            bx0 = bx0.min(ro.0 - w * 0.5);
            bz0 = bz0.min(ro.1 - depth * 0.5);
            bx1 = bx1.max(ro.0 + w * 0.5);
            bz1 = bz1.max(ro.1 + depth * 0.5);
        }
        stamps.push(CloneStamp {
            of: st.of.clone(),
            offset,
            yaw: st.yaw,
            overrides: st.overrides.clone(),
            owned: st.owned,
            owned_because: st.owned_because.clone(),
            note: st.note.clone(),
        });
    }

    let count = pieces.len() + stamps.len();
    drag.held = Some(CloneSet {
        pieces,
        stamps,
        centre_off: ((bx0 + bx1) * 0.5, (bz0 + bz1) * 0.5),
        half: ((bx1 - bx0) * 0.5, (bz1 - bz0) * 0.5),
        yaw: 0.0,
    });
    // **Every verb the set has, said at the moment it exists.** The census carries them — `B,
    // Shift+B, M` collapsed onto one row to stay under the Map context's twelve-row ceiling — but
    // that row lives behind `K`, and "how do I keep this as a composition" was asked twice by an
    // author holding a set. A just-in-time line costs nothing and is read where the census is not.
    state.status.note(format!(
        "{count} piece(s) in hand — click stamps the set, {}/{} turn it, {} keeps it as a \
         composition, Esc puts it away",
        keys::binding(Action::TurnPieceLeft).chord,
        keys::binding(Action::TurnPieceRight).chord,
        keys::chord_text(keys::binding(Action::GroupFromSet)),
    ));
}

/// **Stamp the held set at `target`** — all of it, or none of it, the rule `move_placement` states.
///
/// Fresh ids in set order; in-set hosts repointed to the fresh ids; outside hosts re-found under
/// each new position. Every piece answers the overlap rule against the standing map, and the
/// finished trial re-resolves before anything is committed — one refusal anywhere and the map has
/// not changed.
fn stamp_set(
    commands: &mut Commands,
    assets: &AssetServer,
    project: &mut Project,
    open: &mut OpenMap,
    state: &mut EditorState,
    set: &CloneSet,
    target: (f32, f32),
) {
    let ys = match heights(project, &*open) {
        Ok(ys) => ys,
        Err(e) => {
            state.status.problem(e);
            return;
        }
    };
    let mut n = state.next_id;
    let new_ids: Vec<String> = set
        .pieces
        .iter()
        .map(|piece| {
            n += 1;
            format!("{}@{}", short_id(&piece.descriptor), n)
        })
        .collect();

    let mut rows: Vec<Placed> = Vec::with_capacity(set.pieces.len());
    for (i, piece) in set.pieces.iter().enumerate() {
        // The set's own turn, folded in once — see `CloneSet::placed`.
        let (at, yaw) = set.placed(piece, target);
        let Some(d) = descriptor_with(&project.library, &piece.descriptor, piece.patch.as_ref())
        else {
            state.status.problem(format!(
                "stamp refused: `{}` is not in the library any more",
                piece.descriptor
            ));
            return;
        };
        let d: &bevy_grimoire_core::descriptor::Descriptor = &d;
        let on = match &piece.on {
            CloneHost::Layer => None,
            CloneHost::InSet(h) => Some(new_ids[*h].clone()),
            CloneHost::Outside => {
                // **A piece that may take a surface or the floor answers by where it lands.** A
                // paste over a bench seats on it; a paste on open floor takes the floor. Refusing
                // when `host_under` finds nothing was right for a strict `OnSurface` and is the
                // wrong answer for this mount — the click path accepts the same piece at the same
                // spot, and a paste that disagrees with a click is two answers to one question.
                match bevy_grimoire_core::stack::host_under(&open.map, &project.library, &ys, d, at) {
                    Some((host, _)) => Some(host.id.clone()),
                    None if bevy_grimoire_core::stack::needs_surface(d).is_none() => None,
                    None => {
                        state.status.problem(format!(
                            "stamp refused: `{}` needs a surface and nothing offers one where it \
                             would land",
                            new_ids[i]
                        ));
                        return;
                    }
                }
            }
        };
        if let Some(block) = bevy_grimoire_core::stack::blocking(
            &open.map,
            &project.library,
            d,
            at,
            yaw,
            piece.tip,
            on.as_deref(),
        ) {
            state.status.problem(format!(
                "stamp refused: `{}` already covers where `{}` would land",
                block.id, new_ids[i]
            ));
            return;
        }
        rows.push(Placed {
            paint: 0,
            id: new_ids[i].clone(),
            descriptor: piece.descriptor.clone(),
            at,
            yaw,
            lift: piece.lift,
            tip: piece.tip,
            on,
            owned: piece.owned,
            owned_because: piece.owned_because.clone(),
            patch: piece.patch.clone(),
            note: piece.note.clone(),
            family: Default::default(),
        });
    }

    // The finished trial is the last door — it catches whatever the per-piece checks missed, and
    // nothing has been committed when it refuses.
    let mut trial = open.map.clone();
    trial.placements.extend(rows.iter().cloned());
    if let Err(e) = bevy_grimoire_core::stack::resolve_y(&trial, &project.library) {
        state.status.problem(format!("stamp refused: {e}"));
        return;
    }

    let count = rows.len();
    // **A carried stamp lands as a stamp**, not as the rows it expands to. Copying expanded
    // geometry would flatten the reference `stamping_writes_a_reference_and_undo_takes_it_back`
    // exists to protect — the instance is the thing that was picked up, so the instance is the
    // thing put down.
    let mut fresh: Vec<bevy_grimoire_core::composition::Stamped> = Vec::with_capacity(set.stamps.len());
    for cs in &set.stamps {
        if !project
            .compositions
            .compositions
            .iter()
            .any(|c| c.id == cs.of)
        {
            state.status.problem(format!(
                "stamp refused: `{}` is not a composition any more",
                cs.of
            ));
            return;
        }
        n += 1;
        let (at, yaw) = set.placed_stamp(cs, target);
        fresh.push(bevy_grimoire_core::composition::Stamped {
            id: format!("{}@{}", short_id(&cs.of), n),
            of: cs.of.clone(),
            at,
            yaw,
            overrides: cs.overrides.clone(),
            of_fingerprint: None,
            owned: cs.owned,
            owned_because: cs.owned_because.clone(),
            note: cs.note.clone(),
        });
    }

    let first = open.map.placements.len();
    state.next_id = n;
    open.map.placements.extend(rows);
    let stamped = fresh.len();
    open.map.stamps.extend(fresh);
    spawn_range(commands, assets, project, &*open, state, first);
    // One entry for the whole set: one act the author performed is one act to take back. Two lists
    // means a `Group`, whose inverses run in reverse order — the property that keeps `Undo` closed.
    let op = match (count == 0, stamped == 0) {
        (false, true) => Undo::Added { count },
        (true, false) => Undo::Stamped { count: stamped },
        _ => Undo::Group {
            ops: vec![Undo::Added { count }, Undo::Stamped { count: stamped }],
        },
    };
    state.record(op);
    let what = match (count, stamped) {
        (c, 0) => format!("{c} piece(s)"),
        (0, t) => format!("{t} stamp(s)"),
        (c, t) => format!("{c} piece(s) and {t} stamp(s)"),
    };
    state
        .status
        .note(format!("stamped {what} — click again stamps another"));
}

/// **The cell fine placement is confined to** — captured when the platform modifier goes down.
///
/// Its own resource, and written only on the two frames the modifier changes state, so it does not
/// wake `rebuild_palette` the way a field on [`EditorState`] would. See [`map_at`] for the rule.
#[derive(Resource, Default)]
pub struct FineAnchor {
    /// The snapped map-space point the cursor was over when the modifier went down.
    cell: Option<(f32, f32)>,
}

/// Capture the cell when the modifier goes down, and let it go when the modifier comes up.
///
/// Runs in [`keys::Phase::Sense`], before anything reads the cursor, so the anchor a click sees is the
/// one captured for that press rather than one frame stale.
fn sense_fine_anchor(
    keyboard: Res<ButtonInput<KeyCode>>,
    project: Res<Project>,
    open: Res<OpenMap>,
    pointer: Res<crate::view::Pointer>,
    camera: Option<Single<(&Camera, &GlobalTransform), With<MainCamera>>>,
    over_ui: Res<crate::view::OverUi>,
    mut anchor: ResMut<FineAnchor>,
) {
    // Alt, not the platform modifier: free placement moved so that Shift means exactly one thing.
    let held = keys::alt_held(&keyboard);
    if !held {
        // Only when it changes — `ResMut` marks the resource changed on every mutable deref.
        if anchor.cell.is_some() {
            anchor.cell = None;
        }
        return;
    }
    // Already anchored: the cell is decided by the press, not re-decided every frame the key is down.
    // Re-capturing here would follow the cursor and clamp to nothing.
    if anchor.cell.is_some() {
        return;
    }
    // **A cursor over a panel is not a cursor over a cell.** The modifier is also every chord's
    // modifier — `Cmd+Z`, `Cmd+S`, `Cmd+2` — so it goes down over UI constantly, and an anchor
    // captured there would clamp a later free placement to whatever cell happened to lie under the
    // palette. No anchor at all is the honest answer: `map_at`'s `(true, None)` arm places free and
    // unclamped, exactly as it does when the modifier goes down off the ground plane.
    if over_ui.0 {
        return;
    }
    let Some(camera) = camera else {
        return;
    };
    let (cam, cam_tf) = *camera;
    let Some(hit) = cursor_ground(pointer.0, cam, cam_tf) else {
        return;
    };
    let (x, z) = open.map.to_map_space((hit.x, hit.z));
    // The tile is the region a fine nudge is held inside — a cell, as the doc above says.
    anchor.cell = Some((
        snap_point(&project, x, SnapLevel::Tile),
        snap_point(&project, z, SnapLevel::Tile),
    ));
}

/// Put a piece in the world — **through `bevy_grimoire`**, which is also what the game uses.
///
/// The editor used to own this arithmetic. It cannot: a preview that disagrees with the runtime by
/// half a degree of `front` or a centimetre of `y_offset` is a preview that lies, and the only way to
/// be sure two places agree is for there to be one place. `bevy_grimoire::spawn_descriptor` is it.
pub(crate) fn spawn_piece(
    commands: &mut Commands,
    assets: &AssetServer,
    piece: Pose<'_>,
    at: (f32, f32),
    origin: (f32, f32, f32),
    y: f32,
) -> Option<Entity> {
    bevy_grimoire::spawn_descriptor(
        commands,
        assets,
        piece.d,
        // The editor draws before tags matter; the runtime resolves them from the library. Defaulting
        // here keeps the signature honest rather than pretending the editor knows.
        bevy_grimoire_core::vocab::Masks::default(),
        at,
        piece.yaw,
        piece.tip,
        // The editor authors in the map's own space, so the origin it draws at is the map's own.
        origin,
        y,
    )
}

/// **Where every piece in the map stands.** Resolved together, because a lamp's height is its table's.
///
/// Returns the reason rather than a partial answer. A map whose stacking will not resolve is one an
/// author has to be told about — drawing it half-right is how a lamp ends up looking badly authored
/// when the real problem is the shelf it names.
fn heights(project: &Project,
    open: &OpenMap) -> Result<Vec<f32>, String> {
    crate::ops_terrain::heights_on_ground(project, open)
}

/// Draw the placements from `first` onward — what a fill or a generate just added.
///
/// The map is already complete when this runs, which is the point: heights are resolved once, over the
/// finished map, so a piece added by the solver stands wherever the finished map says it does.
fn spawn_range(
    commands: &mut Commands,
    assets: &AssetServer,
    project: &Project,
    open: &OpenMap,
    state: &mut EditorState,
    first: usize,
) {
    let ys = match heights(project, open) {
        Ok(ys) => ys,
        Err(e) => {
            state.status.problem(e.clone());
            error!("{e}");
            return;
        }
    };
    let paints = bevy_grimoire::drawn_paints(&open.map, &project.library);
    for (i, p) in open.map.placements.iter().enumerate().skip(first) {
        let (Some(d), Some(&y)) = (descriptor_of(&project.library, p), ys.get(i)) else {
            continue;
        };
        if let Some(e) = spawn_piece(
            commands,
            assets,
            Pose { d: &d, yaw: p.yaw, tip: p.tip },
            p.at,
            open.map.origin,
            y,
        ) {
            commands.entity(e).insert(Placement(p.id.clone()));
            let paint = paints[i];
            if paint != 0 {
                commands.entity(e).insert(bevy_grimoire::Paint(paint));
            }
        }
    }
}

/// The removal marker, spawned once and then only moved.
///
/// Once, because a mesh and a material built per frame are a new asset per frame — the editor would
/// grow a handle a tick until it ran out of memory, which is the shape of bug that looks like "it
/// gets slow after a while" rather than like a leak.
fn spawn_removal_tile(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        RemovalTile,
        // A unit rectangle, scaled to whatever it has to cover. `Rectangle` is authored in the XY
        // plane, so it is laid flat by a quarter turn about X in the system below.
        Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: REMOVE_TINT,
            // **Unlit and blended.** Lit, the marker's brightness would report where the key light
            // happens to be rather than what it means; opaque, it would hide the piece it is
            // pointing at, which is the one thing the author is trying to look at.
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
    ));
}

/// **The removal tool**: show what would go, and take it when the button comes up.
///
/// Preview and commit in one system because they are one question asked twice — the rectangle the
/// marker draws IS the rectangle the release removes, and computing it in two places is how a
/// selection box comes to disagree with what it deletes.
#[allow(clippy::too_many_arguments)]
fn drive_removal(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    over_ui: Res<crate::view::OverUi>,
    pointer: Res<crate::view::Pointer>,
    snip: Res<crate::snip::SnipEdit>,
    report: Res<crate::report::ReportEdit>,
    camera: Option<Single<(&Camera, &GlobalTransform), With<MainCamera>>>,
    placed: Query<(Entity, &Placement)>,
    mut marker: Query<(&mut Transform, &mut Visibility), With<RemovalTile>>,
    mut drag: ResMut<RemovalDrag>,
    mut project: ResMut<Project>,
    mut open: ResMut<OpenMap>,
    mut state: ResMut<EditorState>,
    picture: Res<StampPicture>,
    // The removal box had no idea a grid existed; `J` is what sets which one.
    keyboard: Res<ButtonInput<KeyCode>>,
    rung: Res<Rung>,
) {
    // Read, never write, unless something actually happened: `state` is watched by
    // `rebuild_palette`, so an unconditional deref here rebuilds the palette every frame.
    if state.tool != Tool::Remove {
        if drag.from.is_some() {
            drag.from = None;
        }
        for (_, mut vis) in &mut marker {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
        return;
    }

    let Some(camera) = camera else {
        return;
    };
    let (cam, cam_tf) = *camera;
    // A cursor over the panel is not a cursor over the map, and a marker under the palette is a
    // promise about a click that will never reach the world — the same rule the ghost follows.
    //
    // **Unless a box is already being dragged**, in which case the pointer is captured and the panel
    // has no claim on it — see `drive_place`'s note, and the same keyboard report. A removal box
    // swept across the palette used to vanish and then discard itself on release.
    // A snip being drawn or typed owns the pointer, and blocks the box the way a panel does.
    let blocked = snip.claims_pointer()
        || report.claims_pointer()
        || ui_blocks_gesture(over_ui.0, drag.from.is_some());
    let hit = cursor_ground(pointer.0, cam, cam_tf).filter(|_| !blocked);
    let Some(hit) = hit else {
        // **A release ends the drag wherever it happens.** This used to return before the
        // `just_released` branch, so letting go over a panel or off the map left `drag.from` set —
        // and the red "these will be removed" rectangle then followed the cursor around with no
        // button held, claiming a deletion nobody was making, until the next press reset it. A drag
        // that ends outside the world removes nothing, but it does end.
        if mouse.just_released(MouseButton::Left) {
            drag.from = None;
        }
        for (_, mut vis) in &mut marker {
            *vis = Visibility::Hidden;
        }
        return;
    };
    let at = open.map.to_map_space((hit.x, hit.z));

    if mouse.just_pressed(MouseButton::Left) {
        drag.from = Some(at);
    }

    // What the marker covers: the box being dragged, or — before a drag starts — the footprint of
    // the piece that a click would take, which is what makes "this one" a claim rather than a guess.
    let pitch = snap_level(&keyboard, rung.0).pitch(project.lattice.snap_divisor);
    let rect = match drag.from {
        // **Locked to the live rung** — see `removal_box`. The marker and the deletion below read
        // the one expression, so what the red box covers is what goes.
        Some(from) => Some(removal_box(from, at, pitch)),
        None => pick_at(&project, &open, at).and_then(|i| {
            let p = open.map.placements.get(i)?;
            let d = descriptor_of(&project.library, p)?;
            let (w, depth) = crate::fill::cell_extents(&d, p.yaw, p.tip);
            Some((
                p.at.0 - w * 0.5,
                p.at.1 - depth * 0.5,
                p.at.0 + w * 0.5,
                p.at.1 + depth * 0.5,
            ))
        }),
    };

    for (mut tf, mut vis) in &mut marker {
        match rect {
            Some((x0, z0, x1, z1)) => {
                *vis = Visibility::Visible;
                *tf = Transform::from_xyz(
                    open.map.origin.0 + (x0 + x1) * 0.5,
                    open.map.origin.1 + MARKER_LIFT,
                    open.map.origin.2 + (z0 + z1) * 0.5,
                )
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                // A floor of a few centimetres so a zero-area box is still visible as a box.
                .with_scale(Vec3::new(
                    (x1 - x0).max(0.05),
                    (z1 - z0).max(0.05),
                    1.0,
                ));
            }
            None => *vis = Visibility::Hidden,
        }
    }

    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(from) = drag.from.take() else {
        return;
    };

    if (from.0 - at.0).abs() <= CLICK_EPS && (from.1 - at.1).abs() <= CLICK_EPS {
        match pick_subject(&project, &open, &picture, at) {
            Some(Subject::Placement(i)) => {
                delete_index(&mut commands, &project, i, &mut open, &mut state, &placed)
            }
            // **The instance, whole.** A click landed on one member of it; removing that member is
            // reaching *through* the stamp, which is the thing `composition.rs`'s encapsulation rule
            // forbids and which the missing `Placement` used to prevent by making it unreachable.
            Some(Subject::Stamp(id)) => delete_stamp(&id, &project, &mut open, &mut state),
            None => match pick_run(&project, &open.map, at) {
                // **The run, whole**: a click on one post is reaching through the run, which is the
                // rule a stamp follows, so the run and every row it lays go together.
                Some(id) => remove_run(Some(&mut project), &mut open, &mut state, &id),
                None => state.status.note("nothing here to remove"),
            },
        }
        return;
    }

    let (x0, z0, x1, z1) = removal_box(from, at, pitch);
    // **Each group the box touches goes whole**, the same rule the single delete follows: a rider
    // whose host is inside the box but which is itself a few centimetres outside it would otherwise
    // be left pointing at a placement that no longer exists, and `resolve_y` refuses the whole map for
    // it. Sorted and deduped, so a composition caught twice — box over both a table and its lamp — is
    // removed once.
    let mut doomed: Vec<usize> = open
        .map
        .placements
        .iter()
        .enumerate()
        .filter(|(_, p)| p.at.0 >= x0 && p.at.0 <= x1 && p.at.1 >= z0 && p.at.1 <= z1)
        .flat_map(|(i, _)| bevy_grimoire_core::stack::group_of(&open.map, i))
        .collect();
    doomed.sort_unstable();
    doomed.dedup();

    // **A stamp the box touches goes whole**, on the same rule as a group of placements and for a
    // stronger reason: a box clipping the corner of a nurse station cannot take a corner of it, so
    // the choice is the instance or nothing. Caught by any drawn row's centre, which is the test the
    // rows above use, so a stamp and a loose piece are caught by one rule rather than two.
    let mut stamps: Vec<usize> = picture
        .rows
        .iter()
        .filter(|r| r.at.0 >= x0 && r.at.0 <= x1 && r.at.1 >= z0 && r.at.1 <= z1)
        .filter_map(|r| open.map.stamps.iter().position(|s| s.id == r.stamp))
        .collect();
    stamps.sort_unstable();
    stamps.dedup();

    if doomed.is_empty() && stamps.is_empty() {
        state.status.note("nothing inside that box");
        return;
    }

    // **Each piece goes through the registry, back to front.** Removing an earlier row shifts every
    // later one down, so a forward pass would delete the wrong pieces the moment the box held more
    // than one. The stamps follow, back to front by index, for the same reason.
    // **Each piece is taken by id, not by row.** A removal shifts every later row down, and a stack
    // goes whole: taking a host takes the riders on it, which can sit at a lower row than the host.
    // A row the box found a moment ago may therefore hold a different piece by its turn. The ids are
    // collected from the rows the box found, before anything moves, and an id already gone is skipped.
    let doomed_ids: Vec<String> = doomed
        .iter()
        .filter_map(|i| open.map.placements.get(*i).map(|p| p.id.clone()))
        .collect();
    let mut logged: Vec<Undo> = Vec::with_capacity(doomed_ids.len() + stamps.len());
    let (mut n, mut m) = (0usize, 0usize);
    for id in doomed_ids.iter().rev() {
        if !open.map.placements.iter().any(|p| &p.id == id) {
            continue;
        }
        let Some((line, summary, ids)) = remove_placement(&project, &mut open, &mut state, id) else {
            continue;
        };
        for (entity, mark) in &placed {
            if ids.contains(&mark.0) {
                commands.entity(entity).despawn();
            }
        }
        logged.push(Undo::Logged {
            line,
            phrase: summary,
            agent: false,
        });
        n += ids.len();
    }
    for i in stamps.iter().rev() {
        let Some(stamp) = open.map.stamps.get(*i) else {
            continue;
        };
        let id = stamp.id.clone();
        let Some((line, summary, _)) = remove_stamp(&project, &mut open, &mut state, &id) else {
            continue;
        };
        logged.push(Undo::Logged {
            line,
            phrase: summary,
            agent: false,
        });
        m += 1;
    }
    if logged.is_empty() {
        return;
    }
    // **Undo applies its list in order**, and the removals ran back to front, so the list is reversed:
    // each kind ends up lowest index first, which is the order a put-back needs to find every index
    // still in range. Placements and stamps are separate lists, so how they interleave does not matter.
    logged.reverse();
    // **One keypress, one undo.** A box that caught several pieces leaves several entries behind, and
    // `Undo::Group` exists so they come back together: "two separate entries would make one keypress
    // two undos".
    if logged.len() == 1 {
        if let Some(only) = logged.pop() {
            state.record(only);
        }
    } else {
        state.record(Undo::Group { ops: logged });
    }
    // The whole chord, rendered by the census — naming just the modifier told the author to press
    // `Cmd`, which is not a thing anyone can do.
    let what = match (n, m) {
        (n, 0) => format!("{n} placement(s)"),
        (0, m) => format!("{m} stamp(s)"),
        (n, m) => format!("{n} placement(s) and {m} stamp(s)"),
    };
    state.status.note(format!(
        "removed {what} — {} puts them back",
        keys::chord_text(keys::binding(Action::Undo))
    ));
}

/// **Take a stamp off the map, whole, through the operation registry** (`stamp_remove`), so it is
/// saved, logged and taken back the way a placement is.
///
/// FVS-R-14's Delete arm: *"Delete removes the **instance**."* Named by [`Stamped::id`] and never by
/// an index, because the caller picked it a frame ago and the list can have moved since.
///
/// The entities are not despawned here. `redraw_stamps` rebuilds the whole stamped set from
/// `map.stamps` on every change, so there is one place that turns a stamp list into pictures.
fn delete_stamp(id: &str, project: &Project, open: &mut OpenMap, state: &mut EditorState) {
    let Some((line, summary, _)) = remove_stamp(project, open, state, id) else {
        return;
    };
    state.status.note(format!(
        "{summary} — the whole stamp, not a piece of it. {} puts it back",
        keys::chord_text(keys::binding(Action::Undo))
    ));
    state.record_logged(line, summary, false);
}

/// Bring up whatever the map already holds.
/// The first id counter value that cannot collide with anything the loaded map already names.
///
/// Every id this editor mints is `{short}@{n}`, and the counter used to start at ZERO in every
/// session — so reopening a saved map re-minted the exact `wall@1`, `wall@2`, … the file already
/// carried. The map held two placements with one name, and **undo despawns by id match**: taking
/// back one fill swept every same-named entity off the screen, the originals included. The rows
/// were all still in the file — the screen and the map disagreeing, the exact failure the one-path
/// rule exists to prevent. Seeding past the largest `@n` in the file makes minted ids unique by
/// construction; ids with no `@n` tail (hand-authored names) cannot collide with minted ones.
pub fn next_id_after(map: &bevy_grimoire_core::map::Map) -> u32 {
    // **Both lists, because both are minted from this one counter.** `stamp_here` names a stamp
    // `<short>@<n>` from the same `next_id` a placement uses, so scanning only `placements` seeds the
    // counter below every stamp id the file already carries — and the next stamp collides with one,
    // which `Map::validate` then refuses. One mint, one high-water mark.
    let suffix = |id: &str| id.rsplit_once('@').and_then(|(_, n)| n.parse::<u32>().ok());
    let placements = map.placements.iter().filter_map(|p| suffix(&p.id));
    let stamps = map.stamps.iter().filter_map(|st| suffix(&st.id));
    let runs = map.runs.iter().filter_map(|r| suffix(&r.id));
    placements.chain(stamps).chain(runs).max().unwrap_or(0)
}

fn spawn_existing(
    mut commands: Commands,
    assets: Res<AssetServer>,
    project: Res<Project>,
    open: Res<OpenMap>,
    mut state: ResMut<EditorState>,
) {
    state.next_id = next_id_after(&open.map);
    let ys = match heights(&project, &open) {
        Ok(ys) => ys,
        Err(e) => {
            error!("{e}");
            return;
        }
    };
    let paints = bevy_grimoire::drawn_paints(&open.map, &project.library);
    for (i, p) in open.map.placements.iter().enumerate() {
        let Some(d) = descriptor_of(&project.library, p) else {
            // Loud, not silent: a placement naming a descriptor the library does not have is a hole
            // in the map, and an author must be told which one rather than counting missing crates.
            warn!(
                "placement `{}` names descriptor `{}`, which this library does not have",
                p.id, p.descriptor
            );
            continue;
        };
        let Some(&y) = ys.get(i) else { continue };
        if let Some(e) = spawn_piece(
            &mut commands,
            &assets,
            Pose { d: &d, yaw: p.yaw, tip: p.tip },
            p.at,
            open.map.origin,
            y,
        ) {
            commands.entity(e).insert(Placement(p.id.clone()));
            let paint = paints[i];
            if paint != 0 {
                commands.entity(e).insert(bevy_grimoire::Paint(paint));
            }
        }
    }
}

/// **What a library row carries when it is dragged**: the descriptor's id (UI-52, #238).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PieceDrag(pub String);

/// **A library piece let go over the map is placed where the pointer is** (UI-52, #238).
///
/// gooey sends `DroppedOnWorld` when a drag ends over nothing the interface draws; this takes the pointer
/// to the ground the way a click does, snaps it by the same rung and the same modifier (`brush_at`), and
/// commits through `place_one`, which is the `placement_add` operation with the click's own numbers, so a
/// drop is one log line and one undo step exactly like a click. The piece is laid at the facing the brush
/// has now (what `R` last set), so a drop and a click place alike. Nothing is placed when the pointer is
/// not over the ground, and a piece the library no longer has is a problem on the status line.
#[allow(clippy::too_many_arguments)]
fn drop_a_piece_on_the_map(
    dropped: On<bevy_gooey::DroppedOnWorld<PieceDrag>>,
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    rung: Res<Rung>,
    assets: Res<AssetServer>,
    anchor: Res<FineAnchor>,
    camera: Option<Single<(&Camera, &GlobalTransform), With<MainCamera>>>,
    project: Option<Res<Project>>,
    open: Option<ResMut<OpenMap>>,
    state: Option<ResMut<EditorState>>,
    placed: Query<(Entity, &Placement)>,
) {
    let (Some(camera), Some(project), Some(mut open), Some(mut state)) = (camera, project, open, state) else { return };
    let Some(d) = project.library.get(&dropped.payload.0).cloned() else {
        state.status.problem(format!("`{}` is not in the library any more, so nothing was placed", dropped.payload.0));
        return;
    };
    let (cam, cam_tf) = *camera;
    let Some(hit) = cursor_ground(Some(dropped.pointer), cam, cam_tf) else { return };
    let at = brush_at(&project, &open, &keyboard, rung.0, &anchor, hit, Pose { d: &d, yaw: state.brush_yaw, tip: state.brush_tip });
    place_one(&mut commands, &assets, &placed, &project, &mut open, &mut state, &d, at, keys::alt_held(&keyboard));
}

/// **The placing tool**: a click puts one piece down, a dragged box fills the area with them.
///
/// The box is the same gesture the removal tool uses — press, drag, release — because they are the
/// same question about the same rectangle, and an author who has learnt one has learnt the other. The
/// difference is what happens to it and how it is drawn: removal fills its box translucent red and
/// takes what is inside, this outlines its box and fills it in.
///
/// Preview and commit live in one system for the reason `drive_removal` gives: the rectangle drawn IS
/// the rectangle committed, and computing it twice is how a selection box comes to disagree with what
/// it does.
#[allow(clippy::too_many_arguments)]
