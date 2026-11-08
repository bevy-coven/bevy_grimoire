# 2026-11-07 feel series: G-58 scrolls glide and fade at their edges built in bevy_gooey, released as v0.42.0

Focus: epic #330 (owner's steer) · Commits: bevy_gooey 895dec9 (merged 48d9d1b), release e4053a2 · Machine: big · Outcome: shipped (in bevy_gooey)

## Summary
G-58 (bevy-coven/bevy_gooey#65): a scroll the crate makes now glides on a spring, a held key crosses a 10,000-row table as one motion with the cursor in view, and a clipped list fades each edge only where content is hidden. UI-R03 #333 now waits on G-66 (#73) alone; UI-R05 #335 now waits on nothing but the pin move.

## Baseline
bevy_grimoire was not touched, so its gate and `accept.py` did not run. Disk on big hit 100% for about 20 minutes mid-pass (the runners' caches, not this work) and recovered; the first test link took 18 minutes.

## Enhancement: G-58
- **Change:** `src/scroll_glide.rs`: `ScrollGlide` (required by `ListView`) and `update_scroll_glides`; `scroll_to_focus` and `update_tables` request a goal instead of writing `ScrollPosition`; `ScrollEdgeFade` (required by `ListView`) with two `Pickable::IGNORE` overlays beside the viewport.
  - **Takeover:** a `ScrollPosition` that reads otherwise than what the glide wrote (wheel, rail drag, game) ends it.
  - **Landing:** reduced motion, off and no clock land at once.
  - **Table:** steps its own viewport's glide, so the rows it binds are the rows of the scroll it draws.
- **Assumed decisions (on the issue):**
  - The spring is a third of `motion.default`. At the whole 0.3 s the cursor row stood two rows past the viewport's foot on every frame of a held key (measured, 412 px against a 374 px viewport).
  - `tests/type_select.rs` inserts `MotionSettings::off()` (#76, own commit c06e5d8).
  - Menus and sections are not covered.
- **Proof:** `tests/scroll_glide.rs`, 8 tests. A far focus glides and lands where the jump did; a request mid-glide keeps its speed; the wheel takes over and a notch is 1:1; reduced and no motion land in two frames; held Down in 10,000 rows has no step beyond twice the median, the cursor in view and its rows bound every frame; a sort brings the cursor row into view; the fade is 0 at the top, 1 after 15 px, half at 7.5, none at the end, and a settled list writes nothing. None of it existed before this change.
- **bevy_gooey gate:** fmt, clippy -D warnings, nextest 1181/1181, doctests 257, wasm32 check, rustdoc -D warnings.

## Borrowed from Mixar
None. The reference is Rune: `scroll_to`'s glide and `runtime/scroll/edge.rs` (`FADE_IN` 15).

## Performance
Not measured. The fade looks its colour up every frame while an edge shows; a reviewer flagged it (P3) and it is not cached.

## Acceptance
`accept.py` not run: no bevy_grimoire code changed.

## Review
An independent `reviewer` ran: 2 P1, 3 P3.
- **P1:** the glide ran after `update_tables` bound its rows, so a held key drew rows of the previous frame's scroll; fixed by the table stepping its own glide before binding, with a test that the rows cover the viewport every frame.
- **P1:** a sort while scrolled down lost the cursor reveal; fixed (the table ends a glide it moved itself) with a test.
- **P3:** a non-finite goal, a double despawn when the fade is removed: fixed. A per-frame colour lookup: not cached.

The fixes were re-run through the gate, not re-reviewed.

## Test diffs
New `tests/scroll_glide.rs` and its `mod` line in `tests/all.rs`. One line added to `tests/type_select.rs` (assumed owner exception #76); no assertion changed.

## Release
bevy_gooey **v0.42.0**, https://crates.io/crates/bevy_gooey/0.42.0 (tag v0.42.0, commit e4053a2). MINOR: `cargo semver-checks` flagged `update_tables` (11 parameters instead of 9) and `scroll_to_focus` (9 instead of 7). It also ships the sibling pass's G-60 switch motion and G-67 inspector mixed values, unreleased since v0.41.0.

## Next focus
G-66 combobox (#73), then G-53 hover groups (#60) and the pin move for UI-R03/R04/R05 to v0.42.0 (the editor's tooltip tests need delay 0).
