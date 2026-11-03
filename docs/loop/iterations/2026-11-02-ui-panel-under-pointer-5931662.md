# 2026-11-02 ui: a gesture is blocked only by a panel under the pointer (#224)

Focus: #224 UI-30 · Commits: e0d49ef..5931662 · Machine: big · Outcome: fixes only

## Summary
Four passes blamed the shell split for a click that never landed. The cause is the question the gesture
drivers ask: a bare `Query<&Hovered>`, and `Hovered` is true for a node **or any descendant** — measured,
the two docks are hovered and 697 px tall, so the answer was `true` everywhere. `panel_under_pointer` is
the position-aware form; `drive_note` moved onto it and its test now passes.

## Baseline
Gate: green (`e0d49ef`). `accept.py`: not run. Load 3.7, 150 GB free, 15 GB available.

## Bug fixed
### A gesture was blocked by any hovered node anywhere
Must-fix: blocks **yes**, common yes, no workaround yes — every map gesture was refused once anything in
the window was hovered.
Root cause: `ui_blocks_gesture(hovered_ui.iter().any(|h| h.0), dragging)` with `hovered_ui: Query<&Hovered>`.
`Hovered` propagates from any descendant, and the docks are full-height, so the predicate was `true` across
the window. `view::over_ui` already answers correctly; no driver was using it.
Fix: `panel_under_pointer(pointer, camera, nodes)` — the plumbing — and `drive_note` moved onto it.
Regression check: `enter_with_a_note_box_open_saves_it::a_shift_click_that_picks_no_piece_says_what_to_do_instead`
— **failed before (`the line was ""`), passes after** on the same tree.

## What did not ship, and why
The split itself. With the hover fix in place, adding the split **still fails the same test**, so the split
blocks the click through a mechanism this fix does not reach. A probe confirmed the panes and the divider
carry no `Hovered` after stripping, and the viewport measures **906×726 at centre (737,470)** — a real
rect. The remaining blocker is **positional**, and the next pass should compare the test's own pixel
against each hovered rect rather than guess another mechanism.

## Review
Reviewer not spawned. Mine: (1) *Fails before?* Measured on the same tree. (2) *Is the failure the claim?*
The claim is "a gesture is blocked only by a panel under the pointer"; the test exercises the Shift-click
path that was silently blocked. (3) *Right layer?* `view::over_ui` already owned the correct answer; this
is only the plumbing. (4) *Protected files?* None. (5) *What remains?* Six other drivers with the bare
`any()` form, each needing its own commit and reproducer; and the split's positional blocker.

## Test diffs
None — an existing test became the regression check.

## Release
None — `bevy_cauldron` is `publish = false`.

## Next focus
`drive_place` onto `panel_under_pointer`, with a reproducer; then the split's pixel.
