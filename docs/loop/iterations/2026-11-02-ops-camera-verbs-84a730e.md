# 2026-11-02 ops: twelve verbs are camera, not candidates for an operation (#129, E-002)

Focus: #129 E-002 · Commits: ec0a153..84a730e · Machine: big · Outcome: shipped

## Summary
Pass 257 put `FrameView`, the four pans and the orbit keys in `View`, as candidates for a `Ctx`
operation. The code says they are not: `grimoire/frame_view`'s own doc reads *"it is not an operation, it
is not in the registry or the operation log, and it is not undoable"*. `Change::Camera` is the fifth kind
and twelve verbs moved there. **Queue: 57 project, 16 view, 12 camera, 1 held, 3 modal.**

## Baseline
Gate: green (`ec0a153`). `accept.py`: not run. Load 6.8, 150 GB free, 15 GB available.

## Enhancement: the camera kind
Goal: stop listing as work six verbs an agent can already drive. · Files: `keys.rs` (`Change::Camera`),
`operations_have_two_front_ends.rs` (the pinned camera set), `ops_view.rs` (`filter_focus`).
Proof: `the_unmigrated_list_says_which_verbs_can_never_be_operations`; full suite **1154 passed, 0 failed**;
clippy clean; gate passed in 190 s.

**What I set out to do and did not finish.** Last pass promised six view verbs in one pass. Four of the
six turned out to be camera, so the cluster shrank to `FocusFilter`/`FocusTagFilter` — and those move
keyboard focus to a box and *clear its text*, which is a front end's own focus rather than a `Ctx` value.
I left them and said why instead of shipping two thin operations to keep a promise. The `View` field
`filter_focus` is added and unused by an operation; it is the state a real one would need, and it is
recorded here rather than deleted so the next pass starts from it.

**The pinned test caught four errors of mine this pass**: order in two lists, a `PlayPause` left in both
arms, and a dropped closing bracket. Each was a real disagreement with the code.

## Review
Reviewer not spawned. Mine: (1) *Fails before?* The pinned sets disagree with the code until fixed, four
times measured. (2) *Is the failure the claim?* The claim is "these are camera, not work", and the
`frame_view` doc is the source. (3) *Right layer?* `keys.rs` owns what a verb is. (4) *Protected files?*
Added only. (5) *What remains?* The 57 project verbs, and a check of the 16 view verbs for the same trap.

## Test diffs
The classification test gained a `Camera` assertion; nothing was removed or loosened.

## Release
None — `bevy_cauldron` is `publish = false`.

## Next focus
Check the 16 `View` verbs for the camera/focus trap before migrating any of them.
