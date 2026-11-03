#!/usr/bin/env python3
"""**The gate and the guidance use the fast runner** (owner, 2026-11-03).

`cargo test --workspace` is 451 s where `cargo nextest run --workspace` is 43 s on the same 2,810
tests, because `cargo test` runs test binaries one after another and nextest runs every test in its
own process. A gate that drifts back to the slow runner, or guidance that tells agents to, silently
costs every pass ten times what it needs to.

The check is behavioural where it can be: the hook is run against a scratch copy with the nextest
line removed, and must then be seen to have no nextest runner left. A string match alone would pass
on a comment that names nextest, which is exactly how this drifts.

Run: `python3 -m unittest tools/loop/test_fast_tests.py`.
"""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GATE = ROOT / "tools" / "loop" / "hooks" / "pre-push"
COMMAND = ROOT / ".omp" / "commands" / "build-loop.md"
AGENTS = ROOT / "AGENTS.md"

# The lines that actually run the tests, as opposed to a comment or a refusal message that names them.
NEXTEST_RUN = re.compile(r"^\s*timed\s+test\s+\S*cargo nextest run", re.MULTILINE)
DOCTEST_RUN = re.compile(r"^\s*timed\s+doc\s+\S*cargo test\b.*--doc", re.MULTILINE)


class TestTheGateUsesTheFastRunner(unittest.TestCase):
    def setUp(self):
        self.assertTrue(GATE.exists(), f"{GATE} is missing")
        self.text = GATE.read_text(encoding="utf-8")

    def test_the_gate_runs_nextest(self):
        self.assertRegex(
            self.text, NEXTEST_RUN,
            "the gate's test step is no longer `timed test $cargo nextest run`: 451 s became 43 s by "
            "nextest running the binaries in parallel, and that is the whole win")

    def test_the_gate_still_runs_doctests(self):
        # nextest does not run doctests; dropping this step stops testing every example, silently.
        self.assertRegex(
            self.text, DOCTEST_RUN,
            "nextest does not run doctests, so the gate needs its own `timed doc ... --doc` step")

    def test_removing_the_nextest_line_is_what_these_checks_catch(self):
        """The guard sees the drift it exists for: a hook whose nextest step was taken out."""
        drifted = NEXTEST_RUN.sub("  timed test $cargo test --workspace -q --no-fail-fast", self.text)
        self.assertNotEqual(drifted, self.text, "the nextest step was not found to remove")
        self.assertIsNone(NEXTEST_RUN.search(drifted))
        self.assertIsNotNone(DOCTEST_RUN.search(drifted),
                             "the scratch copy should keep its doctest step, so the two checks are "
                             "independent and each catches its own drift")


class TestTheGuidanceSaysTheSame(unittest.TestCase):
    def setUp(self):
        self.assertTrue(COMMAND.exists(), f"{COMMAND} is missing")
        self.command = " ".join(COMMAND.read_text(encoding="utf-8").split())

    def test_the_loop_command_names_nextest_as_the_gate_runner(self):
        self.assertIn("`cargo nextest run --workspace`", self.command,
                      "the loop command tells a pass what the push gate runs; naming the slow "
                      "command there sends every pass back to it")
        self.assertIn("`cargo test --workspace --doc`", self.command,
                      "and the doctest step, or the command implies nextest covers the examples")

    def test_the_repo_rules_name_nextest(self):
        self.assertTrue(AGENTS.exists(), f"{AGENTS} is missing")
        rules = " ".join(AGENTS.read_text(encoding="utf-8").split())
        self.assertIn("cargo nextest", rules,
                      "AGENTS.md is what an agent reads first in this repo; it must not teach "
                      "`cargo test --workspace`")


if __name__ == "__main__":
    unittest.main()
