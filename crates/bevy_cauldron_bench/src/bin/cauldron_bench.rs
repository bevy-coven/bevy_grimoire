//! **`cauldron-bench`: run every task of a suite `n` times, and write what each trial left** (BM-02, #412).
//!
//! ```text
//! cauldron-bench run --suite dev --trials 5
//! cauldron-bench run --agent oracle            # no model: the task's own answer, through the real editor
//! cauldron-bench run --agent nothing           # connects and stops: must pass nothing
//! cauldron-bench run --variant no-frame-view   # an arm from bench/variants.yaml
//! cauldron-bench run --project ../a-game --suite .dark_factory/bench
//! ```
//!
//! **The default suite is the repository's own**, and the default project is the root this binary's
//! repository was built in, so `cauldron-bench run` with no arguments measures this checkout  -  the editor
//! binary built at the commit under test, and the tasks beside it.
//!
//! # Where things land
//!
//! Every trial's directory, its project, its editor log and its `trial.json` go under
//! `~/.local/state/cauldron-bench/<run>/`  -  **never the repository**, because a benchmark is not the
//! project it measures and its raw output must not appear in the project's `git status`. `<run>` is
//! `--run`, or the time this run started.
//!
//! # What it refuses
//!
//! A suite with one task that will not load, an `--agent` this build does not have, a `--variant` the suite
//! does not state, an editor or server binary that is not there, and  -  for `--agent omp`  -  a model this
//! host does not list. Every one of those would otherwise turn into a run of harness errors that says
//! nothing about the editor.

use std::path::PathBuf;

use bevy_cauldron_bench::matrix::{self, Pair};
use bevy_cauldron_bench::run::{self, Agent, Options};

/// **What `--help` prints.**
const USAGE: &str = "\
cauldron-bench: run the Cauldron's benchmark suite against a real editor and a real agent.

usage: cauldron-bench run [options]
       cauldron-bench report <run directory> [--label <name>] [--publish <docs/bench>]
       cauldron-bench compare <a> <b> [--across] [--json]
       cauldron-bench matrix --pairs <agent:model,agent:model,...> [run options]
       cauldron-bench clean [--older-than <days>] [--dry-run]

  --suite <dir>       the task directory (default: bench/tasks). A game's own is .dark_factory/bench
  --project <root>    the project the tasks run against (default: this repository's root)
  --agent <name>      omp | claude-code | oracle | nothing (default: omp)
  --trials <n>        how many trials of each task (default: 1)
  --jobs <n>          how many trials run at once (default: 1)
  --model <name>      the model the harness runs (omp default: ollama-cloud/deepseek-v4.1-flash:1017; claude-code: sonnet)
  --variant <name>    an arm from <suite>/variants.yaml
  --task <id>         only this task; repeatable
  --held-out <dir>    also run the held-out tasks in <dir>, which must be outside every repository (the share
                      keeps them at /mnt/codex_fs/bench/dark_factory/held_out). A model is run on them only
                      after `nothing` fails and `oracle` passes each one; they are reported as a total only
  --editor <path>     the editor binary under test (default: the one beside this binary)
  --server <path>     the MCP server binary beside it (default: beside this binary)
  --run <name>        the name of this run's directory under ~/.local/state/cauldron-bench
  --label <name>      publish the run's report as docs/bench/runs/<date>-<name>.md and .json, and list it in
                      docs/bench/README.md (without it the report stays beside the trials)
  -h, --help          print this and exit

Every trial copies the fixture project, starts the editor on its own port and display, hands an agent the
MCP config the editor itself gives a person's agent, and writes trial.json — the outcome, each check with
what it measured, the tool calls, tokens and wall time. The run ends with a report (success, pass^k, wall
time, tool calls, refusals, undos, edits behind the editor's back, tokens and cost, per task, tag and suite).

`report` builds the same report from a run directory. `compare` takes two reports (the .json files a run
published, or run directories) and says `better`, `worse` or `no measurable change` with a paired 95%
bootstrap interval; it refuses a run with more than 10% harness errors, and two runs of different suites,
agents or trial counts unless --across is given. --json prints the comparison as JSON instead of text, which is
what the release check (`tools/release/bench_gate.py`, BM-05) reads.";

fn main() -> std::process::ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.iter().any(|a| a == "--help" || a == "-h") || argv.is_empty() {
        println!("{USAGE}");
        return if argv.is_empty() {
            std::process::ExitCode::FAILURE
        } else {
            std::process::ExitCode::SUCCESS
        };
    }
    let rest: Vec<String> = match argv.first().map(String::as_str) {
        Some("run") => argv[1..].to_vec(),
        Some("report") => return report_command(&argv[1..]),
        Some("compare") => return compare_command(&argv[1..]),
        Some("matrix") => return matrix_command(&argv[1..]),
        Some("clean") => return clean_command(&argv[1..]),
        Some(other) => {
            eprintln!("cauldron-bench: `{other}` is not a command. The commands are: run, report, compare, matrix, clean.");
            eprintln!("Try `cauldron-bench --help`.");
            return std::process::ExitCode::FAILURE;
        }
        None => argv.clone(),
    };
    let (options, held_out) = match parse(&rest) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("cauldron-bench: {e}");
            eprintln!("Try `cauldron-bench --help`.");
            return std::process::ExitCode::FAILURE;
        }
    };
    // **Ctrl-C and SIGTERM stop the run through its guards**, so a trial's credential directory is not left behind (#433).
    run::install_interrupt_handler();
    let records = match run::run(&options, held_out.as_deref()) {
        Ok(records) => records,
        Err(e) => {
            eprintln!("cauldron-bench: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let mut failed = 0usize;
    let mut harness = 0usize;
    for record in &records {
        println!("{}", run::summary_line(record));
        match record.outcome {
            bevy_cauldron_bench::Outcome::Pass => {}
            bevy_cauldron_bench::Outcome::Fail => failed += 1,
            bevy_cauldron_bench::Outcome::HarnessError => harness += 1,
        }
    }
    println!(
        "\n{} trial(s): {} passed, {failed} failed, {harness} harness error(s). Files are under {}",
        records.len(),
        records.len() - failed - harness,
        options.out.display()
    );
    // **The report the run ends with.** A report that cannot be written is said and fails the run, but
    // the trials above have already been shown: their files are the record.
    match run::write_reports(&options) {
        Ok((_, written)) => {
            for path in written {
                println!("report: {}", path.display());
            }
        }
        Err(e) => {
            eprintln!("cauldron-bench: the trials ran, and their report could not be written: {e}");
            return std::process::ExitCode::FAILURE;
        }
    }
    // **A harness error is a failed run**, whatever the checks said: the numbers are not about the agent.
    if failed == 0 && harness == 0 {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

/// **The options a command line names**, refusing an argument this build does not take, and the directory
/// `--held-out` names (a place on this host, which is not one of a run's settings).
fn parse(argv: &[String]) -> Result<(Options, Option<PathBuf>), String> {
    let here = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .ok_or("this binary's own directory could not be found")?;
    let repo = repository_root().unwrap_or_else(|| PathBuf::from("."));
    let root = repo.clone();
    let mut options = Options {
        tasks: root.join(bevy_cauldron_bench::TASKS_DIR),
        root,
        agent: Agent::Omp,
        trials: 1,
        jobs: 1,
        model: String::new(),
        variant: None,
        out: PathBuf::new(),
        editor: here.join("bevy_cauldron"),
        server: here.join("bevy_cauldron_mcp"),
        only: Vec::new(),
        repo,
        label: String::new(),
        publish: None,
    };
    let mut held_out: Option<PathBuf> = None;
    let mut run_name: Option<String> = None;
    let mut label: Option<String> = None;
    let mut model: Option<String> = None;
    let mut it = argv.iter();
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or_else(|| format!("`{arg}` needs a value"))
        };
        match arg.as_str() {
            "--suite" => options.tasks = PathBuf::from(value()?),
            "--project" => options.root = PathBuf::from(value()?),
            "--agent" => options.agent = Agent::parse(&value()?)?,
            "--trials" => options.trials = number(&value()?, "--trials")?,
            "--jobs" => options.jobs = number(&value()?, "--jobs")?,
            "--model" => model = Some(value()?),
            "--variant" => options.variant = Some(value()?),
            "--task" => options.only.push(value()?),
            "--editor" => options.editor = PathBuf::from(value()?),
            "--server" => options.server = PathBuf::from(value()?),
            "--run" => run_name = Some(value()?),
            "--label" => label = Some(value()?),
            "--held-out" => held_out = Some(PathBuf::from(value()?)),
            other => return Err(format!("`{other}` is not an argument this command takes")),
        }
    }
    options.model = model.unwrap_or_else(|| options.agent.default_model().to_owned());
    if options.trials == 0 {
        return Err("--trials 0 is a run of nothing; say how many trials to make".to_owned());
    }
    let name = run_name.unwrap_or_else(|| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        format!("{now}")
    });
    options.out = run::state_dir(&name)?;
    // **A named run is published**: it is the one a person means to commit and compare. An unnamed run's
    // report stays beside its trials, so a dozen oracle runs leave the working tree alone.
    options.publish = label
        .as_ref()
        .map(|_| options.root.join(bevy_cauldron_bench::index::BENCH_DOCS_DIR));
    options.label = label.unwrap_or(name);
    Ok((options, held_out))
}

/// **`cauldron-bench matrix --pairs a,b,...`**: one suite across several agent and model pairs, success
/// against cost, with the frontier marked (BM-06, #416).
fn matrix_command(argv: &[String]) -> std::process::ExitCode {
    let mut rest: Vec<String> = Vec::new();
    let mut list: Option<String> = None;
    let mut it = argv.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--pairs" => list = it.next().cloned(),
            // **The pair is the agent and the model**, so naming either beside it would be two answers.
            "--agent" | "--model" => {
                eprintln!("cauldron-bench: `{arg}` has no place in a matrix: name each pair in `--pairs agent:model,...`");
                return std::process::ExitCode::FAILURE;
            }
            other => rest.push(other.to_owned()),
        }
    }
    let Some(list) = list else {
        eprintln!("cauldron-bench: `matrix` needs `--pairs`, like `--pairs omp:ollama-cloud/deepseek-v4.1-flash:1017,claude-code:sonnet`");
        return std::process::ExitCode::FAILURE;
    };
    let pairs = match Pair::parse_all(&list) {
        Ok(pairs) => pairs,
        Err(e) => {
            eprintln!("cauldron-bench: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let (options, held_out) = match parse(&rest) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("cauldron-bench: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    match matrix::run(&options, held_out.as_deref(), &pairs) {
        Ok((matrix, written)) => {
            println!("{}", matrix.markdown());
            for path in written {
                println!("report: {}", path.display());
            }
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("cauldron-bench: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// **`cauldron-bench report <run directory>`**: the report of a run that has already finished.
fn report_command(argv: &[String]) -> std::process::ExitCode {
    let mut dir: Option<PathBuf> = None;
    let mut label: Option<String> = None;
    let mut publish: Option<PathBuf> = None;
    let mut it = argv.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--label" | "--publish" => {
                let Some(value) = it.next() else {
                    eprintln!("cauldron-bench: `{arg}` needs a value");
                    return std::process::ExitCode::FAILURE;
                };
                if arg == "--label" {
                    label = Some(value.clone());
                } else {
                    publish = Some(PathBuf::from(value));
                }
            }
            other if !other.starts_with("--") && dir.is_none() => dir = Some(PathBuf::from(other)),
            other => {
                eprintln!("cauldron-bench: `{other}` is not an argument `report` takes. Try `--help`.");
                return std::process::ExitCode::FAILURE;
            }
        }
    }
    let Some(dir) = dir else {
        eprintln!("cauldron-bench: `report` needs the run directory to report");
        return std::process::ExitCode::FAILURE;
    };
    let mut data = match bevy_cauldron_bench::report::RunData::read(&dir) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("cauldron-bench: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    if let Some(label) = &label {
        data.header.label = label.clone();
    }
    let report = bevy_cauldron_bench::report::Report::build(&data);
    println!("{}", report.markdown());
    if label.is_some() || publish.is_some() {
        let bench = publish.unwrap_or_else(|| {
            repository_root().unwrap_or_else(|| PathBuf::from(".")).join(bevy_cauldron_bench::index::BENCH_DOCS_DIR)
        });
        match bevy_cauldron_bench::index::publish(&bench, &report) {
            Ok((md, json)) => println!("published: {} and {}", md.display(), json.display()),
            Err(e) => {
                eprintln!("cauldron-bench: {e}");
                return std::process::ExitCode::FAILURE;
            }
        }
    }
    std::process::ExitCode::SUCCESS
}

/// **`cauldron-bench compare <a> <b>`**: what changed between two runs.
fn compare_command(argv: &[String]) -> std::process::ExitCode {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut across = false;
    let mut json = false;
    for arg in argv {
        match arg.as_str() {
            "--across" => across = true,
            "--json" => json = true,
            other if !other.starts_with("--") => paths.push(PathBuf::from(other)),
            other => {
                eprintln!("cauldron-bench: `{other}` is not an argument `compare` takes. Try `--help`.");
                return std::process::ExitCode::FAILURE;
            }
        }
    }
    let [a, b] = paths.as_slice() else {
        eprintln!("cauldron-bench: `compare` takes two reports or run directories, `a` and then `b`");
        return std::process::ExitCode::FAILURE;
    };
    let result = bevy_cauldron_bench::report::Report::load(a).and_then(|a| {
        let b = bevy_cauldron_bench::report::Report::load(b)?;
        bevy_cauldron_bench::compare::compare(&a, &b, across)
    });
    match result {
        Ok(comparison) if json => match serde_json::to_string_pretty(&comparison) {
            Ok(text) => {
                println!("{text}");
                std::process::ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("cauldron-bench: the comparison did not serialize: {e}");
                std::process::ExitCode::FAILURE
            }
        },
        Ok(comparison) => {
            println!("{}", comparison.markdown());
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("cauldron-bench: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// **A count**, refused rather than defaulted on anything that is not one.
fn number(text: &str, flag: &str) -> Result<u32, String> {
    text.parse::<u32>()
        .map_err(|_| format!("`{flag} {text}`: `{text}` is not a whole number"))
}

/// **The repository this binary was built in**, so `run` with no arguments measures this checkout.
fn repository_root() -> Option<PathBuf> {
    // **`CARGO_MANIFEST_DIR` is where the crate was built**, not where the binary is run, which is what a
    // benchmark wants: `cauldron-bench run` measures the checkout it came from.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("../..").canonicalize().ok()
}

/// **How old a credential directory must be for `clean` to remove it, in days, unless `--older-than` says otherwise.**
const CLEAN_DEFAULT_DAYS: u64 = 1;

/// **`cauldron-bench clean [--older-than <days>] [--dry-run]`**: remove the credential directories (a trial's `home`,
/// `claude-config` and a run's `preflight`) that runs left under `~/.local/state/cauldron-bench` (#434).
///
/// Runs before the credential guard (#432) kept every trial's `home`, with the provider keys it imported. Only inside run
/// directories the runner made (those carrying `run.json`), and only those last modified that long ago. `--older-than 0`
/// removes every one, including a run in progress's: do not use it while a run is going. `--dry-run` prints what would go.
fn clean_command(argv: &[String]) -> std::process::ExitCode {
    let mut days = CLEAN_DEFAULT_DAYS;
    let mut dry_run = false;
    let mut it = argv.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "--older-than" => match it.next().and_then(|v| v.parse::<u64>().ok()) {
                Some(n) => days = n,
                None => {
                    eprintln!("cauldron-bench: `--older-than` needs a number of days, like `--older-than 7`");
                    return std::process::ExitCode::FAILURE;
                }
            },
            other => {
                eprintln!("cauldron-bench: `{other}` is not an argument `clean` takes (`--older-than <days>`, `--dry-run`)");
                return std::process::ExitCode::FAILURE;
            }
        }
    }
    let state = match run::state_root() {
        Ok(state) => state,
        Err(e) => {
            eprintln!("cauldron-bench: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let older = std::time::Duration::from_secs(days * 86_400);
    let paths = run::clean_state(&state, older, dry_run);
    if paths.is_empty() {
        println!("nothing to remove under {} (nothing there is {days} day(s) old or more)", state.display());
        return std::process::ExitCode::SUCCESS;
    }
    for path in &paths {
        println!("{} {}", if dry_run { "would remove:" } else { "removed:" }, path.display());
    }
    if dry_run {
        println!("would remove {} credential director{}", paths.len(), if paths.len() == 1 { "y" } else { "ies" });
    } else {
        println!("removed {} credential director{}", paths.len(), if paths.len() == 1 { "y" } else { "ies" });
    }
    std::process::ExitCode::SUCCESS
}
