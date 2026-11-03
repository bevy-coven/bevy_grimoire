# 2026-11-03 decide: the blank-frame acceptance and all nine demos fresh

Focus: #112 closed, #224's last owed demo · Commits: 5f27879..6a71ded · Machine: big · Outcome: shipped

## Summary
Finished #112's second and third acceptance items with a replayed model and two real fixture frames —
proved on the command as well as in the test — and closed the issue. Re-recorded `build-a-map` from the
two storyboard defects pass 275 named, which makes **all nine standing demos fresh** for the first time
in several passes.

## Baseline
Gate: green at 5f27879. `accept.py`: not run (no behaviour claim this pass). Load 8.8, free disk 148 GB,
available RAM 13 GB.

## #112 closed: the three acceptance items
- **Agreement recorded, test reads it** (43f348e, pass 275): `fixtures/clip_calibration.ron`, 61.5% on 122
  samples, threshold 0.241, 75 correct / 47 wrong. Below the 0.9 bar, so advisory by the issue's own rule.
- **A calibrated blank-frame check fails on a blank frame and passes on a good capture** (c1460ce):
  `tests/a_calibrated_blank_frame_check_fails_the_run.rs` with `fixtures/blank/view.png` (one flat colour)
  and `fixtures/blank/good.png` (structure at the same size), a replay transport that never calls a model.
  **Not vacuous, proved:** removing the calibration turns it red with *"a calibrated blank_frame at 0.97 on
  a blank frame must fail the run: []"*.
- **An uncalibrated check never fails** (the same file's second case, plus the existing authority test).
- **Proved on the command too**, against the served clef-flash on `:8093`:
  `FOUND blank_frame: p(yes) 0.96 vs 0.70 (blocking)` → `EXIT=1`; `ok … 0.01 … (advisory)` → `EXIT=0`.

Wiring a calibrated check into the gate stays the owner's evaluator pass: `build-loop.md` and
`tools/changelog/**` are protected, so the loop cannot add a check to its own gate.

## Demos: all nine fresh
`build-a-map` re-recorded from the two defects pass 275 named: its place and remove clicks were the **same
pixel `[800,494]`, outside the viewport**, and its `Ctrl+Z` carried no modifier. With both fixed the demo
reads correctly end to end; it then needed the same crop lesson `agent_drives` needed — frame the patch the
piece goes in and crop out the palette column and empty floor — which took it from 4.39% to PASS.
`python3 tools/demo/stale.py` reports **nine fresh**, no stale line. `Captured` moved to `19598ac`.

## Review
Reviewer not spawned; self-review, mine:
- **Is the blank-frame test vacuous?** Checked by removing the calibration: it goes red, so the assertion is
  load-bearing.
- **Did I weaken any check?** No: `check.py`, `accept.py` and the thresholds are untouched; the new fixtures
  and test are additions.
- **Is the demo true?** The sheet was looked at: armed, turned, placed, red, removed, back — the caption in
  order.
- **Does closing #112 overclaim?** It records what the acceptance asked and says plainly that gate wiring is
  the owner's, and that the clip check's 61.5% leaves a next step.
- **Anything left half-done?** No: the tree is clean, every demo is fresh, and #224's item 2 (the resized-docks
  clip) remains blocked on the injected drag, already recorded there.

## Test diffs
One new test file and two fixture PNGs. No existing assertion changed.

## Release
None: `bevy_oracle` is `publish = false`, and no packaged crate changed this pass.

## Next focus
The clip check's short-claim measurement; #224's item 2 still waits on the injected drag.
