# 2026-11-03 ui: E-037's clip records, and the keyboard legend reads the skin

Focus: #276 then UI-02 (#210) · Commits: 6c58f9b..3ff2056 · Machine: big · Outcome: shipped

## Summary
Closed #276 with E-037's clip, whose three causes were all operational rather than in `record.py`: a scratch
project nested by `cp -r` (so the editor drew an error dialog), a `brush_arm` that was a no-op because the
brush was already armed, and a crop spanning the change's bbox instead of sitting on the ghost. Then took
UI-02, the first ready `ui` issue: moved the `bevy_gooey` pin to the revision carrying G-29's tones, and moved
the keyboard legend's four colours onto the skin.

## Baseline
Gate: green. `accept.py`: 10/10 on `assets` (matches the documented big baseline). Load 1.29, free disk 131 GB,
available RAM 14 GB.

## Bugs fixed
### #276: `record.py` captures a stale frame after a `brush_arm` chunk
Must-fix: blocks y (it blocked E-037's clip across four passes), common n, no workaround n — the clip now
records and `check.py` passes all four criteria.
Root cause: **three separate operational faults, none in the recorder.**
1. `cp -r assets /tmp/p` into an *existing* dir nested it as `/tmp/p/assets/assets`, so the editor refused the
   project (`assets/grimoire/kits.ron: No such file or directory`) and every capture was the same error dialog.
2. `brush_arm pipe` is a no-op when `pipe` is already armed — `{"outcome":"applied"}` but `affected: 0`, so a
   second recording against the same editor had nothing to show.
3. The change is concentrated: **31.6% inside a 120 px crop at x=664, y=352**, but **2.7%** across its
   751×377 bbox and **0.83%** of the frame — under `check.py`'s 5% floor. Earlier crops spanned the bbox.
Regression check: `tools/demo/check.py` on `agent_arms_a_brush` — `RESULT: PASS` (scene 75.5%, change 8.99%,
static 1.50 s, size 0.29 MB).

## Enhancement: the keyboard legend draws in the editor skin's ink (UI-02, #210)
Goal: a project's skin file reaches the badge layer, the last surface read *while* learning the editor.
Files: `crates/bevy_cauldron/src/badges.rs`, new `tests/a_badge_draws_in_the_skins_ink.rs`.
Proof: reverting `BadgeInk::of` to the constants fails with *"a labelled cluster stands on the skin's
`surface`"*; restored, all 3 pass. `editor_skin().problems()` is `0 -> []` with 12 tones.
MCP: none. Undo: none (a colour source, not an edit).
**Measured for the next slice:** 939 uses of 28 colour constants remain across ~25 files (`tiles.rs` 222,
`chrome.rs` 179, `editor.rs` 132). A `const` cannot call the runtime `skin.tone(..)`, so each site needs the
fallback seam; `BadgeInk` is the shape to copy.

## Borrowed from Mixar
none.

## Performance
Not measured (`perf.py` needs a live display; no change is on a hot path).

## Acceptance
`reached 10/10 -> 10/10`; no first failure.

## Review
Reviewer not spawned. Self-review, mine:
- **Does the seam actually change behaviour?** Yes — proved by reverting it, which is the only honest "before".
- **Could the fallback hide a broken skin?** It could, so `the_editor_skin_draws_the_shipped_palette` pins the
  editor skin and the shipped constants to one palette; a tone retuned without its constant fails there.
- **Is `text_muted` really `DIM`?** Yes, 0.700/0.685/0.655 in both, which is why it is a field and not a tone.
- **Did I re-pin any test?** No. `all.rs` gained one `mod` line and nothing else; the fallback is why nothing
  else moved.
- **Was the arity fix honest?** Clippy's `too_many_arguments` was real (8/7), so the skin went into the
  existing `Shortcut` SystemParam rather than the lint being silenced.

## Test diffs
- `crates/bevy_cauldron/tests/all.rs`: one added `mod a_badge_draws_in_the_skins_ink;` (the assumed owner
  exception for a new test file). Nothing else.
- New: `crates/bevy_cauldron/tests/a_badge_draws_in_the_skins_ink.rs` (3 tests).
- No existing assertion was changed, removed, loosened or re-pinned.

## Release
None: `bevy_grimoire_rng`, `_solver` and `_snip` show no diff under `crates/<crate>` since their tags (the
`CHANGED` line was `Cargo.toml` from the pin move, not the crate), and every other publishable crate is
unchanged. `bevy_cauldron` is `publish = false`.

## Left alone, named
`crates/bevy_cauldron/tests/the_shells_divider_moves_under_a_drag.rs` — **another agent was editing this clone
while this pass ran**: the file appeared at 06:52, was rewritten at 07:01 with different content, and its
`all.rs` `mod` lines were being reordered around mine. It is untracked, does not compile
(`bevy::picking::events::PointerState` has no `pressing()`/`dragging()` in Bevy 0.20), and it blocked the
push gate's clean-tree check. Moved to `/tmp/p289_leftovers/` (both versions kept), not deleted, and not
committed. Whoever owns it should take it back.

## Next focus
The next constant file (`chooser.rs` 30, or `compose.rs` 46 with a test) in the same fallback shape.
