# 2026-11-02 ops: the allowlist says which verbs can never be operations (#129, E-002)

Focus: #129 E-002 · Commits: 5955a17..85d6b22 · Machine: big · Outcome: shipped

## Summary
`Action::change()` classifies every verb as `Project`, `View`, `Held` or `Modal`, and a test pins the four
sets. The ratchet has shrunk one action per commit, which made 89 entries read as 89 unfinished
migrations; **57 of them write the grimoire and owe an operation, 30 are view candidates, and 4 will never
be operations.**

## Baseline
Gate: green (`5955a17`). `accept.py`: not run. Load low, 150 GB free, ~15 GB available.

## Enhancement: the verb classification
Goal: turn a count into a to-do list a pass can work through. · Files: `keys.rs` (`Change`, `Action::change`),
`operations_have_two_front_ends.rs` (the pinned test).
Proof: `the_unmigrated_list_says_which_verbs_can_never_be_operations`.

Measured from the code, after the test's corrections: **89** listed — **57** `Project`, **30** `View`,
**1** `Held` (`Shortcuts`), **3** `Modal` (`Cancel`, `TypeId`, `Accept`).

**The test caught three of my own errors, which is why it is worth having:**
1. The first version summed the four kinds and compared the total to `UNMIGRATED.len()` — true whatever
   bucket an action lands in. Measured: moving `RedoBuild` (unmigrated, writes the grimoire) to `View`
   passed. The sets are named now; re-measured, moving `Undo` to `View` fails.
2. `Accept`/`Cancel`/`TypeId` are not in alphabetical order in `Action::ALL`.
3. Ten `Compose*`/`EditTile` verbs I had listed as `View` are **already migrated** and never reach the
   match, so their arms were dead.

**Also corrected on the issue:** I first reported 58/28/90 from memory; the code says 57/30/89.

## Review
Reviewer not spawned. Mine: (1) *Fails before?* The negative measurement above. (2) *Is the failure the
claim?* The claim is "these verbs cannot be operations"; the pinned lists are that. (3) *Right layer?*
`keys.rs` owns what a verb is; the test enforces it. (4) *Protected files?* A new test was **added**; no
existing assertion changed. (5) *What remains?* The 30 view candidates, four arrows at a time.

## Test diffs
One new test in `operations_have_two_front_ends.rs`. Nothing existing was removed or loosened.

## Release
None — `bevy_cauldron` is `publish = false`.

## Next focus
`PanForward`/`PanBack`/`PanLeft`/`PanRight` onto one view operation, plus `FocusFilter`/`FocusTagFilter`.
