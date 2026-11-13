//! **Every task in the benchmark's suite proves itself, twice, with no model in the loop** (BM-01, #411).
//!
//! A task that cannot be solved, and a task that is already solved before an agent touches it, both look
//! exactly like an agent that failed  -  and the second is the do-nothing loophole the Agentic Benchmark
//! Checklist names (`10.48550/arXiv.2507.02825`). So each task is run twice on a **fresh copy of the
//! fixture**, through the real headless editor and the editor's own dispatch:
//!
//! 1. **the do-nothing run**: `setup`, and nothing else, must leave at least one check failing;
//! 2. **the known answer**: the task's own `solution`, applied the way an agent applies it, must leave
//!    **every** check passing.
//!
//! A task that fails either way fails this test, naming the task, the check and the value the check
//! measured  -  never only that something was wrong.
//!
//! # Why these three shapes, and why they are enough
//!
//! A suite is three kinds of work: **place something**, **refuse something the project cannot do**, and
//! **report without changing anything**. Every check in the closed set is exercised by one of the three, so
//! a check that stops measuring what it says it measures fails here rather than in a report nobody reads.
//!
//! # And a task can be invalid in its own file
//!
//! A file with a field this build does not read, or an operation it does not have, is refused **when the
//! suite loads**, with its path, line and field  -  never skipped, because a skipped task is a task that
//! silently stops measuring anything. The negative cases below are written out and refused, and each
//! asserts the sentence names the file and the field.

use bevy::prelude::*;
use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron_bench::{Handback, Outcome, Snapshot, Suite, Task};
use std::path::{Path, PathBuf};

use crate::fixtures;

/// **The three example tasks the suite ships**, in the order a report lists them. Spelled out rather than
/// discovered, because the acceptance is about *these* three shapes: a suite that gained a fourth task
/// without one of these would still be a suite with a hole.
const EXAMPLES: &[&str] = &["bed-against-the-north-wall", "count-what-the-bedroom-holds", "refuse-a-piece-the-project-lacks"];

/// **A scratch directory this test owns**, named so the fixture sweep in `fixtures` clears it after a run
/// whose process has exited, and removed at the end of the task that made it.
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-fixture-{}-bench-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// **The operations this build has**, which is what a task's steps are checked against. The hand-back is
/// not one of them, and the loader knows that without being told.
fn known() -> Vec<&'static str> {
    ops::registry().iter().map(|op| op.name()).collect()
}

/// **The repository root**, which is what the suite's paths are relative to.
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|e| panic!("cannot resolve the repository root: {e}"))
}

/// **A trial's project**: the fixture laid out under a scratch root, one copy per call.
///
/// The root is what the editor calls a project — the directory **holding** `assets/`, exactly as the
/// repository's own root is — so `assets/grimoire/maps/…` resolves the same way here and in a report.
fn trial(root: &Path, test: &str) -> PathBuf {
    let into = scratch(test);
    bevy_cauldron_bench::fixture::lay_out(root, &into)
        .unwrap_or_else(|e| panic!("the fixture is laid out: {e}"));
    into
}

/// **One editor, opened on a trial's project**, with the agent's bridge registered so a hand-back can be
/// made the way an agent makes it.
fn editor(project: &Path, map: &str) -> App {
    let mut app = bevy_cauldron::harness::build_headless(project, map, None)
        .unwrap_or_else(|e| panic!("the trial's project opens: {e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **The context a dispatch runs with**, over the editor's own project and map.
fn ctx(app: &App) -> Ctx {
    Ctx {
        project: Some(app.world().resource::<bevy_cauldron::project::Project>().clone()),
        open: Some(app.world().resource::<bevy_cauldron::project::OpenMap>().clone()),
        source: ops::Source::Agent,
        session: "bench-every-task-is-valid".to_owned(),
        ..Ctx::default()
    }
}

/// **Make a hand-back over the bridge**, as an agent's `grimoire_present` call does, and answer with the
/// record the grader reads.
fn present(app: &mut App, params: &serde_json::Value) -> Handback {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app
        .world()
        .resource::<RemoteMethods>()
        .get("grimoire/present")
        .cloned()
    else {
        panic!("grimoire/present is not registered");
    };
    let answer = app
        .world_mut()
        .run_system_with(id, Some(params.clone()))
        .unwrap_or_else(|e| panic!("grimoire/present is callable: {e:?}"))
        .unwrap_or_else(|e| panic!("grimoire/present answered: {e:?}"));
    app.update();
    // **The record, from the editor's own `Presenting`**, which is the same value `grimoire/state` shows a
    // person: the words an agent handed back are the words the check reads.
    let presented = app
        .world()
        .resource::<bevy_cauldron::present::Presenting>()
        .0
        .clone()
        .unwrap_or_else(|| panic!("a hand-back is on screen after `{answer}`"));
    Handback {
        id: presented.id,
        title: presented.title,
        summary: presented.summary,
        specified: presented.specified,
        inferred: presented.inferred,
        check: presented.check,
    }
}

/// **Apply a task's steps through the editor's own dispatch**, answering the hand-backs it made.
///
/// A step is either an operation  -  dispatched exactly as an agent's `grimoire_apply_operation` dispatches
/// it, through [`ops::dispatch`]  -  or a hand-back, made over the bridge the way the MCP server makes it.
fn run_steps(app: &mut App, steps: &[bevy_cauldron_bench::Step], label: &str) -> Vec<Handback> {
    let mut ctx = ctx(app);
    let mut handbacks = Vec::new();
    for (n, step) in steps.iter().enumerate() {
        if step.op == bevy_cauldron_bench::PRESENT_STEP {
            handbacks.push(present(app, &step.params));
            continue;
        }
        match ops::dispatch(&mut ctx, &step.op, &step.params) {
            Ok(Applied::Done { .. }) => {}
            other => panic!("{label}: step {n} (`{}`) did not apply: {other:?}", step.op),
        }
    }
    // **The editor's own resources follow the map**, so a hand-back made after a change sees it. The steps
    // were applied to the context; put the result back where the bridge reads it.
    if let Some(open) = ctx.open.clone() {
        *app.world_mut().resource_mut::<bevy_cauldron::project::OpenMap>() = open;
    }
    for _ in 0..4 {
        app.update();
    }
    handbacks
}

/// **Read a trial's project as the grader does.**
fn snapshot(project: &Path, map: &str, handbacks: Vec<Handback>) -> Snapshot {
    Snapshot::read(project, map, handbacks, std::collections::BTreeMap::new())
}

/// **The suite the repository ships**, loaded against this build's registry.
fn shipped() -> Suite {
    let known = known();
    Suite::load(&repo(), &repo().join(bevy_cauldron_bench::TASKS_DIR), &known)
        .unwrap_or_else(|p| panic!("the shipped suite loads: {}", p.line()))
}

/// **One task, proved both ways**, and the sentence a failure quotes.
fn prove(root: &Path, task: &Task) -> Result<(), String> {
    let project = trial(root, &task.id);
    let mut app = editor(&project, &task.map);

    // **The do-nothing run**: the setup, and nothing else.
    run_steps(&mut app, &task.setup, &task.id);
    let untouched = snapshot(&project, &task.map, Vec::new());
    let nothing = bevy_cauldron_bench::Grading::of(task, &untouched, &untouched);
    if nothing.outcome == Outcome::Pass {
        let _ = std::fs::remove_dir_all(&project);
        return Err(format!(
            "{} — every check already holds after `setup`, so an agent that connects and stops passes it. \
             A task has to be wrong on the untouched scene.",
            nothing.sentence()
        ));
    }

    // **The known answer**, applied the way an agent applies one.
    let handbacks = run_steps(&mut app, &task.solution, &task.id);
    let left = snapshot(&project, &task.map, handbacks);
    let graded = bevy_cauldron_bench::Grading::of(task, &untouched, &left);
    let _ = std::fs::remove_dir_all(&project);
    if graded.outcome != Outcome::Pass {
        return Err(format!(
            "{} — the task's own `solution` does not satisfy its checks, so no run of it could pass.",
            graded.sentence()
        ));
    }
    Ok(())
}

/// **Every task in the shipped suite is valid, both ways, with no model.**
#[test]
fn every_task_is_valid() {
    let suite = shipped();
    for id in EXAMPLES {
        assert!(
            suite.tasks.iter().any(|t| t.id == *id),
            "the suite holds `{id}`: it holds {}",
            suite.tasks.iter().map(|t| t.id.as_str()).collect::<Vec<_>>().join(", ")
        );
    }
    let mut failures = Vec::new();
    for task in &suite.tasks {
        if let Err(sentence) = prove(&repo(), task) {
            failures.push(sentence);
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} task(s) are not valid:\n\n{}",
        failures.len(),
        suite.tasks.len(),
        failures.join("\n\n")
    );
}

/// **A project the task-file cases can be written against**, made by the test fixture rather than the
/// shipped corpus: one `bed_double` of a stated footprint on a 6 x 6 m floor, so these cases do not move
/// when a person re-measures the shipped library.
///
/// North is −Z, so a 2.22 m deep bed is flush against the north wall at `z = −3.0 + 1.11 = −1.89`.
fn small_room(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .sized_descriptor("bed_double", "alpha", 1.72, 2.22)
        .side(6.0)
        .build("bench_bedroom")
}

/// **Write one task file under `root`, at `bench/tasks/<id>/task.yaml`**, from [`TASK_TEXT`].
fn write_task(root: &Path, id: &str) -> std::path::PathBuf {
    let task_dir = root.join(bevy_cauldron_bench::TASKS_DIR).join(id);
    std::fs::create_dir_all(&task_dir).unwrap_or_else(|e| panic!("{}: {e}", task_dir.display()));
    std::fs::write(task_dir.join(bevy_cauldron_bench::task::TASK_FILE), TASK_TEXT.replace("bed-against-the-north-wall", id))
        .unwrap_or_else(|e| panic!("{e}"));
    task_dir
}

/// **A task whose own answer does not satisfy its checks fails the proof, naming the check.**
///
/// The answer places the bed at the south wall while the check asks for the north one: the task is
/// unsolvable as stated, and a benchmark that shipped it would score every correct agent as wrong.
#[test]
fn a_task_whose_solution_does_not_satisfy_it_is_refused() {
    let root = small_room("bench_task_wrong_wall");
    let task_dir = write_task(&root, "the-wrong-wall");
    let text = std::fs::read_to_string(task_dir.join(bevy_cauldron_bench::task::TASK_FILE))
        .unwrap_or_else(|e| panic!("{e}"))
        .replace("at: [0.0, -1.89]", "at: [0.0, 1.89]");
    std::fs::write(task_dir.join(bevy_cauldron_bench::task::TASK_FILE), text).unwrap_or_else(|e| panic!("{e}"));
    let known = known();
    let task = Task::load(&task_dir, &root, &known).unwrap_or_else(|p| panic!("the task loads: {}", p.line()));
    let sentence = prove(&root, &task).expect_err("a task solved the wrong way is refused");
    assert!(
        sentence.contains("placement_near_wall"),
        "the failure names the check that caught it: {sentence}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// **A task whose checks already hold after `setup` fails the proof**, which is the do-nothing loophole
/// closed at the scale of one task.
#[test]
fn a_task_already_solved_by_its_setup_is_refused() {
    let root = small_room("bench_task_already_solved");
    let task_dir = write_task(&root, "the-bed-is-already-there");
    let text = std::fs::read_to_string(task_dir.join(bevy_cauldron_bench::task::TASK_FILE))
        .unwrap_or_else(|e| panic!("{e}"))
        .replace(
            "setup: []",
            "setup:\n  - op: placement_add\n    params:\n      piece: bed_double\n      at: [0.0, -1.89]\n      yaw: 0.0",
        );
    std::fs::write(task_dir.join(bevy_cauldron_bench::task::TASK_FILE), text).unwrap_or_else(|e| panic!("{e}"));
    let known = known();
    let task = Task::load(&task_dir, &root, &known).unwrap_or_else(|p| panic!("the task loads: {}", p.line()));
    let sentence = prove(&root, &task).expect_err("a task nothing has to be done for is refused");
    assert!(
        sentence.contains("already holds after `setup`"),
        "the failure says the scene was already solved: {sentence}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// **A task file naming a field this build does not read is refused at load, with its path, line and
/// field**  -  never skipped, which would silently stop the task measuring anything.
#[test]
fn a_task_with_an_unknown_field_is_refused_at_load() {
    let root = small_room("bench_task_unknown_field");
    let task_dir = write_task(&root, "a-field-nobody-reads");
    let text = std::fs::read_to_string(task_dir.join(bevy_cauldron_bench::task::TASK_FILE))
        .unwrap_or_else(|e| panic!("{e}"))
        .replace("tags: [place, relations]", "tags: [place, relations]\nscore_weight: 3");
    std::fs::write(task_dir.join(bevy_cauldron_bench::task::TASK_FILE), text).unwrap_or_else(|e| panic!("{e}"));
    let known = known();
    let problem = Task::load(&task_dir, &root, &known).expect_err("an unknown field is refused");
    let line = problem.line();
    assert!(line.contains("task.yaml"), "the refusal names the file: {line}");
    assert!(line.contains("score_weight"), "the refusal names the field: {line}");
    let _ = std::fs::remove_dir_all(&root);
}

/// **A task naming an operation this build does not have is refused at load, naming it and the step.**
#[test]
fn a_task_with_an_unknown_operation_is_refused_at_load() {
    let root = small_room("bench_task_unknown_op");
    let task_dir = write_task(&root, "an-operation-nobody-has");
    let text = std::fs::read_to_string(task_dir.join(bevy_cauldron_bench::task::TASK_FILE))
        .unwrap_or_else(|e| panic!("{e}"))
        .replace("op: placement_add", "op: placement_teleport");
    std::fs::write(task_dir.join(bevy_cauldron_bench::task::TASK_FILE), text).unwrap_or_else(|e| panic!("{e}"));
    let known = known();
    let problem = Task::load(&task_dir, &root, &known).expect_err("an unknown operation is refused");
    let line = problem.line();
    assert!(line.contains("placement_teleport"), "the refusal names the operation: {line}");
    assert!(line.contains("solution[0].op"), "the refusal names the step's field: {line}");
    let _ = std::fs::remove_dir_all(&root);
}

/// **The task the cases above are made from**, against the fixture's own 6 x 6 m room: a task file's
/// whole shape, so a change to the format shows up here rather than in a corpus nobody re-reads.
///
/// The bed is 2.22 m deep, so in a room 6 m on `z` (half of it 3 m) a flush bed's centre is 1.11 m from
/// the north wall: `z = −3.0 + 1.11 = −1.89`.
const TASK_TEXT: &str = "\
id: bed-against-the-north-wall
title: The bed against the north wall
split: dev
tags: [place, relations]
map: bench_bedroom
prompt: |
  Put a double bed flat against the north wall of the bedroom, centred on that wall.
setup: []
solution:
  - op: placement_add
    params:
      piece: bed_double
      at: [0.0, -1.89]
      yaw: 0.0
  - op: grimoire_present
    params:
      title: The bed against the north wall
      summary: A double bed stands flush against the north wall, centred on it.
checks:
  - placement_exists: {piece: bed_double, count: 1}
  - placement_near_wall: {piece: bed_double, wall: north, max_gap_m: 0.05, centre_within_m: 0.2}
budget: {wall_s: 600, tool_calls: 60}
";

/// **The example tasks are the suite's own three shapes**, so a suite that lost one is visible by name
/// rather than as a count that still looks right.
#[test]
fn the_examples_are_one_of_each_shape() {
    let suite = shipped();
    let of = |id: &str| suite.tasks.iter().find(|t| t.id == id).unwrap_or_else(|| panic!("{id} is in the suite"));
    let bed = of("bed-against-the-north-wall");
    let count = of("count-what-the-bedroom-holds");
    let refuse = of("refuse-a-piece-the-project-lacks");
    assert!(
        bed.solution.iter().any(|s| s.op == "placement_add"),
        "the placement task places something"
    );
    assert!(
        count.checks.iter().any(|c| c.name() == "unchanged") && count.solution.iter().all(|s| s.op == bevy_cauldron_bench::PRESENT_STEP),
        "the read-only task changes nothing and is graded on what it said"
    );
    assert!(
        refuse.checks.iter().any(|c| c.name() == "project_unchanged")
            && refuse.checks.iter().any(|c| c.name() == "handback_contains"),
        "the refusal task requires both no change and the words saying why"
    );
    let _ = fixtures::DEFAULT_KIT;
}
