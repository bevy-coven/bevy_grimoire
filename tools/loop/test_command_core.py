#!/usr/bin/env python3
"""**The loop command's core cannot be tuned away.**

`build-loop.md` now tells a pass it may change how it works, measured against `steps.py report`. The
risk that creates is the one this file holds shut: a pass that finds a rule inconvenient deletes it,
and the next pass reads a prompt with no budget and no protected evaluator. Each assertion below is a
rule the owner made, and their absence is the failure — not their wording, which a pass may improve.

Run: `python3 -m unittest tools/loop/test_command_core.py`.
"""

import os
import unittest
from pathlib import Path

COMMAND = Path(__file__).resolve().parents[2] / ".omp" / "commands" / "build-loop.md"

# A rule is present if the command still says its substance; each entry is any one of these phrases.
CORE = {
    "the budget": ["Budget — a pass is", "45 minutes"],
    "the checkpoints": ["Checkpoints, in this order"],
    "the protected evaluator": ["Do not edit tests", "never edit a test", "protected evaluator"],
    "one gate per push": ["one gate", "One gate"],
    "never --no-verify": ["--no-verify"],
    "Fixes #N only on the last item": ["Fixes #N"],
    "never leave the tree uncommitted": ["Never leave the tree uncommitted"],
    "the two-way street": ["two-way street"],
    "no force push or reset of others' work": ["force-push", "reset --hard"],
}


class TestTheCommandKeepsItsCore(unittest.TestCase):
    def setUp(self):
        self.assertTrue(COMMAND.exists(), f"{COMMAND} is missing")
        self.text = COMMAND.read_text(encoding="utf-8")
        self.flat = " ".join(self.text.split())

    def test_every_core_rule_is_still_in_the_command(self):
        missing = [name for name, phrases in CORE.items()
                   if not any(p in self.flat for p in phrases)]
        self.assertEqual(
            missing, [],
            "a core rule left the command. The tunable half is docs/loop/command-tuning.md; the core "
            "is the owner's and a pass may not remove it")

    def test_the_tunable_half_is_a_file_the_core_cannot_be_moved_into(self):
        """The tuning lives beside the command, never inside the frozen half."""
        tuning = COMMAND.parents[2] / "docs" / "loop" / "command-tuning.md"
        self.assertTrue(tuning.exists(), "the command points at docs/loop/command-tuning.md")
        self.assertIn("docs/loop/command-tuning.md", self.flat)

    def test_a_tuning_must_be_measured_and_reverted_when_it_does_not_work(self):
        for phrase in ("A tuning is a hypothesis with a number", "the tuning is wrong"):
            self.assertIn(phrase, self.flat,
                          "a prompt that only adds rules grows without bound; the revert rule is what "
                          "keeps it from bloating")

    def test_the_pass_times_itself_and_not_its_launcher(self):
        """**The command owns this, so every launcher gets it.** A driver, a shell, Herdr and Oh My Pie
        all start the pass the same way; if timing lived in a wrapper, the third launcher to arrive
        would be the one that silently recorded nothing — which is what happened on 2026-11-02."""
        self.assertIn("First command of this pass", self.flat)
        self.assertIn("steps.py begin", self.flat)
        self.assertIn("steps.py end", self.flat)
        self.assertIn("no arguments", self.flat,
                      "a step that needs an environment variable is a step a new launcher forgets")


if __name__ == "__main__":
    unittest.main()
