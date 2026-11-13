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

use bevy_cauldron_bench::run::{self, Agent, Options, DEFAULT_MODEL};

/// **What `--help` prints.**
const USAGE: &str = "\
cauldron-bench: run the Cauldron's benchmark suite against a real editor and a real agent.

usage: cauldron-bench run [options]

  --suite <dir>       the task directory (default: bench/tasks). A game's own is .dark_factory/bench
  --project <root>    the project the tasks run against (default: this repository's root)
  --agent <name>      omp | oracle | nothing (default: omp)
  --trials <n>        how many trials of each task (default: 1)
  --jobs <n>          how many trials run at once (default: 1)
  --model <name>      the model omp runs (default: ollama-cloud/deepseek-v4.1-flash:1017)
  --variant <name>    an arm from <suite>/variants.yaml
  --task <id>         only this task; repeatable
  --editor <path>     the editor binary under test (default: the one beside this binary)
  --server <path>     the MCP server binary beside it (default: beside this binary)
  --run <name>        the name of this run's directory under ~/.local/state/cauldron-bench
  -h, --help          print this and exit

Every trial copies the fixture project, starts the editor on its own port and display, hands an agent the
MCP config the editor itself gives a person's agent, and writes trial.json — the outcome, each check with
what it measured, the tool calls, tokens and wall time.";

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
        Some(other) => {
            eprintln!("cauldron-bench: `{other}` is not a command. The commands are: run.");
            eprintln!("Try `cauldron-bench --help`.");
            return std::process::ExitCode::FAILURE;
        }
        None => argv.clone(),
    };
    let options = match parse(&rest) {
        Ok(options) => options,
        Err(e) => {
            eprintln!("cauldron-bench: {e}");
            eprintln!("Try `cauldron-bench --help`.");
            return std::process::ExitCode::FAILURE;
        }
    };
    let records = match run::run(&options) {
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
    // **A harness error is a failed run**, whatever the checks said: the numbers are not about the agent.
    if failed == 0 && harness == 0 {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

/// **The options a command line names**, refusing an argument this build does not take.
fn parse(argv: &[String]) -> Result<Options, String> {
    let here = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .ok_or("this binary's own directory could not be found")?;
    let root = repository_root().unwrap_or_else(|| PathBuf::from("."));
    let mut options = Options {
        tasks: root.join(bevy_cauldron_bench::TASKS_DIR),
        root,
        agent: Agent::Omp,
        trials: 1,
        jobs: 1,
        model: DEFAULT_MODEL.to_owned(),
        variant: None,
        out: PathBuf::new(),
        editor: here.join("bevy_cauldron"),
        server: here.join("bevy_cauldron_mcp"),
        only: Vec::new(),
    };
    let mut run_name: Option<String> = None;
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
            "--model" => options.model = value()?,
            "--variant" => options.variant = Some(value()?),
            "--task" => options.only.push(value()?),
            "--editor" => options.editor = PathBuf::from(value()?),
            "--server" => options.server = PathBuf::from(value()?),
            "--run" => run_name = Some(value()?),
            other => return Err(format!("`{other}` is not an argument this command takes")),
        }
    }
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
    Ok(options)
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
