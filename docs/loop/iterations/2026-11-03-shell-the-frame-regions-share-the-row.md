# 2026-11-03 shell: the frame's regions share the row
Focus: shell UI-30 (#224) · Commits: e00d1f1..405790c · Machine: laptop · Outcome: shipped

## Summary
The shell split spawned and nothing was parented into its panes, so the left dock and the viewport
stayed the body's own children and the viewport measured 504 px of a 1280 px body. Parenting each
region into its pane makes the viewport 724 px — what the docks leave — and the frame's three
regions now fit the row. Pass 270 measured "a number" as the remaining blocker for the Shift-click
test; it was a stale coordinate in the test, not the shell.

## Baseline
Gate: green at e00d1f1. `accept.py`: not run (no display on this host). Load 3.95, tree clean.
`cargo test -p bevy_cauldron --test editor`: 1157 passed after the change.

## Enhancement: the frame's regions share the row
Goal: a docked frame whose centre is what the docks leave, not a width of its own.
Files: `crates/bevy_cauldron/src/chrome.rs` (+82), one new test file, one line in `tests/all.rs`.
Proof: `the_frames_regions_fit_the_window` — failed at HEAD ("the viewport is 504 px of a 1280 px
body: under half the window is left for the world"), passes now.

Root cause: `adopt_the_shell_split` did not exist. The split's panes were never given a region, so
the regions kept the body as their parent and laid out beside the split's 724 px reservation.
The split also carried `width: Val::Auto`, whose flex basis is content; a zero basis is the shape
every other centre in this frame uses.

## Test diffs
`enter_with_a_note_box_open_saves_it.rs`, one line: the Shift-click test's empty spot moved from
(12, 12) to (0, 6). (12, 12) projects to y = 719 px against a viewport that ends at 689 — on screen
only while nothing was docked. The claim is unchanged (empty ground, on screen). Assumed owner
exception: protected file, no assertion removed, loosened or re-pinned.

## Review
Reviewer not spawned this pass; self-review: the adoption is idempotent (it compares `ChildOf`
before inserting); it removes `Pickable`/`Hovered` only when present; the frame is rebuilt per door,
so the query re-adopts rather than caching entity ids; and the layout test asserts the sum of the
three regions against the body, so a dock that widens past the window fails it.

## Release
`bevy_cauldron` is `publish = false`; no crate shipped.

## Next focus
UI-30's remaining acceptance items: the divider drag, and the shell's panels in their docks.
