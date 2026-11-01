"""check.py's change-moment window, on synthetic clips.

Each clip is 8 fps, 160x120 px (the caption bar is 56 px), with one Home key at t = 1.0 s. The scene is a fixed
noise pattern, and a change is an inversion of that pattern on the frames listed, so every changed pixel differs
by far more than CHANGE_DIFF.
"""
import json
import os
import subprocess
import sys
import tempfile
import unittest

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
CHECK = os.path.join(HERE, "check.py")
FPS = 8
W, H = 160, 120
FRAMES = 24
EVENT_T = 1.0


def write_clip(directory, changed_frames, keys=("Home",)):
    base = np.random.default_rng(7).integers(60, 201, size=(H, W, 3), dtype=np.uint8)
    frames = []
    for i in range(FRAMES):
        # a per-frame 4x4 patch keeps PIL from merging consecutive frames (a one-pixel tag did not survive quantising)
        arr = 255 - base if i in changed_frames else base.copy()
        arr[0:4, 0:4, :] = (i * 10) % 256
        arr[0, 0, 1] = i
        frames.append(Image.fromarray(arr))
    gif = os.path.join(directory, "synthetic.gif")
    frames[0].save(gif, save_all=True, append_images=frames[1:], duration=125, loop=0)
    events = os.path.join(directory, "synthetic.events.json")
    with open(events, "w") as fh:
        json.dump({"name": "synthetic", "fps": FPS, "chunks": [],
                   "events": [{"t": EVENT_T, "type": "key", "keys": list(keys)}]}, fh)
    return events, gif


def change_line(events, gif):
    """Run check.py and return its one 'change ...' line."""
    done = subprocess.run([sys.executable, CHECK, events, gif], capture_output=True, text=True, cwd=HERE)
    lines = [ln for ln in done.stdout.splitlines() if ln.startswith("change ")]
    if len(lines) != 1:
        raise AssertionError(f"expected one change line, got {lines!r}; stderr: {done.stderr}")
    return lines[0]


class ChangeWindow(unittest.TestCase):
    def check(self, changed_frames, keys=("Home",)):
        with tempfile.TemporaryDirectory() as directory:
            events, gif = write_clip(directory, changed_frames, keys)
            return change_line(events, gif)

    def test_a_change_on_the_event_frame_passes(self):
        # the control: the old and the new measure both see it
        self.assertIn("PASS", self.check(set(range(8, FRAMES))))

    def test_a_change_one_frame_after_the_event_passes(self):
        # frames 9-10 only: the change is gone before the old 0.5 s-after frame (frame 12)
        self.assertIn("PASS", self.check({9, 10}))

    def test_a_change_half_a_second_after_the_event_fails(self):
        # starts at frame 13 (0.625 s after the event): outside the 0.2 s window, after the 0.5 s-after frame
        self.assertIn("FAIL", self.check(set(range(13, FRAMES))))

    def test_ctrl_z_is_an_undo_moment_like_cmd_z(self):
        # a recording made on Linux presses Ctrl+Z; an undo that changed nothing on screen must still fail
        for mod in ("ControlLeft", "SuperLeft"):
            self.assertIn("change undo", self.check(set(range(8, FRAMES)), (mod, "KeyZ")))
            self.assertIn("FAIL", self.check(set(), (mod, "KeyZ")))


if __name__ == "__main__":
    unittest.main()
