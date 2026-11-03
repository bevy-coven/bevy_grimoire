#!/usr/bin/env python3
"""The step timer's reading of an omp session, tested on a session written here.

Run: `python3 -m unittest tools/loop/test_steps.py` (or `python3 tools/loop/test_steps.py`).
"""

import datetime as dt
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import steps  # noqa: E402

T0 = dt.datetime(2026, 10, 31, 12, 0, 0, tzinfo=dt.timezone.utc)


def at(s):
    return (T0 + dt.timedelta(seconds=s)).isoformat().replace("+00:00", "Z")


def assistant(s, *calls):
    return {"type": "message", "timestamp": at(s), "message": {"role": "assistant", "content": [
        {"type": "toolCall", "id": cid, "name": name, "arguments": args} for cid, name, args in calls]}}


def started(s, cid):
    return {"type": "custom", "customType": "tool_execution_start", "timestamp": at(s),
            "data": {"toolCallId": cid, "startedAt": at(s)}}


def result(s, cid, job=None):
    details = {"async": {"state": "running", "jobId": job}} if job else {}
    return {"type": "message", "timestamp": at(s),
            "message": {"role": "toolResult", "toolCallId": cid, "details": details, "content": []}}


SESSION = [
    {"type": "session", "timestamp": at(0), "cwd": "/repo"},
    {"type": "message", "timestamp": at(0), "message": {"role": "user", "content": [{"type": "text", "text": "go"}]}},
    # 5 s of model, then a phase mark and a test run that takes 100 s.
    assistant(5, ("a", "bash", {"command": "python3 tools/loop/steps.py mark build"})),
    started(5, "a"),
    result(6, "a"),
    assistant(10, ("b", "bash", {"command": "cargo test --workspace", "i": "Running the tests"})),
    started(11, "b"),
    result(111, "b"),
    # A background push, the turn ends, and the job reports 300 s later.
    assistant(120, ("c", "bash", {"command": "git push origin HEAD:main"})),
    started(120, "c"),
    result(121, "c", job="bg_1"),
    {"type": "message", "timestamp": at(125), "message": {"role": "assistant", "content": [{"type": "text", "text": "waiting"}]}},
    {"type": "custom_message", "customType": "async-result", "timestamp": at(425),
     "details": {"jobs": [{"jobId": "bg_1", "type": "bash", "status": "completed", "label": "git push origin HEAD:main",
                            "durationMs": 304000}]}},
    assistant(430),
]


class Steps(unittest.TestCase):
    def setUp(self):
        self.rows = steps.steps(SESSION)

    def find(self, kind, category):
        return [r for r in self.rows if r["kind"] == kind and r["category"] == category]

    def test_a_tool_call_lasts_from_its_start_to_its_result(self):
        (test,) = self.find("tool", "cargo test")
        self.assertEqual(test["seconds"], 100.0)
        self.assertIn("Running the tests", test["detail"])

    def test_the_model_is_timed_between_steps(self):
        models = [r["seconds"] for r in self.rows if r["kind"] == "model"]
        self.assertEqual(models[:2], [5.0, 4.0])

    def test_a_mark_sets_the_phase_of_what_follows(self):
        (test,) = self.find("tool", "cargo test")
        self.assertEqual(test["phase"], "build")
        self.assertEqual(self.rows[0]["phase"], "orient")

    def test_waiting_on_a_background_job_is_its_own_step_and_the_job_its_own_row(self):
        (wait,) = [r for r in self.rows if r["kind"] == "wait"]
        self.assertEqual(wait["seconds"], 300.0)
        (bg,) = [r for r in self.rows if r["kind"] == "bg"]
        self.assertEqual((bg["category"], bg["seconds"]), ("push gate", 304.0))

    def test_a_report_ranks_where_the_time_went(self):
        with tempfile.TemporaryDirectory() as d:
            steps.write_rows(Path(d) / "steps.tsv", 7, self.rows)
            out = Path(d) / "out.txt"
            with open(out, "w") as f:
                steps.report(d, 0, out=f)
            text = out.read_text()
        first = [l for l in text.splitlines() if l and not l.startswith(("category", "1 pass"))][0]
        self.assertTrue(first.startswith("wait"), text)

    def test_publish_pushes_the_log_to_its_own_branch_and_leaves_the_clone_alone(self):
        import subprocess

        def run(*a, cwd=None):
            return subprocess.run(a, cwd=cwd, capture_output=True, text=True, check=True).stdout.strip()

        with tempfile.TemporaryDirectory() as d:
            origin, clone, state = Path(d) / "origin.git", Path(d) / "loop_clone", Path(d) / "state"
            run("git", "init", "-q", "--bare", "-b", "main", str(origin))
            run("git", "clone", "-q", str(origin), str(clone))
            for k, v in (("user.name", "t"), ("user.email", "t@t")):
                run("git", "config", k, v, cwd=clone)
            (clone / "a.txt").write_text("a\n")
            run("git", "add", "a.txt", cwd=clone)
            run("git", "commit", "-qm", "a", cwd=clone)
            run("git", "push", "-q", "origin", "HEAD:main", cwd=clone)
            (clone / "dirty.txt").write_text("the pass's own work\n")
            head = run("git", "rev-parse", "HEAD", cwd=clone)
            state.mkdir()
            steps.write_rows(state / "steps.tsv", 7, self.rows)

            self.assertIn("pushed", steps.publish(str(clone), str(state), "loop-stats"))
            # A second publish builds on the branch it fetched, so it fast-forwards or has nothing to do.
            steps.write_rows(state / "steps.tsv", 8, self.rows)
            self.assertIn("pushed", steps.publish(str(clone), str(state), "loop-stats"))
            self.assertEqual(run("git", "rev-list", "--count", "loop-stats", cwd=origin), "2")

            files = run("git", "ls-tree", "-r", "--name-only", "loop-stats", cwd=origin).split()
            self.assertEqual(files, ["steps/loop_clone.md", "steps/loop_clone.tsv"])
            self.assertEqual(run("git", "rev-parse", "HEAD", cwd=clone), head)
            self.assertEqual(run("git", "status", "--short", cwd=clone), "?? dirty.txt")
            self.assertEqual(run("git", "rev-parse", "main", cwd=origin), head)

    def test_the_audit_names_what_a_pass_did_not_ship(self):
        import subprocess

        def run(*a, cwd=None):
            return subprocess.run(a, cwd=cwd, capture_output=True, text=True, check=True).stdout.strip()

        def commit(clone, files, msg):
            for f, text in files.items():
                (clone / f).parent.mkdir(parents=True, exist_ok=True)
                (clone / f).write_text(text)
            run("git", "add", "-A", cwd=clone)
            run("git", "commit", "-qm", msg, cwd=clone)

        with tempfile.TemporaryDirectory() as d:
            origin, clone, state = Path(d) / "origin.git", Path(d) / "c", Path(d) / "s"
            state.mkdir()
            run("git", "init", "-q", "--bare", "-b", "main", str(origin))
            run("git", "clone", "-q", str(origin), str(clone))
            for k, v in (("user.name", "t"), ("user.email", "t@t")):
                run("git", "config", k, v, cwd=clone)
            commit(clone, {"src/lib.rs": "a", "Cargo.toml": '[package]\nname = "demo"\nversion = "0.1.0"\n'}, "base")
            run("git", "push", "-q", "origin", "HEAD:main", cwd=clone)
            start = run("git", "rev-parse", "HEAD", cwd=clone)

            # A pass that shipped everything: code, docs, a GIF, and a release tag, all pushed.
            commit(clone, {"src/lib.rs": "b", "README.md": "b", "docs/media/b.gif": "GIF"}, "feature")
            run("git", "tag", "-a", "v0.1.1", "-m", "v0.1.1", cwd=clone)
            run("git", "push", "-q", "origin", "HEAD:main", "v0.1.1", cwd=clone)
            line = steps.audit(str(clone), start, 1, str(state))
            self.assertIn("missing: -", line)
            self.assertIn("released v0.1.1", line)

            # A pass that changed the code, pushed it, and stopped there.
            start = run("git", "rev-parse", "HEAD", cwd=clone)
            commit(clone, {"src/lib.rs": "c"}, "code only")
            run("git", "push", "-q", "origin", "HEAD:main", cwd=clone)
            line = steps.audit(str(clone), start, 2, str(state))
            self.assertIn("missing: release,docs,demo?", line)

            # A pass that committed and never pushed.
            start = run("git", "rev-parse", "origin/main", cwd=clone)
            commit(clone, {"notes.txt": "d"}, "local only")
            line = steps.audit(str(clone), start, 3, str(state))
            self.assertIn("1 unpushed", line)
            self.assertIn("push", line.split("missing: ")[1])
            self.assertEqual(len((state / "audit.tsv").read_text().splitlines()), 4)

    def test_a_session_is_matched_to_the_driver_pass_that_started_it(self):
        with tempfile.TemporaryDirectory() as d:
            log = Path(d) / "driver.log"
            local = T0.astimezone()
            log.write_text(f"{(local - dt.timedelta(seconds=40)):%Y-%m-%dT%H:%M:%S} pass 41 START command=/x\n"
                           f"{(local + dt.timedelta(hours=1)):%Y-%m-%dT%H:%M:%S} pass 42 START command=/x\n")
            starts = steps.pass_starts(log)
        self.assertEqual(steps.pass_for(T0, starts, -1), 41)
        self.assertEqual(steps.pass_for(T0 + dt.timedelta(days=1), starts, -1), -1)

    def test_rewriting_a_pass_replaces_its_rows_and_keeps_every_other_pass(self):
        with tempfile.TemporaryDirectory() as d:
            out = Path(d) / "steps.tsv"
            for n in (10, 11, 12):
                steps.write_rows(out, n, self.rows)
            steps.drop_passes(out, {12})
            steps.write_rows(out, 12, self.rows[:1])
            passes = [l.split("\t", 1)[0] for l in out.read_text().splitlines()[1:]]
        self.assertEqual(passes.count("10"), len(self.rows))
        self.assertEqual(passes.count("11"), len(self.rows))
        self.assertEqual(passes.count("12"), 1)

    def test_each_pass_records_its_configuration_once(self):
        import io
        import time
        from unittest import mock
        with tempfile.TemporaryDirectory() as d:
            env = {"WIGGUM_THINK": "high", "CARGO_BUILD_JOBS": "10"}
            with mock.patch.dict(os.environ, env):
                os.environ.pop("WIGGUM_MODEL", None)
                os.environ.pop("RUST_TEST_THREADS", None)
                since = time.time() - 600
                steps.write_config(d, 7, since)
                steps.write_config(d, 7, since)
                steps.write_config(d, 8, since)
            rows = list(steps.load_tsv(Path(d) / "config.tsv"))
            self.assertEqual([r["pass"] for r in rows], ["7", "8"])
            self.assertEqual({r["think"] for r in rows}, {"high"})
            self.assertEqual(rows[0]["model"], "-")
            self.assertTrue(599 <= float(rows[0]["seconds"]) <= 602)
            out = io.StringIO()
            steps.config_report(d, out)
            lines = out.getvalue().splitlines()
            self.assertEqual(len(lines), 2)
            self.assertIn("high", lines[1])
            self.assertRegex(lines[1], r"\s2\s+10\.0\s")


if __name__ == "__main__":
    unittest.main()


class TestAPassWithNoDriver(unittest.TestCase):
    """**A hand-run pass records the same as a driven one** (owner, 2026-11-03).

    Measured on `bevy_emerge_loop`: passes 252-291 were driven by re-running `/build-loop` inside one
    omp session. Nothing bumped `pass-count`, nothing stamped `driver.log`, and `backfill` — which
    maps a session to a pass by a `pass N START` line — fell through to a **negative** number, so 17
    passes of recorded work were written under a pass no report ever asked for. "Where did last night
    go?" had no answer that was not a guess.
    """

    def test_begin_numbers_a_pass_and_end_closes_it_without_a_driver(self):
        import time
        with tempfile.TemporaryDirectory() as d:
            state = Path(d)
            # The driver stopped at 233 and this clone has no driver.log at all.
            (state / "pass-count").write_text("233\n")
            n1 = steps.begin(d, "manual")
            n2 = steps.begin(d, "manual")
            ledger = steps.read_passes(state)
            count = (state / "pass-count").read_text().strip()
        self.assertEqual((n1, n2), (234, 235), "a hand pass continues the driver's numbering")
        self.assertEqual(count, "235", "and writes the number the driver's counter would have")
        self.assertEqual([r["via"] for r in ledger], ["manual", "manual"])

    def test_a_number_the_other_source_spent_is_never_reused(self):
        """The ledger and the driver log both count passes; the larger of the two wins."""
        with tempfile.TemporaryDirectory() as d:
            state = Path(d)
            local = T0.astimezone()
            (state / "driver.log").write_text(
                f"{(local - dt.timedelta(minutes=1)):%Y-%m-%dT%H:%M:%S} pass 500 START command=/x\n")
            steps.begin(d, "manual")
            self.assertEqual((state / "pass-count").read_text().strip(), "501",
                             "a ledger of nothing must not restart at 1 beside a log that reached 500")

    def test_backfill_gives_a_driverless_session_its_own_number_instead_of_a_negative_one(self):
        """`backfill` used to fall through to `-i` for a session no driver started: the rows landed
        under a negative pass number and every report ignored them."""
        from unittest import mock
        sessions = [(T0, "a.jsonl"), (T0 + dt.timedelta(hours=2), "b.jsonl")]
        rows = [dict(seq=1, phase="build", kind="model", category="model", tool="", start=T0,
                     seconds=120.0, detail="")]
        with tempfile.TemporaryDirectory() as d:
            state = Path(d)
            (state / "pass-count").write_text("40\n")
            with mock.patch.object(steps, "sessions_for", return_value=sessions), \
                 mock.patch.object(steps, "read_session", return_value=[]), \
                 mock.patch.object(steps, "steps", return_value=rows):
                steps.main(["backfill", "--repo", d, "--state", d])
            passes = sorted({int(r["pass"]) for r in steps.load_tsv(state / "steps.tsv")})
        self.assertEqual(passes, [41, 42], "both sessions are real passes and both are numbered")
        self.assertTrue(all(p > 0 for p in passes), "never a negative pass number")

    def test_one_session_that_ran_the_command_many_times_is_cut_into_its_passes(self):
        """**A hand-driven night is one session and many passes.**

        Measured 2026-11-03: one session held 57 `one build-loop pass` markers across 26 hours.
        `backfill` read it as a single pass, so every report saw one 26-hour "pass" and no per-pass
        timing at all. The marker is the pass's own boundary and the only one available with no
        driver watching.
        """
        mark = {"type": "message", "timestamp": at(0), "message": {"role": "user",
                "content": "# bevy_grimoire: one build-loop pass\n\nYou are one pass of a Wiggum loop"}}
        mark2 = {"type": "message", "timestamp": at(600), "message": {"role": "user",
                 "content": "# bevy_grimoire: one build-loop pass\n\nYou are one pass of a Wiggum loop"}}
        other = {"type": "message", "timestamp": at(300), "message": {"role": "user",
                 "content": "Can you label the blocked issues?"}}
        self.assertTrue(steps.is_pass_mark(mark))
        self.assertFalse(steps.is_pass_mark(other), "a person's message is not a pass boundary")
        slices = steps.pass_slices([mark, other, mark2])
        self.assertEqual([len(s[1]) for s in slices], [2, 1], "two passes, cut at the markers")
        self.assertEqual(len(steps.pass_slices([other, other])), 1, "no marker means one pass")

    def test_the_ledger_keeps_its_header_and_never_restarts_the_numbering(self):
        """Two bugs from one smoke test, both caught here.

        Rewriting the ledger from the rows it could parse once dropped the header, so every later read
        saw zero passes and `begin` restarted at 1. And the ledger is *sparse* — one row per pass it
        opened — while `steps.tsv` holds every pass ever recorded, so numbering from the ledger alone
        collided with a pass `backfill` had recovered from a session.
        """
        import csv
        with tempfile.TemporaryDirectory() as d:
            state = Path(d)
            steps.begin(d, "manual")
            steps.begin(d, "manual")
            steps.end(d, 2, d)          # close one: this rewrote the file
            lines = (state / "passes.tsv").read_text().splitlines()
            self.assertEqual(lines[0].split("\t"), steps.PASSES_COLUMNS, "the header survives `end`")
            # A pass only the step log knows about must still not be re-used.
            rows = [dict(seq=1, phase="build", kind="model", category="model", tool="", start=T0,
                         seconds=90.0, detail="")]
            steps.write_rows(state / "steps.tsv", 900, rows)
            self.assertEqual(steps.begin(d, "manual"), 901)

    def test_end_fills_the_row_it_opened_instead_of_deleting_it(self):
        """A smoke test on `bevy_emerge_loop` read `ledger_rows=0` after a clean begin/end: `end`
        dropped the row *before* looking for it, so every pass closed to nothing."""
        with tempfile.TemporaryDirectory() as d:
            state = Path(d)
            n = steps.begin(d, "manual")
            steps.end(d, n, d)
            rows = steps.read_passes(state)
            self.assertEqual(len(rows), 1, "the pass it opened is still there")
            self.assertEqual(int(rows[0]["pass"]), n)
            self.assertGreaterEqual(int(rows[0]["seconds"]), 0, "and it has a wall time")
