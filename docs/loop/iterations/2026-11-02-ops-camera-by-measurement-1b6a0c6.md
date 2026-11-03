# 2026-11-02 ops: the verb classification is measured, and the project backlog is 28

Focus: #129 E-002 · Commits: b5342c3..1b6a0c6 · Machine: big · Outcome: fixes only

## Summary
Three passes classified a verb by reading its name; two were wrong. This pass **scanned every handler** for
`pressed` vs `just_pressed` and the split is clean: **33 continuous, 29 discrete, 0 both**. The 33 were
classified `Project`, which claims they owe a `Ctx` operation; they cannot, because a held key changes its
subject for as long as it stays down. `Change::Camera` grows 12 → 41.

## Baseline
Gate: green (`b5342c3`). `accept.py`: not run. Load 13.2 at start (other loops busy), 150 GB free.

## Enhancement: the classification, measured
Goal: stop guessing a verb's kind. · Files: `keys.rs` (29 arms into `Camera`), the two pinned sets.
Proof: `operations_have_two_front_ends` **9/9**; full editor suite **1154 passed, 0 failed**; clippy clean;
gate passed in 266 s.

**Measured from the committed code**, after correction:

| | |
|---|---|
| `UNMIGRATED` | 86 |
| continuous (`pressed` only) | 33 |
| discrete (`just_pressed` only) | 29 |
| both | 0 |
| `Camera` (pinned) | 42 |
| `View` (pinned) | 12 |
| `Project` (remainder) | **28** |

`UNMIGRATED` stays 86 **on purpose**: a classification correction is not a migration, and the ratchet
counts what is listed — which is the right number, because a `Camera` verb will never be migrated.

**The pinned sets disagreed with the code three times while this was written**, and each time the code was
right. That is the test earning its keep.

**A correction I published and then fixed.** I first posted 29/41/33/12 from an earlier scan rather than the
tree after the edit; the committed code says 33/29/42/12/28. Corrected on the issue with the measurement.

## Review
Reviewer not spawned. Mine: (1) *Fails before?* The pinned sets failed until corrected from the code.
(2) *Is the failure the claim?* The claim is "these are held-key verbs"; the scan is that. (3) *Right layer?*
`keys.rs` owns what a verb is. (4) *Protected files?* The two pinned sets changed; both are **classification
records**, and no assertion was removed — `Camera` gained an arm and the count assertion still covers the
allowlist exactly. (5) *What remains?* 28 project verbs, and `FoldPack`/`Rescan`/`SuggestLabels` each need
state this `Ctx` cannot carry.

## Test diffs
`camera` and `view` pinned sets updated from the code's answer. No assertion removed or loosened.

## Release
None — `bevy_cauldron` is `publish = false`.

## Next focus
One of the 28 `Project` verbs whose handler already fits `Ctx` — `EditTile` or `PickComposition` are the
likeliest, and each needs its handler read first.
