"""Every panel of the editor can be staged for a capture, and record.py plans one frame per panel.

Run: python3 -m unittest test_stage (from tools/demo). UI-92, #258.

The staging itself is the editor's: `grimoire/stage_panel` puts the shell on one panel, and the plan below is what
record.py turns into one capture per panel. A plan that named fewer frames than panels would record a clip of the panel
before the one it claims to show, which is the failure this file exists to catch.
"""

import unittest

import record


PANELS = ["inspector", "viewport", "place", "settings", "loader", "launcher"]


class OneFramePerStagedPanel(unittest.TestCase):
    def test_each_panel_is_its_own_chunk_with_one_frame(self):
        plan = record.plan_staged_panels("panels", 15, PANELS)
        self.assertEqual(len(plan["chunks"]), len(PANELS), "one chunk per staged panel")
        for chunk, panel in zip(plan["chunks"], PANELS):
            self.assertEqual(chunk["stage"], panel)
            self.assertEqual(chunk["frames"], 1)

    def test_the_frames_are_the_plan(self):
        # A capture's frame count is `seconds * fps`, floored, so a chunk of 1/fps is exactly one frame. One frame
        # short is a chunk that films the panel before it; one frame long is a clip that hangs on nothing.
        fps = 15
        plan = record.plan_staged_panels("panels", fps, PANELS)
        for chunk in plan["chunks"]:
            self.assertEqual(int(chunk["seconds"] * fps), chunk["frames"])
        self.assertEqual(sum(int(c["seconds"] * fps) for c in plan["chunks"]), len(PANELS))
        self.assertEqual(plan["steps"], [], "staged panels need no input")
        self.assertEqual(plan["setup"], [])

    def test_a_plan_with_no_panel_is_refused(self):
        with self.assertRaises(ValueError):
            record.plan_staged_panels("panels", 15, [])
        with self.assertRaises(ValueError):
            record.plan_staged_panels("panels", 0, PANELS)

    def test_a_stage_step_is_its_own_kind(self):
        # The step has to be dispatched as a stage rather than falling through to a caption: a storyboard that
        # silently filmed the panel before it would look like a working take.
        self.assertEqual(record.kind_of({"at": 1.0, "stage": "place"}), "stage")
        self.assertEqual(record.kind_of({"at": 1.0, "cursor": [1, 2]}), "cursor")


if __name__ == "__main__":
    unittest.main()
