#!/usr/bin/env python3
"""changelog.py against throwaway git repositories and real ffmpeg.

What must hold: a clip is capped (20 s, 2 MB, 960 px) and has its still; entries stay newest first;
`check` refuses a line with neither a clip nor a reason, a missing file, a clip no line shows, a clip
outside Git LFS, and dates out of order; `page` plays every clip.

usage: python3 -m unittest tools/changelog/test_changelog.py
"""
import contextlib
import io
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import changelog  # noqa: E402

HAVE_FFMPEG = shutil.which("ffmpeg") is not None and shutil.which("ffprobe") is not None
LFS_LINE = "docs/changelog/media/** filter=lfs diff=lfs merge=lfs -text\n"


def run(*argv):
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        rc = changelog.main(list(argv))
    return rc, out.getvalue(), err.getvalue()


def frames(dirpath, seconds, width=1280, height=720, fps=15):
    os.makedirs(dirpath, exist_ok=True)
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-f", "lavfi", "-i",
                    "testsrc2=size={}x{}:rate={}".format(width, height, fps), "-t", str(seconds),
                    os.path.join(dirpath, "f_%05d.png")], check=True)


class Repo(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp(prefix="changelog_test_")
        self.repo = os.path.join(self.dir, "r")
        os.makedirs(self.repo)
        subprocess.run(["git", "init", "-q", self.repo], check=True)
        with open(os.path.join(self.repo, ".gitattributes"), "w") as fh:
            fh.write(LFS_LINE)

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def cl(self, *argv):
        return run("--repo", self.repo, *argv)

    def text(self):
        with open(os.path.join(self.repo, "CHANGELOG.md")) as fh:
            return fh.read()

    def media(self, name):
        return os.path.join(self.repo, changelog.MEDIA_DIR, name)

    def fake_clip(self, name):
        os.makedirs(os.path.dirname(self.media(name)), exist_ok=True)
        frames(os.path.join(self.dir, "src_" + name), 1, 320, 240)
        changelog.make_clip(self.media(name + ".mp4"), frames=os.path.join(self.dir, "src_" + name))


class Entries(Repo):
    def test_dates_stay_newest_first_and_a_day_keeps_its_newest_line_on_top(self):
        self.cl("add", "--id", "E-1", "--title", "one", "--date", "2026-10-20", "--no-clip", "an API, proved by `a::t`")
        self.cl("add", "--id", "E-2", "--title", "two", "--date", "2026-10-28", "--no-clip", "a refactor, proved by `b::t`")
        self.cl("add", "--id", "E-3", "--title", "three", "--date", "2026-10-25", "--no-clip", "x, proved by `c::t`")
        self.cl("add", "--id", "E-4", "--title", "four", "--date", "2026-10-28", "--no-clip", "y, proved by `d::t`")
        text = self.text()
        order = [text.index(s) for s in ("## 2026-10-28", "E-4", "E-2", "## 2026-10-25", "E-3", "## 2026-10-20", "E-1")]
        self.assertEqual(order, sorted(order))
        self.assertEqual(self.cl("check")[0], 0)

    def test_add_needs_exactly_one_of_clip_link_and_no_clip(self):
        rc, _, err = self.cl("add", "--id", "E-1", "--title", "t")
        self.assertEqual(rc, 2)
        self.assertIn("exactly one", err)
        rc, _, _ = self.cl("add", "--id", "E-1", "--title", "t", "--no-clip", "r", "--link", "x.gif")
        self.assertEqual(rc, 2)

    def test_a_clip_must_exist_before_its_line(self):
        rc, _, err = self.cl("add", "--id", "E-1", "--title", "t", "--clip", "2026-10-28-E-1")
        self.assertEqual(rc, 2)
        self.assertIn("run `clip` first", err)
        self.assertFalse(os.path.exists(os.path.join(self.repo, "CHANGELOG.md")))


class Check(Repo):
    def write(self, body):
        with open(os.path.join(self.repo, "CHANGELOG.md"), "w") as fh:
            fh.write("# Changelog\n\n" + body)

    def test_a_line_with_neither_clip_nor_reason_is_refused(self):
        self.write("## 2026-10-28\n\n- **E-1** something shipped\n")
        rc, out, _ = self.cl("check")
        self.assertEqual(rc, 1)
        self.assertIn("no clip and no `No clip:` reason", out)

    def test_a_missing_file_is_refused(self):
        self.write('## 2026-10-28\n\n- **E-1** x<br><a href="docs/a.gif"><img src="docs/a.gif"></a>\n')
        rc, out, _ = self.cl("check")
        self.assertEqual(rc, 1)
        self.assertIn("docs/a.gif does not exist", out)

    def test_dates_out_of_order_are_refused(self):
        self.write("## 2026-10-20\n\n- **E-1** a *No clip: r*\n\n## 2026-10-28\n\n- **E-2** b *No clip: r*\n")
        rc, out, _ = self.cl("check")
        self.assertEqual(rc, 1)
        self.assertIn("not newest first", out)

    def test_a_linked_existing_gif_passes(self):
        os.makedirs(os.path.join(self.repo, "docs/old"))
        open(os.path.join(self.repo, "docs/old/after.gif"), "wb").close()
        rc, _, _ = self.cl("add", "--id", "B-1", "--title", "old", "--date", "2026-10-21", "--link", "docs/old/after.gif")
        self.assertEqual(rc, 0)
        self.assertEqual(self.cl("check")[0], 0)

    @unittest.skipUnless(HAVE_FFMPEG, "needs ffmpeg")
    def test_a_clip_no_line_shows_is_refused(self):
        self.fake_clip("2026-10-28-E-9")
        self.write("## 2026-10-28\n\n- **E-1** a *No clip: r*\n")
        rc, out, _ = self.cl("check")
        self.assertEqual(rc, 1)
        self.assertIn("2026-10-28-E-9.mp4: no changelog line shows it", out)

    @unittest.skipUnless(HAVE_FFMPEG, "needs ffmpeg")
    def test_a_clip_outside_lfs_is_refused(self):
        self.fake_clip("2026-10-28-E-1")
        self.cl("add", "--id", "E-1", "--title", "t", "--date", "2026-10-28", "--clip", "2026-10-28-E-1")
        self.assertEqual(self.cl("check")[0], 0)
        os.remove(os.path.join(self.repo, ".gitattributes"))
        rc, out, _ = self.cl("check")
        self.assertEqual(rc, 1)
        self.assertIn("not stored by Git LFS", out)


@unittest.skipUnless(HAVE_FFMPEG, "needs ffmpeg")
class Clip(Repo):
    def test_a_wide_capture_becomes_a_capped_clip_with_its_still(self):
        src = os.path.join(self.dir, "frames")
        frames(src, 4)
        out = self.media("2026-10-28-E-1.mp4")
        rc, _, err = self.cl("clip", "--out", out, "--frames", src, "--fps", "15")
        self.assertEqual(rc, 0, err)
        self.assertLessEqual(os.path.getsize(out), changelog.MAX_BYTES)
        probe = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
                                "stream=width,height,codec_name", "-of", "csv=p=0", out],
                               stdout=subprocess.PIPE, universal_newlines=True, check=True).stdout.strip()
        codec, width, height = probe.split(",")
        self.assertEqual(codec, "h264")
        self.assertEqual(int(width), changelog.MAX_WIDTH)
        self.assertEqual(int(height) % 2, 0)
        self.assertAlmostEqual(changelog.probe_seconds(out), 4.0, delta=0.2)
        self.assertTrue(os.path.isfile(self.media("2026-10-28-E-1.png")))

    def test_a_clip_over_the_cap_is_refused_and_a_window_of_it_is_not(self):
        src = os.path.join(self.dir, "frames")
        frames(src, 21, 320, 240)
        out = self.media("long.mp4")
        rc, _, err = self.cl("clip", "--out", out, "--frames", src)
        self.assertEqual(rc, 2)
        self.assertIn("cap is 20 s", err)
        self.assertFalse(os.path.exists(out))
        rc, _, err = self.cl("clip", "--out", out, "--frames", src, "--start", "15", "--duration", "5")
        self.assertEqual(rc, 0, err)
        self.assertAlmostEqual(changelog.probe_seconds(out), 5.0, delta=0.2)

    def test_the_still_is_written_for_any_frame_count(self):
        # 38 frames at 10 fps: seeking an image sequence to its last frame writes nothing and exits 0,
        # which left the editor's clips without a still (2026-10-28).
        src = os.path.join(self.dir, "frames38")
        frames(src, 3.8, 320, 240, fps=10)
        self.assertEqual(len(os.listdir(src)), 38)
        out = self.media("short.mp4")
        rc, _, err = self.cl("clip", "--out", out, "--frames", src, "--fps", "10")
        self.assertEqual(rc, 0, err)
        self.assertTrue(os.path.isfile(self.media("short.png")))

    def test_a_crop_frames_the_subject_before_the_scale(self):
        src = os.path.join(self.dir, "frames")
        frames(src, 1, 640, 480)
        out = self.media("crop.mp4")
        rc, _, err = self.cl("clip", "--out", out, "--frames", src, "--crop", "100,50,420,290")
        self.assertEqual(rc, 0, err)
        dims = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
                               "stream=width,height", "-of", "csv=p=0", out],
                              stdout=subprocess.PIPE, universal_newlines=True, check=True).stdout.strip()
        self.assertEqual(dims, "320,240")
        rc, _, err = self.cl("clip", "--out", out, "--frames", src, "--crop", "420,50,100,290")
        self.assertEqual(rc, 2)
        self.assertIn("x0 < x1", err)
        rc, _, err = self.cl("clip", "--out", out, "--frames", src, "--crop", "200x100")
        self.assertEqual(rc, 0, err)
        dims = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
                               "stream=width,height", "-of", "csv=p=0", out],
                              stdout=subprocess.PIPE, universal_newlines=True, check=True).stdout.strip()
        self.assertEqual(dims, "200,100")


class Page(Repo):
    @unittest.skipUnless(HAVE_FFMPEG, "needs ffmpeg")
    def test_the_page_plays_clips_and_shows_gifs(self):
        self.fake_clip("2026-10-28-E-1")
        self.cl("add", "--id", "E-1", "--title", "new", "--date", "2026-10-28", "--clip", "2026-10-28-E-1")
        os.makedirs(os.path.join(self.repo, "docs/old"))
        open(os.path.join(self.repo, "docs/old/a.gif"), "wb").close()
        self.cl("add", "--id", "B-1", "--title", "old", "--date", "2026-10-21", "--link", "docs/old/a.gif")
        out = os.path.join(self.dir, "page.html")
        self.assertEqual(self.cl("page", "--out", out)[0], 0)
        with open(out) as fh:
            page = fh.read()
        self.assertIn("<video controls", page)
        self.assertIn("2026-10-28-E-1.png", page)
        self.assertIn('<img loading=lazy src="file://' + os.path.join(self.repo, "docs/old/a.gif"), page)
        self.assertLess(page.index("2026-10-28"), page.index("2026-10-21"))


class ForYou(Repo):
    """`--user`: the line a person who never reads the changelog would say, stored as ` *For you: <sentence>*`."""

    def test_the_sentence_is_stored_right_after_the_title_and_before_the_reason(self):
        rc, _, _ = self.cl("add", "--id", "E-1", "--title", "the title", "--date", "2026-10-28", "--user",
                           "Your recent projects are on the first screen.", "--no-clip", "an API, proved by `a::t`")
        self.assertEqual(rc, 0)
        line = [l for l in self.text().splitlines() if l.startswith("- **E-1**")][0]
        self.assertIn("the title *For you: Your recent projects are on the first screen.* *No clip: an API", line)
        self.assertEqual(changelog.user_sentence(line), "Your recent projects are on the first screen.")
        self.assertEqual(self.cl("check")[0], 0)

    def test_a_line_without_the_option_has_no_sentence(self):
        self.cl("add", "--id", "E-1", "--title", "t", "--date", "2026-10-28", "--no-clip", "r")
        line = [l for l in self.text().splitlines() if l.startswith("- **E-1**")][0]
        self.assertNotIn("For you", line)
        self.assertIsNone(changelog.user_sentence(line))

    def test_a_sentence_that_names_an_id_a_path_or_a_code_name_is_refused(self):
        for bad in ("Fixed #296.", "Done as UI-30.", "Reads docs/guide/start.md.", "Makes map_create work.", "", "two\nlines", "has *stars*"):
            rc, _, err = self.cl("add", "--id", "E-1", "--title", "t", "--date", "2026-10-28", "--user", bad, "--no-clip", "r")
            self.assertEqual(rc, 2, bad)
            self.assertIn("--user", err)
        self.assertFalse(os.path.exists(os.path.join(self.repo, "CHANGELOG.md")), "a refusal writes nothing")

    def test_check_refuses_a_sentence_already_in_the_file_that_names_an_id(self):
        with open(os.path.join(self.repo, "CHANGELOG.md"), "w") as fh:
            fh.write("# Changelog\n\n## 2026-10-28\n\n- **E-1** t *For you: Closes #296.* *No clip: r*\n")
        rc, out, _ = self.cl("check")
        self.assertEqual(rc, 1)
        self.assertIn("the `For you` sentence", out)


if __name__ == "__main__":
    unittest.main()
