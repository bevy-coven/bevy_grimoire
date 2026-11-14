#!/usr/bin/env python3
"""A release needs a benchmark run, and a regression stops it (BM-05, #415): `bench_gate.py check`.

# Why this exists

The benchmark (`cauldron-bench`, `docs/bench/README.md`) is how this repository knows whether an agent can
do the work the editor exists for. A release that nobody measured could have made the editor worse at that
and said nothing, so the release step reads the benchmark before it builds anything.

# The rule

`check` refuses the release, with a named reason, unless **a committed report is for the code being
released** and **its comparison with the previous release's report is not `worse`**.

- **The report is for this code.** Its header's commit is the commit being released, or an ancestor of it
  whose `crates/` tree is identical (`git rev-parse <commit>:crates`), so a docs-only commit needs no rerun.
  A `release: <version>` commit carries only the manifest and `Cargo.lock` (`tools/loop/version.py`), so it
  is judged as its parent, the code it bumps. A report run on an uncommitted tree (`+dirty`) is for no
  commit and is refused.
- **The run is the release's run**: the default agent (`omp`) at `RELEASE_TRIALS = 5` trials, on the dev
  suite and the held-out tasks, and fit to compare (`unfit` unset). Anything else is a different question.
- **It is compared with the previous release's report**: the newest report whose commit is the previous
  `bevy_cauldron-v*` tag's code. When no report covers that release (the first release after this rule
  landed), **the baseline report is the previous one** (BM-04: the newest `docs/bench/runs/*-baseline.json`).
  With no previous report and no baseline the release is refused, and says to commit one. A report that IS
  the baseline has nothing earlier to regress from and passes, saying so.
- **`worse` stops it.** `cauldron-bench compare --json` says `worse` for the dev suite when its paired 95%
  interval lies wholly below zero; the refusal names the tasks whose success fell. The held-out total is
  `worse` when it fell and the two runs' 95% intervals do not overlap; it is reported as two totals and
  **never by task**, because naming held-out tasks is what a held-out set exists to prevent (BM-00).
  `no measurable change` and `better` let the release through.

The loop pass that is refused files a `bug` issue with the comparison attached, and the next release waits
for its fix (`docs/loop/RUNNING-A-LOOP.md`).

# Where the run happens

Not here. This reads the committed report. The run happens wherever an editor can open a window (big under
Xvfb, or a Mac) and is committed with `cauldron-bench report <run> --label <name>`; the command a pass runs
before a release is in `docs/loop/RUNNING-A-LOOP.md`.

# Assumed decisions

- Comparison runs with `--across`: the suite grows between releases (the baseline has fewer tasks than a
  later suite) and compare pairs the tasks both runs scored, so a refusal for "the suites differ" would stop
  every release. What differed is printed.
- The held-out verdict is non-overlapping intervals, since `compare` pairs only dev tasks.

`check` is pure over its inputs (`git`, the reports, a `compare` function), so the tests in
`test_bench_gate.py` say what each case does without a repository, a build or a benchmark run.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tools" / "loop"))

import version  # noqa: E402

#: Where committed reports live (`cauldron-bench report --label`).
RUNS_DIR = REPO / "docs" / "bench" / "runs"
#: Trials of each task a release's run makes: the number `compare` needs to see a real move.
RELEASE_TRIALS = 5
#: The agent a release is measured with: the default agent, the one a person's editor is handed to.
RELEASE_AGENT = "omp"
#: The label BM-04's first run was committed under, which the first release is compared with.
BASELINE_LABEL = "baseline"
#: `git rev-parse <commit>:<this>` names the code a report is for.
CODE_DIR = "crates"
#: A commit subject that starts a release bump, and the only files such a commit may carry.
RELEASE_SUBJECT = "release: "
BUMP_FILES = frozenset({"crates/bevy_cauldron/Cargo.toml", "Cargo.lock"})
#: The marker `cauldron-bench` puts on a commit when the tree had uncommitted changes.
DIRTY_SUFFIX = "+dirty"
#: How `cauldron-bench` is run when no built binary is named.
CARGO_BENCH = ["cargo", "run", "-q", "--release", "-p", "bevy_cauldron_bench", "--bin", "cauldron-bench", "--"]
#: Digits of a commit shown in a sentence.
SHORT_SHA = 12
#: A fraction as percent points.
PERCENT = 100.0


class CompareError(Exception):
    """`cauldron-bench compare` could not make the comparison, said in its own words."""


@dataclass(frozen=True)
class Entry:
    """A committed report: its file stem (`<date>-<label>`) and the JSON it holds."""

    name: str
    report: dict

    @property
    def header(self) -> dict:
        return self.report.get("header", {})

    @property
    def commit(self) -> str:
        return str(self.header.get("commit", ""))

    @property
    def label(self) -> str:
        return str(self.header.get("label", ""))

    @property
    def sort_key(self) -> tuple[str, bool, str]:
        """Newest last: by day, then a named run before the baseline of the same day (which is the reference, never the run), then by name."""
        return (str(self.header.get("date", "")), self.label != BASELINE_LABEL, self.name)


@dataclass
class Verdict:
    """What `check` decided: `ok`, every reason it refused, and what a person may want to read when it did not."""

    ok: bool
    reasons: list[str] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)


class Git:
    """The few things the rule asks of a repository."""

    def __init__(self, repo: Path | str = REPO):
        self.repo = str(repo)

    def _run(self, *args: str) -> subprocess.CompletedProcess:
        return subprocess.run(["git", *args], cwd=self.repo, capture_output=True, text=True)

    def _out(self, *args: str) -> str | None:
        done = self._run(*args)
        return done.stdout.strip() if done.returncode == 0 else None

    def resolve(self, rev: str) -> str | None:
        """The full sha of a commit-ish, or `None` when this repository does not have it."""
        return self._out("rev-parse", "--verify", "--quiet", f"{rev}^{{commit}}")

    def crates_tree(self, sha: str) -> str | None:
        """The tree id of `crates/` at a commit: equal ids are equal code."""
        return self._out("rev-parse", "--verify", "--quiet", f"{sha}:{CODE_DIR}")

    def is_ancestor(self, older: str, newer: str) -> bool:
        return self._run("merge-base", "--is-ancestor", older, newer).returncode == 0

    def tags(self) -> list[str]:
        return (self._out("tag", "-l", f"{version.TAG_PREFIX}*") or "").split()

    def code_commit(self, sha: str) -> tuple[str | None, str | None]:
        """**The commit whose code `sha` releases**: itself, or for a `release: <v>` bump its parent.

        A bump changes `crates/bevy_cauldron/Cargo.toml`, so its own `crates/` tree can never equal that of
        the run it follows. It is a bump only while it carries the manifest and `Cargo.lock` and nothing
        else; one that carries more is refused, because its code was not what was run.
        """
        while True:
            subject = self._out("log", "-1", "--format=%s", sha)
            if subject is None:
                return None, f"commit {sha[:SHORT_SHA]} is not in this repository's history"
            if not subject.startswith(RELEASE_SUBJECT):
                return sha, None
            parent = self.resolve(f"{sha}^")
            changed = self._out("diff", "--name-only", f"{sha}^", sha)
            if parent is None or changed is None:
                return None, f"the release commit {sha[:SHORT_SHA]} has no parent in this clone: fetch the full history"
            extra = sorted(set(changed.split()) - BUMP_FILES)
            if extra:
                return None, (
                    f"the release commit {sha[:SHORT_SHA]} carries more than the version bump "
                    f"({', '.join(extra)}): a bump is its own commit, so the code it releases is not the code that was run"
                )
            sha = parent


def why_not(git, code: str, commit: str) -> str | None:
    """**Why a report run at `commit` is not for the code at `code`**, or `None` when it is."""
    if commit in ("", "unknown"):
        return "its header names no commit"
    if commit.endswith(DIRTY_SUFFIX):
        return f"it was run on an uncommitted tree ({commit[:SHORT_SHA]}{DIRTY_SUFFIX}), which is no commit"
    sha = git.resolve(commit)
    if sha is None:
        return f"its commit {commit[:SHORT_SHA]} is not in this repository's history"
    if sha == code:
        return None
    if not git.is_ancestor(sha, code):
        return f"its commit {sha[:SHORT_SHA]} is not the release commit and not an ancestor of it ({code[:SHORT_SHA]})"
    if git.crates_tree(sha) != git.crates_tree(code):
        return (
            f"its commit {sha[:SHORT_SHA]} is an ancestor, but `{CODE_DIR}/` changed between it and the release "
            f"commit ({code[:SHORT_SHA]}): the code was not the code that ran"
        )
    return None


def newest(entries: list[Entry]) -> Entry:
    return max(entries, key=lambda e: e.sort_key)


def run_problems(entry: Entry) -> list[str]:
    """What is wrong with the report as a release's run, apart from which commit it was."""
    found = []
    header = entry.header
    if header.get("agent") != RELEASE_AGENT:
        found.append(
            f"{entry.name} was run with agent `{header.get('agent')}`, and a release is measured with the "
            f"default agent `{RELEASE_AGENT}`"
        )
    if header.get("trials") != RELEASE_TRIALS:
        found.append(f"{entry.name} made {header.get('trials')} trials of each task, and a release makes {RELEASE_TRIALS} trials")
    if entry.report.get("held_out") is None:
        found.append(f"{entry.name} has no held-out total: a release is run on the dev suite and the held-out tasks")
    if entry.report.get("unfit"):
        found.append(f"{entry.name} is unfit for comparison: {entry.report['unfit']}")
    return found


def percent(value: float | None) -> str:
    return "unknown" if value is None else f"{value * PERCENT:.1f}%"


def interval_text(interval: dict | None) -> str:
    if not interval:
        return "no interval"
    return f"[{interval['lo'] * PERCENT:+.1f}, {interval['hi'] * PERCENT:+.1f}]"


def fallen(comparison: dict) -> list[str]:
    """The dev tasks whose success rate fell, each as `task: 5/5 -> 2/5`. A task that rose is not listed."""
    out = []
    for move in comparison.get("moved", []):
        a_trials, b_trials = move.get("a_trials", 0), move.get("b_trials", 0)
        if a_trials == 0 or b_trials == 0:
            continue
        if move["b_successes"] / b_trials < move["a_successes"] / a_trials:
            out.append(f"`{move['task']}` {move['a_successes']}/{a_trials} -> {move['b_successes']}/{b_trials}")
    return out


def held_out_fell(previous: Entry, current: Entry) -> str | None:
    """**A sentence when the held-out total fell beyond what its intervals explain**, else `None`.

    The held-out set is two totals and never a list of tasks, so the sentence is counts only.
    """
    a, b = previous.report.get("held_out"), current.report.get("held_out")
    if not a or not b or a.get("mean_success") is None or b.get("mean_success") is None:
        return None
    ia, ib = a.get("interval"), b.get("interval")
    if not ia or not ib:
        return None
    if b["mean_success"] < a["mean_success"] and ib["hi"] < ia["lo"]:
        return (
            f"the held-out total got worse: {percent(a['mean_success'])} in {previous.name} "
            f"(95% interval {percent(ia['lo'])} to {percent(ia['hi'])}) against {percent(b['mean_success'])} in "
            f"{current.name} ({percent(ib['lo'])} to {percent(ib['hi'])}), and the intervals do not overlap. "
            "Held-out tasks are never named, so look at the dev tasks that fell and at the editor change"
        )
    return None


def check(git, release: str, previous_code: str | None, entries: list[Entry], compare, skipped: list[str] | None = None) -> Verdict:
    """**Whether the release at `release` may go**, given every committed report.

    `release` is the code commit (`Git.code_commit` of what is being tagged); `previous_code` is the same for
    the previous release, or `None`. `compare(a_name, b_name)` is `cauldron-bench compare --json` over two
    reports and raises [`CompareError`].
    """
    skipped = skipped or []
    if not entries:
        return Verdict(False, [
            f"no benchmark report is committed under docs/bench/runs/ ({'; '.join(skipped) if skipped else 'the directory is empty'}). "
            f"Run `cauldron-bench run --trials {RELEASE_TRIALS} --held-out <held-out dir> --label <name>` "
            "where an editor can open a window and commit the report (docs/loop/RUNNING-A-LOOP.md)"
        ])
    refusals = {e.name: why_not(git, release, e.commit) for e in entries}
    covering = [e for e in entries if refusals[e.name] is None]
    if not covering:
        why = "; ".join(f"{name}: {reason}" for name, reason in sorted(refusals.items()))
        return Verdict(False, [
            f"no committed benchmark report is for the code being released ({release[:SHORT_SHA]}): {why}. "
            "Rerun the benchmark on this code and commit the report (docs/loop/RUNNING-A-LOOP.md)"
        ])
    chosen = newest(covering)
    problems = run_problems(chosen)
    if problems:
        return Verdict(False, problems)

    others = [e for e in entries if e is not chosen]
    prior = [e for e in others if previous_code and why_not(git, previous_code, e.commit) is None]
    baselines = [e for e in others if e.label == BASELINE_LABEL]
    notes = []
    if prior:
        previous, what = newest(prior), "the previous release's report"
    elif baselines:
        previous, what = newest(baselines), "the baseline report (no report is for the previous release)"
    elif chosen.label == BASELINE_LABEL:
        return Verdict(True, notes=[f"{chosen.name} is the baseline: nothing earlier to compare it with"])
    else:
        return Verdict(False, [
            f"there is no previous report to compare {chosen.name} with: no report is for the previous release and "
            f"none is labelled `{BASELINE_LABEL}`. Commit BM-04's baseline "
            f"(`cauldron-bench report <run directory> --label {BASELINE_LABEL}`)"
        ])
    notes.append(f"{chosen.name} compared with {what}: {previous.name}")

    try:
        comparison = compare(previous.name, chosen.name)
    except CompareError as error:
        return Verdict(False, [f"{chosen.name} cannot be compared with {previous.name}: {error}"], notes)
    words = str(comparison.get("change", "")).replace("_", " ")
    if words not in ("better", "worse", "no measurable change"):
        return Verdict(False, [f"the comparison's verdict `{words}` is not one this check knows"], notes)
    notes.append(
        f"dev: {words} over {comparison.get('tasks_paired')} task(s), {comparison.get('difference', 0.0) * PERCENT:+.1f} "
        f"points, interval {interval_text(comparison.get('interval'))}"
    )
    if comparison.get("across"):
        notes.append("compared across: " + "; ".join(comparison["across"]))

    reasons = []
    if words == "worse":
        fell = fallen(comparison)
        named = "tasks that fell: " + "; ".join(fell) if fell else "no single task's success fell by count, so the loss is spread thin"
        reasons.append(
            f"the dev suite got worse against {previous.name}: {comparison.get('difference', 0.0) * PERCENT:+.1f} points, "
            f"interval {interval_text(comparison.get('interval'))} wholly below zero; {named}"
        )
    held = held_out_fell(previous, chosen)
    if held:
        reasons.append(held)
    elif not previous.report.get("held_out"):
        notes.append(f"held-out: {previous.name} has no held-out total, so there is nothing to compare it with")
    return Verdict(not reasons, reasons, notes)


def previous_tag(tags: list[str], current: str) -> str | None:
    """The tag of the greatest version strictly below `current`, or `None`. A tag that is no plain X.Y.Z is skipped."""
    below = (-1, -1, -1)
    best = None
    now = version.parse(current)
    for tag in tags:
        try:
            parsed = version.parse(tag[len(version.TAG_PREFIX):])
        except version.Refusal:
            continue
        if parsed < now and parsed > below:
            below, best = parsed, tag
    return best


def load_entries(runs: Path) -> tuple[list[Entry], list[str]]:
    """Every `*.json` report under `runs`, and a line for each file that did not read."""
    entries, skipped = [], []
    for path in sorted(runs.glob("*.json")):
        try:
            data = json.loads(path.read_text())
            if not isinstance(data, dict) or "header" not in data:
                raise ValueError("not a report")
        except (OSError, ValueError) as error:
            skipped.append(f"{path.name} did not read: {error}")
            continue
        entries.append(Entry(path.stem, data))
    return entries, skipped


def bench_compare(command: list[str], runs: Path):
    """`compare` as the real `cauldron-bench compare --across --json`, over two reports by stem."""

    def compare(a: str, b: str) -> dict:
        try:
            done = subprocess.run(
                [*command, "compare", str(runs / f"{a}.json"), str(runs / f"{b}.json"), "--across", "--json"],
                capture_output=True, text=True, cwd=REPO,
            )
        except OSError as error:
            raise CompareError(f"`{command[0]}` did not start: {error}") from error
        if done.returncode != 0:
            raise CompareError(done.stderr.strip() or f"`cauldron-bench compare` exited {done.returncode}")
        try:
            return json.loads(done.stdout)
        except ValueError as error:
            raise CompareError(f"`cauldron-bench compare --json` printed no JSON: {error}") from error

    return compare


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="A release needs a benchmark run, and a regression stops it.")
    sub = parser.add_subparsers(dest="command", required=True)
    one = sub.add_parser("check", help="refuse a release with no benchmark report for its code, or a worse one")
    one.add_argument("--release", default="HEAD", help="the commit being released (default HEAD)")
    one.add_argument("--version", default=None, help="the version being released (default: the manifest's)")
    one.add_argument("--runs", type=Path, default=RUNS_DIR, help="the directory of committed reports")
    one.add_argument("--bench", default=None, help="a built cauldron-bench binary (default: cargo run --release)")
    args = parser.parse_args(argv)

    git = Git()
    tag_version = args.version or version.manifest_version()
    resolved = git.resolve(args.release)
    release, problem = git.code_commit(resolved) if resolved else (None, f"`{args.release}` is not a commit here")
    if release is None:
        print(f"bench gate: refused: {problem}", file=sys.stderr)
        return 1
    previous_code = None
    tag = previous_tag(git.tags(), tag_version)
    if tag:
        sha = git.resolve(tag)
        previous_code = git.code_commit(sha)[0] if sha else None
    entries, skipped = load_entries(args.runs)
    command = [args.bench] if args.bench else CARGO_BENCH
    verdict = check(git, release, previous_code, entries, bench_compare(command, args.runs), skipped)
    for note in verdict.notes:
        print(f"bench gate: {note}")
    if not verdict.ok:
        for reason in verdict.reasons:
            print(f"bench gate: refused: {reason}", file=sys.stderr)
        return 1
    print(f"bench gate: passed for {tag_version}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
