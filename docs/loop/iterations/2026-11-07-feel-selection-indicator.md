# 2026-11-07 feel series: G-61 sliding selection indicator built in bevy_gooey (v0.43.1), and the eight standing demos re-recorded again

Focus: epic #330 (owner's steer: re-record the demo GIFs each loop) · Commits: bevy_gooey a849020, release 04a3cdf; bevy_grimoire demos below · Machine: big · Outcome: shipped

## Summary
G-61 (bevy-coven/bevy_gooey#68): `toggle_group`, `tabs` and `side_tabs` slide one accent puck from the old lit option to the new one on `motion.snappy`. UR06 (#336) still waits on G-62 (#69), G-63 (#70) and G-64 (#71); the sibling pass took UR03 to UR05, which is why every demo was stale again (3 visible changes since 3e9d9bb8) and was re-recorded on a4c4e21a.

## Baseline
bevy_grimoire code was not touched; its gate and `accept.py` did not run. Disk on big 618 GB free.

## Enhancement: G-61
- **Change:** `src/selection_indicator.rs`: `SelectionIndicator` (a backdrop in the raised fill) holding an `IndicatorPuck` (the accent), the last child of the widget, behind its siblings by `ZIndex(-1)`, hidden at rest; `update_selection_indicators` springs the puck's four lanes and marks the options `Indicating` while it travels, `apply_skin` draws marked options clear and colour motion lands them (`color_motion.rs`). Reduced motion, off and no clock never travel.
- **Assumed decisions (on the issue):** the indicator is hidden at rest and shown only for the travel (the options' opaque fills are pinned by protected tests); dock tab strips and menu bars are not covered.
- **Proof:** `tests/selection_indicator.rs`, 5 tests: a four-option group stepped first to third travels only toward the third and lands within 0.5 px; a step back mid-flight turns round smoothly; tabs and side_tabs travel; reduced and no motion never show it; a resting group writes nothing.
- **Gate:** fmt, clippy -D warnings, nextest 1209, doctests 263, wasm32 check, rustdoc -D warnings.

## Review
An independent `reviewer` ran: 4 P1, 2 P3 (the P3 about the unused test lines and the allocation), all fixed.
- **P1:** the indicator as first child broke the child indexing of `tests/toolbar.rs` and `tests/side_tabs.rs`; it is now the last child with `ZIndex(-1)`.
- **P1:** in `tabs` the indicator was under an opaque strip; the backdrop and puck are one node pair and the strip is never filled.
- **P1:** the strip stayed raised after the first travel; it is never filled now.
- **P2:** the lit option faded back in after landing; colour motion lands a node whose `Indicating` changed.

The fixes were re-run through the gate, not re-reviewed.

## Demos (owner: re-record them each loop)
All eight GIFs and `editor.png` re-recorded on the editor built at a4c4e21a with `tools/demo/retake.sh`, each contact sheet or still looked at; `stale.py` says all nine fresh.
- **Takes:** place-and-undo 3 (the `place` change measured 3.02% of scene pixels against 5% on two takes), the others 1, except build-a-map, below.
- **build-a-map storyboard changed:** with the editor at a4c4e21a every take failed `check.py`'s static-run limit (1.8 s): the filter is typed and the sofa armed in the PLACE list, which the crop left out, so the crop showed nothing for 1.7 s. Its crop is now `[450, 170, 1185, 690]` so the filter and the row show. The GIF is 760 x 594 and its caption says so.
- **Caption:** place-and-undo already says the armed sofa's ghost stays after Ctrl+Z.

## Test diffs
New `tests/selection_indicator.rs` and its `mod` line in `tests/all.rs` (bevy_gooey). None in bevy_grimoire. One storyboard (`build_a_map.json`) crop.

## Release
bevy_gooey **v0.43.1**, https://crates.io/crates/bevy_gooey/0.43.1 (tag v0.43.1, commit 04a3cdf). PATCH: `cargo semver-checks` says no update required. It also ships the sibling pass's G-68. bevy_grimoire has no crate change.

## Next focus
G-62 (#69), G-63 (#70) and G-64 (#71), then UR06 (#336). Re-record the demos again after the next visible editor change.
