//! **A dead provider is found by one pre-flight turn, and a run stops after three refusals in a row** (#432, D2).
//!
//! The first real-agent run used a model that `omp models find` lists and whose provider had no quota left
//! (HTTP 429, weekly limit). Listing the model proves nothing about the provider, so each trial found out for
//! itself, 35 s apiece, and the worker loop never stopped: at the baseline's 140 trials that is 82 minutes of
//! guaranteed harness errors and an "unfit" report that does not say why.
//!
//! What is proved here, with a stand-in `omp` (a shell script that prints the events a spent quota prints) and
//! no model:
//!
//! - `check_model` + `preflight_turn` refuse a provider that refuses a turn, naming its own words, after ONE turn;
//! - a refused turn in a trial's transcript is what the circuit breaker counts;
//! - [`run_work`] stops after [`PROVIDER_REFUSALS_MAX`] consecutive refusals and records every trial it did
//!   not start as a harness error that carries the reason, so the report says why;
//! - a success, or a harness error that is not the provider's (an editor that died), resets the count.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use bevy_cauldron::ops;
use bevy_cauldron_bench::run::{
    check_model, is_provider_refusal, preflight_turn, read_transcript, run_work, PROVIDER_REFUSALS_MAX,
};
use bevy_cauldron_bench::{Outcome, Task, TrialRecord, TASKS_DIR};

/// What a spent quota says, as omp prints it in an assistant message.
const QUOTA_WORDS: &str = "HTTP 429: you have reached your weekly usage limit";
/// The model every case names.
const MODEL: &str = "anthropic/claude-sonnet-5-5";

/// **A scratch directory this test owns**, named with the test's name and the process id.
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-bench-dead-provider-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// **A stand-in `omp`** in `<dir>/bin`. It lists the model, exports and imports one credential like omp, and
/// answers `-p` by the contents of the file `mode` beside it: `refuse` prints a refused turn, anything else an
/// answered one. Every `-p` it is given is counted in `turns`, one line each.
fn stand_in(dir: &Path, mode: &str) -> PathBuf {
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap_or_else(|e| panic!("{}: {e}", bin.display()));
    std::fs::write(bin.join("mode"), mode).unwrap_or_else(|e| panic!("{e}"));
    let path = bin.join("omp");
    let script = format!(
        r#"#!/bin/sh
here="$(dirname "$0")"
case "$1 $2" in
  "models find") [ -e "$here/unlisted" ] || echo "{MODEL}"; exit 0 ;;
  "auth export") echo '{{"version":1,"credentials":[{{"provider":"anthropic","credential":{{"type":"api_key","key":"k"}}}}]}}'; exit 0 ;;
  "auth import") mkdir -p "$HOME/.omp/agent"; cp "$3" "$HOME/.omp/agent/agent.db"; exit 0 ;;
esac
if [ "$1" = "-p" ]; then
  echo turn >> "$here/turns"
  if [ "$(cat "$here/mode")" = "refuse" ]; then
    echo '{{"type":"message_end","message":{{"role":"assistant","content":[],"stopReason":"error","errorMessage":"{QUOTA_WORDS}"}}}}'
    exit 1
  fi
  echo '{{"type":"message_end","message":{{"role":"assistant","content":[{{"type":"text","text":"ok"}}],"stopReason":"stop","usage":{{"input":3,"output":1,"totalTokens":4}}}}}}'
  exit 0
fi
echo "unexpected: $*" >&2
exit 2
"#
    );
    std::fs::write(&path, script).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap_or_else(|e| panic!("{e}"));
    path
}

/// How many `-p` turns the stand-in was asked for.
fn turns(omp: &Path) -> usize {
    std::fs::read_to_string(omp.with_file_name("turns")).map_or(0, |t| t.lines().count())
}

/// **The repository root**, which is what the suite's paths are relative to.
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|e| panic!("cannot resolve the repository root: {e}"))
}

/// **A real task of the suite**, which is all a work item needs to carry.
fn task() -> Task {
    let known: Vec<&str> = ops::registry().iter().map(|op| op.name()).collect();
    let dir = repo().join(TASKS_DIR).join("refuse-a-piece-the-project-lacks");
    Task::load(&dir, &repo(), &known).unwrap_or_else(|p| panic!("the task loads: {}", p.line()))
}

/// **A trial record of this outcome and reason**, as a trial that ended so would leave it.
fn record(task: &str, trial: u32, outcome: Outcome, reason: &str) -> TrialRecord {
    TrialRecord {
        task: task.to_owned(),
        trial,
        agent: "omp".to_owned(),
        model: Some(MODEL.to_owned()),
        variant: None,
        outcome,
        reason: reason.to_owned(),
        checks: Vec::new(),
        tool_calls: Vec::new(),
        tool_log: Vec::new(),
        tokens: None,
        cost_reported_usd: None,
        edits_behind: Vec::new(),
        wall_s: 0.0,
        tools_offered: Vec::new(),
        wishes: Vec::new(),
        dir: PathBuf::new(),
        held_out: false,
    }
}

/// **A trial whose provider refuses**, run for real up to the transcript: omp's stream is a refused turn, and the
/// reason is what `read_transcript` says of it.
fn refused_trial(dir: &Path, trial: u32) -> TrialRecord {
    let trial_dir = dir.join(format!("trial-{trial}"));
    std::fs::create_dir_all(&trial_dir).unwrap_or_else(|e| panic!("{e}"));
    let transcript = trial_dir.join("transcript.jsonl");
    std::fs::write(
        &transcript,
        format!(
            "{{\"type\":\"message_end\",\"message\":{{\"role\":\"assistant\",\"content\":[],\"stopReason\":\"error\",\"errorMessage\":\"{QUOTA_WORDS}\"}}}}\n"
        ),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let reason = read_transcript(&transcript, &trial_dir, "cauldron_grimoire").expect_err("a refused turn is a harness error");
    record("refuse-a-piece-the-project-lacks", trial, Outcome::HarnessError, &reason)
}

/// **A work list of `n` trials of one task.**
fn work(n: u32) -> Vec<(Task, String, u32)> {
    let task = task();
    (1..=n).map(|trial| (task.clone(), task.id.clone(), trial)).collect()
}

/// **The not-run record a stand-in writes**, as the runner's own does but with no files.
fn not_run(task: &Task, _label: &str, trial: u32, reason: String) -> TrialRecord {
    record(&task.id, trial, Outcome::HarnessError, &reason)
}

/// **A spent quota is found by the pre-flight turn, in the provider's own words, after one turn.**
#[test]
fn a_spent_quota_is_found_by_one_preflight_turn() {
    let dir = scratch("preflight");
    let omp = stand_in(&dir, "refuse");
    let program = omp.to_string_lossy().into_owned();
    check_model(&program, MODEL).unwrap_or_else(|e| panic!("the model is listed, and listing is all this proves: {e}"));
    let work_dir = dir.join("run/preflight");
    let said = preflight_turn(&program, MODEL, &work_dir).expect_err("a provider that refuses the turn is refused");
    assert!(said.contains(QUOTA_WORDS), "the provider's own words are in the refusal: {said}");
    assert!(said.contains(MODEL), "and it names the model: {said}");
    assert_eq!(turns(&omp), 1, "one turn found it, not one per trial");
    assert!(!work_dir.exists(), "the pre-flight's HOME (credential and natives) is deleted: {}", work_dir.display());
    let _ = std::fs::remove_dir_all(&dir);
}

/// **A provider that answers passes the pre-flight, and a model the host does not list is still refused.**
#[test]
fn a_provider_that_answers_passes_and_an_unlisted_model_is_refused() {
    let dir = scratch("answers");
    let omp = stand_in(&dir, "ok");
    let program = omp.to_string_lossy().into_owned();
    preflight_turn(&program, MODEL, &dir.join("run/preflight")).unwrap_or_else(|e| panic!("a provider that answers passes: {e}"));
    assert_eq!(turns(&omp), 1);
    std::fs::write(dir.join("bin/unlisted"), "").unwrap_or_else(|e| panic!("{e}"));
    let said = check_model(&program, MODEL).expect_err("an unlisted model is refused");
    assert!(said.contains("not a model this host lists"), "{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **A refused turn in a trial is a provider refusal**, and a harness error of another cause is not.
#[test]
fn only_a_refused_turn_is_a_provider_refusal() {
    let dir = scratch("classify");
    let refused = refused_trial(&dir, 1);
    assert!(is_provider_refusal(&refused), "a refused turn is counted: {}", refused.reason);
    assert!(refused.reason.contains(QUOTA_WORDS), "and keeps the provider's words: {}", refused.reason);
    assert!(!is_provider_refusal(&record("t", 1, Outcome::HarnessError, "the editor exited with 1 during the trial")));
    assert!(!is_provider_refusal(&record("t", 1, Outcome::Fail, "")), "a failed trial is the agent's");
    assert!(!is_provider_refusal(&record("t", 1, Outcome::Pass, "")));
    let _ = std::fs::remove_dir_all(&dir);
}

/// **A provider that always refuses stops the run after three trials**, and every trial it did not start says why.
#[test]
fn a_run_stops_after_three_consecutive_refusals() {
    let dir = scratch("breaker");
    let started = AtomicUsize::new(0);
    let records = run_work(
        &work(10),
        1,
        &|task, _, trial| {
            started.fetch_add(1, Ordering::SeqCst);
            let mut r = refused_trial(&dir, trial);
            r.task = task.id.clone();
            r
        },
        &not_run,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(PROVIDER_REFUSALS_MAX, 3);
    assert_eq!(started.load(Ordering::SeqCst), 3, "three trials ran, not ten");
    assert_eq!(records.len(), 10, "every trial of the run has a record, so the report counts them all");
    assert!(records.iter().all(|r| r.outcome == Outcome::HarnessError));
    let skipped: Vec<&TrialRecord> = records.iter().filter(|r| !is_provider_refusal(r)).collect();
    assert_eq!(skipped.len(), 7, "the seven trials that were not started");
    for r in skipped {
        assert!(r.reason.contains("not run"), "{}", r.reason);
        assert!(r.reason.contains("3 consecutive provider refusals"), "it says why the run stopped: {}", r.reason);
        assert!(r.reason.contains(QUOTA_WORDS), "and quotes the provider: {}", r.reason);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// **Parallel workers stop too**: trials already running finish, no new one starts.
#[test]
fn parallel_workers_stop_after_the_third_refusal() {
    let dir = scratch("parallel");
    let started = AtomicUsize::new(0);
    let records = run_work(
        &work(30),
        3,
        &|task, _, trial| {
            started.fetch_add(1, Ordering::SeqCst);
            let mut r = refused_trial(&dir, trial);
            r.task = task.id.clone();
            r
        },
        &not_run,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let ran = started.load(Ordering::SeqCst);
    assert!(
        (3..3 + 3).contains(&ran),
        "three refusals trip it, and at most the two other workers' trials were already in flight: {ran} ran"
    );
    assert_eq!(records.len(), 30);
    assert_eq!(records.iter().filter(|r| r.reason.contains("not run")).count(), 30 - ran);
    let _ = std::fs::remove_dir_all(&dir);
}

/// **A trial that is not a refusal resets the count**, whatever it is: a pass, a fail, or a harness error that is not the provider's.
#[test]
fn a_trial_that_is_not_a_refusal_resets_the_count() {
    let dir = scratch("resets");
    let order = [
        Outcome::HarnessError, // refusal
        Outcome::HarnessError, // refusal
        Outcome::Pass,
        Outcome::HarnessError, // refusal
        Outcome::HarnessError, // refusal
        Outcome::Fail,
        Outcome::HarnessError, // refusal
        Outcome::HarnessError, // refusal
        Outcome::HarnessError, // not the provider's: an editor that died
        Outcome::HarnessError, // refusal
        Outcome::HarnessError, // refusal
    ];
    let records = run_work(
        &work(order.len() as u32),
        1,
        &|task, _, trial| {
            let i = (trial - 1) as usize;
            let mut r = match i {
                8 => record(&task.id, trial, Outcome::HarnessError, "the editor exited with 1 during the trial"),
                _ if order[i] == Outcome::HarnessError => refused_trial(&dir, trial),
                _ => record(&task.id, trial, order[i], ""),
            };
            r.task = task.id.clone();
            r
        },
        &not_run,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(records.len(), order.len());
    assert!(
        records.iter().all(|r| !r.reason.contains("not run")),
        "two refusals, then something else, never three in a row, so every trial ran: {:?}",
        records.iter().map(|r| r.reason.clone()).collect::<Vec<_>>()
    );
    let _ = std::fs::remove_dir_all(&dir);
}
