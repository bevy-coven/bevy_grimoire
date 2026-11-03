# 2026-11-02 ops: the journal is an operation — the second view verb (#129, E-002)

Focus: #129 E-002 · Commits: ca9829a..6214122 · Machine: big · Outcome: shipped

## Summary
`Cmd+E` now dispatches `view_toggle_journal`, so the key and an agent toggle the same panel. The ratchet
falls 90 → 89. Two parity scans that named the UI as four files were reporting a live entry point as
missing — the same false failure that test file already recorded for `piece_import`.

## Baseline
Gate: green (`ca9829a`). `accept.py`: not run. Load low, 150 GB free, ~15 GB available RAM.

## Enhancement: the journal as an operation
Goal: put the second view verb on the template pass 255 proved. · Files: `ops_view.rs` (`journal_open`,
`toggle_journal`), `ops.rs` (`view_toggle_journal`, parity row), `notice.rs` (dispatch), `keys.rs` (mapping).
Proof: `operations_have_two_front_ends` **8/8**; full editor suite **1153 passed, 0 failed**; clippy clean;
gate passed in 189 s.

Design points:
- `writes()` empty and `Undo::None`, for the same reason the grid has them: a panel being up is not project
  data, and `Cmd+Z` after `Cmd+E` must take back the last change to the grimoire.
- The key reads the panel's `Display` into `Ctx::view` before dispatching, so "up or down" is the
  operation's answer rather than a second opinion.

## A finding about the harness
Two scans in `operations_have_two_front_ends.rs` defined *the UI* as four files; `notice.rs` owns the
journal. Both reported `view_toggle_journal` as missing. **The same file already records this exact false
failure** for `piece_import`'s `admit_candidate(` — the lesson was written down and the fix was applied to
one scan only. Both now read five files. This widens the check's reach; no assertion was removed.

## `Shortcuts` is not on this template
`K` is a *held* key: four systems read `pressed(...)` to fade while it is down. An operation whose effect
reverses on release is not a view value. Recommended: document it as staying unmigrated rather than build
a wrapper that pretends it is migrated.

## Review
Reviewer not spawned. Mine: (1) *Fails before?* The parity tests failed until the mapping, the dispatch and
the widened scans existed. (2) *Is the failure the claim?* The claim is "both front ends dispatch one
operation"; the entry-point and UI-path tests are exactly that. (3) *Right layer?* Yes — policy in the
registry and the parity table. (4) *Protected files?* `operations_have_two_front_ends.rs` lost one
`UNMIGRATED` row and the ceiling dropped by one (the ratchet's instruction), and two scans gained a file.
(5) *What remains?* `Shortcuts` written up; `Cancel` still needs a design answer.

## Test diffs
- `UNMIGRATED`: `Action::ShowErrors` removed, `UNMIGRATED_MAX` 90 → 89.
- Three UI-source scans widened from four files to five (`notice.rs` added), with the reason recorded in
  the comment the file already carries for the identical failure.

## Release
None — `bevy_cauldron` is `publish = false`.

## Next focus
Document `Shortcuts` and `Cancel` as staying unmigrated, with the reason each needs a different shape.
