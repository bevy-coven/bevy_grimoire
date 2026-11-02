# 2026-11-02 ui: G-33 landed, the pin moved, and the state colours are gooey roles

Focus: the UI series, seeded by G-33's closure · Commits: 0640571..cf09ccf · Machine: big · Outcome: shipped

## Summary
**G-33 closed** while this loop was idle, so the pin moved to gooey `main` and the editor's
interaction-state colours became skin roles. gooey's own `problems()` then caught **two real defects in
the palette** that the old text-parsing check could not see. A second issue (#259's frame budget) was
attempted and **dropped as flaky by construction**, with the measurement recorded.

## Baseline
Gate green at 0640571. `accept.py` not run: no engine behaviour changed. Load 2.9, free disk 155 GB, RAM 13 GB.

## Enhancement: the state colours are gooey roles (#210, skin half)
**Pin**: `5d156cb` (gooey v0.32.0, G-33's `86caae7`); `cargo check` builds with no other change.
**Skin**: all eight new fields set from the constants they replace — `hover`=ROW_HOVER,
`pressed`=ROW_PRESSED, `held`/`held_ground` the two colours a held overlay dims, `held_fade`=0.55,
`unchecked`=`SUGGEST x 1.45`, `card`=OVERLAY_BG, `preview_edge`=EDGE_STRONG.

**Two real palette defects, caught by `problems()` and invisible before:**
| my first mapping | what the crate said | why it was wrong |
|---|---|---|
| `preview_edge = SLOT_BG` | `LowContrast { pair: "preview_edge on surface", ratio: 1.23, needed: 3.0 }` | `SLOT_BG` is a **fill**; a preview edge is an outline |
| `pressed = ROW_SELECTED` | `Indistinguishable { pair: "pressed and hovered", delta_l: 3.94, needed: 4.0 }` | measured by hand: ROW_HOVER L\* 21.09, ROW_SELECTED L\* 25.03 — **3.94, under the bar** |

The second is the important one: the editor's own rule says a pressed row is *recessed below* ROW_BG,
and `ROW_SELECTED` is a selection, not a press. `ROW_PRESSED` is 11.71 from hover.

**Failed before:** the three cases in `tests/the_skin_carries_the_editors_state_colours.rs` fail on the
base with `these state roles are unset … ["hover", "pressed", "held", "held_ground", "unchecked",
"card", "preview_edge"]`; they pass in 0.00 s with the fields set.

**Not done:** `chrome.rs` still declares its 28 colour constants — every one has readers outside that
module (SUGGEST and ACCENT 13 sites each, DIM and DANGER 10). Deleting them is the rest of #210.

## Dropped: #259's frame budget test, as flaky
Built it as the issue asks (5,000 rows behind gooey's virtualised `Table`, three warm frames then ten
timed) and measured three runs back to back: **PASS, 2.05 ms, 3.23 ms**. The spread is the shared
machine, and **#195/E-023 already records a wall-clock test failing this repo's push gate** for the same
reason. Deleted rather than tuned. Recorded on #259 with three ways to build it honestly, and the
recommendation that a **counted** budget (rows built per frame) is the shape to take.

## Test diffs
One new test file plus its `mod` line (owner exception). No existing assertion touched.

## Release
None: `bevy_cauldron` is `publish = false`; no packaged crate changed.

## Next focus
#210's constant deletion; #259 as a counted budget.
