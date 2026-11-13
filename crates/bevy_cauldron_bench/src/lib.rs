//! **A benchmark task for the Cauldron: its prompt, its fixture, its checks, and the grader that
//! measures what a run left** (BM-01, #411).
//!
//! An agent that can drive an editor is only worth measuring against a task that a run can be *wrong*
//! about. So a task here is not a paragraph of prose with a person judging the result at the end: it is
//! a file with a prompt, the operations that set the scene, a known answer, and a list of checks every
//! one of which must hold on what the run left. The checks measure the **saved files** — the map, the
//! library that gives its pieces their extents, the operation log — and never the operation that made
//! the change, so a bug in an operation cannot also hide in the check that was supposed to catch it.
//!
//! ```yaml
//! id: bed-against-the-north-wall          # the directory's own name
//! title: The bed against the north wall
//! split: dev                              # dev | held_out
//! tags: [place, relations]
//! map: bench_bedroom
//! prompt: |
//!   Put a double bed flat against the north wall of the bedroom, centred on that wall.
//! setup: []                               # operations applied before the agent starts
//! solution:                               # a known answer, proved in the gate
//!   - {op: placement_add, params: {piece: bed_double, at: [0.0, -1.39], yaw: 0.0}}
//!   - {op: grimoire_present, params: {title: …, summary: …}}
//! checks:                                 # every one must hold on what the run left
//!   - placement_near_wall: {piece: bed_double, wall: north, max_gap_m: 0.05, centre_within_m: 0.2}
//!   - no_problem: [overlap]
//! budget: {wall_s: 600, tool_calls: 60}
//! ```
//!
//! # Why every task proves itself, with no model
//!
//! A task that cannot be solved, or that is already solved before an agent does anything, measures
//! nothing  -  and both failures look exactly like an agent that failed. The Agentic Benchmark Checklist
//! (`10.48550/arXiv.2507.02825`) names the second as the do-nothing loophole: a task whose checks hold
//! on the untouched scene is passed by an agent that connects and stops. So a task in this suite is
//! proved in the push gate (`crates/bevy_cauldron_bench/tests/every_task_is_valid.rs`), on a copy of the
//! fixture and with no model in the loop: the **do-nothing run must fail at least one check**, and the
//! task's own `solution`, applied through the editor's dispatch, must pass every one of them.
//!
//! # The fixture is the repository's own project
//!
//! `assets/` is the project a task edits, and `bench/fixture/` is laid over the trial's copy of it  -
//! the suite's own map is there, so a person editing the shipped maps cannot silently move a task's
//! starting position, and nothing under `assets/grimoire/maps/` changes for the tests that count those
//! files.
//!
//! # Engine-free, and the runner beside it
//!
//! Nothing in `check`, `grade`, `suite`, `task` or `fixture` imports `bevy`: the grader is a walk over files
//! and the core crate's own arithmetic, which is what lets the same code score a headless proof in the gate
//! and a real agent's trial. `run` is the harness around it (BM-02, #412) — it starts an editor and an
//! agent, and it is the only module here that owns a process or a socket.
//!
//! **The proof that a task is valid lives in this crate's own `tests/`**
//! (`tests/every_task_is_valid.rs`), and the editor is a dev-dependency for it: a test filed under another
//! crate's name is a proof nothing asks for, because `cargo nextest run -p bevy_cauldron_bench` is what a
//! reviewer runs to ask this crate what it does.

pub mod check;
pub mod fixture;
pub mod grade;
// **The runner** (BM-02, #412): a real agent against a real editor, on a fresh copy, n times. It holds the
// HTTP client and the process handling that a trial needs; the grader above it holds none of that, which is
// why the gate can prove a task with no editor and this can prove the editor with no model.
pub mod run;
pub mod suite;
pub mod task;

pub use check::{
    Axis, Check, Direction, HandbackContains, HandbackFact, NoProblem, PlacementAt, PlacementExists,
    PlacementNearWall, PlacementRef, PlacementRefField, PlacementRelative, PlacementRestsOn, ProblemKind,
    Unchanged, Verdict, Wall, CHECK_NAMES,
};
pub use grade::{Grading, Handback, Left, Outcome, Snapshot};
pub use suite::{Suite, Variant, Variants};
pub use run::{Agent, Options, TrialRecord};
pub use task::{Budget, Step, Task, TASK_TOOL_CALLS_DEFAULT, TASK_WALL_S_DEFAULT};

/// **Where the suite's tasks live**, relative to the repository (or a game) root: one directory per
/// task, each holding a `task.yaml`.
pub const TASKS_DIR: &str = "bench/tasks";

/// **Where the files laid over the fixture live**, relative to the repository (or game) root: a tree of
/// the same shape as the project, so `bench/fixture/assets/grimoire/maps/x.map.yaml` lands at
/// `assets/grimoire/maps/x.map.yaml` in the trial's copy.
pub const FIXTURE_DIR: &str = "bench/fixture";

/// **The step that hands a result back**, as an agent's `grimoire_present` call is named. A task's step
/// is either an operation the editor's registry knows, or this  -  a hand-back is not an operation (it
/// changes no file), and it is the only evidence a person's agent leaves that it answered at all.
pub const PRESENT_STEP: &str = "grimoire_present";
