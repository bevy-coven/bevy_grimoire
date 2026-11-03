# 2026-11-03 demos: two clips that could not be honestly recorded, and the bug behind one

Focus: owed demo clips · Commits: 68da3cd..HEAD · Machine: big · Outcome: nothing shipped

## Summary

The pass set out to record the clips #193 had been blocking. It recorded none, because neither of the
two it tried showed what its changelog line claims — and the second attempt, done properly on a build
that actually renders the preview, found a real defect and filed it (#282) after withdrawing a wrong
report of my own (#281) the same pass.

## Baseline

Gate: green before (`68da3cd`). `stale.py`: every standing demo fresh. No code changed, so no gate ran.

## What was tried, and why it did not ship

### `98-preview` — cannot be recorded in this repository at all

Its storyboard frames `placements: ["abbey", "well"]`. This repository ships **one** map, `bedroom`, an
interior, and `grep -c 'abbey\|well' assets/grimoire/maps/bedroom.map.ron` is `0`. The clip needs either a
re-aimed storyboard or an outdoor map. The second is the better fix: a sky preview demo wants a sky.

### `98-clock-scrub` — recorded, measured, and the sky does not move

`clock_scrub.json` recorded 48 frames cleanly on `bedroom` with the preview demonstrably working (editor
built `--features bevy_elements`, `family.ron` declaring the crate, whole-viewport mean RGB
`(127,137,153)` against `(36,33,28)` from a default build). Sampling the sky region at the storyboard's
three hour clicks:

| t | storyboard clicks | sky mean RGB |
|---|---|---|
| 0.6 | noon (defaults) | (128,157,194) |
| 1.9 | 6:00 | (128,157,194) |
| 4.4 | 18:30 | (129,157,194) |
| 6.3 | 23:00 | (129,158,194) |

Whole-frame means are `(92,96,100)` at both 6:00 and 23:00. The status line does report the change
(`LAST bevy_elements clock: 6.0 / 18.5 / 23.0`, `UNDO nothing` throughout — the other half of the line's
claim, and it holds). The line's own words are *"the clock scrubber holds the preview at an hour of day"*;
a clip whose sky does not move does not show that, so it keeps its `not yet recorded` reason.

## Bugs filed

### #282 — the clock scrubber updates LAST but the sky does not change
Two candidate causes named on the issue: the storyboard's fixed-pixel clicks `(60,531)`, `(170,531)`,
`(225,531)` may be landing on a readout rather than the scrubber (written against an older panel layout),
or the value reaches the clock and the crate does not re-drive the sky while the clock is paused. The
first is likelier and cheaper to rule out, and the issue says how: aim the click by label or by the
editor's reported position, then measure again. If the sky moves, it is a stale storyboard and the issue
closes.

## #281 — filed and withdrawn the same pass

I filed #281 claiming the same symptom, on a measurement taken in an editor that **was not rendering the
preview at all**: the binary was a default-feature build, and the editor's own banner said *"the editor
was built without the `bevy_elements` feature; rebuild with `--features bevy_elements`"*, beside an
earlier one reading *"bevy_elements is not listed in family.ron"*. My discriminator (compare dressed
against undressed) was sound and I read its answer backwards: identical pixels both times meant *never
engaged*, which is the opposite of what I reported. Withdrawn with the measurement that killed it, and
closed not-planned before anyone acted on it; #282 is the re-take on a working preview.

## Test diffs

None.

## Review

Reviewer could not run. Five questions, mine: (1) did I ship a clip that does not show its claim — no, and
that is this pass's whole point; (2) was the withdrawn issue's flaw my measurement or my reasoning — my
reading of a sound measurement, so I re-took it rather than patching the conclusion; (3) is #282's evidence
taken on a live preview — yes, with the build and the `family.ron` named; (4) did I check whether the
editor was warning me before reporting — not at first, which is the mistake that cost this pass its first
hour; (5) what remains — `98-preview` needs an outdoor map or a re-aimed storyboard, and #282 needs the
label-aimed click test.

## Release

None: nothing packaged changed.

## Next focus

Aim `clock_scrub.json`'s click at the scrubber by label and re-measure. If the sky moves, the clip ships
and #282 closes; if not, the crate-side cause is worth a `bevy_elements` issue.
