//! **Every task in the benchmark's suite proves itself, twice, with no model in the loop** (BM-01, #411).
//!
//! A task that cannot be solved, and a task that is already solved before an agent touches it, both look
//! exactly like an agent that failed  -  and the second is the *do-nothing loophole* the Agentic Benchmark
//! Checklist names (`10.48550/arXiv.2507.02825`): a task whose checks hold on the untouched scene is passed
//! by an agent that connects and stops. So each task is run twice on a **fresh copy of the fixture**,
//! through the real headless editor and the editor's own dispatch:
//!
//! 1. **the do-nothing run**: `setup`, and nothing else, must leave at least one check failing;
//! 2. **the known answer**: the task's own `solution`, applied the way an agent applies it, must leave
//!    **every** check passing.
//!
//! A task that fails either way fails this test, naming the task, the check and the value the check
//! measured  -  never only that something was wrong.
//!
//! # Why these three shapes, and what they do not cover
//!
//! A suite is three kinds of work: **place something**, **refuse something the project cannot do**, and
//! **report without changing anything**. Between them the three tasks exercise the placement, the byte
//! comparisons, the log, the problem passes and the hand-back checks.
//!
//! **Three checks are not exercised by any of them**: `placement_relative`, `placement_rests_on` and
//! `decide`. The first two need a second piece placed in a relation to the first (a lamp on a desk), which is
//! a fourth task shape; `decide` needs a calibrated decision a model or the decision API makes, which is
//! outside this test's no-model rule. That is stated rather than claimed away: the proof here is for the three
//! shapes the suite ships, and a task of another shape is proved when it is written.
//!
//! # And a task can be invalid in its own file
//!
//! A file with a field this build does not read, an operation it does not have, or a check naming a map
//! nothing holds is refused **when the suite loads**, with its path and line  -  never skipped, because a
//! skipped task is a task that silently stops measuring anything.

use bevy::prelude::*;
use bevy_cauldron::ops::{self, Applied, Ctx};

use bevy_cauldron_bench::{fixture, Grading, Handback, Outcome, Snapshot, Step, Suite, Task, PRESENT_STEP};

/// **The three example tasks the suite ships**, in the order a report lists them. Spelled out rather than
/// discovered, because the acceptance is about *these* three shapes: a suite that gained a fourth task
/// without one of these would still be a suite with a hole.
const EXAMPLES: &[&str] = &[
    "bed-against-the-north-wall",
    "count-what-the-bedroom-holds",
    "refuse-a-piece-the-project-lacks",
];

/// **A scratch path this test owns**, named with the test's name and the process id so a rerun and a
/// parallel test never share one.
fn scratch(test: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "grimoire-fixture-{}-bench-{test}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// **The operations this build has**, which is what a task's steps are checked against. The hand-back is
/// not one of them, and the loader knows that without being told.
fn known() -> Vec<&'static str> {
    ops::registry().iter().map(|op| op.name()).collect()
}

/// **The repository root**, which is what the suite's paths are relative to.
fn repo() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|e| panic!("cannot resolve the repository root: {e}"))
}

/// **A trial's project**: `root`'s fixture laid out under a scratch directory, one copy per call.
///
/// The scratch directory is what the editor calls a project  -  the directory **holding** `assets/`, exactly
/// as the repository's own root is  -  so `assets/grimoire/maps/…` resolves here as it does in a report.
fn trial(root: &std::path::Path, test: &str) -> std::path::PathBuf {
    let into = scratch(test);
    fixture::lay_out(root, &into).unwrap_or_else(|e| panic!("the fixture is laid out: {e}"));
    into
}

/// **One editor, opened on a trial's project**, with the agent's bridge registered so a hand-back is made
/// the way an agent makes one.
fn editor(project: &std::path::Path, map: &str) -> App {
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
/// it, through [`ops::dispatch`]  -  or a hand-back, made over the bridge the MCP server calls.
fn run_steps(app: &mut App, steps: &[Step], label: &str) -> Vec<Handback> {
    let mut ctx = ctx(app);
    let mut handbacks = Vec::new();
    for (n, step) in steps.iter().enumerate() {
        if step.op == PRESENT_STEP {
            handbacks.push(present(app, &step.params));
            continue;
        }
        match ops::dispatch(&mut ctx, &step.op, &step.params) {
            Ok(Applied::Done { .. }) => {}
            other => panic!("{label}: step {n} (`{}`) did not apply: {other:?}", step.op),
        }
    }
    // **The editor's own resource follows the map**, so a hand-back made after a change sees it: the steps
    // were applied to the context, and the bridge reads the resource.
    if let Some(open) = ctx.open.clone() {
        *app.world_mut().resource_mut::<bevy_cauldron::project::OpenMap>() = open;
    }
    for _ in 0..4 {
        app.update();
    }
    handbacks
}

/// **Read a trial's project as the grader does.**
fn snapshot(project: &std::path::Path, map: &str, handbacks: Vec<Handback>) -> Snapshot {
    Snapshot::read(project, map, handbacks, Default::default())
}

/// **One task, proved both ways**, and the sentence a failure quotes.
fn prove(root: &std::path::Path, task: &Task) -> Result<(), String> {
    let project = trial(root, &task.id);
    let mut app = editor(&project, &task.map);

    // **The do-nothing run**: the setup, and nothing else.
    run_steps(&mut app, &task.setup, &task.id);
    let untouched = snapshot(&project, &task.map, Vec::new());
    let nothing = Grading::of(task, &untouched, &untouched);
    if nothing.outcome == Outcome::Pass {
        let _ = std::fs::remove_dir_all(&project);
        return Err(format!(
            "{} — every check already holds after `setup`, so an agent that connects and stops passes it. \
             A task has to be wrong on the untouched scene.",
            nothing.sentence()
        ));
    }

    // **The known answer**, applied the way an agent applies it.
    let handbacks = run_steps(&mut app, &task.solution, &task.id);
    let left = snapshot(&project, &task.map, handbacks);
    let graded = Grading::of(task, &untouched, &left);
    let _ = std::fs::remove_dir_all(&project);
    if graded.outcome != Outcome::Pass {
        return Err(format!(
            "{} — the task's own `solution` does not satisfy its checks, so no run of it could pass.",
            graded.sentence()
        ));
    }
    Ok(())
}

/// **A project the task-file cases can be written against**, made by the test fixture rather than the
/// shipped corpus: one `bed_double` of a stated footprint on a 6 x 6 m floor, so these cases do not move when
/// a person re-measures the shipped library.
///
/// North is `−Z`, so a 2.22 m deep bed is flush against the north wall at `z = −3.0 + 1.11 = −1.89`.
///
/// **This is the one test that writes a project with the editor's own fixture builder**, which is why it
/// reaches `bevy_cauldron`'s test support rather than shipping it: the builder is a test's, not a product.
fn small_room(test: &str) -> std::path::PathBuf {
    let root = scratch(test);
    let grimoire = root.join("assets/grimoire");
    std::fs::create_dir_all(grimoire.join("maps")).unwrap_or_else(|e| panic!("{e}"));
    let fonts = root.join("assets/fonts");
    std::fs::create_dir_all(&fonts).unwrap_or_else(|e| panic!("{e}"));
    let font = repo().join("assets/fonts/FiraMono-Regular.ttf");
    std::fs::copy(&font, fonts.join("FiraMono-Regular.ttf"))
        .unwrap_or_else(|e| panic!("{}: {e}", font.display()));
    let write = |rel: &str, text: &str| {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{e}"));
        }
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    };
    write(
        "assets/grimoire/kits.yaml",
        "version: 3\nnote: null\nlattice:\n  face_bands: 1\n  snap_divisor: 3\n  cell_height: 4.0\nbind:\n- namespace: furniture\n  dir: furniture\nbash: []\nauthoring: furniture\n",
    );
    write(
        "assets/grimoire/furniture/project.yaml",
        "version: 2\nnote: null\nexclude: []\npatches: []\n",
    );
    write(
        "assets/grimoire/furniture/library.yaml",
        "version: 1\nnote: null\ndescriptors:\n- id: bed_double\n  mesh: null\n  align:\n    scale: null\n    stretch_y: null\n    y_offset: 0.0\n    pivot: null\n    rotate: null\n    front: South\n  extent:\n    footprint:\n    - 1.72\n    - 2.22\n    height: 0.714\n  mount: OnFloor\n  clearance: []\n  offers:\n    surfaces: []\n    faces: []\n    sockets: []\n  kind:\n  - prop\n  effects: []\n  look: []\n  placement:\n    rooms: []\n    group: null\n  subgrid: null\n  note: a test's bed\n",
    );
    let vocab = std::fs::read_to_string(repo().join("assets/grimoire/vocab.yaml"))
        .unwrap_or_else(|e| panic!("the repository's vocabulary: {e}"));
    write("assets/grimoire/vocab.yaml", &vocab);
    write(
        "assets/grimoire/maps/bench_bedroom.map.yaml",
        "version: 9\nname: bench_bedroom\nnote: null\norigin:\n- 0.0\n- 0.0\n- 0.0\nbounds:\n- 6.0\n- 3.0\n- 6.0\nbash: null\nplacements: []\n",
    );
    root
}

/// **Write one task file under `root`**, at `bench/tasks/<id>/task.yaml`, from [`TASK_TEXT`].
fn write_task(root: &std::path::Path, id: &str, text: &str) -> std::path::PathBuf {
    let task_dir = root.join(bevy_cauldron_bench::TASKS_DIR).join(id);
    std::fs::create_dir_all(&task_dir).unwrap_or_else(|e| panic!("{}: {e}", task_dir.display()));
    std::fs::write(
        task_dir.join(bevy_cauldron_bench::task::TASK_FILE),
        text.replace("bed-against-the-north-wall", id),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    task_dir
}

/// **Every task in the shipped suite is valid, both ways, with no model.**
#[test]
fn every_task_is_valid() {
    let known = known();
    let suite = Suite::load(&repo(), &repo().join(bevy_cauldron_bench::TASKS_DIR), &known)
        .unwrap_or_else(|p| panic!("the shipped suite loads: {}", p.line()));
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
    assert_eq!(suite.tasks.len(), EXAMPLES.len(), "the suite is exactly the three example shapes");
}

/// **The example tasks are the suite's own three shapes**, so a suite that lost one is visible by name
/// rather than as a count that still looks right.
#[test]
fn the_examples_are_one_of_each_shape() {
    let known = known();
    let suite = Suite::load(&repo(), &repo().join(bevy_cauldron_bench::TASKS_DIR), &known)
        .unwrap_or_else(|p| panic!("the shipped suite loads: {}", p.line()));
    let of = |id: &str| {
        suite
            .tasks
            .iter()
            .find(|t| t.id == id)
            .unwrap_or_else(|| panic!("{id} is in the suite"))
    };
    let bed = of("bed-against-the-north-wall");
    let count = of("count-what-the-bedroom-holds");
    let refuse = of("refuse-a-piece-the-project-lacks");
    assert!(
        bed.solution.iter().any(|s| s.op == "placement_add"),
        "the placement task places something"
    );
    assert!(
        count.checks.iter().any(|c| c.name() == "unchanged")
            && count.solution.iter().all(|s| s.op == PRESENT_STEP),
        "the read-only task changes nothing and is graded on what it said"
    );
    assert!(
        refuse.checks.iter().any(|c| c.name() == "project_unchanged")
            && refuse.checks.iter().any(|c| c.name() == "handback_contains"),
        "the refusal task requires both no change and the words saying why"
    );
}

/// **A task whose own answer does not satisfy its checks fails the proof, naming the check.**
///
/// The answer places the bed at the south wall while the check asks for the north one: the task is
/// unsolvable as stated, and a benchmark that shipped it would score every correct agent as wrong.
#[test]
fn a_task_whose_solution_does_not_satisfy_it_is_refused() {
    let root = small_room("bench_task_wrong_wall");
    let text = TASK_TEXT.replace("at: [0.0, -1.89]", "at: [0.0, 1.89]");
    let task_dir = write_task(&root, "the-wrong-wall", &text);
    let known = known();
    let task = Task::load(&task_dir, &root, &known).unwrap_or_else(|p| panic!("the task loads: {}", p.line()));
    let sentence = prove(&root, &task).expect_err("a task solved the wrong way is refused");
    assert!(
        sentence.contains("placement_near_wall"),
        "the failure names the check that caught it: {sentence}"
    );
    assert!(
        sentence.contains('m'),
        "the failure states the measured gap: {sentence}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// **A piece sunk into the wall fails `placement_near_wall`**, which a gap's `abs()` would have accepted.
///
/// The review of #411 measured this: with `max_gap_m: 0.05`, a bed 3 cm *through* the north wall passed,
/// because the check compared the distance's absolute value. North is `−Z`, so a bed sunk 0.03 m into the
/// north wall stands at `z = −1.92` and its own edge is 0.03 m the wrong side of the plane.
#[test]
fn a_piece_sunk_into_the_wall_fails_the_near_wall_check() {
    let root = small_room("bench_task_sunk_wall");
    let text = TASK_TEXT.replace("at: [0.0, -1.89]", "at: [0.0, -1.92]");
    let task_dir = write_task(&root, "the-bed-in-the-wall", &text);
    let known = known();
    let task = Task::load(&task_dir, &root, &known).unwrap_or_else(|p| panic!("the task loads: {}", p.line()));
    let sentence = prove(&root, &task).expect_err("a bed through the wall is refused");
    assert!(
        sentence.contains("placement_near_wall"),
        "the failure names the near-wall check: {sentence}"
    );
    assert!(
        sentence.contains("through it"),
        "the failure says the piece is through the wall: {sentence}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// **A task whose graded map does not read fails every check about the map**, rather than passing
/// `no_problem` on a default map and reporting "nothing placed".
///
/// A run that leaves the map unparseable has broken the project. The check that would otherwise pass is
/// `no_problem`: nothing found, in a map that does not exist.
#[test]
fn an_unreadable_map_fails_the_map_checks() {
    let root = small_room("bench_task_unreadable_map");
    let task_dir = write_task(&root, "the-broken-map", TASK_TEXT);
    let known = known();
    let task = Task::load(&task_dir, &root, &known).unwrap_or_else(|p| panic!("the task loads: {}", p.line()));
    // **The baseline first, while the map still reads**, so the only difference the grade sees is the map
    // that stops reading. Corrupting it before the baseline would make both snapshots broken.
    let before = snapshot(&root, &task.map, Vec::new());
    assert!(before.map_problem.is_none(), "the baseline's map reads");
    // **A map file that is not a map**, which is what a run that corrupted it leaves.
    let map = root
        .join("assets/grimoire/maps")
        .join(format!("bench_bedroom{}", bevy_grimoire_core::map::MAP_SUFFIX));
    std::fs::write(&map, "this: [is not, a map\n").unwrap_or_else(|e| panic!("{e}"));
    let after = snapshot(&root, &task.map, Vec::new());
    assert!(after.map_problem.is_some(), "the snapshot says the map did not read");
    let graded = Grading::of(&task, &before, &after);
    assert_eq!(graded.outcome, Outcome::Fail, "a run that broke the map failed the task");
    for check in ["placement_exists", "placement_near_wall"] {
        let verdict = graded
            .verdicts
            .iter()
            .find(|v| v.check == check)
            .unwrap_or_else(|| panic!("the task states `{check}`"));
        assert!(!verdict.passed, "`{check}` fails on an unreadable map: {}", verdict.measured);
        assert!(
            verdict.measured.contains("did not read"),
            "`{check}` says the map did not read: {}",
            verdict.measured
        );
    }
    // **And the check that would otherwise PASS**: `no_problem` finds nothing in a map that does not exist,
    // which is the loophole this rule closes. Graded directly, because no shipped task states it.
    let no_problem = bevy_cauldron_bench::Check::NoProblem(bevy_cauldron_bench::NoProblem {
        kinds: vec![bevy_cauldron_bench::ProblemKind::Overlap],
    });
    let verdict = bevy_cauldron_bench::check::grade(&no_problem, &before, &after);
    assert!(
        !verdict.passed && verdict.measured.contains("did not read"),
        "`no_problem` fails on an unreadable map rather than passing on an empty default: {}",
        verdict.measured
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// **A task whose checks already hold after `setup` fails the proof**, which is the do-nothing loophole
/// closed at the scale of one task.
#[test]
fn a_task_already_solved_by_its_setup_is_refused() {
    let root = small_room("bench_task_already_solved");
    let text = TASK_TEXT.replace(
        "setup: []",
        "setup:\n  - op: placement_add\n    params:\n      piece: bed_double\n      at: [0.0, -1.89]\n      yaw: 0.0",
    );
    let task_dir = write_task(&root, "the-bed-is-already-there", &text);
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
    let text = TASK_TEXT.replace(
        "tags: [place, relations]",
        "tags: [place, relations]\nscore_weight: 3",
    );
    let task_dir = write_task(&root, "a-field-nobody-reads", &text);
    let known = known();
    let problem = Task::load(&task_dir, &root, &known).expect_err("an unknown field is refused");
    let line = problem.line();
    assert!(line.contains("task.yaml"), "the refusal names the file: {line}");
    assert!(line.contains("score_weight"), "the refusal names the field: {line}");
    // **The line is in the sentence, which is the reader's own shape**: `yaml::from_str` reports a
    // deserializer's fault as `file (field): line N, column M: why`, for every project file. The
    // structured `Location.line` is for a fault a caller builds from a path it names.
    assert!(
        line.contains("line 5"),
        "the refusal names the line the field stands on: {line}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// **A task naming an operation this build does not have is refused at load, naming it and the step.**
#[test]
fn a_task_with_an_unknown_operation_is_refused_at_load() {
    let root = small_room("bench_task_unknown_op");
    let text = TASK_TEXT.replace("op: placement_add", "op: placement_teleport");
    let task_dir = write_task(&root, "an-operation-nobody-has", &text);
    let known = known();
    let problem = Task::load(&task_dir, &root, &known).expect_err("an unknown operation is refused");
    let line = problem.line();
    assert!(line.contains("placement_teleport"), "the refusal names the operation: {line}");
    assert!(line.contains("solution[0].op"), "the refusal names the step's field: {line}");
    assert!(
        line.contains("at line"),
        "the refusal names the line in its sentence: {line}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// **A check naming a map nothing holds is refused at load**, so a task cannot be graded against a file the
/// fixture and the project do not have.
#[test]
fn a_check_naming_a_missing_map_is_refused_at_load() {
    let root = small_room("bench_task_missing_map");
    let text = TASK_TEXT.replace(
        "map: bench_bedroom",
        "map: a_room_nobody_made",
    );
    let task_dir = write_task(&root, "a-map-nobody-made", &text);
    let known = known();
    let problem = Task::load(&task_dir, &root, &known).expect_err("a missing map is refused");
    let line = problem.line();
    assert!(line.contains("a_room_nobody_made"), "the refusal names the map: {line}");
    assert!(line.contains("task.yaml"), "the refusal names the file: {line}");
    // **The line, in the sentence a person reads** (review of #411, nit): `Location`'s `Display` renders
    // the file and the field and never the line, so a refusal that only set it on the value named no line.
    assert!(
        line.contains("at line"),
        "the refusal names the line in its sentence: {line}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// **A check naming a file nothing holds is refused at load**, for the byte comparisons.
#[test]
fn a_check_naming_a_missing_file_is_refused_at_load() {
    let root = small_room("bench_task_missing_file");
    let text = TASK_TEXT.replace(
        "  - placement_exists: {piece: bed_double, count: 1}",
        "  - placement_exists: {piece: bed_double, count: 1}\n  - unchanged: [maps/a_map_nobody_made.map.yaml]",
    );
    let task_dir = write_task(&root, "a-file-nobody-made", &text);
    let known = known();
    let problem = Task::load(&task_dir, &root, &known).expect_err("a missing file is refused");
    let line = problem.line();
    assert!(
        line.contains("a_map_nobody_made.map.yaml"),
        "the refusal names the file: {line}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// **A check whose name no variant has is refused by name, with the ones this build does have.**
#[test]
fn a_check_this_build_does_not_have_is_refused() {
    let root = small_room("bench_task_unknown_check");
    let text = TASK_TEXT.replace(
        "  - placement_exists: {piece: bed_double, count: 1}",
        "  - placement_is_pretty: {}",
    );
    let task_dir = write_task(&root, "a-check-nobody-has", &text);
    let known = known();
    let problem = Task::load(&task_dir, &root, &known).expect_err("an unknown check is refused");
    let line = problem.line();
    assert!(line.contains("placement_is_pretty"), "the refusal names the check: {line}");
    assert!(line.contains("placement_near_wall"), "the refusal lists the checks that exist: {line}");
    let _ = std::fs::remove_dir_all(&root);
}

/// **The task the cases above are made from**, against the fixture's own 6 x 6 m room: a task file's whole
/// shape, so a change to the format shows up here rather than in a corpus nobody re-reads.
///
/// The bed is 2.22 m deep, so in a room 6 m on `z` (half of it 3 m) a flush bed's centre is 1.11 m from the
/// north wall: `z = −3.0 + 1.11 = −1.89`.
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
