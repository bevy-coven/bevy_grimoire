#!/usr/bin/env python3
"""A push through `tools/loop/hooks/pre-push` uploads the commit's new LFS objects (owner, 2026-11-01).

`core.hooksPath=tools/loop/hooks` replaces git-lfs's own `pre-push`, so a clip's pointer reached main and its
file stayed on the clone (eight changelog media files, GH008). The hook now runs `git lfs pre-push` with the
pushed refs. The remote here is a bare repository on disk, where git-lfs keeps the objects under `lfs/objects`.

    python3 tools/loop/test_pre_push_lfs.py
"""
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

HOOK = Path(__file__).resolve().parent / "hooks" / "pre-push"


def git(cwd, *args, check=True, input=None):
    env = dict(os.environ, GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_SYSTEM="/dev/null",
               GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@t", GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@t")
    r = subprocess.run(["git", *args], cwd=cwd, env=env, text=True, capture_output=True, input=input)
    if check and r.returncode != 0:
        raise AssertionError(f"git {' '.join(args)}: {r.stdout}{r.stderr}")
    return r


def lfs_objects(bare):
    root = Path(bare) / "lfs" / "objects"
    return sorted(p.name for p in root.rglob("*") if p.is_file()) if root.exists() else []


class PrePushUploadsLfsObjects(unittest.TestCase):
    def setUp(self):
        if shutil.which("git-lfs") is None:
            self.skipTest("git-lfs is not installed")
        self.tmp = Path(tempfile.mkdtemp(prefix="pre_push_lfs"))
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.bare = self.tmp / "remote.git"
        self.work = self.tmp / "work"
        git(self.tmp, "init", "-q", "--bare", "-b", "main", str(self.bare))
        git(self.tmp, "init", "-q", "-b", "main", str(self.work))
        git(self.work, "remote", "add", "origin", str(self.bare))
        git(self.work, "lfs", "install", "--local")
        # The two checks the hook runs on a docs-only push, as stand-ins that pass: this test is about LFS.
        for name in ("tools/loop/backlog.py", "tools/changelog/changelog.py"):
            path = self.work / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("import sys\nsys.exit(0)\n")
        hooks = self.work / "tools" / "loop" / "hooks"
        hooks.mkdir(parents=True, exist_ok=True)
        shutil.copy(HOOK, hooks / "pre-push")
        (self.work / ".gitattributes").write_text("docs/changelog/media/*.png filter=lfs diff=lfs merge=lfs -text\n")
        (self.work / "docs" / "changelog" / "media").mkdir(parents=True)
        (self.work / "README.md").write_text("start\n")
        git(self.work, "add", "-A")
        git(self.work, "commit", "-qm", "start")
        # The first push of a branch runs the full gate (a cargo build this temp repository has none of);
        # it carries no LFS object, so it goes without the hook.
        git(self.work, "-c", "core.hooksPath=/dev/null", "push", "-q", "origin", "main")
        git(self.work, "config", "core.hooksPath", "tools/loop/hooks")

    def commit_clip(self, content):
        (self.work / "docs" / "changelog" / "media" / "clip.png").write_bytes(content)
        git(self.work, "add", "-A")
        git(self.work, "commit", "-qm", "docs: a clip")

    def test_a_pushed_clip_reaches_the_remote_with_its_pointer(self):
        self.assertEqual(lfs_objects(self.bare), [], "the remote starts with no LFS object")
        self.commit_clip(b"\x89PNG not really, but long enough to be a file " * 20)
        pointer = git(self.work, "show", "HEAD:docs/changelog/media/clip.png").stdout
        self.assertIn("oid sha256:", pointer, "the clip is stored as an LFS pointer")
        oid = pointer.split("oid sha256:")[1].split()[0]

        pushed = git(self.work, "push", "origin", "main", check=False)

        self.assertEqual(pushed.returncode, 0, pushed.stderr)
        self.assertEqual(lfs_objects(self.bare), [oid], "the file never reached the remote, only its pointer did")

    def test_a_push_whose_lfs_upload_fails_is_refused(self):
        self.commit_clip(b"another clip, long enough to matter " * 20)
        # Take the object away from the clone, so git-lfs has nothing to upload: the upload fails and the push must too.
        pointer = git(self.work, "show", "HEAD:docs/changelog/media/clip.png").stdout
        oid = pointer.split("oid sha256:")[1].split()[0]
        shutil.rmtree(self.work / ".git" / "lfs" / "objects")

        pushed = git(self.work, "push", "origin", "main", check=False)

        self.assertNotEqual(pushed.returncode, 0, "a push whose clip could not be uploaded went through")
        self.assertIn("git lfs pre-push failed", pushed.stderr)
        self.assertEqual(lfs_objects(self.bare), [], f"object {oid} should not be on the remote")
        self.assertNotEqual(git(self.bare, "rev-parse", "main").stdout, git(self.work, "rev-parse", "HEAD").stdout)


if __name__ == "__main__":
    unittest.main()
