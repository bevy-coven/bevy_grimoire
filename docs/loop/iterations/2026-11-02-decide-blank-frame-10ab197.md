# 2026-11-02 decide: the blank_frame check measured at 100% on 60 samples, and blocked from shipping it

Focus: #112 (D4), then #129 (E-002) · Commits: ef8a26f..10ab197 · Machine: big · Outcome: fixes only

## Summary
Measured `blank_frame` against a labelled set of 60 samples with clef-flash served: **100 % agreement,
threshold 0.475**. Writing that calibration into `fixtures/checks.ron` turns a **protected** assertion red
(`builtin_checks_parse_and_name_the_five`: "none has earned a calibration yet"), so the item stops there per
the rule and the change is the owner's. Then triaged #129 and found its first slice is larger than a pass.

## Baseline
Gate: green (`ef8a26f`). `accept.py`: not run. Load 5.6, 150 GB free, 15 GB available RAM.
Model served on 127.0.0.1:8093.

## Enhancement: the blank-frame check's measured agreement
Goal: close #112's second acceptance item (a calibrated blank check failing on a blank). · Files:
`docs/decide/checking-captures.md`.
Proof, measured 2026-11-02:

```
blank_frame   agreement 100.0% on 60 samples, threshold 0.475
blanks  P(yes) min 0.923, median 0.968
stills  P(yes) max 0.027, median 0.014
```

Set: 30 stills from the changelog's own clips (not blank) and 30 blanks in five shapes (black, magenta,
flat grey, flat white, flat slate). Clean separation, so the gap's midpoint classifies all 60.

**Blocked, correctly.** `tests/checks.rs::builtin_checks_parse_and_name_the_five` asserts
`all(|c| c.calibration.is_none())`. That is an existing assertion, now false in fact; changing it is a re-pin,
which this pass may not do. The calibration is reverted from the tree and the measurement lives in the doc.

## #129 E-002 — triaged, not sliced
92 unmigrated actions; **none** maps to an operation that already exists, so each migration is a new
operation plus schema, plan, undo rule, parity row and UI callsite. The three easy-looking candidates
(`CycleGrid`, `ShowErrors`, `Shortcuts`) are **view** state and `ops::Ctx` has no view section. The decision
between re-classifying view verbs and widening `Ctx` is put on the issue with a recommendation (widen `Ctx`),
because `AGENTS.md` has no exception for view verbs.

## Review
Reviewer not spawned. Mine: (1) *Fails before?* The calibration is a measurement, not a code path; the number
is the proof and it is reproducible from the recorded recipe. (2) *Is the failure the claim?* The claim was
"the check is calibrated"; the measurement shows it separates cleanly. (3) *Right layer?* The block is in a
protected assertion, so the layer is right and the owner is the one to move it. (4) *Protected files?* None
edited; `checks.ron` reverted. (5) *What remains?* One assertion changed and the gate wired — owner work.

## Test diffs
None. `checks.ron` was edited and reverted in full.

## Release
None — nothing packaged changed.

## Next focus
#129 option B: `Ctx` gains a view section, proved by "the same view change by key and by MCP is one operation".
