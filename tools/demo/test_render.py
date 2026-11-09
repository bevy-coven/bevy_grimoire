#!/usr/bin/env python3
"""Tests for render.py: the GIF ends after the last event, not the last captured frame.

Run: python3 -m unittest test_render   (from tools/demo)
"""
import unittest

import render


def rec(last_frame_s, events, fps=15):
    frames = [{"t": round(i / fps, 3), "path": f"f{i}.png"} for i in range(round(last_frame_s * fps) + 1)]
    return {"fps": fps, "frames": frames, "events": events}


class TheGifEndsAfterTheLastEvent(unittest.TestCase):
    def test_the_end_is_the_last_event_plus_the_settle(self):
        # Overrun frames keep coming after the last event (captured 9 s); the GIF stops at 3.0 + 0.5.
        r = rec(9.0, [
            {"t": 1.0, "type": "caption", "text": "a"},
            {"t": 2.0, "type": "key", "keys": ["KeyZ"]},
            {"t": 3.0, "type": "agent", "tool": "placement_remove", "result": "{}"},
        ])
        self.assertAlmostEqual(render.timeline_end(r, settle=0.5), 3.5)

    def test_the_end_never_passes_the_last_captured_frame(self):
        r = rec(2.0, [{"t": 1.9, "type": "click", "x": 1, "y": 1}])
        self.assertAlmostEqual(render.timeline_end(r, settle=0.5), 2.0)

    def test_with_no_events_the_end_is_the_last_frame(self):
        r = rec(4.0, [])
        self.assertAlmostEqual(render.timeline_end(r, settle=0.5), 4.0)


if __name__ == "__main__":
    unittest.main()
