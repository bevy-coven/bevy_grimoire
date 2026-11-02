# 2026-11-02 ui: #231's clip, and the toolbar issue closes

Focus: UI-37 #231 · Commits: 0059d33..d870ac1 · Machine: big · Outcome: shipped (one clip; #231 closed)

## Summary
UI-37's second acceptance item was its clip, owed for four passes. This pass recorded it — *"Clicking Fill
arms the fill exactly as its key does"* — and closed the issue. The palette question that blocked the
earlier takes is real but was not needed: the brush can be armed through `brush_arm`, the operation a
palette click dispatches, so the clip shows the toolbar's Fill and the key's flood.

## Baseline
Gate: green (`0059d33`). `accept.py`: not run. Load 13.5, 150 GB free, 13 GB available RAM.

## Enhancement: the Fill button's clip
Files: `docs/changelog/media/2026-11-02-231-fill-button.{mp4,png}`,
`tools/demo/storyboards/fill_from_the_toolbar.json`, `CHANGELOG.md` (one line's reason replaced by its clip).
Proof: `check.py` **PASS** — scene 48.7 %, longest static run 1.33 s, 0.10 MB, 97 ticks.
Contact sheet read: `BRUSH sofa` → **Fill** clicked with its `F` keycap → *"there is no ground under the
button"* → `F` → `13 placed`, *"filled 2 with sofa"*.

## Four take costs, now written down
1. **`brush_arm` as a `chunks[].operation` is a no-op for the checker**: it writes nothing
   (`writes(&[])`), the chunk reports 0.00 %, and `check.py` fails the agent moment. As an `mcp` step it
   costs no moment.
2. **A viewport click is a "place" moment** even when it places nothing, so a storyboard that only wants
   the pointer over the ground uses `cursor`. That removed a spurious 5 % failure.
3. **A sofa flood changes 3–5 % of a frame**: the whole window measured 3.40 %, the room alone 4.70 %, and
   only the tightened beats cleared the static rule too.
4. **A `chunks[].operation` runs at its chunk's start**, so an arm chunk appended last runs *after* the
   captions that claim it — the ordering trap pass 245 recorded, hit again.

## Still open, and worth its own issue
The palette row's scroll. Pass 246 proved it: the list's `ScrollPosition` is 1553.75 while rows report
laid-out positions, so a click computed from `UiGlobalTransform` lands where no row is. It does not block
any acceptance now, and it should be filed rather than carried in a comment.

## Test diffs
None.

## Review
Reviewer not spawned. Mine: the clip shows what the line claims; the storyboard is reproducible; the
palette question is recorded as open rather than folded in.

## Release
None: tools and docs only.

## Next focus
The next ready `ui` issue, or #231's successor items in UI-00.
