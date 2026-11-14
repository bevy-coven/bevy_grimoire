#!/usr/bin/env python3
"""Tests for the release's benchmark rule (BM-05, #415): `tools/release/bench_gate.py`.

Run with `python3 -m unittest tools/release/test_bench_gate.py`. The first four tests are the issue's acceptance
cases: no report is refused, a report for an unrelated commit is refused, a report whose comparison is `worse`
is refused (naming the tasks that fell), and a `no measurable change` report passes. The rest pin the rules
around them: a docs-only commit needs no rerun, the run must be the default agent at five trials on dev and
held-out, the first release compares with the baseline, and a held-out fall is a count and never a name.

The comparison is not computed here (that is `cauldron-bench compare`, in Rust, with its own tests): `compare`
is handed in, so each test says what the verdict was and the gate is judged on what it does with it.
"""

import os
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(__file__))

import bench_gate  # noqa: E402

RELEASE = "a" * 40
CODE_TREE = "tree-of-the-code"


def report(commit=RELEASE, label="run", date="2026-11-10", agent="omp", trials=5, held_out=(0.6, 0.5, 0.7), unfit=None):
    """A report as `cauldron-bench report` writes it, with only the fields the gate reads."""
    ho = None
    if held_out is not None:
        mean, lo, hi = held_out
        ho = {"name": "held-out", "tasks": 4, "mean_success": mean, "interval": {"lo": lo, "hi": hi}}
    return {
        "header": {
            "version": 1, "label": label, "commit": commit, "agent": agent, "model": "ollama-cloud/m",
            "suite": "bench/tasks", "trials": trials, "date": date,
        },
        "unfit": unfit,
        "dev": {"name": "dev", "tasks": 20, "mean_success": 0.7, "interval": {"lo": 0.6, "hi": 0.8}},
        "held_out": ho,
        "tasks": [],
    }


def comparison(change, moved=()):
    """A comparison as `cauldron-bench compare --json` prints it, with only the fields the gate reads."""
    return {
        "change": change, "across": [], "tasks_paired": 20, "difference": 0.0,
        "interval": {"lo": -0.1, "hi": 0.1},
        "moved": [
            {"task": t, "a_successes": a, "a_trials": 5, "b_successes": b, "b_trials": 5}
            for t, a, b in moved
        ],
    }


class FakeGit:
    """Commits as names: each has a `crates/` tree id, and `ancestors` says which one is behind which."""

    def __init__(self, trees, ancestors=()):
        self.trees = trees
        self.ancestors = set(ancestors)

    def resolve(self, rev):
        return rev if rev in self.trees else None

    def crates_tree(self, sha):
        return self.trees.get(sha)

    def is_ancestor(self, older, newer):
        return older == newer or (older, newer) in self.ancestors


class Compare:
    """A scripted `compare`: what it was asked, and the one answer it gives."""

    def __init__(self, answer=None, error=None):
        self.answer = answer
        self.error = error
        self.asked = []

    def __call__(self, a, b):
        self.asked.append((a, b))
        if self.error:
            raise bench_gate.CompareError(self.error)
        return self.answer


def entry(name, **kwargs):
    return bench_gate.Entry(name, report(**kwargs))


GIT = FakeGit({RELEASE: CODE_TREE})


class TheAcceptanceCases(unittest.TestCase):
    def test_no_report_is_refused(self):
        verdict = bench_gate.check(GIT, RELEASE, None, [], Compare(comparison("better")))
        self.assertFalse(verdict.ok)
        self.assertIn("no benchmark report", " ".join(verdict.reasons))
        self.assertIn("cauldron-bench", " ".join(verdict.reasons))

    def test_a_report_for_an_unrelated_commit_is_refused(self):
        other = "b" * 40
        git = FakeGit({RELEASE: CODE_TREE, other: "some-other-tree"}, ancestors=[(other, RELEASE)])
        verdict = bench_gate.check(
            git, RELEASE, None, [entry("2026-11-10-run", commit=other)], Compare(comparison("better"))
        )
        self.assertFalse(verdict.ok)
        text = " ".join(verdict.reasons)
        self.assertIn("2026-11-10-run", text)
        self.assertIn(other[:12], text)
        self.assertIn("crates/", text)

    def test_a_worse_report_is_refused_and_names_the_tasks_that_fell(self):
        entries = [entry("2026-11-01-baseline", commit=RELEASE, label="baseline"), entry("2026-11-10-run")]
        moved = [("place-a-bed", 5, 2), ("paint-a-lake", 1, 4), ("raise-a-wall", 5, 3)]
        verdict = bench_gate.check(GIT, RELEASE, None, entries, Compare(comparison("worse", moved)))
        self.assertFalse(verdict.ok)
        text = " ".join(verdict.reasons)
        self.assertIn("worse", text)
        self.assertIn("place-a-bed", text)
        self.assertIn("raise-a-wall", text)
        self.assertNotIn("paint-a-lake", text, "a task that rose did not fall")

    def test_no_measurable_change_passes(self):
        entries = [entry("2026-11-01-baseline", label="baseline", date="2026-11-01"), entry("2026-11-10-run")]
        verdict = bench_gate.check(GIT, RELEASE, None, entries, Compare(comparison("no_measurable_change")))
        self.assertTrue(verdict.ok, verdict.reasons)
        self.assertEqual(verdict.reasons, [])


class WhichReportCountsAsTheReleases(unittest.TestCase):
    def test_a_docs_only_commit_needs_no_rerun(self):
        """The report's commit is behind the release, and `crates/` is the same tree: it is the release's."""
        behind = "c" * 40
        git = FakeGit({RELEASE: CODE_TREE, behind: CODE_TREE}, ancestors=[(behind, RELEASE)])
        entries = [entry("2026-11-01-baseline", label="baseline", commit=behind), entry("2026-11-10-run", commit=behind)]
        verdict = bench_gate.check(git, RELEASE, None, entries, Compare(comparison("better")))
        self.assertTrue(verdict.ok, verdict.reasons)

    def test_the_same_tree_that_is_not_behind_the_release_is_refused(self):
        """A commit on another line with equal bytes is not what was released: ancestry is part of the rule."""
        side = "d" * 40
        git = FakeGit({RELEASE: CODE_TREE, side: CODE_TREE})
        verdict = bench_gate.check(git, RELEASE, None, [entry("2026-11-10-run", commit=side)], Compare(comparison("better")))
        self.assertFalse(verdict.ok)
        self.assertIn("ancestor", " ".join(verdict.reasons))

    def test_a_dirty_or_unknown_commit_is_refused(self):
        for commit in (RELEASE + "+dirty", "unknown"):
            verdict = bench_gate.check(GIT, RELEASE, None, [entry("2026-11-10-run", commit=commit)], Compare(comparison("better")))
            self.assertFalse(verdict.ok, commit)
            self.assertIn("2026-11-10-run", " ".join(verdict.reasons))

    def test_the_newest_report_of_the_code_is_the_one_judged(self):
        old = entry("2026-11-09-run", date="2026-11-09")
        new = entry("2026-11-10-run", date="2026-11-10")
        base = entry("2026-11-01-baseline", label="baseline", date="2026-11-01")
        compare = Compare(comparison("better"))
        bench_gate.check(GIT, RELEASE, None, [old, base, new], compare)
        self.assertEqual(compare.asked, [("2026-11-01-baseline", "2026-11-10-run")])


    def test_on_the_baseline_s_own_day_the_run_is_still_the_one_judged(self):
        """`2026-11-10-baseline` sorts after `2026-11-10-bad` by name; the baseline is the reference all the same."""
        base = entry("2026-11-10-baseline", label="baseline")
        run = entry("2026-11-10-bad", label="bad")
        compare = Compare(comparison("worse", [("place-a-bed", 5, 1)]))
        verdict = bench_gate.check(GIT, RELEASE, None, [base, run], compare)
        self.assertFalse(verdict.ok)
        self.assertEqual(compare.asked, [("2026-11-10-baseline", "2026-11-10-bad")])


class WhatTheRunMustBe(unittest.TestCase):
    def refused(self, **kwargs):
        entries = [entry("2026-11-01-baseline", label="baseline", date="2026-11-01"), entry("2026-11-10-run", **kwargs)]
        verdict = bench_gate.check(GIT, RELEASE, None, entries, Compare(comparison("better")))
        self.assertFalse(verdict.ok)
        return " ".join(verdict.reasons)

    def test_the_default_agent_at_five_trials(self):
        self.assertIn("5 trials", self.refused(trials=1))
        self.assertIn("omp", self.refused(agent="oracle"))

    def test_held_out_is_part_of_the_run(self):
        self.assertIn("held-out", self.refused(held_out=None))

    def test_an_unfit_run_is_refused(self):
        self.assertIn("unfit", self.refused(unfit="3 of 20 trials were harness errors"))


class WhatItIsComparedWith(unittest.TestCase):
    def test_the_previous_release_s_report_is_the_one_compared_with(self):
        previous = "e" * 40
        git = FakeGit({RELEASE: CODE_TREE, previous: "older-tree"}, ancestors=[(previous, RELEASE)])
        entries = [
            entry("2026-10-20-baseline", label="baseline", commit=previous, date="2026-10-20"),
            entry("2026-10-25-release", commit=previous, date="2026-10-25"),
            entry("2026-11-10-run"),
        ]
        compare = Compare(comparison("better"))
        verdict = bench_gate.check(git, RELEASE, previous, entries, compare)
        self.assertTrue(verdict.ok, verdict.reasons)
        self.assertEqual(compare.asked, [("2026-10-25-release", "2026-11-10-run")])

    def test_the_first_release_compares_with_the_baseline(self):
        previous = "e" * 40
        git = FakeGit({RELEASE: CODE_TREE, previous: "older-tree"}, ancestors=[(previous, RELEASE)])
        entries = [entry("2026-10-20-baseline", label="baseline", commit="f" * 40, date="2026-10-20"), entry("2026-11-10-run")]
        compare = Compare(comparison("no_measurable_change"))
        verdict = bench_gate.check(git, RELEASE, previous, entries, compare)
        self.assertTrue(verdict.ok, verdict.reasons)
        self.assertEqual(compare.asked, [("2026-10-20-baseline", "2026-11-10-run")])

    def test_with_nothing_to_compare_with_it_says_so_and_refuses(self):
        verdict = bench_gate.check(GIT, RELEASE, None, [entry("2026-11-10-run")], Compare(comparison("better")))
        self.assertFalse(verdict.ok)
        self.assertIn("baseline", " ".join(verdict.reasons))

    def test_the_baseline_itself_has_nothing_to_regress_from(self):
        verdict = bench_gate.check(
            GIT, RELEASE, None, [entry("2026-11-10-baseline", label="baseline")], Compare(comparison("worse"))
        )
        self.assertTrue(verdict.ok, verdict.reasons)
        self.assertIn("baseline", " ".join(verdict.notes))

    def test_a_comparison_that_cannot_be_made_refuses(self):
        entries = [entry("2026-11-01-baseline", label="baseline", date="2026-11-01"), entry("2026-11-10-run")]
        verdict = bench_gate.check(GIT, RELEASE, None, entries, Compare(error="the first run is unfit for comparison"))
        self.assertFalse(verdict.ok)
        self.assertIn("unfit for comparison", " ".join(verdict.reasons))


class TheHeldOutTotal(unittest.TestCase):
    def judged(self, before, after):
        entries = [
            entry("2026-11-01-baseline", label="baseline", date="2026-11-01", held_out=before),
            entry("2026-11-10-run", held_out=after),
        ]
        moved = [("place-a-bed", 5, 5)]
        return bench_gate.check(GIT, RELEASE, None, entries, Compare(comparison("no_measurable_change", moved)))

    def test_a_fall_the_intervals_cannot_explain_is_refused_as_a_count_and_never_a_name(self):
        verdict = self.judged((0.80, 0.70, 0.90), (0.40, 0.30, 0.50))
        self.assertFalse(verdict.ok)
        text = " ".join(verdict.reasons)
        self.assertIn("held-out", text)
        self.assertIn("80.0%", text)
        self.assertIn("40.0%", text)
        self.assertNotIn("place-a-bed", text)

    def test_a_fall_inside_the_noise_passes(self):
        self.assertTrue(self.judged((0.80, 0.60, 0.95), (0.70, 0.50, 0.90)).ok)

    def test_a_rise_passes(self):
        self.assertTrue(self.judged((0.40, 0.30, 0.50), (0.80, 0.70, 0.90)).ok)

    def test_a_previous_run_with_no_held_out_is_not_a_regression(self):
        verdict = self.judged(None, (0.40, 0.30, 0.50))
        self.assertTrue(verdict.ok, verdict.reasons)
        self.assertIn("held-out", " ".join(verdict.notes))


def git(repo, *args):
    return subprocess.run(
        ["git", "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", *args],
        cwd=repo, capture_output=True, text=True, check=True,
    ).stdout.strip()


class WithARealRepository(unittest.TestCase):
    """`Git` against a scratch repository: the crates tree, ancestry, and the release commit's own rule."""

    def setUp(self):
        self.repo = tempfile.mkdtemp(prefix=f"bench_gate_{self._testMethodName}_{os.getpid()}_")
        git(self.repo, "init", "-q", "-b", "main")
        self.commit_files({"crates/a/lib.rs": "1\n", "crates/bevy_cauldron/Cargo.toml": 'version = "0.1.0"\n', "Cargo.lock": "x\n", "docs/a.md": "a\n"}, "code")
        self.code = git(self.repo, "rev-parse", "HEAD")
        self.git = bench_gate.Git(self.repo)

    def commit_files(self, files, message):
        for path, text in files.items():
            full = os.path.join(self.repo, path)
            os.makedirs(os.path.dirname(full), exist_ok=True)
            with open(full, "w") as handle:
                handle.write(text)
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-q", "-m", message)
        return git(self.repo, "rev-parse", "HEAD")

    def test_a_docs_commit_keeps_the_crates_tree_and_a_code_commit_changes_it(self):
        docs = self.commit_files({"docs/a.md": "b\n"}, "docs: words")
        code = self.commit_files({"crates/a/lib.rs": "2\n"}, "feat: code")
        self.assertEqual(self.git.crates_tree(self.code), self.git.crates_tree(docs))
        self.assertNotEqual(self.git.crates_tree(docs), self.git.crates_tree(code))
        self.assertTrue(self.git.is_ancestor(self.code, code))
        self.assertFalse(self.git.is_ancestor(code, self.code))

    def test_a_bump_is_judged_as_the_code_it_bumps(self):
        bump = self.commit_files(
            {"crates/bevy_cauldron/Cargo.toml": 'version = "0.1.1"\n', "Cargo.lock": "y\n"}, "release: 0.1.1"
        )
        code, problem = self.git.code_commit(bump)
        self.assertEqual((code, problem), (self.code, None))
        run = [entry("2026-11-01-baseline", label="baseline", commit=self.code), entry("2026-11-10-run", commit=self.code)]
        verdict = bench_gate.check(self.git, code, None, run, Compare(comparison("better")))
        self.assertTrue(verdict.ok, verdict.reasons)

    def test_a_release_commit_that_carries_more_than_the_bump_is_refused(self):
        bump = self.commit_files({"crates/bevy_cauldron/Cargo.toml": 'version = "0.1.1"\n', "crates/a/lib.rs": "9\n"}, "release: 0.1.1")
        code, problem = self.git.code_commit(bump)
        self.assertIsNone(code)
        self.assertIn("crates/a/lib.rs", problem)

    def test_an_ordinary_commit_is_its_own_code(self):
        self.assertEqual(self.git.code_commit(self.code), (self.code, None))

    def test_the_previous_release_is_the_greatest_tag_below_this_version(self):
        tags = ["bevy_cauldron-v0.6.5", "bevy_cauldron-v0.6.10x", "bevy_cauldron-v0.6.7", "bevy_cauldron-v0.6.9", "other-v0.1.0"]
        self.assertEqual(bench_gate.previous_tag(tags, "0.6.8"), "bevy_cauldron-v0.6.7")
        self.assertIsNone(bench_gate.previous_tag(tags, "0.6.5"))


if __name__ == "__main__":
    unittest.main()
