# 2026-11-07 feel series: UI-R01 the editor adopts bevy_gooey's motion core

Focus: epic #330 (owner's steer) · Commits: 4f706ad8, 541d7875 · Machine: big · Outcome: shipped

## Summary
UI-R01 (#331) moves the editor's bevy_gooey pin c6b714c → e264418 (v0.40.1, with G-39 to G-46, G-47 to G-50, G-52 and G-44) and adopts it: its dialogs leave on gooey's presence, colours ease, and Reduced motion is gooey's flag. Both needs (G-46 #53 and G-44 #51) were closed by passes 464 and 466.

## Baseline
Gate: not run by hand (the push is the gate). Editor tests at the new pin before the work: 1860 of 1863 passed (palette tests ×2 and one click test). `accept.py` not run: no operation, map or library code changed.

## Enhancement: UI-R01
- **Pin:** `crates/bevy_cauldron/Cargo.toml` (dependency and dev-dependency) and `Cargo.lock`.
- **Dialogs leave.** The command palette, settings and keymap close with `Leave` (fade, then despawn) where they despawned.
  - Chosen palette commands still run during the fade. The marker outlives the close by the exit's length, and `not_typing` and `sense_context` read it as "typing". `editor::palette_is_open` now counts only a palette that is open or coming back.
  - The chord pressed again mid-exit inserts `Stay`: the palette comes back, never a second one.
- **Reduced motion.** `ui::follow_the_motion_setting` writes only `MotionSettings::reduced` from the setting, compared before writing. The setting's words now say what it does.
- **Colours.** `paint_marked_buttons` runs before gooey's `update_color_motion`, so its accent and chosen-tab colours are targets that ease. It takes the `Opacity` above a button, so the chosen tab and its rail fade with a leaving dialog (reviewer P2).
- **Proof:** `tests/the_editor_moves_on_springs.rs`, 3 tests on a 60 Hz manual clock.
  - **Exit:** the palette is still drawn and partly faded (`Leaving::opacity` between 0 and 1) mid-exit, and gone within `motion.default` plus a frame.
  - **Chosen command:** a command chosen mid-fade is not refused as typing, and the chord again brings the palette back.
  - **Reduced:** the setting writes gooey's flag and only it, and taking it out puts it back.
  - The palette test failed before: with `despawn()` the card was gone in the frame it was asked to close.
- **Report fix (own commit, 4f706ad8).** The report's scene cell is `floor(point / TILE)` of a pixel put back through the camera. At the new pin `a_click_on_the_map_picks_the_piece_there` failed alone (cell [2,0] for a piece at 3.0: the point read 2.999999) and passed at the old pin. The cell label now snaps by `CELL_EDGE_SNAP` (1e-4 cell). The test is unchanged.

## Assumed decisions (on the issue)
- The Motion row is the existing "Reduced motion" switch: Full or Reduced. Off stays a host value, since the settings files have no third state.
- The clip acceptance is owed: no display on big (no Xvfb). The changelog line says so; a Mac pass records it.
- The cell snap fixes the product side of a fragile boundary click rather than editing the protected test.

## Borrowed from Mixar
None.

## Performance
Not measured.

## Acceptance
`accept.py` not run (no map, library or operation code changed).

## Review
An independent `reviewer` ran. Two P1s, one P2 and three P3s, all fixed before the commit.
- **P1:** a palette command was refused while the card faded. `palette_is_open` fixes it, and a new test proves it.
- **P1:** a stray, always-failing test file appeared in the worktree (`the_editors_dialogs_leave_on_springs.rs`) and its `mod` line in `all.rs`. I did not write it; its author is unknown. It duplicated my test and asserted a `Leave` that is removed in the exit's first frame, so I removed both from this pass's worktree.
- **P2:** the chosen tab and its rail did not fade. They take the `Opacity` above them now.
- **P3:**
  - A second chord press during the exit was spent. It is `Stay` now.
  - The setting's description and the `EditorSettings::reduced_motion` doc were stale. Both are fixed.
  - The palette test now asserts `Leaving` and a partial opacity.

The fixes were not re-reviewed.

## Test diffs
New `tests/the_editor_moves_on_springs.rs` and its `mod` line in `tests/all.rs`. No existing test changed.

## Release
See the log line.

## Next focus
UI-R02 #332: the editor's skin from three seeds, on the Desk preset. Needs G-47..G-49 and G-52, which are built and in v0.40.1.
