# 2026-11-02 ui: the hovered nodes are measured — four split hypotheses eliminated

Focus: #224 UI-30 · Commits: c7ed758 (none) · Machine: big · Outcome: nothing shipped

## Summary
Last pass asked *which node carries `Hovered`?* The answer is `bevy_gooey`'s `split_divider` — a
**full-height** node with `Hovered::default()`. Stripping it did not fix the click, which is the fourth
eliminated mechanism. A throwaway probe then listed all 62 hovered nodes with sizes, and the answer is
worth more than the fix: **the two docks are 697 px tall and hovered**, so `over_ui` reads "interface"
anywhere inside them.

## Baseline
Gate: green (`c7ed758`). `accept.py`: not run. Load 4.1, 150 GB free, 15 GB available.
**Base confirmed**: `a_shift_click_that_picks_no_piece` passes on `main` in 0.17 s — my change is what
breaks it, not a pre-existing failure.

## What was eliminated
1. The panes being pickable — `Pickable::IGNORE` added; no change.
2. The viewport laying out at zero width — a width test passes.
3. The panes carrying `Hovered` — none does.
4. **The divider carrying `Hovered`** — it does (`widgets.rs:2212`), it is full height, and stripping it in
   `adopt_the_shell_split` still left the test failing with an empty status line.

## What the probe measured
```
HOVERED NODES: 62
  843v0  size=456x697  at=(242,470)   ← left dock, full height
  1034v0 size=317x697  at=(1362,470)  ← right dock, full height
  1127v0 size=279x43   at=(1358,238)
  879v0  size=418x24   at=(238,374)   ← first of a run of list rows
```
The map occupies `456 < x < 1362`. Both docks are hovered and full-height, which is true on `main` as
well — so they are not the whole answer, but they are where a fifth guess would have gone.

## Why four guesses failed — the finding
Each was a **mechanism** tested by editing `chrome.rs` and paying a two-minute rebuild. None was a
**measurement of the failing predicate**. The probe took one build and answered the question directly.

**The right first move for #224**: assert inside a test the exact value `view::over_ui` returns **at the
pixel the failing test aims at**, with and without the split, then bisect the nodes that make it true.

## Reverted
`chrome.rs` reverted; the probe file and its `all.rs` line removed. Tree clean.

## Review
Reviewer not spawned. Mine: (1) *Fails before?* Confirmed the base passes and my change breaks it.
(2) *Is the failure the claim?* The claim was "this node makes `over_ui` true"; the probe answers that
question directly, which the guesses did not. (3) *Right layer?* The probe is the layer that answers it.
(4) *Protected files?* Nothing shipped changed; the probe was removed. (5) *What remains?* The
`over_ui`-at-the-target assertion, one test, no rebuild loop.

## Test diffs
None in the shipped tree. A throwaway probe was written, run once, and removed.

## Release
None.

## Next focus
Assert `over_ui` at the failing test's own pixel, with and without the split; bisect from there.
