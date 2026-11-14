//! **The held-out tasks are out of every place an agent or a loop can read** (BM-04, #414).
//!
//! The held-out half of a benchmark is what a change is scored on and never tuned against, and it stops
//! being that the moment a pass, an agent or a commit can read it. So the rule is kept by tests rather than
//! by care, and each case here is one way it could break:
//!
//! - the directory they are read from is **inside a repository** (a `git add` away from committed);
//! - a task in it is **not marked held out**, so it would be scored as one without being one;
//! - a trial's project carries **a held-out task's prompt, solution or checks**, or another task's scene;
//! - the run's console line **names a held-out task** or the checks it failed;
//! - the repository's own suite holds **a held-out task** at all.
//!
//! None of it needs an editor: it is a walk over directories and the loader's own refusals.

use bevy_cauldron_bench::run::{summary_line, TrialRecord};
use bevy_cauldron_bench::{fixture, Outcome, Suite};

/// **A scratch directory this test owns**, named with the test's name and the process id.
fn scratch(test: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "grimoire-bench-held-out-{}-{test}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// **Write one file**, making its directory.
fn write(path: &std::path::Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{}: {e}", parent.display()));
    }
    std::fs::write(path, text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

/// **A map file** called `name`, which is all a task needs of a scene to be loaded.
fn map_text(name: &str) -> String {
    format!(
        "version: 9\nname: {name}\nnote: null\norigin:\n- 0.0\n- 0.0\n- 0.0\nbounds:\n- 4.0\n- 3.0\n- 4.0\nbash: null\nplacements: []\n"
    )
}

/// **A task file**, with `marker` in its prompt, its solution and its checks, so a leak is found by text.
fn task_text(id: &str, split: &str, map: &str, marker: &str) -> String {
    format!(
        "id: {id}\ntitle: {marker}-title\nsplit: {split}\ntags: [place]\nmap: {map}\nprompt: |\n  {marker}-prompt\nsetup: []\nsolution:\n  - op: grimoire_present\n    params:\n      title: {marker}-solution\n      summary: {marker}-summary\nchecks:\n  - handback_contains: [{marker}-fact]\nbudget: {{wall_s: 60, tool_calls: 5}}\n"
    )
}

/// **A project root** holding one map the dev suite would use.
fn project(test: &str) -> std::path::PathBuf {
    let root = scratch(test);
    write(&root.join("assets/grimoire/maps/house.map.yaml"), &map_text("house"));
    root
}

/// **A held-out directory outside any repository**, with two tasks, each bringing its own scene.
fn held_dir(test: &str) -> std::path::PathBuf {
    let dir = scratch(&format!("{test}-held"));
    for (id, map, marker) in [("alpha-task", "alpha_room", "ALPHA-SECRET"), ("beta-task", "beta_room", "BETA-SECRET")] {
        write(&dir.join(id).join("task.yaml"), &task_text(id, "held_out", map, marker));
        write(
            &dir.join(id).join("fixture/assets/grimoire/maps").join(format!("{map}.map.yaml")),
            &map_text(map),
        );
    }
    dir
}

/// **Every file under a directory, with its bytes as text.**
fn everything(dir: &std::path::Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(everything(&path));
        } else {
            let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap_or_default()).into_owned();
            out.push((path.display().to_string(), text));
        }
    }
    out
}

/// **A directory inside a repository is refused**, however it got there.
#[test]
fn a_held_out_directory_inside_a_repository_is_refused() {
    let root = project("in_repo");
    let checkout = scratch("in_repo-checkout");
    write(&checkout.join(".git/HEAD"), "ref: refs/heads/main\n");
    let held = checkout.join("bench/held_out");
    write(&held.join("alpha-task/task.yaml"), &task_text("alpha-task", "held_out", "house", "ALPHA-SECRET"));
    let problem = Suite::load_held_out(&root, &held, &[]).expect_err("a held-out directory in a checkout is refused");
    let said = problem.line();
    assert!(said.contains("inside the repository"), "the refusal says why: {said}");
    assert!(said.contains(&checkout.display().to_string()) || said.contains("checkout"), "it names the checkout: {said}");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&checkout);
}

/// **A task that is not marked held out is refused in the held-out directory**, so it is not scored as one.
#[test]
fn a_dev_task_in_the_held_out_directory_is_refused() {
    let root = project("dev_in_held");
    let held = scratch("dev_in_held-held");
    write(&held.join("a-dev-task/task.yaml"), &task_text("a-dev-task", "dev", "house", "DEV-MARKER"));
    let problem = Suite::load_held_out(&root, &held, &[]).expect_err("a dev task there is refused");
    let said = problem.line();
    assert!(said.contains("a-dev-task") && said.contains("held_out"), "the refusal names the task and the split: {said}");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&held);
}

/// **A trial of a held-out task carries its scene and nothing else of it**, and none of another task's.
#[test]
fn a_trial_carries_the_scene_of_its_own_task_and_nothing_else() {
    let root = project("trial");
    let held = held_dir("trial");
    let suite = Suite::load_held_out(&root, &held, &[]).unwrap_or_else(|p| panic!("the held-out tasks load: {}", p.line()));
    assert_eq!(suite.tasks.len(), 2, "both held-out tasks load, each against its own scene");
    let alpha = suite.tasks.iter().find(|t| t.id == "alpha-task").expect("alpha loads");
    let into = scratch("trial-into");
    let laid = fixture::lay_out_task(&root, &alpha.dir, &into).unwrap_or_else(|e| panic!("the trial lays out: {e}"));
    assert!(
        laid.iter().any(|f| f.ends_with("alpha_room.map.yaml")),
        "the task's own scene is laid over the trial: {laid:?}"
    );
    let files = everything(&into);
    assert!(
        files.iter().any(|(path, _)| path.ends_with("assets/grimoire/maps/alpha_room.map.yaml")),
        "its own map is in the trial's project"
    );
    for (path, text) in &files {
        assert!(!path.contains("beta_room"), "another task's scene is not in the trial: {path}");
        assert!(!path.ends_with("task.yaml"), "a task file is not in the trial: {path}");
        for secret in ["ALPHA-SECRET", "BETA-SECRET", "alpha-task", "beta-task"] {
            assert!(!text.contains(secret), "`{secret}` is in the trial's file {path}");
        }
        assert!(!path.starts_with(&held.display().to_string()), "the trial reaches into the held-out directory: {path}");
    }
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&held);
    let _ = std::fs::remove_dir_all(&into);
}

/// **A held-out trial's line on the console names no task and no check.**
#[test]
fn a_held_out_trial_is_one_line_that_names_no_task() {
    let record = |held_out: bool, outcome: Outcome| TrialRecord {
        task: "alpha-task".to_owned(),
        trial: 2,
        agent: "omp".to_owned(),
        model: None,
        variant: None,
        outcome,
        reason: "ALPHA-REASON".to_owned(),
        checks: vec![bevy_cauldron_bench::Verdict {
            check: "placement_at".to_owned(),
            asks: "ALPHA-ASKS".to_owned(),
            passed: false,
            measured: "ALPHA-MEASURED".to_owned(),
        }],
        tool_calls: Vec::new(),
        tool_log: Vec::new(),
        tokens: None,
        cost_reported_usd: None,
        edits_behind: Vec::new(),
        wall_s: 1.0,
        tools_offered: Vec::new(),
        wishes: Vec::new(),
        dir: std::path::PathBuf::new(),
        held_out,
    };
    for outcome in [Outcome::Pass, Outcome::Fail, Outcome::HarnessError] {
        let line = summary_line(&record(true, outcome));
        for secret in ["alpha-task", "placement_at", "ALPHA"] {
            assert!(!line.contains(secret), "a held-out line leaks `{secret}`: {line}");
        }
        assert!(line.contains("held-out"), "it says what it is: {line}");
    }
    let dev = summary_line(&record(false, Outcome::Fail));
    assert!(dev.contains("alpha-task") && dev.contains("placement_at"), "a dev line still names both: {dev}");
}

/// **The repository's own suite holds no held-out task**, and no directory called `held_out`.
///
/// Read as text, so that a task which would not even load is still found.
#[test]
fn the_repositorys_suite_holds_no_held_out_task() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bench = repo.join("bench");
    let mut checked = 0usize;
    for (path, text) in everything(&bench) {
        assert!(
            !path.contains("held_out") && !path.contains("held-out"),
            "the repository holds a path of a held-out task: {path}"
        );
        if path.ends_with("/task.yaml") {
            checked += 1;
            let split = text
                .lines()
                .find_map(|l| l.strip_prefix("split:"))
                .map(str::trim)
                .unwrap_or_default()
                .to_owned();
            assert_eq!(split, "dev", "{path} is in the repository, so it is a dev task, not `{split}`");
        }
    }
    assert!(checked >= 20, "the suite holds its twenty dev tasks, and this walk saw {checked}");
}

/// **A held-out trial is filed by ordinal**, so the task's id is not in the path of the agent's working directory.
#[test]
fn a_held_out_trial_is_filed_by_its_place_and_not_its_id() {
    assert_eq!(bevy_cauldron_bench::run::held_out_label(1), "held-out-1");
    assert_eq!(bevy_cauldron_bench::run::held_out_label(8), "held-out-8");
}

/// **What `trial.json` says of a held-out trial is a count and nothing of the task**, because the file sits in a
/// directory that is an ancestor of every later trial's working directory, and a model can read `..`.
#[test]
fn a_held_out_record_says_how_many_checks_held_and_nothing_of_the_task() {
    let root = project("record");
    let held = held_dir("record");
    let suite = Suite::load_held_out(&root, &held, &[]).unwrap_or_else(|p| panic!("the held-out tasks load: {}", p.line()));
    let alpha = suite.tasks.iter().find(|t| t.id == "alpha-task").expect("alpha loads");
    let verdict = |name: &str, passed: bool| bevy_cauldron_bench::Verdict {
        check: name.to_owned(),
        asks: "ALPHA-ASKS".to_owned(),
        passed,
        measured: "ALPHA-MEASURED".to_owned(),
    };
    let record = |held_out: bool| TrialRecord {
        task: if held_out { bevy_cauldron_bench::run::held_out_label(1) } else { "alpha-task".to_owned() },
        trial: 2,
        agent: "omp".to_owned(),
        model: None,
        variant: None,
        outcome: Outcome::Fail,
        reason: "ALPHA-REASON".to_owned(),
        checks: vec![verdict("placement_at", true), verdict("handback_contains", false)],
        // **What the agent did is the task's content**: for the oracle the log is the solution, arguments and all.
        tool_calls: vec!["ALPHA-CALL".to_owned()],
        tool_log: vec![bevy_cauldron_bench::run::CallLog { tool: "ALPHA-CALL".to_owned(), args: r#"{"piece":"ALPHA-ARGS"}"#.to_owned(), refused: true }],
        tokens: None,
        cost_reported_usd: None,
        edits_behind: vec!["ALPHA-FILE.yaml".to_owned()],
        wall_s: 1.0,
        tools_offered: Vec::new(),
        wishes: vec![serde_json::json!({"tool": "ALPHA-WISH", "task": "ALPHA-WISH-TASK"})],
        dir: std::path::PathBuf::new(),
        held_out,
    };
    let shown = bevy_cauldron_bench::run::record_json(&record(true), alpha, "ALPHA-SENTENCE").to_string();
    for secret in ["alpha-task", "ALPHA", "placement_at", "handback_contains", "title", "prompt", "tags", "sentence"] {
        assert!(!shown.contains(secret), "a held-out record carries `{secret}`: {shown}");
    }
    assert!(shown.contains("held-out-1"), "it is recorded by its place: {shown}");
    assert!(shown.contains(r#""split":"held_out""#), "and still says what it is, which a report needs: {shown}");
    assert!(shown.contains(r#""passed":1"#) && shown.contains(r#""total":2"#), "and keeps the score: {shown}");
    // **The counts survive and the contents do not**: a report counts calls, refusals, wishes and files changed behind the tools.
    let value: serde_json::Value = serde_json::from_str(&shown).unwrap_or_else(|e| panic!("{e}: {shown}"));
    assert_eq!(value["tool_call_count"], 1, "the number of calls is kept: {shown}");
    assert_eq!(value["refusals"], 1, "and the refusals: {shown}");
    assert_eq!(value["tool_calls"].as_array().map(Vec::len), Some(1), "a list still counts what it held: {shown}");
    assert_eq!(value["edits_behind"].as_array().map(Vec::len), Some(1), "{shown}");
    assert_eq!(value["wishes"].as_array().map(Vec::len), Some(1), "{shown}");
    assert_eq!(value["tool_log"].as_array().map(Vec::len), Some(0), "the arguments of the calls are gone: {shown}");
    // A dev record is the whole account, as a report is built from it.
    let dev = bevy_cauldron_bench::run::record_json(&record(false), alpha, "ALPHA-SENTENCE").to_string();
    for kept in ["alpha-task", "ALPHA-ASKS", "ALPHA-MEASURED", "ALPHA-SENTENCE", "ALPHA-SECRET-prompt", "ALPHA-ARGS", "ALPHA-FILE", "ALPHA-WISH-TASK"] {
        assert!(dev.contains(kept), "a dev record still carries `{kept}`: {dev}");
    }
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&held);
}

/// **A held-out directory the runner refuses is refused by place, never by name** (review): the loader's own sentence names the task and its
/// path, and the run's output is a log. The runner says which task of the directory failed, as a count, and a refusal about the
/// directory as a whole names no task either.
#[test]
fn a_refused_held_out_directory_is_named_by_place_and_not_by_task() {
    let root = project("refusal");
    let held = held_dir("refusal");
    // The second task loses the scene it opens, so it does not load.
    std::fs::remove_dir_all(held.join("beta-task/fixture")).unwrap_or_else(|e| panic!("{e}"));
    let said = bevy_cauldron_bench::run::held_out_refusal(&root, &held, &[]);
    assert!(said.contains("task 2 of 2"), "the refusal says which place failed: {said}");
    let held_path = held.display().to_string();
    for secret in ["alpha", "beta", "BETA-SECRET", "ALPHA-SECRET", held_path.as_str()] {
        assert!(!said.contains(secret), "the refusal names `{secret}`: {said}");
    }
    // **A refusal about the directory** (a dev task in it) names no task.
    let mixed = scratch("refusal-mixed-held");
    write(&mixed.join("a-dev-task/task.yaml"), &task_text("a-dev-task", "dev", "house", "DEV-MARKER"));
    let said = bevy_cauldron_bench::run::held_out_refusal(&root, &mixed, &[]);
    assert!(!said.contains("a-dev-task") && !said.contains("DEV-MARKER"), "the refusal names the task: {said}");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&held);
    let _ = std::fs::remove_dir_all(&mixed);
}
