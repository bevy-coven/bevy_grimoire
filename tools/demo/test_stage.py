"""Every panel of the editor can be staged for a capture, and record.py plans one frame per panel.

Run: python3 -m unittest test_stage (from tools/demo). UI-92, #258.

The staging itself is the editor's: `grimoire/stage_panel` puts the shell on one panel, and the plan below is what
record.py turns into one capture per panel. A plan that named fewer frames than panels would record a clip of the panel
before the one it claims to show, which is the failure this file exists to catch.
"""

import time
import unittest

import record


PANELS = ["inspector", "viewport", "place", "settings", "loader", "launcher"]


class OneFramePerStagedPanel(unittest.TestCase):
    def test_each_panel_is_its_own_chunk_of_one_frame(self):
        plan = record.plan_staged_panels("panels", 15, PANELS)
        self.assertEqual(len(plan["chunks"]), len(PANELS), "one chunk per staged panel")
        for chunk, panel in zip(plan["chunks"], PANELS):
            self.assertEqual(chunk["stage"], panel)

    def test_the_frame_count_the_editor_computes_is_one_per_panel(self):
        # The editor's own count is `seconds * fps`, floored (`sequencer::drive_sequencer`), so a chunk of 1/fps is
        # exactly one frame. One frame short films the panel before it; one frame long is a clip hanging on nothing.
        # The plan carries no count of its own: a second number would be a copy of the editor's arithmetic, free to
        # drift from it.
        fps = 15
        plan = record.plan_staged_panels("panels", fps, PANELS)
        for chunk in plan["chunks"]:
            self.assertNotIn("frames", chunk, "the plan names a panel and a duration; the editor counts the frames")
            self.assertEqual(int(chunk["seconds"] * fps), 1, "each chunk is exactly one frame at this rate")
        self.assertEqual(sum(int(c["seconds"] * fps) for c in plan["chunks"]), len(PANELS))
        self.assertEqual(plan["steps"], [], "staged panels need no input")
        self.assertEqual(plan["setup"], [])

    def test_a_plan_with_no_panel_or_an_out_of_range_rate_is_refused(self):
        with self.assertRaises(ValueError):
            record.plan_staged_panels("panels", 15, [])
        # The editor refuses a rate outside `sequencer::MIN_FPS..=MAX_FPS`; the plan refuses it here rather than by a
        # capture that fails halfway through a take.
        for bad in (0, 1, record.MIN_FPS - 1, record.MAX_FPS + 1):
            with self.assertRaises(ValueError, msg=f"{bad} fps is outside the editor's range"):
                record.plan_staged_panels("panels", bad, PANELS)
        with self.assertRaises(ValueError):
            record.plan_staged_panels("panels", record.MIN_FPS if record.MIN_FPS < 2 else 1, PANELS)

    def test_a_stage_step_is_its_own_kind(self):
        # The step has to be dispatched as a stage rather than falling through to a caption: a storyboard that
        # silently filmed the panel before it would look like a working take.
        self.assertEqual(record.kind_of({"at": 1.0, "stage": "place"}), "stage")
        self.assertEqual(record.kind_of({"at": 1.0, "cursor": [1, 2]}), "cursor")


class StubClient:
    """Stands in for McpClient: answers a capture with the frames its plan asked for, and records the labels."""

    calls = []

    def __init__(self, server, project, port, label):
        self.label = label

    def json_call(self, tool, args):
        StubClient.calls.append((self.label, args["label"], args["fps"], args["seconds"]))
        # A capture takes its own length, as the editor's does, so the chunks are separated by the chunk length and a
        # stage stamped on the *previous* chunk's clock is a chunk away rather than a microsecond.
        time.sleep(args["seconds"])
        count = StubClient.frames.get(args["label"], 1)
        return {"status": "done", "frames": [{"index": k, "t_s": k / args["fps"], "actual_t_s": k / args["fps"],
                                              "path": f"{args['label']}/f{k:03d}.png"} for k in range(count)]}

    def close(self):
        pass


class RunChunksStagesEachPanelBeforeItsCapture(unittest.TestCase):
    """**`run_chunks` must stage a panel before the capture that films it, and stamp each stage on its own chunk's clock.**

    The first chunk of every staged plan raised `UnboundLocalError`: `timeline.add(t=wall, ...)` ran before `wall` was
    assigned, so the stage event was stamped with the *previous* chunk's start — and the first chunk had no previous one.
    A take would have died on the first panel, or, once the name `wall` existed, put every stage on the frame before it.
    """

    def setUp(self):
        self.original_client = record.McpClient
        self.original_stage = record.stage_a_panel
        StubClient.calls = []
        StubClient.frames = {}
        record.McpClient = StubClient
        record.stage_a_panel = lambda port, panel: {"panel": panel, "screen": "Editor", "staged": True}

    def tearDown(self):
        record.McpClient = self.original_client
        record.stage_a_panel = self.original_stage

    def test_every_chunk_stamps_its_stage_on_its_own_start(self):
        panels = ["inspector", "viewport", "place"]
        plan = record.plan_staged_panels("panels", 15, panels)
        timeline = record.Timeline(len(plan["chunks"]))
        errors = []
        record.run_chunks(plan, "stub-server", "/tmp/project", 0, timeline, errors)
        self.assertEqual(errors, [], f"run_chunks raised: {errors}")
        stages = [e for e in timeline.events if e.get("type") == "stage"]
        self.assertEqual([e["panel"] for e in stages], panels, "every chunk staged its own panel, in order")
        for chunk_index, event in enumerate(stages):
            # Two reads of the same clock a fraction of a millisecond apart, so the tolerance is a millisecond; a stage
            # stamped on the *previous* chunk's clock is a whole chunk (66 ms at 15 fps) away, which this catches.
            self.assertAlmostEqual(
                event["t"], timeline.chunk_started[chunk_index], delta=0.01,
                msg=f"panel {event['panel']} was stamped at {event['t']:.4f} s and its chunk started at "
                    f"{timeline.chunk_started[chunk_index]:.4f} s: a stage stamped on the frame before it films the "
                    f"wrong panel")
            # And it is stamped before the capture it belongs to starts, which is what makes frame 0 the staged panel.
            self.assertLessEqual(event["t"], timeline.chunk_started[chunk_index])
        # One frame per panel, and each frame on its own chunk's clock.
        self.assertEqual(len(timeline.frames), len(panels))
        for chunk_index, chunk in enumerate(timeline.chunks):
            self.assertEqual(chunk["index"], chunk_index)
            self.assertAlmostEqual(chunk["start"], round(timeline.chunk_started[chunk_index], 3), delta=0.001)
        for k in range(1, len(timeline.chunks)):
            self.assertGreater(
                timeline.chunks[k]["start"], timeline.chunks[k - 1]["start"] + 0.01,
                "each chunk starts at its own scheduled time, not on the one before it")

    def test_a_capture_that_writes_fewer_frames_than_planned_is_refused(self):
        panels = ["inspector", "place"]
        plan = record.plan_staged_panels("panels", 15, panels)
        # The editor answers with no frames at all: a take of nothing is not a take.
        StubClient.frames = {"demo_panels_0": 0}
        timeline = record.Timeline(len(plan["chunks"]))
        errors = []
        record.run_chunks(plan, "stub-server", "/tmp/project", 0, timeline, errors)
        self.assertTrue(errors, "a chunk that planned one frame and captured none must refuse the take")
        self.assertIn("frame", str(errors[0]).lower(), f"the refusal says what is missing: {errors[0]}")


if __name__ == "__main__":
    unittest.main()
