# 2026-11-02 loop: the LFS hook test made its own premise false; the shell slice was withdrawn

Focus: the gate's LFS protection, then #224 · Commits: 5cf181f..7ab3527 · Machine: big · Outcome: fixes only

## Summary
Fixed the red check on `main`: `test_pre_push_lfs.py` deleted `.git/lfs/objects` to make an upload fail, but
git-lfs re-hashes the working-tree file and uploads that, so the push it expected to fail succeeded. Then
took #224's shell slice — a real gooey `Split` around the left dock and viewport — which passed its own test
and was withdrawn when the editor suite caught it breaking the viewport's picking surface.

## Baseline
Gate: green (`05f73d3`). `accept.py`: not run. Load 6.9, 150 GB free, 13 GB available RAM.

## Bugs fixed
### B-251: the LFS hook test proved nothing, and main carried a red check
Must-fix: blocks yes, common no, no workaround yes — the gate's own LFS protection was unproven.
Root cause: `git lfs pre-push` recovers a missing object from the working-tree file (measured on a bare
remote: deleting `.git/lfs/objects` still uploaded 1/1). The hook was always correct; the fixture made its
own premise false. Repaired by pointing `lfs.url` at an endpoint that cannot answer — which git-lfs reports
and the hook turns into `REFUSED: git lfs pre-push failed`. Every assertion unchanged.
Regression check: `python3 tools/loop/test_pre_push_lfs.py` — failed before (`0 == 0 : a push whose clip
could not be uploaded went through`), 2 tests OK after.

## Enhancement: the shell split — withdrawn
Built and passing its own test (`the_divider_is_a_gooey_split_between_the_dock_and_the_viewport`; failed
before at `a78bb21` with "found 0", passed after). The full editor suite then caught it: **1152 passed, 1
failed** in `a_shift_click_that_picks_no_piece_says_what_to_do_instead` — the map stopped receiving the
click. That test passes at `a78bb21`. Two repairs (restoring every field of `split`'s own `Node`; giving
the split `flex_grow` alone) failed identically, so per the stop-on-repeated-failure rule the slice was
dropped rather than left in the tree: `chrome.rs` and its test are restored byte-for-byte to `a78bb21`, and
the commit pair never reached `main`.

## Review
Reviewer not spawned (subagent unavailable this pass). Mine, answered: (1) *Does the test fail before?* Yes,
quoted above. (2) *Is the failure the claim?* The split test asserted structure; the suite asserted the
viewport still takes a click — it does not, which is the real claim. (3) *Is the fix in the right layer?* Yes,
and the lesson is that the dock migration must keep `ViewportSlot` pickable. (4) *Any protected file touched?*
No. (5) *What is left?* The viewport's picking surface across the reparent, then the clip.

## Test diffs
None on `main`: the new test and its source change were withdrawn together.

## Release
None — only `tools/loop/test_pre_push_lfs.py` changed, which ships nothing.

## Next focus
Re-do #224's split so the viewport pane still receives pointer hits, and prove it with a click.
