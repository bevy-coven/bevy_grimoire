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

    def test_a_click_label_step_is_a_click_and_not_a_caption(self):
        """**`kind_of` did not carry `click_label`**, so the step matched nothing and fell through to
        `caption`: the words were printed on the take and no click was ever sent. From `events.json`
        it read as a storyboard that only had captions, which is why it stayed unnoticed until the
        first storyboard aimed at a widget whose words move."""
        self.assertEqual(record.kind_of({"at": 1.0, "click_label": "Stop: Baking"}), "click_label")
        self.assertEqual(record.kind_of({"at": 1.0, "click": [10, 20]}), "click")
        self.assertEqual(record.kind_of({"at": 1.0, "caption": "words"}), "caption")

    def test_the_scale_factor_does_not_move_the_click(self):
        """**`UiGlobalTransform`'s translation is already in the input space.**

        The editor's interface is built at 1.2 scale, so a node reports `inverse_scale_factor` 0.833.
        This multiplied the translation by it, which moved every `click_label` 17% up and left — off
        the window for the palette's rows at x ~ 1122 — while the fixture below, written with 1.0,
        could never notice. Measured on a running editor: clicking the raw centre of five palette rows
        armed exactly those five pieces.
        """
        row = {"entity": 1, "components": {
            "bevy_ui::widget::text::Text": "pipe",
            "bevy_ui::ui_node::ComputedNode": {"size": [60.0, 16.0], "inverse_scale_factor": 0.8333333},
            "bevy_ui::ui_transform::UiGlobalTransform": [1.0, 0.0, 0.0, 1.0, 1122.0, 293.0],
        }}
        answer = {"result": [row]}
        original = record.brp
        record.brp = lambda port, method, params: answer
        try:
            self.assertEqual(record.widget_centre(1, "pipe"), (1122.0, 293.0))
        finally:
            record.brp = original

    def test_a_prefix_finds_a_widget_whose_words_move(self):
        """**The status band's job item counts up**, so a storyboard cannot write its words down. A
        prefix names the widget; the number after it is whatever the work has reached."""
        rows = self.rows([("Baking previews 3 of 40", [1.0, 0.0, 0.0, 1.0, 480.0, 700.0], [80.0, 12.0])])
        answer = {"result": rows}
        original = record.brp
        record.brp = lambda port, method, params: answer
        try:
            self.assertEqual(record.widget_centre(1, "Baking previews", None, True), (480.0, 700.0))
            # **Without the prefix the same words find nothing**, which is what keeps an exact match
            # exact: `floor` still must not match `floor_light`.
            with self.assertRaises(RuntimeError):
                record.widget_centre(1, "Baking previews")
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


class ScrollToStopsWhenTheWordsArrive(unittest.TestCase):
    """**`scroll_to` asks the editor after every notch**, so a storyboard names a widget and a panel and
    never a notch count (#282). A fixed count was right for one window height and put the clock
    buttons at y = 6, inside the top bar, at another: the next click opened `kits & maps`."""

    class Wheel:
        """A panel whose one label moves 120 px per notch, as the inspector's content does."""

        def __init__(self, y, floor=-400.0):
            self.y = y
            self.floor = floor
            self.notches = 0
            self.cursor_at = None
            self.cleared = False
            self.port = 1

        def cursor(self, x, y):
            self.cursor_at = (x, y)

        def clear_cursor(self):
            self.cleared = True

        def send(self, **params):
            self.notches += 1
            self.y = max(self.floor, self.y + 120.0 * params["y"]) if params["y"] < 0 else self.y + 120.0

        def reply(self, port, method, params):
            row = {"entity": 1, "components": {
                "bevy_ui::widget::text::Text": "6:00",
                "bevy_ui::ui_node::ComputedNode": {"size": [40.0, 20.0]},
                "bevy_ui::ui_transform::UiGlobalTransform": [1.0, 0.0, 0.0, 1.0, 56.0, self.y],
            }}
            return {"result": [row]}

    def run_scroll(self, wheel, region):
        original = record.brp
        record.brp = wheel.reply
        try:
            return record.scroll_to(1, wheel, "6:00", (120, 300), region, pause=0.0)
        finally:
            record.brp = original

    def test_it_stops_inside_the_panel_rather_than_after_a_fixed_count(self):
        wheel = self.Wheel(927.5)
        x, y = self.run_scroll(wheel, [0, 120, 230, 690])
        self.assertTrue(120 <= y <= 690, y)
        self.assertEqual(wheel.notches, 2, "927.5 -> 807.5 -> 687.5: the second notch is inside the panel")
        self.assertTrue(wheel.cleared, "the wheel's cursor is let go")

    def test_a_widget_already_in_the_region_takes_no_notch(self):
        wheel = self.Wheel(300.0)
        self.run_scroll(wheel, [0, 120, 230, 690])
        self.assertEqual(wheel.notches, 0)

    def test_a_widget_above_the_region_is_scrolled_back_down(self):
        wheel = self.Wheel(6.0)
        _, y = self.run_scroll(wheel, [0, 120, 230, 690])
        self.assertTrue(120 <= y <= 690, y)

    def test_a_widget_that_never_arrives_is_an_error_not_a_click_on_the_bar(self):
        wheel = self.Wheel(927.5, floor=900.0)
        original = record.brp
        record.brp = wheel.reply
        try:
            with self.assertRaises(RuntimeError):
                record.scroll_to(1, wheel, "6:00", (120, 300), [0, 120, 230, 690], max_notches=3, pause=0.0)
        finally:
            record.brp = original
        self.assertTrue(wheel.cleared)

    def test_a_scroll_to_step_is_a_scroll_and_not_a_caption(self):
        self.assertEqual(record.kind_of({"at": 1.0, "scroll_to": "6:00", "over": [120, 300]}), "scroll_to")
