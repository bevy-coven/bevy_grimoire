//! **What a run's header says about the machine and the build it ran on** (BM-03, #413).
//!
//! A report is only worth citing if a reader can run it again, so the header names the commit the
//! editor and the MCP server were built at, the SHA-256 of the guide an agent was handed, the agent and
//! its version, the model and the price it was listed at, the host and the day. Each is read from the
//! machine, never typed, and one that cannot be read says `unknown` rather than guessing.

use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::report::{Header, Price, REPORT_VERSION};
use crate::run::{Agent, Options};

/// **Where the shipped guide is**, relative to the repository the binary was built in.
pub const GUIDE_PATH: &str = "crates/bevy_cauldron_mcp/src/guide.md";

/// **What a header says when a fact could not be read.**
pub const UNKNOWN: &str = "unknown";

/// **Seconds in a day**, for turning the clock into a calendar date.
const DAY_S: u64 = 86_400;

/// **The header of a run that is about to start.**
pub fn header(options: &Options, label: &str) -> Header {
    let (agent_version, price) = match options.agent {
        Agent::Omp => (
            output("omp", &["--version"]).unwrap_or_else(|| UNKNOWN.to_owned()),
            price_of(&options.model),
        ),
        // **Claude Code's model is an alias** (`sonnet`) that no price list spells, so it has no listed price
        // and its report uses the cost the harness itself reported.
        Agent::ClaudeCode => (output("claude", &["--version"]).unwrap_or_else(|| UNKNOWN.to_owned()), None),
        Agent::Oracle | Agent::Nothing => ("scripted".to_owned(), None),
    };
    Header {
        version: REPORT_VERSION,
        label: label.to_owned(),
        commit: commit_of(&options.repo),
        guide_sha256: guide_sha256(&options.repo),
        agent: options.agent.name().to_owned(),
        agent_version,
        model: (options.agent == Agent::Omp).then(|| options.model.clone()),
        variant: options.variant.clone(),
        suite: options
            .tasks
            .strip_prefix(&options.root)
            .unwrap_or(&options.tasks)
            .display()
            .to_string(),
        trials: options.trials,
        host: output("hostname", &[]).unwrap_or_else(|| UNKNOWN.to_owned()),
        date: today(),
        price,
    }
}

/// **A command's first line of output**, or `None` when it did not run or printed nothing.
fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).lines().next().map(|l| l.trim().to_owned()).filter(|l| !l.is_empty())
}

/// **The commit a checkout is at**, with `+dirty` when tracked files have uncommitted changes.
pub fn commit_of(repo: &Path) -> String {
    let git = |args: &[&str]| {
        Command::new("git").arg("-C").arg(repo).args(args).output().ok().filter(|o| o.status.success())
    };
    let Some(head) = git(&["rev-parse", "HEAD"]) else {
        return UNKNOWN.to_owned();
    };
    let sha = String::from_utf8_lossy(&head.stdout).trim().to_owned();
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
        .is_some_and(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty());
    if dirty {
        format!("{sha}+dirty")
    } else {
        sha
    }
}

/// **The SHA-256 of the shipped guide** in `repo`, as hex, or `unknown`.
pub fn guide_sha256(repo: &Path) -> String {
    match std::fs::read(repo.join(GUIDE_PATH)) {
        Ok(bytes) => Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect(),
        Err(_) => UNKNOWN.to_owned(),
    }
}

/// **What `omp models find --json` lists a model at**, or `None`.
fn price_of(model: &str) -> Option<Price> {
    let last = model.rsplit('/').next().unwrap_or(model);
    let out = Command::new("omp").args(["models", "find", last, "--json"]).output().ok()?;
    Price::from_models(&String::from_utf8_lossy(&out.stdout), model)
}

/// **Today's date in UTC**, `YYYY-MM-DD`.
pub fn today() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    civil_date(now / DAY_S)
}

/// **The calendar date of a day count since 1970-01-01** (Howard Hinnant's `civil_from_days`).
pub fn civil_date(days: u64) -> String {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}")
}
