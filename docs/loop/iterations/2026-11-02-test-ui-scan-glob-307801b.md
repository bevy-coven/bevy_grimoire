# 2026-11-02 test: the UI scan is a glob over src/, not a list of names (#129)

Focus: #129 harness · Commits: f4d37d2..307801b · Machine: big · Outcome: fixes only

## Summary
Four passes in a row had to widen a hand-maintained array of source-file names because a migration landed
in a file nobody had listed: `screen.rs`/`chooser.rs`, then `notice.rs`, then `snip.rs`. Each widening
reported a live entry point as missing. `ui_source()` now reads **every** `.rs` under `src/`.

## Baseline
Gate: green (`f4d37d2`). `accept.py`: not run. Load 16.2 at start (other loops busy), 150 GB free.

## Bug fixed
### The UI scan could not see the thing it checked
Must-fix: blocks no, common yes, no workaround yes — every migration could produce a false failure.
Root cause: three scans in `operations_have_two_front_ends.rs` each held a literal array of file names;
**60+ source files were never in any of them** (`anim_tab.rs`, `build.rs`, `compose.rs`, `tabs.rs`, …).
Fix: one `ui_source()` that globs `src/*.rs`, sorted so a failure's message is stable.

**Proved by measurement, both directions on the same tree**: a marker string placed only in
`audio_panel.rs` with a `UI_ENTRY_POINTS` row pointed at it — **glob passes, old six-file list fails**
(`these entry points are not in the source`). Probes removed; suite **1154 passed, 0 failed**.

**A probe that proved nothing, recorded because it is instructive.** My first probe added a
`dispatch(` call to `audio_panel.rs`; the test still passed. It only checks the *places named in the
table*, so a stray dispatch cannot fail it. A test that passes when you add something it looks for is
telling you it looks elsewhere.

## Review
Reviewer not spawned. Mine: (1) *Fails before?* Yes — the list fails on the marker tree. (2) *Is the
failure the claim?* The claim is "a UI path anywhere under `src/` is found"; that is what was measured.
(3) *Right layer?* The harness reads the crate's own tree. (4) *Protected files?* A test file changed: three
scans widened to the whole crate and one read pinned narrower with a stated reason. **No assertion was
removed or loosened** — the glob is strictly stronger than the list, and the `editor.rs`-only assertions
are unchanged and now explain why a glob would make them vacuous. (5) *What remains?* The 55 `Project`
verbs.

## Test diffs
`operations_have_two_front_ends.rs`: three `["src/…", …]` arrays replaced by `ui_source()`; one
`editor.rs` read restored with a comment. No assertion added, removed or re-pinned.

## Release
None — `bevy_cauldron` is `publish = false`.

## Next focus
`FoldPack` — read the handler, then classify.
