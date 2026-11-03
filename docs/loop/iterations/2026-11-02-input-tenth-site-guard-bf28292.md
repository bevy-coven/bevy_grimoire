# 2026-11-02 input: the tenth site, and a guard so an eleventh cannot come back

Focus: #224 UI-30 · Commits: 69613c2..bf28292 · Machine: big · Outcome: shipped

## Summary
Pass 268 replaced the `Query<&Hovered>` idiom in nine systems and *hoped* a tenth would not reappear. This
pass wrote the check first — and it failed immediately on `filter.rs:381`. That system's question was
narrower than `OverUi`'s, so it got its own answer rather than the resource.

## Baseline
Gate: green (`69613c2`). `accept.py`: not run. Load 9.9, 150 GB free, 15 GB available.

## Bug fixed: the tenth site
### `blur_on_world_click` kept the keyboard when a click landed anywhere
Must-fix: blocks no, common **yes**, no workaround yes — an author had to find the filter box to stop
typing; a click anywhere else did nothing.
Root cause: `hovered.iter().any(|h| h.0)` — true for the whole window while the docks are hovered.
Fix: the question this system actually has is **did the click land outside every filter box**, not *is a
panel under the pointer*. It now tests the `FilterBox` rects against `Pointer`. That is narrower and
correct: a palette row is a panel, and clicking one must not take the keyboard from a half-typed query.
Regression check: `headless::no_gesture_driver_reads_hovered_instead_of_over_ui` — **failed before** with
`filter.rs:381: if hovered.iter().any(|h| h.0) {`, passes after.

## The guard
`no_gesture_driver_reads_hovered_instead_of_over_ui` reads every `.rs` under `src/` and fails on any line
matching `.iter().any(|h| h.0)`. **Its limit is stated in its own doc**: it cannot prove a driver behaves
correctly, only that none went back to the idiom that was wrong for five passes. The behavioural half is
`a_shift_click_that_picks_no_piece_says_what_to_do_instead`.

## Review
Reviewer not spawned. Mine: (1) *Fails before?* Measured — the guard failed on a real site. (2) *Is the
failure the claim?* The claim is "no driver asks the old question"; the scan is that claim. (3) *Right
layer?* `filter.rs` owns blurring; it asks its own question of its own boxes. (4) *Protected files?* A test
was **added** to an existing module. (5) *What remains?* The split's positional blocker.

## Test diffs
One new test in `headless.rs`. Nothing removed or loosened.

## Release
None — `bevy_cauldron` is `publish = false`.

## Next focus
Compare the click test's own pixel against every hovered rect with the split in.
