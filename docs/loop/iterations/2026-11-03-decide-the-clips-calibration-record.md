# 2026-11-03 decide: the clip check's calibration is a recorded file, three demos re-recorded

Focus: #112's first acceptance item, then #224's owed demos · Commits: 8d8f6a4..c9adadc · Machine: big · Outcome: shipped

## Summary
Made #112's calibration a machine-readable record a test reads, and found the figure had already been
measured and written only into prose. Ran the calibration again against the live clef-flash on `:8093`
and reproduced it exactly — 61.5% on 122 samples — so the check stays advisory. Three of the four demos
pass 274 owed are re-recorded; `build-a-map` stays owed with its two real storyboard defects named.

## Baseline
Gate: green at 8d8f6a4. `accept.py`: not run (no display claim this pass). Load 14.6, free disk 149 GB,
available RAM 13 GB.

## Enhancement: the calibration is a file (#112)
Goal: #112's *"measured and recorded (test reads the record)"*.
Files: `crates/bevy_oracle/src/bin/decide_check.rs`, `src/calibrate.rs`,
`fixtures/clip_calibration.ron`, one new test, `docs/decide/checking-captures.md`.
Proof: `tests/the_clips_calibration_is_recorded.rs` — failed first with *"fixtures/clip_calibration.ron:
No such file or directory"*, passes now. `cargo test -p bevy_oracle` all green; clippy clean.

`--out` writes the record; `Calibration` gained `correct`/`wrong` (`#[serde(default)]`, so an existing
`checks.ron` still loads). The record: 122 samples, threshold 0.241, **75 correct / 47 wrong**, 61.5%,
`clef-flash`, 2026-11-03. Below the 0.9 bar, so advisory by the issue's own rule.

**Three bugs fixed on the way, all mine or latent:**
1. The 122 `dir` fields in `clip_samples.ron` read `fixtures/clips/…` while `load_labelled` resolves a
   relative `dir` against the manifest's own directory — every sample looked in `fixtures/fixtures/…` and
   the first refused. Now `clips/…`.
2. The first record had **no counts**, so its agreement could not be checked against anything. Exactly what
   the new test exists to catch.
3. Its `date` read `"+20760 days"` — the runner formatted a day count as a RON duration. Now a civil date.

`fixtures/clips/` (24 MB of derived PNGs) is ignored; `clip_set.py` remakes it in 26 s.

## Demos: 3 of the 4 owed, re-recorded
- **agent-removes-then-undo**, **review-labels**: PASS with no storyboard change. Their pass-274 failures
  were the *shared editor*, not the storyboards — the README's own rule.
- **agent-drives**: framed on the bed (`[-2.9,-1.9,-0.3,0.4]`; the bed is at (-1.7,-1.1)) and cropped to the
  action band `[440,300,980,690]`, because the frame's top 40% is black floor that diluted the change to
  3.38%. 4.41% then PASS; the sheet still shows the whole story.

**Owed: `build-a-map`.** Its place click and its remove click were the **same pixel `[800,494]`, outside the
viewport**, so the sofa was never placed, and its `Ctrl+Z` carried no modifier. Fixed, the demo reads
correctly but its place moment measures **4.73%** against the 5% floor; four attempts did not close the last
0.27%, so the storyboard is reverted rather than shipped half-right.

## Review
Reviewer not spawned; self-review, mine:
- **Is the record reproducible?** Yes: the command and the endpoint are in the docs, and the run reproduced
  the prose figure to the tenth of a percent.
- **Does the test have teeth?** It caught two real defects in the first record (no counts, a duration where
  a date belongs), so it is not a rubber stamp.
- **Did I weaken a check?** No: `check.py` and `check.py`'s policy are untouched; the new test only refuses a
  record that contradicts itself.
- **Are the demos true?** Three re-recorded and looked at, `Captured` moved to `19598ac`; `build-a-map` keeps
  its old GIF and old caption.
- **Is the live model call allowed?** A calibration run is permitted; a test never calls one, and this one
  reads a file.

## Test diffs
One new test file. Two existing literals gained `correct`/`wrong` with `#[serde(default)]` on the struct —
an addition under the schema-field exception, marked in both.

## Release
**bevy_grimoire_anim v0.1.7** — https://crates.io/crates/bevy_grimoire_anim/0.1.7 — API unchanged (202
semver checks pass) but its published `Cargo.toml` now requires `bevy_grimoire_core` 0.7.0, a requirement
pass 272's core release left unreleased. `bevy_oracle` is `publish = false`.

## Next focus
`build-a-map`'s place moment: a storyboard that places a larger piece, not a looser check.
