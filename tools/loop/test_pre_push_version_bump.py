#!/usr/bin/env python3
"""A push of a version bump runs no test; any other manifest change still runs the gate (owner, 2026-11-12).

A release commit changes a manifest's `version = "x"` line and the same line in `Cargo.lock`, and nothing else; the gate
(clippy and the whole test suite, ~20 minutes) used to run on it and could not find anything. `cargo` here is `false`, so a
push that reaches the clippy step fails, and a push that skips it succeeds.

    python3 tools/loop/test_pre_push_version_bump.py
"""
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

HOOK = Path(__file__).resolve().parent / "hooks" / "pre-push"


def git(cwd, *args, check=True, env_extra=None):
    env = dict(os.environ, GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_SYSTEM="/dev/null", GIT_AUTHOR_NAME="t",
               GIT_AUTHOR_EMAIL="t@t", GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@t", **(env_extra or {}))
    r = subprocess.run(["git", *args], cwd=cwd, env=env, text=True, capture_output=True)
    if check and r.returncode != 0:
        raise AssertionError(f"git {' '.join(args)}: {r.stdout}{r.stderr}")
    return r


MANIFEST = 'name = "app"\nversion = "0.6.4"\n\n[dependencies]\ncore = { path = "../core", version = "0.10.0" }\n'
LOCK = '[[package]]\nname = "app"\nversion = "0.6.4"\n\n[[package]]\nname = "core"\nversion = "0.10.0"\n'


class VersionBumpNeedsNoTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="pre_push_bump"))
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.bare = self.tmp / "remote.git"
        self.work = self.tmp / "work"
        git(self.tmp, "init", "-q", "--bare", "-b", "main", str(self.bare))
        git(self.tmp, "init", "-q", "-b", "main", str(self.work))
        git(self.work, "remote", "add", "origin", str(self.bare))
        for name in ("tools/loop/backlog.py", "tools/changelog/changelog.py", "tools/loop/version.py"):
            path = self.work / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("import sys\nsys.exit(0)\n")
        hooks = self.work / "tools" / "loop" / "hooks"
        hooks.mkdir(parents=True, exist_ok=True)
        shutil.copy(HOOK, hooks / "pre-push")
        (self.work / "crates" / "app").mkdir(parents=True)
        (self.work / "crates" / "app" / "Cargo.toml").write_text(MANIFEST)
        (self.work / "crates" / "app" / "lib.rs").write_text("pub fn f() {}\n")
        (self.work / "Cargo.lock").write_text(LOCK)
        git(self.work, "add", "-A")
        git(self.work, "commit", "-qm", "start")
        git(self.work, "-c", "core.hooksPath=/dev/null", "push", "-q", "origin", "main")
        git(self.work, "config", "core.hooksPath", "tools/loop/hooks")

    def push(self):
        return git(self.work, "push", "origin", "main", check=False, env_extra={"LOOP_GATE_CARGO": "false"})

    def commit(self, files, message):
        for name, text in files.items():
            (self.work / name).write_text(text)
        git(self.work, "add", "-A")
        git(self.work, "commit", "-qm", message)

    def test_a_manifest_and_lock_version_line_is_pushed_without_a_test_run(self):
        self.commit({"crates/app/Cargo.toml": MANIFEST.replace('version = "0.6.4"', 'version = "0.6.5"', 1),
                     "Cargo.lock": LOCK.replace('name = "app"\nversion = "0.6.4"', 'name = "app"\nversion = "0.6.5"')},
                    "release: 0.6.5")
        pushed = self.push()
        self.assertEqual(pushed.returncode, 0, pushed.stderr)
        self.assertIn("a version bump", pushed.stderr)

    def test_a_dependency_version_in_a_manifest_still_runs_the_gate(self):
        self.commit({"crates/app/Cargo.toml": MANIFEST.replace('version = "0.10.0"', 'version = "0.11.0"')}, "deps: bump core")
        pushed = self.push()
        self.assertNotEqual(pushed.returncode, 0, "a dependency version change skipped the gate")
        self.assertIn("clippy", pushed.stderr)

    def test_a_source_change_beside_the_bump_still_runs_the_gate(self):
        self.commit({"crates/app/Cargo.toml": MANIFEST.replace('version = "0.6.4"', 'version = "0.6.5"', 1),
                     "crates/app/lib.rs": "pub fn f() { let _ = 1; }\n"}, "release: 0.6.5, and a code change")
        pushed = self.push()
        self.assertNotEqual(pushed.returncode, 0, "a bump that also changed code skipped the gate")

    def test_a_manifest_change_that_is_not_a_version_line_still_runs_the_gate(self):
        self.commit({"crates/app/Cargo.toml": MANIFEST + 'extra = "yes"\n'}, "manifest: an extra line")
        pushed = self.push()
        self.assertNotEqual(pushed.returncode, 0, "a non-version manifest change skipped the gate")


if __name__ == "__main__":
    unittest.main()
