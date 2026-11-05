#!/usr/bin/env python3
"""The prose checker's own arithmetic, tested without a model.

`tools/decide/news_prose.py` asks a decision model whether the What's new page's sentences read as
English (`feedback-1793905698793`'s *"Could we actually use a decision model to run all labels and prose
through"*). What it decides by itself is **which sentences the page shows and what file it hands the
check to**, and both are reads of two files — so they are provable here, exactly as `test_accept.py`
proves the acceptance script's arithmetic without an editor. The asking path is `bevy_oracle`'s, and it is
proven by running it: `python3 tools/decide/news_prose.py` lists the ten sentences, and `--ask` sends them.

usage: python3 -m unittest tools/decide/test_news_prose.py
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import news_prose  # noqa: E402


def a_changelog(*entries):
    """`(id, has_a_sentence)` pairs as changelog lines, newest first, in the shape `launcher::news` reads."""
    lines = ["# Changelog", ""]
    for ident, sentence in entries:
        if sentence:
            lines.append(f"- **{ident}** `abc1234` a title *For you: {sentence}* *No clip: a reason*")
        else:
            lines.append(f"- **{ident}** `abc1234` a title with no sentence")
    return "\n".join(lines) + "\n"


class ThePage(unittest.TestCase):
    def test_only_entries_with_a_sentence_are_the_pages_rows(self):
        # The launcher lists only `for_you` entries, so a checker that judged a title would be judging
        # something no person reads.
        text = a_changelog(("one", "the first sentence"), ("two", None), ("three", "the third sentence"))
        self.assertEqual(
            news_prose.entries(text),
            [("one", "the first sentence"), ("three", "the third sentence")],
        )

    def test_only_the_entries_the_page_shows_are_judged(self):
        # `NEWS_SHOWN` is what the page lists; an older sentence is history, and the page does not show it.
        text = a_changelog(*[(f"entry-{i}", f"sentence {i}") for i in range(news_prose.NEWS_SHOWN + 4)])
        rows = news_prose.entries(text)
        self.assertEqual(len(rows), news_prose.NEWS_SHOWN)
        self.assertEqual(rows[0], ("entry-0", "sentence 0"), "the newest first, as the page orders them")

    def test_a_sentence_ends_where_its_italic_span_does(self):
        # The changelog puts the sentence inside `*…*` and more markup after it; a sentence that swallowed
        # the clip note would be judged on text no person reads.
        text = a_changelog(("one", "the real sentence"))
        rows = news_prose.entries(text)
        self.assertEqual(rows, [("one", "the real sentence")])


class TheCheck(unittest.TestCase):
    def test_the_question_is_one_ron_string(self):
        # RON has no implicit concatenation: a question split across source lines is a parse error, and a
        # parse error means nobody was ever asked.
        ron = news_prose.checks_file()
        self.assertEqual(ron.count('question: "'), 1, f"one question line in:\n{ron}")
        self.assertIn("facts: true", ron, "the check reads facts, since the evidence is sentences")
        self.assertIn("images: []", ron, "and no picture, since there is nothing to look at")

    def test_the_question_is_phrased_declaratively(self):
        # A `noul` head asked an interrogative can answer p≈0 regardless of the evidence, which makes the
        # number meaningless — the harness finding the home-cloud runbook records for laya's `noul` head.
        self.assertFalse(
            news_prose.QUESTION.strip().endswith("?"),
            "the question is a statement the model answers with a probability, not a question",
        )
        self.assertIn("hard for an ordinary person to read", news_prose.QUESTION)


class TheRealChangelog(unittest.TestCase):
    def test_the_shipped_page_has_sentences_and_they_are_all_readable_length(self):
        # A guard on the file this tool reads: a sentence that is a paragraph is as hard to read as a line
        # that is too long, and the measure test in the editor cannot see the words.
        rows = news_prose.entries((news_prose.REPO / "CHANGELOG.md").read_text(encoding="utf-8"))
        self.assertGreaterEqual(len(rows), 5, "the page lists at least five changes with a sentence")
        for ident, sentence in rows:
            self.assertGreater(len(sentence.split()), 5, f"{ident}'s sentence is a fragment: {sentence!r}")
            self.assertLess(len(sentence), 400, f"{ident}'s sentence is a paragraph a person will skip: {len(sentence)} chars")


if __name__ == "__main__":
    unittest.main()
