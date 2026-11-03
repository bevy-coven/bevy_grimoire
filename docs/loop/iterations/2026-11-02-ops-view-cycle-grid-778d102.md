# 2026-11-02 ops: a view verb is an operation — the grid rung on both front ends (#129, E-002)

Focus: #129 E-002 · Commits: 775db45..778d102 · Machine: big · Outcome: shipped

## Summary
`ops::Ctx` gained `view: Option<View>`, and `view_cycle_grid` is the first operation whose whole effect is
what a person is looking through. The grid key now dispatches it, so key and agent really are one
operation. The unmigrated ratchet falls 91 → 90.

## Baseline
Gate: green (`775db45`). `accept.py`: not run. Load 2.5, 150 GB free, 15 GB available RAM.

## Enhancement: a view verb as an operation
Goal: make `AGENTS.md`'s "both front ends dispatch it" true for verbs that write no project data.
· Files: `ops.rs` (`Ctx::view`, `view_cycle_grid`, parity row), `ops_view.rs` (new), `editor.rs`
(`cycle_grid` dispatches), `keys.rs` (`Action::CycleGrid` maps), plus `view: None` in the Ctx literals.
Proof: `operations_have_two_front_ends` **8/8**; the two parity tests failed before the row and the
dispatch existed. Full editor suite **1153 passed, 0 failed**; clippy clean; gate passed in 191 s.

Design points a reviewer should check:
- **`View` is a value, not a resource**, mirroring `Ctx::open`. That is what lets a test compare a key
  press and an MCP call without either owning a `World`.
- **The rung serialises by name** (`tile`/`fine`/`finer`), so a reordering of `SnapLevel` — which lives in
  the engine-free core — can never silently change a requested or recorded rung. `SnapLevel` itself gained
  no serde; the wire form stays in the crate that owns the wire.
- **`undo()` is `Undo::None`.** Cmd+Z after a grid change must take back the piece placed after it, not
  the rung. A view change is not an edit.

MCP: `view_cycle_grid`, no arguments, answers `{from, to, pitch_m}`. No preview (nothing is written), no
undo entry.

## Borrowed from Mixar
None — a plumbing slice with no user-visible surface.

## Performance
Not measured: no per-frame work changed; `cycle_grid` runs on a key press.

## Acceptance
`reached` not run (no display on big). First failure unchanged.

## Review
Reviewer not spawned. Mine: (1) *Fails before?* The two parity tests failed with the operation and without
the row/dispatch. (2) *Is the failure the claim?* The claim is "both front ends dispatch one operation";
the entry-point and UI-path tests are exactly that. (3) *Right layer?* Yes: the policy lives in the
registry and the parity table, not in a key handler. (4) *Protected files?* `operations_have_two_front_ends.rs`
lost one `UNMIGRATED` row and the ceiling dropped by one — the ratchet's own instruction, made in the
commit that migrates the action. (5) *What remains?* Every other view verb now has a template; `Cancel`
needs a design answer and should be documented rather than left unexplained.

## Test diffs
- `operations_have_two_front_ends.rs`: `Action::CycleGrid` removed from `UNMIGRATED`, `UNMIGRATED_MAX`
  91 → 90. The ratchet's rule is that the list may only shrink *in the commit that migrates one*, which
  this is.
- `view: None` added to `Ctx` literals in five test files: the sanctioned additive empty default.

## Release
None — `bevy_cauldron` is `publish = false`; no packaged crate changed.

## Next focus
`ShowErrors` and `Shortcuts` on the `view_cycle_grid` template; document `Cancel` as staying unmigrated.
