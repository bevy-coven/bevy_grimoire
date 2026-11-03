# 2026-11-03 UI: the bench and journal ink come from the skin

Focus: UI-02 (#210) · Commits: d02357b..c594dbe · Machine: big · Outcome: shipped

## Summary

Adopted the parked `#210` draft that a concurrent agent left uncommitted mid-pass yesterday, finished it,
and found the reason it was a draft: `BenchInk::of` claimed to read a `label` tone and assigned the
constant, so the test it shipped failed. Three of its four tests passed and the fourth named the bug —
which is exactly why a parked tree is adopted and gated rather than trusted.

## Baseline

Gate: green before (`d02357b`). `accept.py`: not run (no solver path touched). `stale.py`: all demos
fresh. Timing report: `model` 23.4%, `cargo test` 17.6%, `wait: tests` 9.8%, `wait: gate` 7.9%.

## Enhancement: bench labels and journal ink from the skin (#210, slice 4d)

Goal: two more ink surfaces stop reading constants · Files: `anim_tab.rs`, `compose.rs`, `labels.rs`,
`notice.rs`, `tests/the_benchs_labels_and_the_journal_take_the_skins_ink.rs` (new), `tests/all.rs` (one `mod`)

**Adopted, then finished.** `wiggum/parked-pass-371-concurrent-b` held `BenchInk` gaining `label` and `on`,
`compose::PanelInk`, the journal's ink and a four-test file. It did not pass:

```
a column's label is the skin's `label` tone
  left:  Srgba(0.615, 0.6, 0.575)   // chrome::LABEL
  right: Srgba(0.13, 0.14, 0.15)    // the project skin's `label`
```

The fix is one expression: `skin.tone("label").unwrap_or(LABEL)`. The draft's comment already described the
right behaviour; the code did not do it.

**The editor's own skin still carries no `label` tone**, and must not — its tone set is asserted exactly by
`the_skin_carries_the_editors_other_colours.rs`. The fallback therefore keeps the editor byte-identical,
while a **project's** skin that names `label` gets the bench quieted in its own palette.

Proof: 4/4 in the new file (the fourth failed on the draft, passes now); `clippy -D warnings` clean; the
colour ratchet plus the panel-ground tests still 11/11.

MCP: none. Preview/undo: n/a.

## Test diffs

- `crates/bevy_cauldron/tests/all.rs`: one `mod the_benchs_labels_and_the_journal_take_the_skins_ink;`
  (standing exception).
- New test file, adopted from the parked branch and unmodified. No existing assertion changed.

## Review

Reviewer could not run (no subagent spawned inside the budget). Five questions, mine: (1) does the adopted
test fail without the fix — yes, measured, quoted above; (2) does the fallback keep the editor unchanged —
yes, `editor_skin.ron` carries no `label` tone and the test asserts the no-tone path; (3) did I edit a
protected assertion — no, only one `mod` line; (4) is the parked draft now fully used — the `-c` branch
holds only a partial `compose.rs` snapshot and `-concurrent`/`-b` the full tree; `-b` is the one adopted;
(5) what remains — 28 constants and ~551 product uses.

## Release

None: nothing packaged changed (`bevy_cauldron` is `publish = false`).

## Next focus

#210 slice 5: `TEXT` (76 uses) and `LABEL` (105 uses) are the two largest remaining, both already
reachable through `GooeySkin::text` / `text_muted` / a tone.
