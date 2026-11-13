//! **Where a committed report goes, and the README that lists every one** (BM-03, #413).
//!
//! A run's report is committed as `docs/bench/runs/<date>-<label>.md` beside its `.json`, which is what
//! `cauldron-bench compare` reads. **The raw trial directories are never committed**: they hold an
//! agent's whole project and transcript and live under the state directory. `docs/bench/README.md` is
//! written from the reports on disk, newest first, so the list cannot disagree with the files.

use std::path::{Path, PathBuf};

use crate::compare::headline;
use crate::report::{curve, usd, Report};

/// **Where a project keeps the benchmark's documents**, relative to its root.
pub const BENCH_DOCS_DIR: &str = "docs/bench";

/// **Where committed reports go**, inside [`BENCH_DOCS_DIR`].
pub const RUNS_DIR: &str = "runs";

/// **The README's name.**
pub const README: &str = "README.md";

/// **Write a report's `.md` and `.json` under `<bench_dir>/runs/`**, then rewrite the README.
///
/// Answers the two paths written. A report whose stem is already on disk is replaced: one run, one
/// label, one day is one report.
pub fn publish(bench_dir: &Path, report: &Report) -> Result<(PathBuf, PathBuf), String> {
    let runs = bench_dir.join(RUNS_DIR);
    std::fs::create_dir_all(&runs).map_err(|e| format!("{}: {e}", runs.display()))?;
    let stem = report.header.stem();
    let md = runs.join(format!("{stem}.md"));
    let json = runs.join(format!("{stem}.json"));
    bevy_grimoire_core::atomic_file::write(&md, report.markdown()).map_err(|e| format!("{}: {e}", md.display()))?;
    bevy_grimoire_core::atomic_file::write(&json, report.json()).map_err(|e| format!("{}: {e}", json.display()))?;
    let readme = bench_dir.join(README);
    bevy_grimoire_core::atomic_file::write(&readme, index(bench_dir)?)
        .map_err(|e| format!("{}: {e}", readme.display()))?;
    Ok((md, json))
}

/// **Every committed report under `<bench_dir>/runs/`**, newest first (by date, then label).
pub fn reports(bench_dir: &Path) -> Result<Vec<(String, Report)>, String> {
    let runs = bench_dir.join(RUNS_DIR);
    let Ok(entries) = std::fs::read_dir(&runs) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_owned();
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let report = Report::from_json(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        out.push((stem, report));
    }
    out.sort_by(|a, b| b.1.header.date.cmp(&a.1.header.date).then(a.0.cmp(&b.0)));
    Ok(out)
}

/// **The README's text**: what the directory is, and every run with its headline numbers.
pub fn index(bench_dir: &Path) -> Result<String, String> {
    let mut out = String::from(
        "# The benchmark's runs\n\n\
         Each row is one committed run of `cauldron-bench` (BM-03, #413): a report beside the JSON \
         `cauldron-bench compare` reads. The raw trial directories are not committed. This file is \
         written by `cauldron-bench report`, newest first, so edit the reports and not this list.\n\n\
         ```sh\n\
         cauldron-bench report <run directory> --label baseline   # writes docs/bench/runs/<date>-baseline.md\n\
         cauldron-bench compare docs/bench/runs/<a>.json docs/bench/runs/<b>.json\n\
         ```\n\n\
         | run | date | agent and model | variant | commit | dev tasks: mean success (95% interval) | mean pass^k | cost per success | harness errors | held-out total |\n\
         |---|---|---|---|---|---|---|---|---|---|\n",
    );
    for (stem, report) in reports(bench_dir)? {
        let h = &report.header;
        let commit: String = h.commit.chars().take(10).collect();
        out.push_str(&format!(
            "| [{stem}]({RUNS_DIR}/{stem}.md){} | {} | {} | {} | `{commit}` | {} | {} | {} | {} | {} |\n",
            if report.unfit.is_some() { " (unfit)" } else { "" },
            h.date,
            h.pair(),
            h.variant.as_deref().unwrap_or("none"),
            headline(&report),
            curve(&report.dev.mean_pass_hat),
            usd(report.dev.cost_per_success_usd),
            report.harness_errors,
            report
                .held_out
                .as_ref()
                .map_or_else(|| "none".to_owned(), |a| crate::report::pct(a.mean_success)),
        ));
    }
    Ok(out)
}
