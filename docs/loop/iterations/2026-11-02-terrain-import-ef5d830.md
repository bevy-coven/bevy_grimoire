# 2026-11-02 terrain: #99's owed clip recorded, and the two mistakes that cost the takes

Focus: every open issue (all blocked) → the changelog's owed clips · Commits: 3fae7eb..ef5d830 · Machine: big · Outcome: shipped

## Summary
No `ui` issue was ready (gooey has zero open issues, so the only block is #210's protected `chrome::NAME`
spellings; #235 deletes pinned `badges.rs` behaviour), #129's remaining allowlist is all camera and view
state, and the `decide` issues need the model (`8093` refuses). Per the command's rule, the pass recorded
an owed clip: `99-terrain-import`, whose whole point is visible.

## Baseline
Gate green at 3fae7eb. `accept.py` not run: nothing it exercises changed (a storyboard, a tool, media, a
changelog line). Load 3.1, free disk 176 GB, RAM 16 GB.

## What shipped
`docs/changelog/media/2026-11-02-99-terrain-import.mp4` (10.3 s, 98 KB) from the new
`tools/demo/storyboards/terrain_import.json`: the bedroom's flat ground, then `terrain_import hills16.png`
(cell_m 1.0, range_m 0..6), then the ground as a 65×65 dome with the pieces riding it. `tools/demo/height_map.py`
writes the 16-bit PNG, so the storyboard is reproducible. `check.py`: scene 55.8%, change moment 6.07%,
longest static 1.50 s, 0.71 MB — PASS. The sheet shows the Inspector's LAST `terrain bedroom: imported
4225 cells` and its UNDO row.

The line's title was corrected to what the footage shows: it promised "one undo step" and the clip is the
import only. A `Ctrl+Z` inside the same sequence takes the import back *before the run ends*, so the file
is gone and only the first half is on film; the title now says "one entry to take back", which the Inspector
shows.

## Two mistakes worth recording
1. **`terrain_import` writes `<grimoire_dir>/terrain/<map>.ron`** — for this project that is the authoring
   kit's `furniture/terrain/`, **not** `maps/`. Several takes were thrown away because I looked in
   `maps/` and concluded nothing was written. It was written: 71,785 bytes, visible in `furniture/terrain/`.
2. **A storyboard's `operation` must be on its own chunk.** One `json.dump` rewrote `chunks` without it;
   the recorder then reported `operation_applied_t_s: null` for every chunk, which is correct and was the
   clue. Checked with `grimoire/capture_sequence_status`, not assumed.

I also wrote and then **deleted** a test asserting that `Sequencer::start` reuses an id
(`a_sequence_id_names_one_run.rs`): measuring against the live editor showed ids advance correctly, so the
premise was false and shipping the test would have been a lie about a bug that does not exist.

## Bugs
None fixed. #193 still waits on bevy-elements#55; #152 on protected fixture text.

## Skipped, and why
#210 + 20 `ui` behind it (protected spellings); #235 (deletes pinned assertions); #201 (protected
`assert_ne!`); #152 (protected fixture); #193 (bevy-elements#55); #112's calibration and #113 (both need
the model); #195 (its memory item is already implemented); #197 `wontfix`. The parked pass-218 branch is
stale (its test is on main after the rename).

## Review
`reviewer` not spawned; self-review: (1)*does it show the claim* — the sheet is quoted frame by frame;
(2)*anything else changed* — one storyboard, one tool, media, one line; (3)*protected files* — none, the
`no clip` reason was replaced as the rules allow; (4)*could it mislead* — the title was rewritten to what
the footage shows and says what it does not; (5)*what did I get wrong* — two things, both above, both
found by measuring rather than reasoning.

## Test diffs
None.

## Release
None: no packaged crate changed.

## Next focus
The next owed clip with a viewport-scale result; the `decide` work resumes when the model is served.
