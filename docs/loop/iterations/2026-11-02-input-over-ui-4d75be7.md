# 2026-11-02 input: one answer per frame to "is a panel under the pointer"

Focus: #224 UI-30 · Commits: 6dcfb6e..4d75be7 · Machine: big · Outcome: shipped

## Summary
Pass 267 fixed `drive_note` and recorded six other drivers with the same wrong question. This replaces the
**question** instead of the nine answers: `view::OverUi` is a resource filled once a frame by
`sense_over_ui`, which calls `view::over_ui` — the function that was already correct and that nothing was
calling. Nine sites in six files now read `OverUi.0`.

## Baseline
Gate: green (`6dcfb6e`). `accept.py`: not run. Load 6.3, 150 GB free, 15 GB available.

## Bug fixed: nine drivers asked whether *anything* was hovered
Must-fix: blocks **yes**, common yes, no workaround yes — every map gesture was refused whenever any node
in the window carried `Hovered`.
Root cause: `Query<&Hovered>` and `.any(..)`. `Hovered` is true for a node **or any descendant**, and the two
docks are hovered and **697 px tall**, so the answer was `true` across the window. Five passes (251, 258,
265, 266, 267) chased this as a property of the shell split.
Fix: `OverUi` + `sense_over_ui`. **One copy of the question**, so a tenth driver cannot copy the old idiom.
`panel_under_pointer` (pass 267) is removed — the resource supersedes it.
`OverUi` is `Session` in `screen::OWNERSHIP`: recomputed from the pointer each frame.

Regression check: full editor suite **1155 passed, 0 failed**, `a_shift_click_that_picks_no_piece` among
them; clippy over every target clean; gate 189 s.

## What this does not do
The shell split from #224 is **not** in the tree. Pass 267 measured that with the hover question fixed the
split *still* fails the click test, through a mechanism not reached here, and that the remaining blocker is
**positional**. Unchanged by this commit; the next question is to compare the test's pixel against each
hovered rect.

## Review
Reviewer not spawned. Mine: (1) *Fails before?* The suite failed on the census until `OverUi` was classified,
and the gesture tests are the behavioural check. (2) *Is the failure the claim?* The claim is "one answer per
frame"; nine call sites now read it. (3) *Right layer?* `view` owns where the pointer is; the drivers read
it. (4) *Protected files?* `screen::OWNERSHIP` gained one row — an **addition**, the sanctioned assumed
exception for a new `Resource`. (5) *What remains?* The split's positional blocker.

## Test diffs
One row added to `screen::OWNERSHIP` (`bevy_cauldron::view::OverUi`, `Session`). No assertion removed,
loosened or re-pinned.

## Release
None — `bevy_cauldron` is `publish = false`.

## Next focus
Compare the click test's own pixel against every hovered rect with the split in; name the blocker.
