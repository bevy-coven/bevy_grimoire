# 2026-11-02 demo: a storyboard clicks a widget by its words (#231)

Focus: the standing demos' cause of lost takes · Commits: 292f7cb..d9cc52e · Machine: big · Outcome: shipped (a tooling fix and two demo refreshes; #231's clip still owed)

## Summary
Three passes lost clip takes to storyboards that clicked by pixel. This pass fixed the cause instead of
working around it: `click_label` asks the running editor where a widget's words are drawn and clicks that
centre. Two storyboards that had been the victims were converted, and the measurement shows the pixel
version had still been missing — 2.12 % vs **9.07 %** on the same step, both above the 5 % bar, so neither
run had said anything.

## Baseline
Gate: green (`292f7cb`). `accept.py`: not run. Load 10.3, 150 GB free, 13 GB available RAM.

## Enhancement: `click_label` in `tools/demo/record.py`
Goal: a storyboard step that cannot drift when the layout moves.
Files: `tools/demo/record.py` (the step and `widget_centre`), `tools/demo/test_record.py` (+3 cases),
`tools/demo/README.md` (documented), `storyboards/place_and_undo.json`, `storyboards/build_a_map.json`.
Proof: three cases fail to compile/run without `widget_centre`; the converted storyboards both PASS
`check.py` (place 9.07 %, undo 8.28 %; and build_a_map 15.13 %).

**Three facts the live editor taught**, each now a test:
1. `UiGlobalTransform` reflects as an **affine array** `[a,b,c,d,tx,ty]`, not an object with a
   `translation` — reading it as an object skipped all 394 palette texts and reported "no visible widget"
   for a full palette.
2. `Text` reflects as its **bare string**.
3. A row scrolled out of its panel sits at y = 1900 in a 720 px window, so a centre outside the window is
   refused rather than clicked into nothing.

## What is still owed on #231
**Its clip**, and the reason is now named rather than guessed: a fill of a *floor* tile on an
already-floored bedroom changes too little of the picture for `check.py`'s 5 % rule at any framing. Five
takes were tried; the ones whose substance was right (the Fill button lit with its F keycap, the note
*"asked for fill with the brush from the toolbar"*, `13 placed` after the flood) still failed the numeric
bar. The storyboard is dropped. The next pass should pick a subject whose fill is visible — an empty room,
or a piece with a silhouette.

Also unchanged: the mode group's click, with three measurements recorded on the issue.

## Performance
Not measured (`perf.py` needs a real display).

## Review
Reviewer not spawned. The five questions, mine: the tool fixes the stated cause and its tests come from
real failures; the risk is that `click_label` matches text, so two widgets with the same words resolve to
the first — acceptable for a recorder, and the refusal is loud; the demos' captions still describe their
files; the clip remains honestly owed.

## Test diffs
`tools/demo/test_record.py` +3 cases (not protected). Two storyboards changed their click steps; both were
re-recorded and both pass.

## Release
None: `bevy_cauldron` is `publish = false` and this pass touched only `tools/` and `docs/`.

## Next focus
#231's clip on a subject whose fill shows, aimed with `click_label`.
