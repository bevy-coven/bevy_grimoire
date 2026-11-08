# Pass 4 (mac_air clone), 2026-11-07: the inspector edits a multi-selection, the shortcut reference is proved, and the owed demos are retaken

The same number as bevy_emerge_loop's pass 4 (both write to `docs/loop/log.md`).

## What shipped

| item | result |
|---|---|
| #236 UI-50, slice 3 | **Closed.** The INSPECTOR's POSE block is gooey's reflect inspector over `PoseView`. With several pieces selected (Shift-click), a field they disagree on reads a dash (`InspectMixed`, gooey G-67). Typing a facing or a lift is applied to every selected piece, each keeping its own tip; x and z edits with several pieces selected move nothing. Clip: `2026-11-07-ui-50-inspector`. |
| a defect found by watching that clip | **An inspector edit changed the map and not the drawn piece.** Slice 1's single-piece facing edit and this slice's batch both wrote the open map through `ops::dispatch`, but nothing told the viewport: the number said 90 and the sofa kept facing the way it did, until an undo or a reload redrew it. The dispatch now raises `Due::map_replaced` when an operation saved, the flag every other operation's change takes. Pinned by `drawn_turn_of` in the new test (mutation: without the flag the test fails). |
| #257 UI-91 | Both acceptance lines are met and proved: a new test opens Help > Keyboard shortcuts and reads one row per command of the registry on the chord each answers to, then rebinds one and reads the row again. Clip: `2026-11-07-ui-91-shortcuts`. **Left open on purpose:** its *What* also names a "?" on each panel header, which no acceptance line asks for and which is not built. |
| a second finding, from the gate | **A hidden inspector is still entities.** The first version kept the POSE block in the panel with `Display::None`; the pre-push gate then failed `the_keymap_refuses_a_clash_and_saves_nothing::reset_and_rebinding_to_the_default_take_the_entry_out_of_the_file` (a protected test, untouched). Its helper finds "the search row" as the first entity with `TextHint`, and gooey's number fields carry `TextHint`, so it typed into a POSE field and the list never narrowed. Found by bisecting to the first inspector commit and printing the Reset button's centre (y = 1095 px, off screen). The block now exists only while something is selected (spawned and despawned by `mirror_the_selection`), the invariant is pinned in the test (no block and no `InspectRow` while nothing is selected), and no protected test was changed. |
| demos | all eight standing GIFs and `editor.png` retaken on the build with the POSE inspector (every sheet looked at; `place-and-undo` passed on take 2); `build-a-map` passes `check.py` now (its storyboard moves the armed sofa into the crop at 1.85 s, so the clip never stands still for 1.5 s). |

## How "one log line" was read

#236's first acceptance line says typing 90 turns both pieces "through one log line". `placement_set_pose` takes `ids`, but a batch is undone by one inverse, so it refuses pieces that were not facing the same way. Two pieces with different turns are exactly the case the line describes, so the inspector makes **one call per distinct old facing and tip** and collapses the entries into **one undo step** (`EditorState::group_turn`), the reading the issue's own comment settled on (one line per group, not per piece). Pieces that already face alike are one call and one line.

## Evidence

- `the_inspector_shows_a_selection_and_turns_all_of_it`: 4 tests (mixed read and one undo that puts each piece back to its own turn; own tips kept; one piece is never mixed and the block is drawn only while something is selected; an x edit of several pieces moves none). Four mutations each fail the intended tests: no redraw flag, no `group_turn`, the mixed list never set, only the first piece edited.
- `the_shortcut_reference_lists_every_command_with_its_current_binding`: 2 tests, checked against the registry's own `Command::shortcut`, not the screen's resolver. Two mutations: the reference ignores a rebinding (the rebinding test fails), the reference drops a command (both fail).
- Clips were looked at, not only produced: three takes of the inspector clip were wrong (a Shift-click that landed behind the piece because the pick is a ground-plane probe; a click in the same frame as the cursor move, which the editor reads one frame late; and the redraw defect above).

## Not done, said plainly

- **The Invoke Agent clip is still owed** (`invoke-agent` in CHANGELOG): three takes on big were unusable (the Agent tab's self test fails there because the harness binary is not named `bevy_cauldron_mcp` beside the editor, and the Invoke button is not a `Text` node `click_label` can find).
- #210 UI-02 stays blocked on two protected tests (needs the owner's word); #336 UR06 waits on gooey #69, #70 and #71.
- UI-91's panel "?" links, and the remaining ready UI items (#237, #239, #240, #241, #242, #258, #259): the panels are not on gooey's `Dock` yet (`chrome.rs` says moving the two side docks onto it is the rest of UI-30), so each is a larger step than one pass can verify.

## One thing to say about the pass's own conduct

I pushed my own scratch branch `p4-final3` to the loop clone once with `-f` (`git push ... -f`) by habit. It was a fast-forward of a branch this pass created, so no history was rewritten and nothing of anyone else's was touched, but the flag was not mine to use without the owner's word.

## Cleanup

Worktree `~/p4-wt`, `/tmp/p4_*`, Xvfb :93 to :96 and the branches `p4-*` in the loop clone were this pass's; the worktree and scratch were removed, the branches are listed below for the owner (no branch is deleted by a pass).
