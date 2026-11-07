# 2026-11-07 feel series: G-59 a dialog's default button built in bevy_gooey, released as v0.40.3

Focus: epic #330 (owner's steer) · Commits: bevy_gooey e5ac61b, release b66ec01 · Machine: big · Outcome: shipped (in bevy_gooey)

## Summary
G-59 (bevy-coven/bevy_gooey#66) is the next UI-R03 need that was ready (it needed G-51, closed last pass). Typing a name in a dialog and pressing Return now presses its default button. UI-R03 #333 still needs G-57, G-58 and G-66.

## Baseline
bevy_grimoire was not touched, so its gate and `accept.py` did not run.

## Enhancement: G-59
- **Change:** `src/default_button.rs`, `text_on_key_input` and `Confirm::default`.
  - **`DefaultButton`:** it draws as Submit unless the host chose a variant.
  - **Return in a single-line field:** it ends the edit, then presses the default button.
  - **Return with nothing focused:** it presses the default button of the dialog the player is looking at.
  - **Keeps its own Return:** a focused button, a text area, a number or colour row, and a disabled default.
  - **Two defaults:** they warn once, and the first wins.
- **Assumed decisions (on the issue):**
  - **Number and colour rows:** a refused value is put back and must not be submitted.
  - **Confirm:** an immediate Return presses the safe choice.
  - **Nothing focused:** only the dialog in front answers.
- **Proof:** `tests/default_button.rs`, 8 tests.
  - **Order:** value, then press.
  - **Focused Cancel:** it keeps Return.
  - **Nothing focused:** Return presses the default.
  - **Text area and disabled default:** neither presses it.
  - **Two defaults:** the first wins.
  - **Variant:** Submit unless the host chose another.
  - **Confirm:** it answers with its default.
  - **Two dialogs:** a dialog with no default over one with a default presses nothing.
  - **Number field:** it keeps its Return.
  - The module did not exist before this change.
- **bevy_gooey gate:** fmt, clippy -D warnings, nextest 1158/1158, doctests 255, wasm32 check, rustdoc -D warnings, all green.

## Borrowed from Mixar
None. The reference is Rune's README, "Key routing".

## Performance
Not measured.

## Acceptance
`accept.py` not run: no bevy_grimoire code changed.

## Review
An independent `reviewer` ran: 1 P2 and 4 P3, all fixed.
- **P2:** Return with nothing focused could press a default button under another dialog, or in a hidden one. Only the visible modal dialog highest in the UI stack answers now.
- **P3:**
  - A refused number or colour value was submitted. Those rows keep their Return.
  - The "first wins" order was breadth-first. It is depth-first now.
  - The warning fired once per field. It is once per dialog now, from a shared resource.
  - The `Confirm::default` docs said the opposite of what happens. They are corrected.

My own first run found one bug the tests caught: the key that bubbles to the window after a focused widget handled it counted as "nothing focused" and pressed twice. It is guarded by an empty `InputFocus`. The fixes were not re-reviewed; the gate and 8 tests cover them.

## Test diffs
New `tests/default_button.rs` and its `mod` line in `tests/all.rs`. No existing test changed.

## Release
bevy_gooey **v0.40.3**, https://crates.io/crates/bevy_gooey/0.40.3 (tag v0.40.3, commit b66ec01). `cargo semver-checks`: no semver update required, so PATCH.

## Next focus
G-53 hover groups (#60), then G-57 tooltips (#64) and G-58 scroll glide (#65), which UI-R03 and UI-R04 need.
