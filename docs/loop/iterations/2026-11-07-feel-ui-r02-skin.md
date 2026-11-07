# 2026-11-07 feel series: UI-R02 the editor's skin states its depth

Focus: epic #330 (owner's steer) · Commits: 0b9ef016 · Machine: big · Outcome: shipped (smaller than the issue's title)

## Summary
UI-R02 (#332) asked for the editor's skin to become `SkinPreset::Desk` or a `from_seed` skin with elevation stacks. The editor keeps its own colours and measures, and `editor_skin.ron` now states gooey's elevation stacks as its own data.

## Baseline
Editor tests at the pin before the work: 1872/1872. `accept.py` not run: no map, library or operation code changed.

## Enhancement: UI-R02
- **What changed:** `editor_skin.ron` gains an `elevation:` block (raised, floating, overlay: a drop shadow and a hairline ring each).
  - The values equal `Elevation::on_dark()`.
  - They are not new on screen: `GooeySkin` is `serde(default)` and its default is `dark()`, so the editor already drew these stacks once UI-R01 moved the pin. I assumed first that the default was empty. The reviewer found that (P2), and I reworded the file's comment and the commit.
- **Not adopted, and why (assumed decision, on the issue):**
  - **The Desk preset.** Protected tests pin the editor's colours, radius, padding and gap to `chrome.rs` constants (`tests/editor_skin.rs`, the state- and other-colour tests). Desk's colours differ: the editor's greys are warm and its accent is amber, where Rune's neutral is cool and blue. Its type scale (13 against 11) and spacing would reflow every panel.
  - **`from_seed` with the editor's seeds.** The derived ladder would not equal the measured one those tests hold.
  - The owner can reopen both by editing `editor_skin.ron` and the constants together.
- **Whole-pixel text** (G-52) needs no editor change: gooey writes it.
- **Proof:** `tests/the_editor_skin_has_depth.rs`, 3 tests.
  - **Ladder and heights:** `problems()` is empty, the eight surfaces are strictly lighter step by step, and the three stacks are each softer and drop further.
  - **File states its stacks:** the RON names `elevation`; the test fails if the block is deleted.
  - **Running editor:** a panel in the running editor casts one shadow per raised layer.
  - The first and last tests hold at the base too: they guard the acceptance, not a change. Only the stated-in-file test is new behaviour.

## Borrowed from Mixar
None.

## Performance
Not measured.

## Acceptance
`accept.py` not run (no map, library or operation code changed). Acceptance item 1 (`problems()` empty, surfaces a ladder) holds and is now tested. Item 2 (clip) is owed: no display on big.

## Review
An independent `reviewer` ran: one P2 and nothing else. The P2: the RON addition equals gooey's default, so the first draft's tests could not fail without it. I reworded the file and commit, added the stated-in-file test, and dropped an incidental equality pin I had added. The data matches `Elevation::on_dark()` exactly; no editor code or test is affected by shadows; gooey writes shadows only on change.

## Test diffs
New `tests/the_editor_skin_has_depth.rs` and its `mod` line in `tests/all.rs`. Nothing else.

## Release
See the log line.

## Next focus
UI-R03 #333 (buttons by role, sizes, the dialog's default button) needs G-51, G-57 and G-58 (#58, #64, #65) and G-66: open, so blocked. UI-R04 #334, UI-R05 #335 and UI-R06 #336 need open gooey issues too. The next gooey issues to build: G-53 hover groups (#60), then G-51 button variants (#58).
