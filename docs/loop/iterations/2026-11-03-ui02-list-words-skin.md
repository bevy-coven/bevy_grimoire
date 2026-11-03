# 2026-11-03 UI-02: the shared list's words come from the editor skin

Focus: UI-02 (#210), the `tiles.rs` seam · Commits: 838056e..35d6696 · Machine: big · Outcome: shipped

## Summary

`draw_ink` is the one function every row of the Meshes/Tiles list is drawn by, and it could not see a
project's skin file. Its three ink sites now read `tiles::ListInk` — `RowFills`' shape — and the change is
asserted invisible to the eye, because the editor skin's four values *are* the four constants today.

The pass also re-measured what UI-02's first acceptance item needs, and the answer is unchanged: 28
constants, 431 uses, and three protected test files that pin them by name.

## Baseline

Gate: green before (838056e). `accept.py`: not run (no UI path is scored by it). Load 7.9, free disk
116 GB, available RAM 13 GB.

## Bugs fixed

none.

## Enhancement: the shared list's words read the skin (UI-02, #210)

Goal: a project's skin file reaches the words in every row of the Meshes and Tiles lists.
Files: `crates/bevy_cauldron/src/tiles.rs` (`ListInk`, `draw_ink`'s signature, `rebuild_candidates`' skin
param), `tests/the_lists_words_come_from_the_skin.rs` (new), `tests/all.rs` (one `mod` line).
Proof: `cargo test -p bevy_cauldron --test editor the_lists_words_come_from_the_skin::` — 3 passed; the
full editor suite **1210 passed, 0 failed**; clippy clean over `--all-targets`.

`ListInk::of` reads `GooeySkin`'s `text`, `text_muted`, `accent` and `danger`, and falls back to
`chrome::TEXT`, `DIM`, `ACCENT` and `DANGER` when no skin is running — the harness case. One expression,
read by the system and by the test, so an assertion here is an assertion about what is painted.

**The three tests and what each can fail on:** the editor skin's four values are those four constants (so
no protected assertion moved when the list started reading it); the no-skin fallback is the shipped
palette; and a theme that moves `text` moves a row's words while a field it did not touch keeps the
editor's own value.

MCP: none. Preview: none. Undo: none.

## Borrowed from Mixar

none.

## Performance

Not measured with `perf.py` (it needs a live display). The change adds one `Option<Res<GooeySkin>>`
parameter to a system that already rebuilds a list, and one struct copy per rebuild — not per frame.

## Acceptance

`reached` unchanged.

**UI-02's first acceptance item, re-measured:** 28 colour constants still declared; **431 uses outside
`chrome.rs`** (up 1 from 430, because the seam names the four constants once as its fallback — every seam
has done the same). `tiles.rs` 182, `editor.rs` 103, `compose.rs` 36, `anim_tab.rs` 32.

## Review

Reviewer could not run (no subagent spawned inside the budget). Five questions, mine:

1. **Does the skin actually reach the pixels?** Yes for the three ink sites: `draw_ink` reads `words.*`,
   and the test calls the same `ListInk::of` the system calls.
2. **Could this change the editor's look?** No, and asserted: `the_lists_ink_is_the_skins_and_the_skin_is_the_palette`
   would fail if the skin and the constants diverged.
3. **Does the harness still paint the shipped palette?** Yes — `ListInk::of(None)` is asserted to be the
   four constants.
4. **Did any protected assertion move?** No. The suite is 1210 passed, 0 failed.
5. **Is the acceptance item met?** No, and the number is in the report: 28 constants remain, because the
   tests that pin them by name would have to be re-pinned.

## Test diffs

- `crates/bevy_cauldron/tests/all.rs`: one `mod the_lists_words_come_from_the_skin;` line, in order (the
  standing exception for a new editor test file).
- `crates/bevy_cauldron/tests/the_lists_words_come_from_the_skin.rs`: new.
- No existing assertion changed.

## Release

None: `bevy_cauldron` is `publish = false`; no publishable crate's files changed.

## Next focus

UI-02's first item is blocked on the owner (re-pin three protected files to read the skin instead of the
constants); every other `ui` issue waits on #224 → #210. #129's migrations are blocked on the pinned
classification sets. #195's options 1 and 3 are the owner's.
