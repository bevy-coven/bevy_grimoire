//! **`cauldron-bench compare --json` has the shape the release check reads** (BM-05, #415).
//!
//! `tools/release/bench_gate.py` decides whether a release may go from this JSON, in Python, so its keys are a
//! contract between two languages that no type checker joins. This test pins the contract from this side: the
//! real binary, two recorded runs written into a scratch directory (no model, no editor, no network), and the
//! keys and spellings the gate reads, each asserted by name: the comparison's `change`, `moved`, `interval`,
//! `across` (whose suite-only entries start with the words the gate lets through) and `tasks_paired`, and the
//! published report's header and held-out total (`cauldron-bench report --label`), which the gate reads for
//! the commit, agent, model, variant and trial count it must match and for the held-out mean and interval.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

/// **Trials of each task** the fixtures record, the release's own count.
const TRIALS: u32 = 5;
/// **Dev tasks in the fixture suite**, enough for a paired bootstrap to be wholly below zero.
const TASKS: usize = 6;

/// **A scratch directory that includes the test's name and this process**, removed first.
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("compare-json-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// **The words an `across` entry about the suite starts with**, which the gate lets through and nothing else.
const SUITE_WORDS: &str = "the suites differ";

/// **A recorded run**: `run.json` and `TASKS` dev tasks, task `i` passing `passes[i]` of its `TRIALS` trials.
fn run(root: &Path, name: &str, passes: [u32; TASKS]) -> PathBuf {
    run_with(root, name, "bench/tasks", passes, &[])
}

/// **A recorded run of a named suite**, with held-out tasks `h0..` passing `held[i]` of their `TRIALS` trials.
fn run_with(root: &Path, name: &str, suite: &str, passes: [u32; TASKS], held: &[u32]) -> PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).expect("a run directory");
    let header = json!({
        "version": 1, "label": name, "commit": "0123456789abcdef0123456789abcdef01234567",
        "guide_sha256": "0", "agent": "omp", "agent_version": "1", "model": "m", "variant": null,
        "suite": suite, "trials": TRIALS, "host": "test", "date": "2026-11-13", "price": null
    });
    std::fs::write(dir.join("run.json"), header.to_string()).expect("run.json");
    for (i, passed) in passes.iter().enumerate() {
        for trial in 0..TRIALS {
            let at = dir.join(format!("t{i}")).join(format!("trial-{trial}"));
            std::fs::create_dir_all(&at).expect("a trial directory");
            let outcome = if trial < *passed { "pass" } else { "fail" };
            let data = json!({ "task": format!("t{i}"), "trial": trial, "split": "dev", "outcome": outcome });
            std::fs::write(at.join("trial.json"), data.to_string()).expect("trial.json");
        }
    }
    for (i, passed) in held.iter().enumerate() {
        for trial in 0..TRIALS {
            let at = dir.join(format!("h{i}")).join(format!("trial-{trial}"));
            std::fs::create_dir_all(&at).expect("a trial directory");
            let outcome = if trial < *passed { "pass" } else { "fail" };
            let data = json!({ "task": format!("h{i}"), "trial": trial, "split": "held_out", "outcome": outcome });
            std::fs::write(at.join("trial.json"), data.to_string()).expect("trial.json");
        }
    }
    dir
}

/// **`cauldron-bench compare a b --across --json`**, parsed.
fn compare_json(a: &Path, b: &Path) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_cauldron-bench"))
        .args(["compare"])
        .arg(a)
        .arg(b)
        .args(["--across", "--json"])
        .output()
        .expect("the binary runs");
    assert!(out.status.success(), "compare failed: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).expect("compare --json prints one JSON document")
}

#[test]
fn a_run_that_fell_says_worse_and_lists_the_tasks_with_their_counts() {
    let root = scratch("worse");
    let before = run(&root, "before", [5, 5, 5, 5, 5, 4]);
    let after = run(&root, "after", [0, 1, 0, 0, 2, 5]);
    let got = compare_json(&before, &after);

    assert_eq!(got["change"], "worse", "the verdict is snake_case words the gate reads: {got}");
    assert!(got["interval"]["hi"].as_f64().expect("interval.hi") < 0.0, "worse is wholly below zero: {got}");
    assert!(got["interval"]["lo"].as_f64().is_some(), "interval.lo is a number: {got}");
    assert!(got["difference"].as_f64().expect("difference") < 0.0, "{got}");
    assert_eq!(got["tasks_paired"], TASKS as u64);
    assert_eq!(got["across"], json!([]), "alike runs differ in nothing");

    let moved = got["moved"].as_array().expect("moved is an array");
    let first = moved.iter().find(|m| m["task"] == "t0").expect("t0 moved");
    for (key, want) in [("a_successes", 5), ("a_trials", 5), ("b_successes", 0), ("b_trials", 5)] {
        assert_eq!(first[key], want, "t0's {key}: {first}");
    }
    let rose = moved.iter().find(|m| m["task"] == "t5").expect("t5 moved, and upward");
    assert_eq!((rose["a_successes"].as_u64(), rose["b_successes"].as_u64()), (Some(4), Some(5)));
}

#[test]
fn two_alike_runs_say_no_measurable_change_spelled_with_underscores() {
    let root = scratch("alike");
    let a = run(&root, "a", [5, 4, 3, 5, 2, 5]);
    let b = run(&root, "b", [5, 4, 3, 5, 2, 5]);
    let got = compare_json(&a, &b);

    assert_eq!(got["change"], "no_measurable_change", "{got}");
    assert_eq!(got["moved"], json!([]), "no task's count moved: {got}");
}

#[test]
fn a_difference_of_suite_is_said_in_the_words_the_gate_lets_through() {
    let root = scratch("suite");
    let a = run_with(&root, "a", "bench/tasks", [5, 4, 3, 5, 2, 5], &[]);
    let b = run_with(&root, "b", "bench/other", [5, 4, 3, 5, 2, 5], &[]);
    let got = compare_json(&a, &b);

    let across = got["across"].as_array().expect("across is an array");
    assert_eq!(across.len(), 1, "only the suite differs: {got}");
    let text = across[0].as_str().expect("an across entry is a sentence");
    assert!(text.starts_with(SUITE_WORDS), "the gate keys on these words: {text}");
}

#[test]
fn a_published_report_carries_the_keys_the_release_gate_reads() {
    let root = scratch("report");
    let dir = run_with(&root, "gate", "bench/tasks", [5, 5, 5, 5, 5, 4], &[5, 5, 4, 5]);
    let published = root.join("published");
    let out = Command::new(env!("CARGO_BIN_EXE_cauldron-bench"))
        .arg("report")
        .arg(&dir)
        .args(["--label", "gate", "--publish"])
        .arg(&published)
        .output()
        .expect("the binary runs");
    assert!(out.status.success(), "report failed: {}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(published.join("runs").join("2026-11-13-gate.json")).expect("the published .json");
    let got: Value = serde_json::from_str(&text).expect("a report is JSON");

    let header = &got["header"];
    assert_eq!(header["commit"], "0123456789abcdef0123456789abcdef01234567");
    assert_eq!(header["label"], "gate");
    assert_eq!(header["date"], "2026-11-13");
    assert_eq!(header["agent"], "omp");
    assert_eq!(header["model"], "m");
    assert!(header.get("variant").is_some(), "the variant key is always written (null when no arm): {header}");
    assert_eq!(header["trials"], TRIALS);
    assert!(got.get("unfit").is_some(), "the unfit key is always written: {got}");
    assert!(got["dev"]["mean_success"].as_f64().is_some(), "{got}");
    let held = &got["held_out"];
    assert!(held["mean_success"].as_f64().is_some(), "held_out.mean_success is a number: {held}");
    assert!(held["interval"]["lo"].as_f64().is_some() && held["interval"]["hi"].as_f64().is_some(), "held_out.interval: {held}");
    assert!(got.to_string().find("\"h0\"").is_none(), "a held-out task is never named in a report: {got}");
}
