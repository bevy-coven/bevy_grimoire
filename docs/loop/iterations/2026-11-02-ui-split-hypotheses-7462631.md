# 2026-11-02 ui: #224's split — three hypotheses tested, three wrong, none shipped

Focus: #224 UI-30 · Commits: 7462631..7462631 (none) · Machine: big · Outcome: nothing shipped

## Summary
Took the shell split with a specific diagnosis neither earlier attempt had: `split_pane` ships no
`Pickable::IGNORE` and the frame forbids any pickable node under it. Measured; wrong. Then measured width
and `Hovered`; both fine. **Three failed attempts at one step — the command says stop, and it is right.**

## Baseline
Gate: green (`7462631`). `accept.py`: not run. Load 7.6, 150 GB free, 15 GB available.

## What was eliminated
1. **`Pickable::IGNORE` on the split and its panes.** Added via `adopt_the_shell_split`; also hit and fixed
   the duplicate-`Node` panic from pass 251 (`split` brings its own `Node`, so a second is a bundle
   conflict — set every field after the spawn instead). `a_shift_click_that_picks_no_piece` still failed
   with an empty status line.
2. **The viewport's width after adoption.** `split_pane` is a `Display::Grid` with one `1fr`; the viewport
   asks for `flex_grow`, a flex property that does nothing in a grid, which would break `world_to_viewport`.
   A test asserting width > 1 px **passed**.
3. **The panes carrying `Hovered`.** `view::over_ui` asks `Query<…, With<Hovered>>` globally, and `Hovered`
   is true for an entity *or any descendant*. A test querying every `Role::SplitPane` child for
   `Option<Hovered>` **passed**.

So the split is structurally right, unpickable, correctly sized and non-hoverable — **and the click still
does not land.**

## Reverted
`chrome.rs` and the two diagnostic tests are reverted: the tests name `ShellSplit`, which only the feature
adds, so they cannot ship without it. They are three lines each and worth reinstating the moment the split
lands.

## The next question, which has an answer rather than a hypothesis
`view::over_ui` reads **every** node with `Hovered` in the world. With the split in the tree, **which node
carries it?** The frame's descendants are the suspects; `split`'s observers and whatever `dock`/`tabs`
machinery it spawns may add one. Enumerate them rather than guess a fourth mechanism.

## Review
Reviewer not spawned. Mine: (1) *Fails before?* Each hypothesis was measured, not assumed. (2) *Is the
failure the claim?* Yes — and each measurement eliminated a real candidate. (3) *Right layer?* The third
test asks the same question `over_ui` asks. (4) *Protected files?* A test was added and reverted with its
feature; nothing existing changed. (5) *What remains?* Enumerate the `Hovered` nodes with the split present.

## Test diffs
None in the shipped tree. Two diagnostic tests written, run, and reverted with the reverted feature.

## Release
None.

## Next focus
With the split in, list every entity carrying `Hovered` and name the one that should not.
