#!/usr/bin/env python3
"""A release needs a benchmark run, and a regression stops it (BM-05, #415): `bench_gate.py check`.

# Why this exists

The benchmark (`cauldron-bench`, `docs/bench/README.md`) is how this repository knows whether an agent can
do the work the editor exists for. A release that nobody measured could have made the editor worse at that
and said nothing, so the release step reads the benchmark before it builds anything heavy.

# The rule

`check` refuses the release, with a named reason, unless **a committed report is for the code being
released** and **its comparison with the previous release's report is not `worse`**.

- **The report is for this code.** Its header's commit is a full 40-hex sha (never a ref name) and is the
  commit being released, or an ancestor of it whose `crates/` tree is identical (`git rev-parse
  <commit>:crates`), so a docs-only commit needs no rerun. A `release: <version>` commit carries only
  manifests (`Cargo.toml`, any `crates/*/Cargo.toml`) and `Cargo.lock` (`tools/loop/version.py`: the editor's
  bump also moves the library crates it releases with it), and while it changes nothing but version numbers
  (a package's own `version`, a path dependency's pinned `version`, and the `version` of those same packages in
  `Cargo.lock`) it is judged as its parent, the code it bumps; one that changes anything else (a feature, a
  dependency, a checksum, an unbumped package's version) is judged as itself, which no report can match. A
  report run on an uncommitted tree (`+dirty`) is for no commit and is refused.
- **The run is the release's run**: the default agent (`omp`) at `RELEASE_TRIALS = 5` trials, on the dev
  suite and the held-out tasks, and fit to compare (`unfit` unset). Anything else is a different question.
- **It is compared like with like.** The previous report must have the same agent, model, variant and trial
  count as the release's; only the **suite** may differ (it grows between releases, and `compare --across` pairs
  the tasks both runs scored). A baseline run with `--agent nothing` would make every release read
  `better`, and a model swap can hide a regression or fake one, so either is refused, naming what differs.
- **It is compared with the previous release's report**: the newest report whose commit is the previous
  `bevy_cauldron-v*` tag's code. When no report covers that release (the first release after this rule
  landed), **the baseline report is the previous one** (BM-04: the newest `docs/bench/runs/*-baseline.json`).
  With no previous report and no baseline the release is refused, and says to commit one. A report that IS
  the baseline has nothing earlier to regress from and passes, saying so.
- **`worse` stops it.** `cauldron-bench compare --json` says `worse` for the dev suite when its paired 95%
  interval lies wholly below zero; the refusal names the tasks whose success fell. The held-out total is
  `worse` when it fell and the two runs' 95% intervals do not overlap; it is reported as two totals and
  **never by task**, because naming held-out tasks is what a held-out set exists to prevent (BM-00). A
  release report whose held-out total has no mean or no interval is refused, not passed.
  `no measurable change` and `better` let the release through.

The loop pass that is refused files a `bug` issue with the comparison attached, and the next release waits
for its fix (`docs/loop/RUNNING-A-LOOP.md`).

# Where the run happens

Not here. This reads the committed report. The run happens wherever an editor can open a window (big under
Xvfb, or a Mac) and is committed with `cauldron-bench run --label <name>`; the command a pass runs before a
release is in `docs/loop/RUNNING-A-LOOP.md`, and every refusal says it too.

# Assumed decisions

- The held-out verdict is non-overlapping intervals, since `compare` pairs only dev tasks.
- The tree compared is `crates/` alone (the issue's decision). `Cargo.lock` and `assets/` are not in it, so a
  change to only those needs no rerun: an owner follow-up if that should change.

`check` is pure over its inputs (`git`, the reports, a `compare` function), so the tests in
`test_bench_gate.py` say what each case does without a repository, a build or a benchmark run.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tools" / "loop"))

import version  # noqa: E402

#: Where committed reports live (`cauldron-bench run --label`).
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
LOCK_PATH = "Cargo.lock"
ROOT_MANIFEST_PATH = "Cargo.toml"
#: Every crate's manifest: a release moves the editor's version and the library crates it publishes with it.
CRATE_MANIFEST = re.compile(r"crates/.+/Cargo\.toml")
#: A table header that holds a dependency's own keys, as in `[dependencies.bevy_grimoire_rhai]`.
DEPENDENCY_SUBTABLE = re.compile(r"\[(?:workspace\.)?(?:target\..+\.)?(?:dev-|build-)?dependencies\.[^\]]+\]")
#: A table header whose lines are dependencies, as in `[dependencies]`, where one may be an inline table.
DEPENDENCY_TABLE = re.compile(r"\[(?:workspace\.)?(?:target\..+\.)?(?:dev-|build-)?dependencies\]")
VERSION_LINE = re.compile(r'version\s*=\s*"[^"]*"')
#: What a version line reads as once the version is taken out, so two files can be compared without it.
VERSION_PLACEHOLDER = "<release version>"
#: The marker `cauldron-bench` puts on a commit when the tree had uncommitted changes.
DIRTY_SUFFIX = "+dirty"
#: A report's commit must be a whole sha: a ref name (`HEAD`, `main`) would be resolved to whatever it is today.
FULL_SHA = re.compile(r"[0-9a-f]{40}")
#: How `cauldron-bench` is run when no built binary is named.
CARGO_BENCH = ["cargo", "run", "-q", "--release", "-p", "bevy_cauldron_bench", "--bin", "cauldron-bench", "--"]
#: The held-out tasks live outside every repository, on the share.
HELD_OUT_DIR = "/mnt/codex_fs/bench/dark_factory/held_out"
#: The three binaries a run needs side by side: the runner finds the editor and the MCP server beside itself.
BUILD_COMMAND = "cargo build --release -p bevy_cauldron -p bevy_cauldron_mcp -p bevy_cauldron_bench"
#: The only difference between two runs a comparison may cross: the suite grows between releases.
SUITE_DIFFERENCE = "the suites differ"
#: Digits of a commit shown in a sentence.
SHORT_SHA = 12
#: A fraction as percent points.
PERCENT = 100.0
#: The verdict words `Comparison::change` serializes as, with `_` read as a space.
VERDICTS = ("better", "worse", "no measurable change")


def run_howto(label: str) -> str:
    """**The sentence that says how to make the report a refusal asks for.** Every refusal ends with it."""
    return (
        f"Where an editor can open a window (big, or a Mac): `{BUILD_COMMAND}` (the editor, its MCP server and the "
        f"runner must sit side by side in target/release), then `target/release/cauldron-bench run --agent "
        f"{RELEASE_AGENT} --trials {RELEASE_TRIALS} --held-out {HELD_OUT_DIR} --label {label}`. `--label` writes "
        f"docs/bench/runs/<date>-{label}.md and .json, and those files must then be COMMITTED "
        "(docs/loop/RUNNING-A-LOOP.md)"
    )


class CompareError(Exception):
    """`cauldron-bench compare` could not make the comparison, said in its own words."""


@dataclass(frozen=True)
class Entry:
    """A committed report: its file stem (`<date>-<label>`) and the JSON it holds."""

    name: str
    report: dict

    @property
    def header(self) -> dict:
        header = self.report.get("header")
        return header if isinstance(header, dict) else {}

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


def is_bump_file(path: str) -> bool:
    """Whether a release commit may touch `path` at all: a manifest or the lock."""
    return path in (LOCK_PATH, ROOT_MANIFEST_PATH) or bool(CRATE_MANIFEST.fullmatch(path))


def without_manifest_version(text: str) -> str:
    """**A `Cargo.toml` with its version numbers taken out**: the package's own, and a path dependency's pin.

    Everything else stays byte for byte, so a changed feature, an added dependency or a registry
    dependency's requirement is still a difference. A path dependency is a table with a `path` key
    (`[dependencies.x]` with `path = ...`) or an inline `x = { path = ..., version = ... }` line.
    """
    groups: list[tuple[str, list[str]]] = [("", [])]
    for line in text.splitlines():
        if line.strip().startswith("["):
            groups.append((line.strip(), []))
        groups[-1][1].append(line)
    out = []
    for header, body in groups:
        has_path = any(re.match(r"\s*path\s*=", line) for line in body)
        out.extend(_mask_line(line, header, has_path) for line in body)
    return "\n".join(out)


def _mask_line(line: str, section: str, has_path: bool) -> str:
    """One manifest line with its version number taken out, when it is a version this check allows to move."""
    stripped = line.strip()
    if not section.startswith("[") or not stripped or stripped.startswith("#"):
        return line
    if section in ("[package]", "[workspace.package]"):
        return VERSION_LINE.sub(f'version = "{VERSION_PLACEHOLDER}"', line) if VERSION_LINE.fullmatch(stripped) else line
    if DEPENDENCY_SUBTABLE.fullmatch(section):
        return VERSION_LINE.sub(f'version = "{VERSION_PLACEHOLDER}"', line) if has_path and VERSION_LINE.fullmatch(stripped) else line
    if DEPENDENCY_TABLE.fullmatch(section) and re.search(r'=\s*\{[^}]*\bpath\s*=', stripped):
        return VERSION_LINE.sub(f'version = "{VERSION_PLACEHOLDER}"', line)
    return line


def package_identity(text: str) -> tuple[str | None, str | None]:
    """A manifest's `[package]` name and version, either `None` when it has none (a virtual workspace)."""
    name = version_text = None
    in_package = False
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("["):
            in_package = stripped == "[package]"
        elif in_package:
            if match := re.fullmatch(r'name\s*=\s*"([^"]*)"', stripped):
                name = match.group(1)
            elif match := re.fullmatch(r'version\s*=\s*"([^"]*)"', stripped):
                version_text = match.group(1)
    return name, version_text


def without_lock_versions(text: str, names: set[str]) -> str:
    """A `Cargo.lock` with the version of the named packages taken out and every other byte left."""
    blocks = text.split("[[package]]")
    for i, block in enumerate(blocks):
        match = re.search(r'^name = "([^"]*)"$', block, re.MULTILINE)
        if match and match.group(1) in names:
            blocks[i] = re.sub(r'^version = "[^"]*"$', f'version = "{VERSION_PLACEHOLDER}"', block, count=1, flags=re.MULTILINE)
    return "[[package]]".join(blocks)


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

    def _only_versions_changed(self, sha: str, changed: list[str]) -> bool:
        """**Whether `sha` differs from its parent by version numbers and nothing else.**

        Each changed manifest must be equal to its parent's once its package version and its path
        dependencies' pins are masked; `Cargo.lock` must be equal once the packages whose manifests were bumped
        have their version masked. A file added or removed is not a bump.
        """
        bumped: set[str] = set()
        for path in changed:
            if path == LOCK_PATH:
                continue
            before, after = self._out("show", f"{sha}^:{path}"), self._out("show", f"{sha}:{path}")
            if before is None or after is None or without_manifest_version(before) != without_manifest_version(after):
                return False
            (name, old), (_, new) = package_identity(before), package_identity(after)
            if name and old != new:
                bumped.add(name)
        if LOCK_PATH in changed:
            before, after = self._out("show", f"{sha}^:{LOCK_PATH}"), self._out("show", f"{sha}:{LOCK_PATH}")
            if before is None or after is None:
                return False
            return without_lock_versions(before, bumped) == without_lock_versions(after, bumped)
        return True

    def code_commit(self, sha: str) -> tuple[str | None, str | None]:
        """**The commit whose code `sha` releases**: itself, or for a `release: <v>` bump its parent.

        A bump changes `crates/bevy_cauldron/Cargo.toml`, so its own `crates/` tree can never equal that of
        the run it follows. It is a bump only while it carries manifests and `Cargo.lock` and nothing else
        (the editor's bump also moves the library crates released with it), and only version numbers in them
        (`_only_versions_changed`): a commit that carries more files is refused, and one that also changes a
        dependency, a feature or a checksum is judged as itself, because its code was not what ran.
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
            extra = sorted(path for path in changed.split() if not is_bump_file(path))
            if extra:
                return None, (
                    f"the release commit {sha[:SHORT_SHA]} carries more than the version bump "
                    f"({', '.join(extra)}): a bump is its own commit, so the code it releases is not the code that was run"
                )
            if not self._only_versions_changed(sha, changed.split()):
                return sha, None
            sha = parent


def why_not(git, code: str, commit: str) -> str | None:
    """**Why a report run at `commit` is not for the code at `code`**, or `None` when it is."""
    if commit in ("", "unknown"):
        return "its header names no commit"
    if commit.endswith(DIRTY_SUFFIX):
        return f"it was run on an uncommitted tree ({commit[:SHORT_SHA]}{DIRTY_SUFFIX}), which is no commit"
    if not FULL_SHA.fullmatch(commit):
        return f"its commit `{commit}` is not a full 40-hex commit sha, and a name like that means whatever it points at today"
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


def unlike(previous: Entry, current: Entry) -> list[str]:
    """**What differs between two runs besides the suite**: agent, model, variant, trials. Empty is like with like."""
    found = []
    for key, word in (("agent", "agent"), ("model", "model"), ("variant", "suite variant"), ("trials", "trials per task")):
        a, b = previous.header.get(key), current.header.get(key)
        if a != b:
            found.append(f"the {word} differs ({previous.name}: `{a}`, {current.name}: `{b}`)")
    return found


def percent(value: float | None) -> str:
    return "unknown" if value is None else f"{value * PERCENT:.1f}%"


def interval_text(interval: dict | None) -> str:
    if not isinstance(interval, dict) or "lo" not in interval or "hi" not in interval:
        return "no interval"
    return f"[{interval['lo'] * PERCENT:+.1f}, {interval['hi'] * PERCENT:+.1f}]"


def interval_known(interval) -> bool:
    return isinstance(interval, dict) and all(isinstance(interval.get(k), (int, float)) for k in ("lo", "hi"))


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


def held_out_unreadable(entry: Entry) -> str | None:
    """A refusal when a release report's held-out total has no mean or no interval: it measured nothing to judge."""
    total = entry.report.get("held_out")
    if isinstance(total, dict) and total.get("mean_success") is not None and interval_known(total.get("interval")):
        return None
    return (
        f"{entry.name}'s held-out total has no mean or no interval, so it cannot be judged: the run scored no "
        f"held-out task (is the held-out directory {HELD_OUT_DIR} there?)"
    )


def held_out_verdict(previous: Entry, current: Entry) -> tuple[str | None, str | None]:
    """**The held-out total's verdict as `(refusal, note)`**.

    The held-out set is two totals and never a list of tasks, so the sentence is counts only. The release's
    own total with no mean or no interval is a refusal (the run measured nothing it can be judged on); the
    previous report's having none is a note, since there is then nothing to regress from.
    """
    a, b = previous.report.get("held_out"), current.report.get("held_out")
    if refusal := held_out_unreadable(current):
        return refusal, None
    if not isinstance(a, dict):
        return None, f"held-out: {previous.name} has no held-out total, so there is nothing to compare it with"
    if a.get("mean_success") is None or not interval_known(a.get("interval")):
        return None, f"held-out: {previous.name}'s total has no mean or no interval, so there is nothing to compare it with"
    ia, ib = a["interval"], b["interval"]
    if b["mean_success"] < a["mean_success"] and ib["hi"] < ia["lo"]:
        return (
            f"the held-out total got worse: {percent(a['mean_success'])} in {previous.name} "
            f"(95% interval {percent(ia['lo'])} to {percent(ia['hi'])}) against {percent(b['mean_success'])} in "
            f"{current.name} ({percent(ib['lo'])} to {percent(ib['hi'])}), and the intervals do not overlap. "
            "Held-out tasks are never named, so look at the dev tasks that fell and at the editor change",
            None,
        )
    return None, None


def judge(previous: Entry, chosen: Entry, comparison: dict, notes: list[str]) -> list[str]:
    """**The reasons a comparison of `previous` with `chosen` stops the release**, and notes for what passed."""
    words = str(comparison.get("change", "")).replace("_", " ")
    if words not in VERDICTS:
        return [f"the comparison's verdict `{words}` is not one this check knows"]
    difference = comparison.get("difference", 0.0) * PERCENT
    notes.append(
        f"dev: {words} over {comparison.get('tasks_paired')} task(s), {difference:+.1f} points, "
        f"interval {interval_text(comparison.get('interval'))}"
    )
    across = [str(a) for a in comparison.get("across", [])]
    other = [a for a in across if not a.startswith(SUITE_DIFFERENCE)]
    if other:
        return [f"{chosen.name} and {previous.name} are not like with like: {'; '.join(other)}. Only the suite may differ"]
    if across:
        notes.append("compared across: " + "; ".join(across))

    reasons = []
    if words == "worse":
        fell = fallen(comparison)
        named = "tasks that fell: " + "; ".join(fell) if fell else "no single task's success fell by count, so the loss is spread thin"
        reasons.append(
            f"the dev suite got worse against {previous.name}: {difference:+.1f} points, "
            f"interval {interval_text(comparison.get('interval'))} wholly below zero; {named}"
        )
    refusal, note = held_out_verdict(previous, chosen)
    if refusal:
        reasons.append(refusal)
    if note:
        notes.append(note)
    return reasons


def check(git, release: str, previous_code: str | None, entries: list[Entry], compare, skipped: list[str] | None = None) -> Verdict:
    """**Whether the release at `release` may go**, given every committed report.

    `release` is the code commit (`Git.code_commit` of what is being tagged); `previous_code` is the same for
    the previous release, or `None`. `compare(a_name, b_name)` is `cauldron-bench compare --json` over two
    reports and raises [`CompareError`]. `skipped` says which files under docs/bench/runs/ did not read; it is
    printed whether or not the release goes, because a report that is silently ignored may be the newest.
    """
    skipped = skipped or []
    notes = [f"report not read, and ignored: {line}" for line in skipped]
    if not entries:
        return Verdict(False, [
            f"no benchmark report is committed under docs/bench/runs/ ({'; '.join(skipped) if skipped else 'the directory is empty'}). "
            + run_howto("baseline") + ". The first run on this repository is `--label baseline`; later releases use "
            "`--label release-<version>`"
        ], notes)
    refusals = {e.name: why_not(git, release, e.commit) for e in entries}
    covering = [e for e in entries if refusals[e.name] is None]
    if not covering:
        why = "; ".join(f"{name}: {reason}" for name, reason in sorted(refusals.items()))
        return Verdict(False, [
            f"no committed benchmark report is for the code being released ({release[:SHORT_SHA]}): {why}. "
            "Rerun the benchmark on this code. " + run_howto("release-<version>")
        ], notes)
    chosen = newest(covering)
    notes.extend(
        f"a newer report is ignored: {e.name}: {refusals[e.name]}"
        for e in sorted(entries, key=lambda e: e.sort_key)
        if refusals[e.name] and e.sort_key > chosen.sort_key
    )
    problems = run_problems(chosen)
    if problems:
        return Verdict(False, [*problems, run_howto("release-<version>")], notes)

    others = [e for e in entries if e is not chosen]
    prior = [e for e in others if previous_code and why_not(git, previous_code, e.commit) is None]
    baselines = [e for e in others if e.label == BASELINE_LABEL]
    if prior:
        previous, what = newest(prior), "the previous release's report"
    elif baselines:
        previous, what = newest(baselines), "the baseline report (no report is for the previous release)"
    elif chosen.label == BASELINE_LABEL:
        if refusal := held_out_unreadable(chosen):
            return Verdict(False, [refusal], notes)
        return Verdict(True, notes=[*notes, f"{chosen.name} is the baseline: nothing earlier to compare it with"])
    else:
        return Verdict(False, [
            f"there is no previous report to compare {chosen.name} with: no report is for the previous release and "
            f"none is labelled `{BASELINE_LABEL}`. A baseline is the first run, kept as the reference every release is "
            f"compared with: make one on the earliest code worth measuring, or label this release's own run "
            f"`{BASELINE_LABEL}` if it is the first. " + run_howto(BASELINE_LABEL)
        ], notes)
    notes.append(f"{chosen.name} compared with {what}: {previous.name}")

    differ = unlike(previous, chosen)
    if differ:
        return Verdict(False, [
            f"{previous.name} and {chosen.name} are not like with like: {'; '.join(differ)}. Only the suite may "
            "differ between two runs a release compares: rerun the older side with the release's agent, model and "
            f"trials, or commit a new `{BASELINE_LABEL}` made that way"
        ], notes)

    try:
        comparison = compare(previous.name, chosen.name)
    except CompareError as error:
        return Verdict(False, [f"{chosen.name} cannot be compared with {previous.name}: {error}"], notes)
    try:
        reasons = judge(previous, chosen, comparison, notes)
    except (AttributeError, KeyError, TypeError, ValueError) as error:
        return Verdict(False, [f"the comparison of {chosen.name} with {previous.name} is not one this check can read ({error!r})"], notes)
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
    """Every `*.json` report under `runs`, and a line for each file that did not read as a report."""
    entries, skipped = [], []
    for path in sorted(runs.glob("*.json")):
        try:
            data = json.loads(path.read_text())
        except (OSError, ValueError) as error:
            skipped.append(f"{path.name} did not read: {error}")
            continue
        if not isinstance(data, dict) or not isinstance(data.get("header"), dict):
            skipped.append(f"{path.name} is not a report: it has no `header` object")
            continue
        if any(data.get(key) is not None and not isinstance(data[key], dict) for key in ("dev", "held_out")):
            skipped.append(f"{path.name} is not a report: its `dev` or `held_out` is not an object")
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


def main(argv: list[str] | None = None, git: Git | None = None) -> int:
    parser = argparse.ArgumentParser(description="A release needs a benchmark run, and a regression stops it.")
    sub = parser.add_subparsers(dest="command", required=True)
    one = sub.add_parser("check", help="refuse a release with no benchmark report for its code, or a worse one")
    one.add_argument("--release", default="HEAD", help="the commit being released (default HEAD)")
    one.add_argument("--version", default=None, help="the version being released (default: the manifest's)")
    one.add_argument("--runs", type=Path, default=RUNS_DIR, help="the directory of committed reports")
    one.add_argument("--bench", default=None, help="a built cauldron-bench binary (default: cargo run --release)")
    args = parser.parse_args(argv)

    git = git or Git()
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
