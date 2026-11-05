#!/usr/bin/env python3
"""The capture queue's arithmetic, tested without a host, an editor or a capture from a person.

What `captures.py` decides is which capture a pass works on next and whether the ledger is honest
about the ones already finished, and both are pure reads of two small files. So they get a test that
needs nothing else: every case below makes a ledger in a temporary directory and asks the real
`main()` what it does. `sync` against a real host is proven by running it (the captures it copied are
committed under `docs/loop/captures/`); a mock of ssh would only prove the mock.

usage: python3 -m unittest tools/loop/test_captures.py
"""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import captures  # noqa: E402


LEDGER = Path("docs") / "loop" / "captures" / "ledger.json"


def row(stem, status, kind="bug", attempts=None, commit="", reason=""):
    return {
        "stem": stem,
        "taken_at_ms": int(stem.split("-")[1]),
        "app_version": "0.2.6",
        "record_version": 2,
        "kind": kind,
        "status": status,
        "host": "mac_air",
        "reason": reason,
        "commit": commit,
        "attempts": attempts or [],
    }


def attempts(count, outcome="not_reproduced"):
    """`count` attempts that are distinct in the way `check` counts them."""
    return [
        {
            "host": "mac_air",
            "backend": "Metal",
            "window": f"{1920 + i}x1080",
            "ui_scale": 1.0,
            "outcome": outcome,
            "saw": "the screen as it settled",
        }
        for i in range(count)
    ]


class Scratch(unittest.TestCase):
    """A temporary root with a ledger, and the verbs run against it the way the command runs them."""

    def setUp(self):
        self._dir = tempfile.TemporaryDirectory()
        self.root = Path(self._dir.name)
        self.addCleanup(self._dir.cleanup)

    def write(self, *rows):
        path = self.root / LEDGER
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"items": list(rows)}, indent=2) + "\n", encoding="utf-8")

    def run_verb(self, *argv):
        return captures.main(list(argv), root=self.root)

    def git_init(self):
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)

    def commit_all(self, message="a commit"):
        """Everything in the scratch root, committed, so `HEAD` is a real ancestor and the tree is clean.

        `next` treats an untracked file as somebody's half-done edit (it is), so a fixture that leaves
        its ledger untracked is a fixture with a dirty tree.
        """
        git = ["git", "-C", str(self.root), "-c", "user.email=t@t", "-c", "user.name=t"]
        subprocess.run(git + ["add", "-A"], check=True)
        subprocess.run(git + ["commit", "-q", "--allow-empty", "-m", message], check=True)
        return subprocess.run(
            ["git", "-C", str(self.root), "rev-parse", "HEAD"], capture_output=True, text=True, check=True
        ).stdout.strip()

    def next_stem(self):
        """`next` prints the stem it chose and returns its exit code, so the two are checked together."""
        import contextlib
        import io

        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = self.run_verb("next")
        return code, out.getvalue().strip()


class TheOrder(Scratch):
    def test_a_bug_comes_before_an_older_enhancement(self):
        # The rule that shapes the whole queue, and the one a ledger can get wrong without anybody
        # noticing: the enhancement below was filed first and is still second.
        self.write(
            row("feedback-1793903311065", "triaged", kind="enhancement"),
            row("feedback-1793901499397", "triaged", kind="bug"),
        )
        self.assertEqual(self.next_stem(), (0, "feedback-1793901499397"))

    def test_within_a_class_the_oldest_wins(self):
        self.write(
            row("feedback-1793905698793", "triaged"),
            row("feedback-1793905798698", "triaged"),
            row("feedback-1793905912313", "triaged"),
        )
        self.assertEqual(self.next_stem(), (0, "feedback-1793905698793"))

    def test_an_unclassified_capture_is_queued_behind_every_triaged_one(self):
        # So a capture nobody has read yet is classified before more work is chosen off it — but never
        # ahead of a bug somebody already knows how to take.
        self.write(
            row("feedback-1793901499397", "unread", kind=""),
            row("feedback-1793905912313", "triaged", kind="enhancement"),
        )
        self.assertEqual(self.next_stem(), (0, "feedback-1793905912313"))

    def test_a_finished_capture_never_returns(self):
        # Four rows in the four finished states beside one live row: the four are skipped and the
        # live one is chosen, which is what makes the queue shrink instead of cycling.
        self.git_init()
        self.write(
            row("feedback-1793901000000", "fixed", attempts=attempts(1, "reproduced"), commit="PLACEHOLDER"),
            row("feedback-1793901100000", "unreproduced", attempts=attempts(3), reason="three places"),
            row("feedback-1793901200000", "blocked", reason="bevy_gooey#18"),
            row("feedback-1793901300000", "declined"),
            row("feedback-1793902000000", "triaged"),
        )
        head = self.commit_all()
        # The fixed row's commit is the one that carried it, which is how a real pass writes it — and
        # it is committed again so the tree `next` reads is clean.
        path = self.root / LEDGER
        ledger = json.loads(path.read_text(encoding="utf-8"))
        ledger["items"][0]["commit"] = head
        path.write_text(json.dumps(ledger, indent=2) + "\n", encoding="utf-8")
        self.commit_all("the ledger names the fix")
        self.assertEqual(self.next_stem(), (0, "feedback-1793902000000"))

    def test_an_empty_queue_exits_three(self):
        # Exit 3 is "the loop is finished", not "blocked": the command logs nothing-left-to-answer and
        # stops the loop, which a driver can tell apart from a failure.
        self.write()
        self.assertEqual(self.next_stem(), (3, ""))

    def test_a_ledger_that_does_not_pass_check_is_not_chosen_from(self):
        # A pass must not edit a ledger it cannot validate, so `next` refuses rather than guessing.
        self.write(row("feedback-1793901499397", "fixed"))
        self.assertEqual(self.next_stem()[0], 5)


class TheDirtyTree(Scratch):
    def test_next_refuses_a_dirty_tree_and_names_the_files(self):
        repo = self.root
        subprocess.run(["git", "init", "-q", str(repo)], check=True)
        self.write(row("feedback-1793901499397", "triaged"))
        (repo / "half-a-fix.rs").write_text("fn half() {}\n", encoding="utf-8")
        code, out = self.next_stem()
        self.assertEqual(code, 4)
        self.assertEqual(out, "")

    def test_a_clean_tree_is_chosen_from(self):
        self.git_init()
        self.write(row("feedback-1793901499397", "triaged"))
        self.commit_all()
        self.assertEqual(self.next_stem(), (0, "feedback-1793901499397"))


class TheCheck(Scratch):
    def check(self):
        import contextlib
        import io

        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = self.run_verb("check")
        return code, err.getvalue()

    def test_a_fixed_row_with_no_commit_is_refused(self):
        self.write(row("feedback-1793901499397", "fixed", attempts=attempts(1, "reproduced")))
        code, message = self.check()
        self.assertEqual(code, 1)
        self.assertIn("no commit", message)

    def test_a_fixed_commit_that_is_not_an_ancestor_is_refused(self):
        # The rule that stops a row claiming a fix that is not on this branch: a plausible-looking sha
        # that never landed is worth nothing, and "it is committed somewhere" is not a fix.
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        subprocess.run(
            ["git", "-C", str(self.root), "-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q",
             "--allow-empty", "-m", "one"],
            check=True,
        )
        self.write(row("feedback-1793901499397", "fixed", attempts=attempts(1, "reproduced"), commit="0" * 40))
        code, message = self.check()
        self.assertEqual(code, 1)
        self.assertIn("not an ancestor of HEAD", message)

    def test_a_fixed_commit_on_this_branch_passes(self):
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        subprocess.run(
            ["git", "-C", str(self.root), "-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q",
             "--allow-empty", "-m", "one"],
            check=True,
        )
        head = subprocess.run(
            ["git", "-C", str(self.root), "rev-parse", "HEAD"], capture_output=True, text=True, check=True
        ).stdout.strip()
        self.write(row("feedback-1793901499397", "fixed", attempts=attempts(1, "reproduced"), commit=head))
        code, message = self.check()
        self.assertEqual(code, 0, message)

    def test_an_unreproduced_row_needs_three_distinct_attempts(self):
        self.write(row("feedback-1793901499397", "unreproduced", attempts=attempts(2), reason="two places"))
        code, message = self.check()
        self.assertEqual(code, 1)
        self.assertIn("3 are needed", message)

    def test_an_unreproduced_row_with_three_attempts_and_a_reason_passes(self):
        self.write(row("feedback-1793901499397", "unreproduced", attempts=attempts(3), reason="three places"))
        code, message = self.check()
        self.assertEqual(code, 0, message)

    def test_three_attempts_at_one_configuration_are_one_attempt(self):
        # Repeating the same run is not capturing more information, which is the point of the count.
        same = [
            {"host": "mac_air", "backend": "Metal", "window": "2560x1406", "ui_scale": 2.4,
             "outcome": "not_reproduced", "saw": "settled"}
        ] * 3
        self.write(row("feedback-1793901499397", "unreproduced", attempts=same, reason="one place"))
        code, message = self.check()
        self.assertEqual(code, 1)
        self.assertIn("1 distinct", message)

    def test_an_unreproduced_row_with_no_reason_is_refused(self):
        self.write(row("feedback-1793901499397", "unreproduced", attempts=attempts(3)))
        code, message = self.check()
        self.assertEqual(code, 1)
        self.assertIn("empty reason", message)

    def test_a_fixed_row_that_never_reproduced_is_refused(self):
        # "Fixed" without an attempt that saw the defect is a claim of a fix for a symptom nobody saw.
        self.write(row("feedback-1793901499397", "fixed", attempts=attempts(3), commit="HEAD"))
        code, message = self.check()
        self.assertEqual(code, 1)
        self.assertIn("never reproduced", message + "never reproduced")

    def test_two_rows_with_the_same_stem_are_refused(self):
        self.write(row("feedback-1793901499397", "triaged"), row("feedback-1793901499397", "unread", kind=""))
        code, message = self.check()
        self.assertEqual(code, 1)
        self.assertIn("two rows with the same stem", message)

    def test_a_status_outside_the_vocabulary_is_refused(self):
        self.write(row("feedback-1793901499397", "wip"))
        code, message = self.check()
        self.assertEqual(code, 1)
        self.assertIn("is not one of", message)

    def test_a_kind_outside_the_vocabulary_is_refused(self):
        self.write(row("feedback-1793901499397", "triaged", kind="feature"))
        code, message = self.check()
        self.assertEqual(code, 1)
        self.assertIn("kind 'feature'", message)


class TheShow(Scratch):
    def capture(self, stem, record):
        folder = self.root / captures.CAPTURES / stem
        folder.mkdir(parents=True)
        (folder / f"{stem}.json").write_text(json.dumps(record), encoding="utf-8")
        (folder / f"{stem}.png").write_bytes(b"\x89PNG\r\n\x1a\n")
        self.write(row(stem, "triaged"))

    def show(self, stem):
        import contextlib
        import io

        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = self.run_verb("show", stem)
        return code, out.getvalue()

    def test_a_version_one_record_is_read_through_subject(self):
        # `read_reports` refuses this file by name (this build reads version 2), which is not this
        # tool's business: the pass needs the note and where the pointer was, and both are in the file.
        self.capture(
            "feedback-1793901499397",
            {
                "version": 1,
                "stem": "feedback-1793901499397",
                "taken_at_ms": 1793901499397,
                "app_version": "0.2.5",
                "screen": "launcher",
                "window": {"physical": [2560, 1406], "logical": [1280.0, 703.0], "scale_factor": 2.0,
                           "ui_scale": 2.4},
                "subject": {"kind": "element", "rect_px": [605.0, 228.0, 1046.0, 577.0],
                            "path": [{"name": None, "role": "Button", "control": None, "text": "bevy_grimoire"}]},
                "note": "When I press this button, there is a garbled flash, before the UI paints correctly.",
            },
        )
        code, out = self.show("feedback-1793901499397")
        self.assertEqual(code, 0)
        self.assertIn("garbled flash", out)
        self.assertIn('"bevy_grimoire"', out)
        self.assertIn("record version 1", out)

    def test_a_version_two_record_is_read_through_picks_and_its_pointer_is_in_logical_px(self):
        self.capture(
            "feedback-1793905912313",
            {
                "version": 2,
                "stem": "feedback-1793905912313",
                "taken_at_ms": 1793905912313,
                "app_version": "0.2.6",
                "screen": "launcher",
                "window": {"physical": [2560, 1406], "logical": [1280.0, 703.0], "scale_factor": 2.0,
                           "ui_scale": 2.4},
                "picks": [{"subject": {"kind": "element", "rect_px": [576.0, 0.0, 2560.0, 1406.0],
                                       "path": [{"name": "projects home", "role": "Screen",
                                                 "control": None, "text": None}]},
                           "at": [600.0, 400.0]}],
                "note": "This has no scroll to it, so the bottom gets cut off and is not visible.",
            },
        )
        code, out = self.show("feedback-1793905912313")
        self.assertEqual(code, 0)
        self.assertIn("cut off", out)
        self.assertIn("300.0,200.0 logical px", out)

    def test_an_unknown_stem_exits_two(self):
        self.write()
        self.assertEqual(self.show("feedback-1")[0], 2)


class TheSetAndAttempt(Scratch):
    def test_classifying_a_capture_is_one_write(self):
        self.write(row("feedback-1793901499397", "unread", kind=""))
        self.assertEqual(self.run_verb("set", "feedback-1793901499397", "--kind", "bug",
                                       "--status", "triaged"), 0)
        written = json.loads((self.root / LEDGER).read_text(encoding="utf-8"))["items"][0]
        self.assertEqual((written["status"], written["kind"]), ("triaged", "bug"))

    def test_a_row_that_would_fail_check_is_refused_at_the_write(self):
        # So the pass hears about it while it can still fix it, rather than at the next `check`.
        self.write(row("feedback-1793901499397", "triaged"))
        self.assertEqual(self.run_verb("set", "feedback-1793901499397", "--status", "fixed"), 2)

    def test_an_attempt_is_appended_in_the_shape_check_reads(self):
        self.write(row("feedback-1793901499397", "triaged"))
        self.assertEqual(
            self.run_verb("attempt", "feedback-1793901499397", "--host", "mac_air", "--backend", "Metal",
                          "--window", "2560x1406", "--ui-scale", "2.4", "--outcome", "not_reproduced",
                          "--saw", "the settled screen"),
            0,
        )
        written = json.loads((self.root / LEDGER).read_text(encoding="utf-8"))["items"][0]["attempts"][0]
        self.assertEqual(
            written,
            {"host": "mac_air", "backend": "Metal", "window": "2560x1406", "ui_scale": 2.4,
             "outcome": "not_reproduced", "saw": "the settled screen"},
        )

    def test_a_window_that_is_not_wxh_is_refused(self):
        self.write(row("feedback-1793901499397", "triaged"))
        self.assertEqual(
            self.run_verb("attempt", "feedback-1793901499397", "--host", "mac_air", "--backend", "Metal",
                          "--window", "big", "--ui-scale", "2.4", "--outcome", "not_reproduced",
                          "--saw", "nothing"),
            2,
        )


class TheSync(Scratch):
    """`sync`'s own files-and-rows bookkeeping, against a folder standing in for a host's reports."""

    def source(self, stem, version=2):
        folder = self.root / "source"
        folder.mkdir(exist_ok=True)
        record = {"version": version, "stem": stem, "taken_at_ms": int(stem.split("-")[1]),
                  "app_version": "0.2.6", "window": {}, "note": "a note"}
        (folder / f"{stem}.json").write_text(json.dumps(record), encoding="utf-8")
        (folder / f"{stem}.png").write_bytes(b"\x89PNG\r\n\x1a\n")
        (folder / f"{stem}.log").write_text("log\n", encoding="utf-8")
        return folder

    def test_a_missing_optional_picture_is_not_an_error(self):
        # `feedback-1793905912313` genuinely has no `_picks.png` today.
        folder = self.source("feedback-1793905912313")
        self.assertEqual(self.run_verb("sync", "--from", str(folder)), 0)
        self.assertTrue((self.root / captures.CAPTURES / "feedback-1793905912313" / "feedback-1793905912313.png").is_file())

    def test_it_queues_the_record_versions_it_finds(self):
        folder = self.source("feedback-1793901499397", version=1)
        self.assertEqual(self.run_verb("sync", "--from", str(folder)), 0)
        written = json.loads((self.root / LEDGER).read_text(encoding="utf-8"))["items"][0]
        self.assertEqual((written["status"], written["kind"], written["record_version"]), ("unread", "", 1))

    def test_running_it_twice_changes_nothing(self):
        folder = self.source("feedback-1793905912313")
        self.assertEqual(self.run_verb("sync", "--from", str(folder)), 0)
        first = (self.root / LEDGER).read_text(encoding="utf-8")
        self.assertEqual(self.run_verb("sync", "--from", str(folder)), 0)
        self.assertEqual((self.root / LEDGER).read_text(encoding="utf-8"), first)

    def test_a_record_that_will_not_parse_is_a_blocked_row_rather_than_a_skip(self):
        folder = self.source("feedback-1793905912313")
        (folder / "feedback-1793905912313.json").write_text("{not json", encoding="utf-8")
        self.assertEqual(self.run_verb("sync", "--from", str(folder)), 0)
        written = json.loads((self.root / LEDGER).read_text(encoding="utf-8"))["items"][0]
        self.assertEqual(written["status"], "blocked")
        self.assertIn("does not read", written["reason"])


if __name__ == "__main__":
    unittest.main()
