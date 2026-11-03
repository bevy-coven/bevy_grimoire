# 2026-11-02 decide: the clip check's agreement is measured, and it did not earn its authority

Focus: #112 D4, the loop's visual gate · Commits: a9f1414..054fe45 · Machine: big · Outcome: shipped

## Summary
Built the missing caller for `score_set` (`decide_check --calibrate SET.ron`) and measured the clip check
against 122 labelled samples with clef-flash served: **61.5 % agreement, threshold 0.241**. Getting the
number exposed three real defects — a wrong answer key that panicked on sample one, a panic on a missing
answer, and four frames too large for the model's batch.

## Baseline
Gate: green (`a9f1414`). Load 8.0, 150 GB free, 15 GB available RAM. Model served on 127.0.0.1:8093.

## Bugs fixed
### The clip check read the wrong answer key
Must-fix: blocks yes, common yes, no workaround yes — every calibration run died on sample one.
Root cause: `score_set` read `clip::QUESTION_ID` (`"shows"`); `Check::request` writes
`check::QUESTION_ID` (`"check"`). `check::QUESTION_ID` is public now with one copy of the value.
Regression check: `clip::tests::a_scored_sample_reads_the_answer_the_request_asked_for` — failed before,
passes after.

### Two more, found on the way
A missing answer panicked instead of naming the sample and the keys received. Four 960 px frames are
~2 100 tokens and a served clef-flash refuses over its 2 048-token batch; `clip_set.py` now extracts at
480 px (724 tokens for four).

## Enhancement: the clip check's measured agreement
Goal: close #112's first acceptance item with a figure. · Files: `crates/bevy_oracle/src/{clip,check}.rs`,
`src/bin/decide_check.rs`, `tools/decide/clip_set.py`, `docs/decide/checking-captures.md`.
Proof: the calibration run quoted below; `cargo test -p bevy_oracle` all green; clippy clean.

```
clip_shows_its_title   agreement 61.5% on 122 samples, threshold 0.241, model clef-flash
```

**61.5 % is barely better than guessing on a two-sided set**, so the check carries no `calibration` and
stays **advisory** — the issue's own rule applied to a measured figure.

## Review
Reviewer not spawned. Mine: (1) *Fails before?* Yes, the key mismatch test. (2) *Is the failure the claim?*
The panic was the claim; the fix makes the answer readable. (3) *Right layer?* Yes — the id belongs in one
place, and the binary is where the file system and network live. (4) *Protected files?* None. (5) *What
remains?* A sample design with shorter, concrete claims before any threshold is trusted; #113 is separate.

## Test diffs
One test added to `crates/bevy_oracle/src/clip.rs`'s `#[cfg(test)]` block, in the file whose behaviour it
covers. No existing assertion touched.

## Release
None — `bevy_oracle` is at its published version with no packaged-file change.

## Next focus
A clip check that asks about one short, concrete claim, measured the same way.
