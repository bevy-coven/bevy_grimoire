# 2026-11-03 palette: a click on a row arms it first time, because a blur no longer rebuilds the list

Focus: the editor's UI · Commits: 5bc2765..e0b6455 · Machine: big · Outcome: shipped

## Summary

**#278 is fixed.** Its product half was the last pass's open question and it is now measured and closed:
`on_row_click` blurs the filter after arming, `blur` writes `Filters`, and `rebuild_palette` watched
`resource_changed::<Filters>` — so the click that armed a row also despawned it. The gate now watches
what the lists are *narrowed to*, not the whole resource. First tap after typing: **0 rebuilds, arms the
piece** (was 1 rebuild, armed nothing).

## Baseline

Gate: green before (5bc2765). `accept.py`: not run — no solver path touched. Load 6.3, free disk 121 GB,
available RAM 13 GB.

## Bugs fixed

### #278: the first click on a palette row after typing is swallowed

Must-fix: blocks y (no palette row could be clicked reliably), common y (every filtered search ends in
one), no workaround y (the last pass's double-tap in the storyboard was a workaround, now removed).

Root cause: `Filters` is one resource holding four texts **and** which box has the keyboard.
`on_row_click` blurs the box after arming — it must, or `drive_place`, which is gated on `not_typing`,
never fires — and `blur()` writes `Filters`. `rebuild_palette` was gated on
`resource_changed::<crate::filter::Filters>`, so that write tore down every row and built new ones,
including the row the click was about.

Measured on a running editor, `rebuild_palette` instrumented with a rebuild counter:

| | rebuilds | arms |
|---|---|---|
| first tap after typing | **1** | nothing |
| second tap, same pixel | **0** | the piece |

The earlier candidates were all ruled out on the way and are recorded on the issue: the press is
delivered (2 messages seen), the observer fires (instrumented at entry), and the split of a `Tap` into
press and release across frames does not matter.

**Fix.** `filter::FilterTexts` is the four texts; `editor::the_filter_text_changed` compares the current
texts to the last it saw and returns false when only the focus moved. `arm_palette_rows`, which exists
precisely to paint a selection without rebuilding, now does its job on a click.

Regression check: `a_row_click_while_filtering_arms_it::blurring_the_filter_leaves_the_palette_rows_standing`
— asserts every `PaletteRow` entity survives a blur. **Fails on the old gate** (the entity ids change)
and passes on this one; the two click tests beside it still pass.

Proved end to end as well: the `build-a-map` take ends `armed_piece: sofa`, `placements: 11`, with **one**
tap where it needed two, and `check.py` passes.

## Enhancement

none.

## Borrowed from Mixar

none.

## Performance

`perf.py` not measured (it needs a live editor on a real display; this machine times clips only).
The change removes a palette rebuild from every click that moves focus, so it cannot be slower.

## Acceptance

`reached` unchanged — no solver path touched.

## Review

Reviewer could not run (no subagent spawned inside the budget). Five questions, mine:

1. **Does it fix the reported behaviour?** Yes, measured both ways — 1 rebuild/arms nothing before,
   0 rebuilds/arms after — and the real take does its whole demo with one tap.
2. **Can a `Filters` text change still reach the list?** Yes: `the_filter_text_changed` compares the
   four texts, so typing (which changes one) still rebuilds, and only focus moves are ignored.
3. **Does the selection still repaint?** Yes — `arm_palette_rows` runs off `EditorState`, which was
   never the gate, and the take shows the row highlighted.
4. **Is the workaround gone?** Yes: the storyboard's second tap is deleted, and the take passes
   without it.
5. **Did anything else rely on a focus write rebuilding?** Searched: `blur_on_world_click` and
   `on_click` are the only other writers, and neither expects the rows to be respawned.

## Test diffs

- `crates/bevy_cauldron/tests/a_row_click_while_filtering_arms_it.rs`: one test replaced
  (`blurring_the_filter_does_not_ask_the_palette_to_rebuild` → `blurring_the_filter_leaves_the_palette_rows_standing`)
  because the first asserted the helper's output and **passed with the fix reverted** — it could not
  fail. The replacement asserts row identity and does fail. No other assertion changed.

## Release

None: `bevy_cauldron` is `publish = false`; no publishable crate's files changed.

## Next focus

#278 closed; every other open issue waits on #210's owner re-pin (#224 → the `ui` series), on
bevy-elements#55 (#193), or is `wontfix` (#197). Next pass: the `ui` series' first ready issue, or E-002.
