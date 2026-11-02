# 2026-11-02 issues: 34 issues carry `blocked`, and a missing gooey reveal key

Focus: the owner's ask (label everything blocked), then the only ready `ui` issue · Commits: 97c1e8d..HEAD · Machine: big · Outcome: fixes only

## Summary
The owner asked for a `blocked` label on every blocked issue. All 34 were computed from each issue's own
`Depends on` / `Needs` lines (not by eye) and labelled with a comment naming the exact blocker. The only
ready `ui` issue, #235, turned out to need a capability `bevy_gooey` does not have, which is now filed as
G-32. Two candidate slices of #129 were tried and rejected because the core forbids the act.

## Baseline
Gate green at 97c1e8d (pass 224's push). `accept.py` not run: nothing it exercises changed. Load 5.6, free disk 175 GB, RAM 14 GB.

## What shipped
- **34 issues labelled `blocked`**, each with a comment naming its blocker. Created the label
  (`B60205`, "Cannot be worked until something else closes").
  - 27 wait on an open `ui` dependency; the root is **#210**, blocked on protected `chrome::NAME` spellings.
  - #210, #152, #201, #193, #112, #113 wait on a protected file, an upstream crate, or the decision model.
  - #260 waits on 18 open issues; it is the series' last step.
  - Left unlabelled because they are workable or not blockers: #235 (now G-32, also labelled), #208 (the
    tracking issue), #197 (`wontfix`), #195 (a decided report), #129 (workable, the only one).
- **bevy-coven/bevy_gooey#36 (G-32)** filed and added to UI-00's request list: `ShortcutBadges::hold` is a
  `Modifiers`, but this editor's overlay is opened by **`K`**, a plain key, so `ShortcutBadges` can never be
  revealed by it. Asks for `hold: Reveal { Modifier | Key | Never }` (default unchanged), a readable
  `shown`, and host-supplied legend rows for `keys::Home::Legend`.

## Rejected, and why — each is a real finding
- **`composition_clear`** (clear a composition's members). Built, then refused by the core:
  *"`site/room` has no members. An empty composition stamps nothing, which looks exactly like a stamp that
  failed."* `ClearTile` is therefore draft-only state by design, not a document write. Reverted whole.
- **`CycleGrid`** changes a view `Resource` (`Rung`), not the document — correctly absent from the registry.
- **`UndoTile`/`RedoTile`** do write `library.ron`, but their undo is already the tiles' own two-stack history
  (`build::TileHistory`), the same shape as `edit.undo`; wrapping it would be a second history.

## The clip that would not pass
`tools/demo/storyboards/map_open_menu.json` is a working recipe: it opens `annex`, then `map_open bedroom`
through the operation, and the title and bounds box both follow. But `check.py` measures **2.16 %** at the
switch against its 5 % bar — a map swap at a fixed camera changes only the viewport's contents — and the
static run before the switch is 4.62 s. `E-078`/`E-067` stay `not yet recorded`; shipping a clip that fails
its own check is not shipping it. The storyboard is kept for the next pass to pair with a bigger delta.

## Review
`reviewer` not spawned. Self-review: (1) *is every label right* — computed from the `Depends on` lines, with
#223 re-checked after it slipped the first pass; (2) *did anything else change* — one new storyboard and the
issue labels, no product code; (3) *protected files* — none; (4) *could the labels mislead* — each names its
blocker on the issue, so a wrong label is visible where it is read; (5) *what did I get wrong* — #223 was
missed on the first sweep, and the `map_open` clip cost more takes than it was worth.

## Test diffs
None: no product code changed.

## Release
None: nothing packaged changed.

## Next focus
Every `ui` issue now says what it waits for; the unblock is #210's protected re-pin by an owner/evaluator pass.
