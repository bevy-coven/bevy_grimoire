# 2026-11-02 clock: #98's clock-scrubber clip, and the two facts that cost the takes

Focus: every open issue (all blocked) → the changelog's owed clips · Commits: 8cdd7f1..6ca8d80 · Machine: big · Outcome: shipped

## Summary
No `ui` issue was ready (gooey has zero open issues; the only block is #210's protected `chrome::NAME`
spellings), and the rest are blocked as pass 220 logged. So the pass recorded an owed clip, as the
command's rule for that case says: `98-clock-scrub`, whose stated reason ("no panel or preview yet") had
been stale for days.

## Baseline
Gate green at 8cdd7f1. `accept.py` not run: nothing it exercises changed. Load 6.1, free disk 176 GB, RAM 16 GB.

## What shipped
`docs/changelog/media/2026-11-02-98-clock-scrub.mp4` (7.0 s, 69 KB) from the new
`tools/demo/storyboards/clock_scrub.json` on a scratch `assets/` with a `family/bevy_elements.ron`. Frames:
the ENVIRONMENT panel at noon → 6:00 → 18:30 → 23:00 (night); each button lights in turn, the sky follows,
and **LAST stays `before the first change` while UNDO stays `nothing`** — the line's exact claim (no file
write, no undo step). `check.py`: scene 51.8%, longest static 1.38 s, 1.25 MB — PASS.

## Two facts worth the next pass's time
1. **`bevy_fairy/input` needs the `debugger` feature for a synthetic click to reach a UI widget.** Without
   it the click falls through and *places a piece on the map* — I confirmed that twice before building with
   `--features bevy_elements,debugger`. Every clip that clicks a widget needs that build.
2. **A `bevy_elements` clock jump across the night flashes white for ~2 frames** while auto-exposure catches
   up (measured: 44.8 % → 40.3 % → 2.0 % white). That is the defect #193 already records at 22:00. The clip
   ends on the 23:00 hold rather than showing a 23:00 → 12:00 jump, because a clip ending white would be
   evidence of the wrong thing.

## Clips attempted and dropped
`111-check-view` and `98-panel-season-climate` were each tried and dropped rather than shipped unverified:
- **Check view**: the click works (the journal fills with the endpoint error and the checks it could not
  ask), but `check.py` reads a `record.py` click as a drag (its own cursor event shares the click's
  timestamp) so it never scores the moment, and the three lines paint over two frames. Six takes, no pass.
  `check.py` is protected, so the right move is to report the gap, not to work around it.
- **Season/climate**: the four season buttons write `family_set` correctly (the sheet shows each lit and
  LAST/UNDO updating) but at noon in a temperate climate the four skies are nearly identical, so there is
  no capture change to score.

## Bugs
None fixed. #193's exposure flash is the cause of the clip's tail being trimmed; that issue stays blocked
on bevy-elements#55.

## Skipped, and why
#210 + 20 `ui` issues behind it (protected spellings); #235 (deletes pinned assertions); #201 (protected
`assert_ne!`); #152 (protected fixture text); #193 (bevy-elements#55); #112's calibration and #113 (need the
model; `8093` refuses); #195 (memory item already implemented); #197 `wontfix`. The parked pass-218 branch
is stale. #129's allowlist is 91 entries, all camera and view state.

## Review
`reviewer` not spawned; self-review: (1) *does it show the claim* — the sheet is quoted, LAST and UNDO are
the evidence and they are in every frame; (2) *anything else changed* — one storyboard, media, one line;
(3) *protected files* — none; the `no clip` reason was replaced as the rules allow; (4) *could it mislead* —
the clip stops before the exposure flash rather than ending on it, and the commit says so; (5) *what did I
get wrong* — four takes were wasted on a build without `debugger`, and two clips were attempted before
their blocker was understood.

## Test diffs
None.

## Release
None: no packaged crate changed.

## Next focus
The next owed clip whose change the capture can score; report the `check.py` click-vs-drag gap to the owner
so a UI-click demo can be scored at all.
