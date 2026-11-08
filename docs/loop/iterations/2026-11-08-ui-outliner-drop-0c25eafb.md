# Pass 5 (mac_air clone), 2026-11-08: the outliner, a piece dragged onto the map, and four traps the protected tests found

Focus: the editor's UI on bevy_gooey (panels UI-50..61, UI-91..) · Commits: 28669dc3..0c25eafb and the docs and release that follow · Machine: mac_air edits, big builds and tests · Outcome: shipped

The same number as bevy_emerge_loop's pass 478 (both write to `docs/loop/log.md`).

## The cluster

| issue | state |
|---|---|
| #237 UI-51 the outliner | **Slice committed, both acceptance lines met, left open.** View > Show outliner puts an OUTLINER section at the top of the INSPECTOR: a search box over gooey's `tree`, the map's pieces grouped by the first `kind` of their descriptor. `lamp` leaves the lamps and their group; a row selects its piece as the primary (Ctrl or Shift selects several); the tree follows the viewport. Clip `2026-11-08-ui-51-outliner`. Not built: compositions, people, triggers and notes as groups, visibility and lock toggles. |
| #238 UI-52 the library browser | **Slice committed, its acceptance line and clip met, left open.** Every PLACE row is a drag source (`PieceDrag`); the dock that covers the viewport is the drop target; a drop places the piece at the pointer through `place_one`, the click's own commit, in one undo step. Clip `2026-11-08-ui-52-piece-drop`. Not built: the thumbnail grid with tag filters as its own panel. |
| #239 UI-53 the history panel | **Skipped, with the reason on the issue.** A gooey table over the log would replace the hand-built `HistorySlot` window that three protected test files pin (`a_history_jump_is_one_undo_group`, `the_editor_chrome_speaks_plainly`, `the_history_lists_only_the_rows_on_screen`); an assumed decision is recorded for the owner. |
| #240, #241, #242, #243, #246, #247, #258, #259 | **Not taken.** #240 needs relation violations the editor does not have; #258's own recommendation (a `panel.show_*` command per panel) is still not met; the rest are panel conversions of the same shape as #239, each a protected-test question. Said on the issues earlier or in this report, not re-argued. |

## Four traps, each found by a protected test or a clip, not by my own tests

1. **A hidden or always-present gooey widget is still entities that other screens find.** The first outliner was a closed gooey `section` with a text box and a tree, spawned with the INSPECTOR. The pre-push gate refused it on four protected tests: two dialogs and the background-jobs band count every `ButtonText` in the world (a section's header is a button, so "OUTLINER" was counted as an answer), and the settings search takes the first empty `TextInputValue` as "the search box" (mine). The outliner now exists only while View > Show outliner is ticked (a menu row, which the bar does not count as a button); the tick follows whether the section is there. Pass 4 hit the same family (the hidden POSE block's `TextHint`).
2. **A system with `Res<OpenMap>` panics on a door with no map.** The first outliner systems did, through the default error handler, in a protected Kit-door test; they take `Option`s now, and the section is queued into the pane so a door change in the same frame cannot hand a command a despawned pane.
3. **gooey sends `DroppedOnWorld` only over nothing the interface draws, and this editor draws a dock over the whole map.** My first observer listened for `DroppedOnWorld`, passed its test (the test triggered the event itself) and did nothing in the real editor: the recording showed the drag lifting, the map lighting as a target, and nothing placed. The dock is the `DropTarget<PieceDrag>` now and the test asserts it carries the target.
4. **A python insertion that walked back to a doc block dropped everything after `fn drive_place`** (`editor.rs` lost 357 000 characters in a commit, found by the compiler the next minute and repaired from the previous commit). The repair is its own commit; `git diff HEAD~1 --stat` on the file is now the check after any scripted insertion.

## Evidence

- `the_outliner_searches_and_selects`: 5 tests. Three mutations, each failing the intended test: the search filters nothing, a row selects nothing, the tree does not follow the viewport.
- `a_library_piece_dropped_on_the_map_is_placed`: 3 tests. Three mutations, each failing the intended test: the drop places nothing, a row carries nothing, the dock is not a target.
- Gate: 3166 tests, 260 s on the push that landed (the first push was refused for the four tests above).
- Every clip and contact sheet looked at. The drop clip took seven takes: a piece that goes on a surface was refused (a lamp), a drop onto the bed replaced the bed (a click does the same; the clip's setup vacates the bed first), and one click row coordinate was a group heading.

## Demos

Seven of the eight standing GIFs and `editor.png` are retaken on the build with the outliner (hidden by default) and the drop, every sheet looked at. **`place-and-undo` is owed**: `check.py` measures 3.0 % of scene pixels changed within 0.2 s of the place click and wants 5 %, on eight takes (four with the gate and three builds running beside it, four alone). Its sheet reads right (filter, arm, click, the sofa placed, Ctrl+Z), so the cause is the placed sofa's mesh landing after the 0.2 s window under software rendering, the same borderline take 1 of pass 4 showed (3.05 %). Neither the hidden outliner nor the drop changes what the clip shows, so the file in `docs/media/` is kept as it is and is still true; its README *Captured* column is left on its old commit so `stale.py` keeps saying it is stale. The next pass should either widen `CHANGE_WINDOW_S` for a place (a change to `tools/demo/check.py`, which is not mine) or retake it on a lighter machine.

## Release

0.4.8 (the editor's `version.py next`), tagged after the gate.

## Not done, said plainly

- The Invoke Agent clip is still owed (pass 4's reason stands).
- #210 UI-02 waits on the owner's word about two protected tests; #336 UR06 waits on gooey #69, #70 and #71.
- The outliner has no visibility or lock toggles and no groups but placements; the library browser is not a panel.

## Cleanup

Worktree `~/p5-wt` and `/tmp/p5_*` were this pass's and are removed; the `p5-*` branches in the loop clone are listed for the owner.
