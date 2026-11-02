"""Timing regressions for record.py. Run: python3 -m unittest test_record (from tools/demo)."""
import time
import unittest

import record


class SlowAgent:
    """Stands in for McpClient: an MCP call that takes a while to answer."""

    def __init__(self, seconds):
        self.seconds = seconds

    def json_call(self, tool, args):
        time.sleep(self.seconds)
        return {"ok": True}


class McpEventIsStampedWhenCalled(unittest.TestCase):
    def test_the_event_is_stamped_at_call_start_not_return(self):
        tl = record.Timeline()
        time.sleep(0.05)
        before = tl.now()
        step = {"_kind": "mcp", "mcp": "grimoire_frame_view", "args": {}, "at": 0.0}
        record.do_step(step, tl, None, SlowAgent(0.5))
        (event,) = tl.events
        # The call took 0.5 s. The event must be stamped when it started, not when it returned.
        self.assertLess(event["t"] - before, 0.25, f"stamped {event['t'] - before:.3f} s after the call began")


class StepsAreTimedFromTheirChunk(unittest.TestCase):
    def test_a_step_falls_in_the_chunk_that_covers_its_storyboard_time(self):
        self.assertEqual(record.chunk_of(7.0, [6.0, 2.5, 3.0]), (1, 1.0))
        self.assertEqual(record.chunk_of(0.0, [6.0, 2.5, 3.0]), (0, 0.0))
        self.assertEqual(record.chunk_of(8.5, [6.0, 2.5, 3.0]), (2, 0.0))

    def test_a_step_at_a_boundary_belongs_to_the_later_chunk(self):
        self.assertEqual(record.chunk_of(6.0, [6.0, 2.5, 3.0]), (1, 0.0))


class RecordingInjector:
    """Stands in for Injector: writes each injected input into the same log as the timeline's events."""

    def __init__(self, log):
        self.log = log

    def cursor(self, x, y):
        self.log.append(("inject", "cursor"))

    def send(self, **kw):
        self.log.append(("inject", kw.get("action")))


class ClickIsStampedBeforeItLands(unittest.TestCase):
    def test_the_click_event_is_stamped_before_the_tap_is_injected(self):
        log = []
        tl = record.Timeline()
        tl.add = lambda **event: (log.append(("stamp", event["type"])), record.Timeline.add(tl, **event))
        step = {"_kind": "click", "click": [10, 20], "at": 0.0}
        record.do_step(step, tl, RecordingInjector(log), None)
        click_stamp = log.index(("stamp", "click"))
        tap = log.index(("inject", "Tap"))
        # A click's stamp must come before its Tap is sent, as a key's does; stamped after, the event
        # claims the moment the click finished and lands a frame late.
        self.assertLess(click_stamp, tap, f"log was {log}")


class EventsLandOnTheFrameThatShowsThem(unittest.TestCase):
    def test_an_event_moves_to_the_first_frame_captured_at_or_after_its_stamp(self):
        frames = [0.0, 0.1, 0.2, 0.3]
        events = [{"t": 0.12, "type": "key"}, {"t": 0.3, "type": "caption"}, {"t": 0.0, "type": "caption"}]
        record.align_events(events, frames)
        self.assertEqual([e["t"] for e in events], [0.2, 0.3, 0.0])
        self.assertEqual([e["t_stamp"] for e in events], [0.12, 0.3, 0.0])

    def test_an_event_after_the_last_frame_stays_on_the_last_frame(self):
        events = [{"t": 9.0, "type": "caption"}]
        record.align_events(events, [0.0, 0.1])
        self.assertEqual(events[0]["t"], 0.1)


if __name__ == "__main__":
    unittest.main()


class ClickLabelAsksTheEditor(unittest.TestCase):
    """**A label click asks the running editor where the words are**, never a guessed pixel.

    Three passes lost takes to a coordinate that drifted when a bar was added above the palette, so the
    step this file added reads BRP's `world.query` instead. These tests pin the two things that cost a
    run each to get wrong: `UiGlobalTransform` reflects as an **affine array**, not an object with a
    `translation`, and a row scrolled out of its panel must be refused rather than clicked off-window.
    """

    def rows(self, texts):
        out = []
        for words, at, size in texts:
            out.append({"entity": 1, "components": {
                "bevy_ui::widget::text::Text": words,
                "bevy_ui::ui_node::ComputedNode": {"size": size, "inverse_scale_factor": 1.0},
                "bevy_ui::ui_transform::UiGlobalTransform": at,
            }})
        return out

    def test_the_centre_comes_from_the_affine_array(self):
        rows = self.rows([("sofa", [1.0, 0.0, 0.0, 1.0, 400.0, 300.0], [40.0, 20.0])])
        answer = {"result": rows}
        original = record.brp
        record.brp = lambda port, method, params: answer
        try:
            self.assertEqual(record.widget_centre(1, "sofa"), (400.0, 300.0))
        finally:
            record.brp = original

    def test_a_row_outside_the_window_is_refused(self):
        rows = self.rows([("floor", [1.0, 0.0, 0.0, 1.0, 400.0, 1900.0], [40.0, 20.0])])
        answer = {"result": rows}
        original = record.brp
        record.brp = lambda port, method, params: answer
        try:
            with self.assertRaises(RuntimeError):
                record.widget_centre(1, "floor")
        finally:
            record.brp = original

    def test_the_exact_words_are_matched(self):
        rows = self.rows([("floor_light", [1.0, 0.0, 0.0, 1.0, 10.0, 10.0], [4.0, 4.0]),
                          ("floor", [1.0, 0.0, 0.0, 1.0, 900.0, 200.0], [40.0, 20.0])])
        answer = {"result": rows}
        original = record.brp
        record.brp = lambda port, method, params: answer
        try:
            self.assertEqual(record.widget_centre(1, "floor"), (900.0, 200.0))
        finally:
            record.brp = original

    def test_within_chooses_the_widget_of_that_name(self):
        """**Two widgets can carry one word.** `sofa` is a palette row *and* a piece on the map; the
        rectangle is the storyboard saying which one it means, rather than entity order deciding."""
        rows = self.rows([("sofa", [1.0, 0.0, 0.0, 1.0, 700.0, 300.0], [40.0, 20.0]),
                          ("sofa", [1.0, 0.0, 0.0, 1.0, 1150.0, 220.0], [40.0, 20.0])])
        answer = {"result": rows}
        original = record.brp
        record.brp = lambda port, method, params: answer
        try:
            self.assertEqual(record.widget_centre(1, "sofa", [1080, 100, 1280, 400]), (1150.0, 220.0))
            self.assertEqual(record.widget_centre(1, "sofa", [600, 100, 900, 500]), (700.0, 300.0))
        finally:
            record.brp = original
