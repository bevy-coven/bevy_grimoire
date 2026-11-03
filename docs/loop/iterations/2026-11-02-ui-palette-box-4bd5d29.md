# 2026-11-02 ui: the palette names its box; the click at a row's position still arms nothing (#275)

Focus: #275 (filed this pass from five passes' measurements) · Commits: f630cd0..4bd5d29 · Machine: big · Outcome: shipped (a filed issue, a public marker, three stable tests, one measured failure)

## Summary
The palette-scroll defect had been carried in comments for five passes. This pass filed it as **#275** with
every measurement, then attacked the half that was blocking the *test*: the palette had no nameable box, so
every attempt to measure it found the inspector or a window scroller instead. `editor::PaletteList` is now
`pub`, the three characterisation tests measure the real box and are stable, and the acceptance's second
item now fails for exactly the right reason with its numbers on it.

## Baseline
Gate: green (`f630cd0`). `accept.py`: not run. Load 6.9, 150 GB free, 13 GB available RAM.

## The issue: #275
Filed with the table of five passes' measurements (`ScrollPosition` 1553.75 while `sofa` reported 907,
−1440, 2771 against a viewport of 185–659), the four things ruled out, three proposed fixes, and the
acceptance. No such issue existed; the findings were only in comments.

## The slice (`4bd5d29`)
- **`editor::PaletteList` is `pub`** — one keyword. It ends the harness problem: the palette and the
  inspector are the same shape, so size-based discovery found the wrong node three times, and the same test
  failed for a different reason on different runs.
- **Three tests, stable against the real box** `Vec2(1362.0, 494.0)` x `Vec2(286.0, 618.0)`: the palette is a
  scrolling viewport; a visible row reports a centre inside it; a scrolled-out row reports one clearly
  outside.
- **One `#[ignore]`d case, the acceptance's second item**, failing for the right reason: a press at
  `Vec2(1373.5, 286.0)` — inside that box — leaves `floor` armed instead of `wall`.
- One case written and dropped: "every row shares the palette's column" catches the inspector's rows at the
  same heights by any filter a test can write.

## Test diffs
New `a_palette_row_reports_a_clickable_position.rs` with its `mod` line (owner exception): 3 pass, 1
`#[ignore]`d with its measurement. `editor::PaletteList` changed from private to `pub` — a visibility
change, no behaviour.

## Review
Reviewer not spawned. Mine: the marker is the smallest thing that ends a three-attempt harness problem; the
passing cases are characterisation and say so; the failing case is `#[ignore]`d with its numbers rather than
quietly dropped or made to pass.

## Release
None: `bevy_cauldron` is `publish = false`, and this pass touched one keyword and tests.

## Next focus
Turn the ignored case green: a counter in `on_row_click` says whether the press reaches the row at all.
