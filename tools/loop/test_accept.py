#!/usr/bin/env python3
"""The acceptance script's arithmetic, tested without an editor.

`reached`, `of` and `first_failure` are what the loop grades itself on, so they get a test that does
not need a build. Everything else in accept.py is I/O against a live editor and is proven by running
it — a mock of the editor would only prove the mock.

usage: python3 -m unittest tools/loop/test_accept.py
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import ast  # noqa: E402

import accept  # noqa: E402
from accept import summarize, steps_ok, _skip  # noqa: E402


def step(step_id, status):
    return {"id": step_id, "status": status, "ms": None, "detail": ""}


class Summarize(unittest.TestCase):
    def test_every_step_passing_is_a_full_reached(self):
        steps = [step("tools", "pass"), step("make_map", "pass"), step("open_map", "pass")]
        self.assertEqual(summarize(steps), {"reached": 3, "of": 3, "first_failure": None})

    def test_a_skip_is_not_a_pass_and_not_a_failure(self):
        # The distinction the whole report rests on: a step nobody could attempt has not been shown
        # to be broken, and counting it as broken blames the wrong thing.
        steps = [step("tools", "pass"), step("place", "skip"), step("accept", "skip")]
        self.assertEqual(summarize(steps), {"reached": 1, "of": 3, "first_failure": None})

    def test_the_first_failure_is_the_first_in_the_order_given(self):
        steps = [step("tools", "pass"), step("make_map", "fail"), step("place", "fail")]
        self.assertEqual(summarize(steps)["first_failure"], "make_map")

    def test_a_skip_ahead_of_a_failure_does_not_become_the_first_failure(self):
        steps = [step("tools", "pass"), step("open_map", "skip"), step("place", "fail")]
        self.assertEqual(summarize(steps)["first_failure"], "place")

    def test_an_empty_list_is_zero_of_zero_rather_than_an_error(self):
        self.assertEqual(summarize([]), {"reached": 0, "of": 0, "first_failure": None})


class Skips(unittest.TestCase):
    def test_a_skip_names_the_step_it_waited_for(self):
        steps = []
        _skip(steps, "place", "open_map")
        self.assertEqual(steps[0]["status"], "skip")
        self.assertIn("open_map", steps[0]["detail"])

    def test_a_dependency_that_skipped_is_not_a_pass(self):
        # Otherwise a skip would satisfy its dependents, and one early failure would leave the rest
        # of the report claiming they passed.
        steps = [step("open_map", "skip")]
        self.assertFalse(steps_ok(steps, "open_map"))

    def test_a_missing_dependency_is_not_a_pass(self):
        self.assertFalse(steps_ok([], "open_map"))


if __name__ == "__main__":
    unittest.main()


class TheImportStep(unittest.TestCase):
    """**`--glb` adds an eleventh step and moves `of`.**

    The count is the loop's headline denominator, so a run with a mesh and a run without must be
    told apart rather than both reporting "of 10".
    """

    def test_ten_steps_without_a_mesh_eleven_with_one(self):
        without = [step(i, "pass") for i in ("tools", "make_map", "save")]
        with_import = without + [step("import", "pass")]
        self.assertEqual(summarize(without)["of"], 3)
        self.assertEqual(summarize(with_import)["of"], 4)
        self.assertEqual(summarize(with_import)["reached"], 4)

    def test_a_failed_import_is_the_first_failure(self):
        steps = [step("tools", "pass"), step("import", "fail")]
        self.assertEqual(summarize(steps)["first_failure"], "import")


class TheOpenMapStep(unittest.TestCase):
    """**The step that measures the product carries no workaround (E-078).**

    `open_map` used to relaunch the editor on the map and pass on that, recording `workaround` in the
    row. A row with a workaround is a row that says the agent could not do it — so once `map_open`
    enters the editor, the honest record has no such field at all, and this pins that: a relaunch
    creeping back in would show up as the key returning rather than as a passing step.
    """

    def step_code(self):
        """**`step_open_map`'s executable body**, read from the script.

        The first version of these tests built a row with `_record` and asserted the dict it had just
        built, which cannot fail for the regression it names: putting `workaround="relaunched the
        editor on the map"` back at the real `_record` call left it green. A test about a script has
        to read the script.

        **Code and not the whole function**, because the docstring records this step's history on
        purpose — it says in words that the relaunch is gone, and a test that grepped the prose would
        fail on the explanation of the fix. The docstring is dropped by taking the body without its
        first statement, which is what a docstring is.
        """
        tree = ast.parse(Path(accept.__file__).read_text())
        run = next(n for n in tree.body
                   if isinstance(n, ast.ClassDef) and n.name == "Run")
        step = next(n for n in run.body
                    if isinstance(n, ast.FunctionDef) and n.name == "step_open_map")
        body = step.body
        if body and isinstance(body[0], ast.Expr) and isinstance(body[0].value, ast.Constant):
            body = body[1:]
        return "\n".join(ast.get_source_segment(Path(accept.__file__).read_text(), n) or ""
                          for n in body)

    def test_the_open_map_step_records_no_workaround(self):
        self.assertNotIn("workaround", self.step_code(),
                         "the open_map step must pass on the product, not on a workaround")

    def test_the_open_map_step_starts_no_editor(self):
        # The relaunch is the workaround: a second `Editor(...)` here is a second process, a second
        # client and a leaked process per failed run.
        body = self.step_code()
        self.assertNotIn("Editor(", body, "the open_map step must not relaunch the editor")
        self.assertNotIn("relaunch", body, "nor describe one as what it does")

    def test_the_open_map_step_waits_for_the_screen_and_not_only_the_map(self):
        # **The map's name is not evidence the editor was entered** — a build with the defect this
        # step catches reports `map.name` on the menu. The screen is what tells them apart.
        body = self.step_code()
        self.assertIn('s.get("screen") == "editor"', body,
                      "the step must wait for the screen, or it passes on a map loaded on the menu")
