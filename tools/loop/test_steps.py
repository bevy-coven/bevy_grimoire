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
            commit(clone, {"src/lib.rs": "a"}, "base")
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


if __name__ == "__main__":
    unittest.main()
