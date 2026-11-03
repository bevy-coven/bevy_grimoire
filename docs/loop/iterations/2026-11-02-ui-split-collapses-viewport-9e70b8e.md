# 2026-11-02 ui: the split's positional blocker is measured — its panes are 0×0

Focus: #224 UI-30 · Commits: 9e70b8e (none) · Machine: big · Outcome: nothing shipped

## Summary
Last pass asked for the click test's pixel to be compared against every hovered rect. Done, in three tree
states, with one probe. **The "positional blocker" is not the rects: the split's panes are 0×0 until a region
is parented into them, and a zero-size viewport makes `world_to_viewport` project off the window.**

## Baseline
Gate: green (`9e70b8e`). `accept.py`: not run. Load 6.2, 150 GB free, 15 GB available.

## The measurement

| tree | pixel for map (12,12) | viewport | `OverUi` | rects covering the pixel |
|---|---|---|---|---|
| **no split** (`main`) | (705, 621) | — | false | **0** |
| split, nothing parented | (640, **825**) | **0×0** | false | 0 |
| split, regions adopted into panes | (595, **718.6**) | **724×582** at (595,398) | false | 0 |

**Why every earlier attempt failed:** with a 0×0 viewport, `world_to_viewport` returns y=825 on a 720-px
window. The test's own `pixel_over` then aims off-screen and no click can land. **Not** picking, not
`Hovered`, not the divider — all three were real bugs, all three are fixed, and none of them was this one.

**What remains, with the number that names it:** with the regions adopted the viewport is 724×582 and the
target pixel is **1.4 px from the window's bottom edge** — a camera framing a 32 m map should not produce
that. So the pane places the viewport, but the camera is being fitted to something other than the pane.
The next pass compares `surface::fit_viewport`'s rect against the pane's rect with the split in.

## Reverted
`chrome.rs` reverted; the probe file and its `all.rs` line removed. Tree clean.

## Review
Reviewer not spawned. Mine: (1) *Fails before?* Three tree states measured with one probe. (2) *Is the failure
the claim?* The claim is "the blocker is positional"; the viewport's own size is the measurement. (3) *Right
layer?* `surface::fit_viewport` is the named next layer. (4) *Protected files?* Nothing shipped changed.
(5) *What remains?* The camera's fit against the pane's rect.

## Test diffs
None in the shipped tree. A throwaway probe was written, run twice, and removed.

## Release
None.

## Next focus
Compare `surface::fit_viewport`'s rect against the adopted pane's rect with the split in.
