# 2026-11-03 demo: build-a-map shows the sofa armed and undone again, and #278's cause is narrowed

Focus: the editor's demos · Commits: e030f05..HEAD · Machine: big · Outcome: fixes only

## Summary

`build-a-map` had been showing a demo that could not work: it clicked a guessed pixel 18 px off the
filtered row (arming nothing) and left the editor in remove mode, so the take placed a lamp and its
caption said sofa. It now clicks the row by label, taps twice, and leaves remove mode; the take ends
with the sofa armed and **11 placements** and `check.py` passes.

**#278's product bug is narrowed but not fixed.** The first tap on a palette row after a filter write
is lost; the second arms it. The palette *respawns its rows* between the two — measured entity id
`12884899586` before and `12884899514` after — so the observer is handed a row that no longer exists.

## Baseline

Gate: green at e030f05. `accept.py`: not run (no solver path touched). Load 8.4, free disk 121 GB,
available RAM 10 GB.

## Bugs fixed

### #278 (storyboard half): build-a-map armed nothing and never left remove mode

Must-fix: blocks n, common n, no workaround n — but it is the reason a standing demo lied, which the
owner's rule treats as a bug in the docs.

Root cause, three parts, each measured on a running editor:

1. **The click was a stale pixel.** `{"at": 1.2, "click": [1100, 233]}`; the filtered row sits at
   `(1118.5, 239)`. Both clicked in one session: `(1100,233) -> armed=lamp_tall`,
   `(1118,239) -> armed=sofa`.
2. **It never left remove mode.** `X` arms the remove tool and the storyboard ended there, so the
   editor a take left behind read `tool: Remove`; pressing `X` returned it to `Place` and the next row
   click armed the sofa.
3. **The first tap after a filter write is lost** — #278 proper, worked around in the storyboard by
   tapping twice.

Regression check: the take itself, which is the demo's own proof — `grimoire/state` after it reads
`armed_piece: sofa`, `placements: 11`, and `tools/demo/check.py` passes (longest static run 1.10 s,
every change moment over 5%).

### #278 (product half): NOT fixed

Measured and ruled out, so the next pass does not repeat them:

| tried | result |
|---|---|
| reorder `on_row_click` to arm before the blur | take still ended `lamp_tall` / 12 placements — reverted |
| deliver the move one frame before the press in `inject_clicks` | same — reverted |
| instrument `inject_clicks` | **2 press messages seen** on the failing tap, so the press is delivered |
| instrument `on_row_click` | it **does** fire, and is handed an entity that is not a live `PaletteRow` |
| row entity across one tap | `12884899586` → `12884899514`: **the palette respawned its rows** |

What is left: why the respawn happens on that frame. Next pass: log `rebuild_palette`'s entry with the
`Filters` generation on the frames either side of a failing tap.

## Enhancement

none.

## Borrowed from Mixar

none.

## Performance

`perf.py` not measured — a storyboard and a GIF changed; no frame path.

## Acceptance

`reached` unchanged (no solver path touched).

## Review

Reviewer could not run (no subagent spawned inside the budget). Five questions, mine:

1. **Does the demo now show what its caption says?** Yes — measured, not inferred: 11 placements and
   `armed_piece: sofa` after the take, and a frame checked by eye.
2. **Is the double tap hiding a real bug?** It *is* a workaround and says so in the commit body and on
   #278; it is what a person does when a click does not take.
3. **Could the double tap arm then disarm?** No: `arm_piece` refuses a re-arm of the piece already
   held, so the second tap is a no-op once the first lands.
4. **Is `(1118.5,239)` stable?** It is resolved by label at take time, not written down, so a later
   layout change moves the click with the row.
5. **Is anything left uncommitted?** No — the tree is clean after each revert.

## Test diffs

none.

## Release

None: no publishable crate's files changed.

## Next focus

#278's product half, with the two instruments above; everything else is blocked on #210's owner re-pin
or bevy-elements#55.
