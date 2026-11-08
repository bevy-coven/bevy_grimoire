#!/usr/bin/env python3
"""Tests for the release smoke's relaunch rule: only a window that never took the foreground is tried again.

Run with `python3 -m unittest tools/release/macos/test_smoke_app.py`. The app itself is not launched here:
`run_attempts` is the decision, given what one launch reported.
"""

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(__file__))

import smoke_app  # noqa: E402

UNFOCUSED = "the window never took focus within 40 s: nothing can be judged; another app held the foreground"


class Launches:
    """A scripted run: what each launch reports, and how many were made."""

    def __init__(self, *results):
        self.results = list(results)
        self.made = 0
        self.said = []
        self.paused = 0

    def once(self, n):
        self.made += 1
        return self.results[min(self.made, len(self.results)) - 1]

    def pause(self):
        self.paused += 1

    def run(self, attempts=smoke_app.LAUNCH_ATTEMPTS):
        return smoke_app.run_attempts(self.once, attempts, pause=self.pause, say=self.said.append)


class RelaunchTests(unittest.TestCase):
    def test_a_launch_that_passes_is_not_repeated(self):
        launches = Launches((None, False))
        self.assertIsNone(launches.run())
        self.assertEqual(launches.made, 1)
        self.assertEqual(launches.paused, 0)

    def test_a_window_that_never_took_focus_is_launched_again_until_one_does(self):
        launches = Launches((UNFOCUSED, True), (UNFOCUSED, True), (None, False))
        self.assertIsNone(launches.run())
        self.assertEqual(launches.made, 3)
        self.assertEqual(launches.paused, 2, "a pause before each relaunch, none after the last launch")
        self.assertEqual(len(launches.said), 2)

    def test_a_window_that_never_takes_focus_fails_after_exactly_the_attempts_it_is_given(self):
        launches = Launches((UNFOCUSED, True))
        self.assertEqual(launches.run(attempts=3), UNFOCUSED)
        self.assertEqual(launches.made, 3)
        self.assertEqual(launches.paused, 2)

    def test_any_other_failure_is_the_apps_and_is_returned_at_once(self):
        black = "the editor's window self-check says the window is black"
        launches = Launches((black, False), (None, False))
        self.assertEqual(launches.run(), black, "a black window is never retried into a pass")
        self.assertEqual(launches.made, 1)
        self.assertEqual(launches.paused, 0)

    def test_a_failure_that_follows_an_unfocused_launch_is_the_one_returned(self):
        launches = Launches((UNFOCUSED, True), ("the window self-check saw 3 colours, fewer than 8", False))
        self.assertEqual(launches.run(), "the window self-check saw 3 colours, fewer than 8")
        self.assertEqual(launches.made, 2)

    def test_never_focused_is_a_smoke_failure_so_a_caller_that_catches_one_catches_both(self):
        self.assertTrue(issubclass(smoke_app.NeverFocused, smoke_app.SmokeFailure))


if __name__ == "__main__":
    unittest.main()
