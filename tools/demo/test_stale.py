"""stale.py on a throwaway repository: which standing demos a visible change has left behind."""
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import stale  # noqa: E402


def git(repo, *args, date=None):
    env = dict(os.environ, GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_SYSTEM="/dev/null",
               GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@t", GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@t")
    if date:
        env["GIT_AUTHOR_DATE"] = env["GIT_COMMITTER_DATE"] = date
    r = subprocess.run(["git", "-C", str(repo), *args], env=env, text=True, capture_output=True)
    assert r.returncode == 0, r.stderr
    return r.stdout.strip()


def commit(repo, name, date):
    (repo / name).write_text(name)
    git(repo, "add", "-A")
    git(repo, "commit", "-qm", name, date=date)
    return git(repo, "rev-parse", "HEAD")


CLIP = '<br><a href="docs/changelog/media/x.mp4"><img src="docs/changelog/media/x.png" width="320" alt="clip"></a>'


class Stale(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.repo = Path(self.tmp.name)
        git(self.repo, "init", "-q", "-b", "main")
        (self.repo / "docs" / "media").mkdir(parents=True)

    def write(self, rows, changes):
        table = "| File | Command | Captured | Shows |\n|---|---|---|---|\n" + "\n".join(
            f"| `{name}` | `cmd` | {captured} | shows |" for name, captured in rows)
        (self.repo / "docs" / "media" / "README.md").write_text(table + "\n")
        body = "\n".join(f"- **{cid}** `{sha}` a change{CLIP if clip else ' *No clip: an API*'}" for cid, sha, clip in changes)
        (self.repo / "CHANGELOG.md").write_text("# Changelog\n\n## 2026-11-01\n\n" + body + "\n")

    def test_a_visible_change_after_the_capture_makes_the_demo_stale_and_names_the_change(self):
        first = commit(self.repo, "a", "2026-10-01T10:00:00+0000")
        seen = commit(self.repo, "b", "2026-10-02T10:00:00+0000")
        self.write([("hero.gif", f"2026-10-01 {first[:8]}")], [("shows-up", seen[:8], True)])
        text, any_stale = stale.report(self.repo)
        self.assertTrue(any_stale)
        self.assertIn("hero.gif: stale (1 visible change(s)", text)
        self.assertIn("shows-up " + seen[:8], text)

    def test_a_change_with_no_clip_and_a_change_before_the_capture_leave_the_demo_fresh(self):
        before = commit(self.repo, "a", "2026-10-01T10:00:00+0000")
        captured = commit(self.repo, "b", "2026-10-02T10:00:00+0000")
        hidden = commit(self.repo, "c", "2026-10-03T10:00:00+0000")
        self.write([("hero.gif", f"2026-10-02 {captured[:8]}")],
                   [("an-api", hidden[:8], False), ("earlier", before[:8], True), ("at-capture", captured[:8], True)])
        text, any_stale = stale.report(self.repo)
        self.assertFalse(any_stale, text)
        self.assertIn("hero.gif: fresh", text)

    def test_each_demo_is_judged_by_its_own_commit(self):
        old = commit(self.repo, "a", "2026-10-01T10:00:00+0000")
        mid = commit(self.repo, "b", "2026-10-02T10:00:00+0000")
        newest = commit(self.repo, "c", "2026-10-03T10:00:00+0000")
        self.write([("old.gif", f"2026-10-01 {old[:8]}"), ("new.gif", f"2026-10-03 {newest[:8]}")],
                   [("middle", mid[:8], True)])
        text, any_stale = stale.report(self.repo)
        self.assertTrue(any_stale)
        self.assertIn("old.gif: stale", text)
        self.assertIn("new.gif: fresh", text)

    def test_a_date_with_no_commit_means_the_last_commit_of_that_day(self):
        early = commit(self.repo, "a", "2026-10-01T10:00:00+0000")
        late = commit(self.repo, "b", "2026-10-01T23:00:00+0000")
        after = commit(self.repo, "c", "2026-10-02T09:00:00+0000")
        self.write([("editor.png", "2026-10-01")], [("same-day", late[:8], True), ("next-day", after[:8], True)])
        text, any_stale = stale.report(self.repo)
        self.assertTrue(any_stale)
        self.assertIn("editor.png: stale (1 visible change(s) since " + late[:8], text)
        self.assertIn("next-day", text)
        self.assertNotIn("same-day", text)
        self.assertNotEqual(early, late)

    def test_a_captured_commit_the_repository_does_not_have_is_stale(self):
        commit(self.repo, "a", "2026-10-01T10:00:00+0000")
        self.write([("hero.gif", "2026-10-01 deadbeef")], [])
        text, any_stale = stale.report(self.repo)
        self.assertTrue(any_stale)
        self.assertIn("names no commit", text)


if __name__ == "__main__":
    unittest.main()
