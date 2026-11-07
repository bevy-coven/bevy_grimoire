# 2026-11-07 feel series: G-56 type-select built in bevy_gooey

Focus: epic #330 (owner's steer) · Commits: bevy_gooey 2b4dd73 · Machine: big · Outcome: shipped (in bevy_gooey)

## Summary
The epic's bevy_grimoire items (UI-R01 #331 to UI-R06 #336) still wait on bevy_gooey. UI-R01's needs G-39 to G-41 were closed by the gooey loop; G-42 to G-46 are still open. This pass built G-56, type-select (bevy-coven/bevy_gooey#63), which is one of UI-R04 (#334)'s needs, in the clone at `~/family_src/bevy_emerge_loop/bevy_gooey`. It claimed the issue first because the gooey loop takes ready motion items.

## Baseline
bevy_grimoire gate and `accept.py`: not run, because no bevy_grimoire code changed. Load 29.6 at start (gooey and HomeStill builds), free disk 58 GB (99% used).

## Enhancement: G-56 (bevy-coven/bevy_gooey#63)
- Goal: typing a name goes to it in a table, a tree, a list, an open menu and a select of eight options or fewer.
- Files (bevy_gooey): `src/type_select.rs` (new), `src/table.rs` (`table_by_input`), `src/tree.rs` (its own type-ahead removed), `src/choice.rs` (`Move::To`), `src/menu.rs` (`MenuItem::label`), `src/lib.rs`, `tests/type_select.rs` (new) with its `mod` line, `docs/research/type-select.md`, the guides for lists, tables, menus and select, `README.md`, `AGENTS.md`, `CHANGELOG.md`.
- The rules (1 s pause, whitespace and Ctrl, Alt and Super ignored) were measured from Rune's `runtime/input/focus.rs` at `f73f72c`. Matching reuses gooey's `model::type_ahead`, which the tree already shipped. There is one shared `TypedPrefix`.
- Proof: on `origin/main` (c5b3b5a), 4 of the 5 new tests fail:
  - the table test: cursor `None`, expected `Some(9000)`;
  - the list test: focus 459v0, expected 461v0;
  - the menu test: focus 467v0, expected 473v0;
  - the select test: value 0, expected 2.
  The tree test passes on both, because a tree already had type-ahead. All 122 tests in the type_select, tree, table, list, menu, select and choice modules pass after the change, including the tree's existing type-ahead tests.
- bevy_gooey gate: all green. fmt, clippy `--all-targets --all-features -D warnings`, nextest 1057/1057, doctests 238, the wasm check and rustdoc `-D warnings` all passed. The first clippy run refused `too_many_arguments`; I allowed it on the new system, as the crate already does for `table_by_input`.

## Borrowed from Mixar
None (the reference is Rune).

## Performance
Not measured. The new system returns at once unless a character was typed.

## Acceptance
`accept.py` not run: no bevy_grimoire code changed.

## Review
The reviewer subagent ran and confirmed the core logic was right. Its findings and what was done:
1. Skipped-frame backlog: as a run-condition-gated system, characters typed while a binding row captured or a text field had focus could be type-selected in the next frame. The system now reads every frame and drops them, with the gate checked in the body.
2. Out-of-range row: a table row index could be past the end of the rows when the source shrank earlier in the frame. It is now guarded with `row < table.len()`.
3. Stale doc: AGENTS.md still described the tree's old type-ahead. Fixed.
4. Weak tests: the space test now proves a space is no part of a prefix ("c e" lands on Cedar), there is a new test with a text field inside a list, and the list test checks the scroll.
5. Wrong guide line: the select guide said typing on a filtering select goes into its list; it now says typing does nothing until the list is open.
6. CHANGELOG nuance: the prefix now survives leaving a tree and coming back within a second, which the CHANGELOG now says.

## Test diffs
bevy_gooey: one new file, `tests/type_select.rs`, and its `mod type_select;` line in `tests/all.rs`. No existing test changed.

## Release
None here: nothing in bevy_grimoire's packages changed. bevy_gooey's release is its own loop's.

## Next focus
The next of the epic's gooey needs that the gooey loop has not taken (G-47, G-48, G-52 or G-57), then UI-R04 #334 once G-53, G-54 and G-57 close.
