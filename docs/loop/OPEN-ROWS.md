# The nine rows the loop cannot move on its own

Written by pass 399, when the queue returned `nothing left to answer` for the fifth consecutive pass. The
ledger is the authority; this is the same nine rows sorted by **what would actually move each one**, so a
person can act without reading nine `reason` strings.

Nothing here is a pass's opinion of priority. It is the classification the passes' own reasons already
support, gathered in one place.

## A. Waiting on a pin bump — 2 rows, no work here

Both are in `bevy_gooey`, both already filed, both with an in-tree `#[ignore]`d test named after the issue.

**This is not a forgotten bump — checked by pass 400.** This repo's `bevy_gooey` pin is
`1cc4c7da2b3e21d0e9c1c50a7d2d3b1f6e3ddf05`, and `git ls-remote` says that *is* gooey's `main` today. Both
issues are still `OPEN` on that repository. So these two rows wait on upstream work that has not been done
yet, not on a pin this repo failed to move; the fix lands in `bevy_gooey` first, and this repo's rev follows.

| stem | issue | what |
|---|---|---|
| `feedback-1793927671762` | [gooey#44](https://github.com/bevy-coven/bevy_gooey/issues/44) | the `Role::PromptKey` layout arm sets `min_width` and never centres its content, so a keycap's letter sits off-centre |
| `feedback-1793927744511` | [gooey#45](https://github.com/bevy-coven/bevy_gooey/issues/45) | `shortcut_label` emits `⌘⌃⌥↩⎋⇥`; this repo's FiraMono lacks 6 of them and `system_font_discovery` is off, so Mac chords draw as tofu |

## B. Waiting on an instrument — 4 rows, one tool would likely clear all

Every one of these was reproduced or measured, and then could not be tested because the headless harness
cannot reach the state: menus spawn `Display::None` and `show_menu` is `pub(crate)`, so no test can open one;
and `Hovered` is not driven by `DebugCursor` the way it is live.

| stem | what it needs |
|---|---|
| `feedback-1793927950267` | press the What's new tab — a **synthetic `Activate` already switches the page**, so this may not be a defect at all; the live click could never be shown to fail |
| `feedback-1793928048623` | reach the Meshes tab so the Pieces panel is on screen; the containment test passes headless because the panel never overflows there |
| `feedback-1793928091805` | open a menu, then measure the shortcut column — the alignment is likely a consequence of gooey#45's tofu widths rather than a layout fault (`MenuText flex_grow=1`, `MenuShortcut flex_shrink=0`) |
| `feedback-1793928159971` | reproduce the hover offset (measured 38.5 logical px below the pointer) *with* a working hover — needs a live sweep on mac_air in **logical** px, which pass 392 established is the cursor's space |

**The one tool that would unblock B**: a picking/hit report — for a given surface point, name the entity
`bevy_picking` reports and whether it is `Hovered`. Cheap, reusable, and it is also exactly what row D needs.

## C. Specified, waiting on build time — 1 row

| stem | state |
|---|---|
| `feedback-1793968919654` | **specification complete, implementation not started.** Collapsing and its key already exist (`folded_packs`, `Space`); the gap is the default. `scan`'s first look folds every pack the kit does *not* draw from, so every pack it *does* draw from starts open — the inverse of the note. The blocker is ordering: the fold set is computed before `state.selected` settles, and the selected row's pack must stay open or the first look highlights a row nobody can see. One pass with its 20 build-minutes should do it. |

## D. One measurement from a diagnosis — 1 row

| stem | state |
|---|---|
| `feedback-1793969010186` | **reproduced with a real picking click** (Settings leaves `SettingsDialog` closed; `write_message(OpenSettings)` opens it, so the handler is fine). Ruled out: the chip is laid out (260x28), no ancestor swallows the press, `Pickable::IGNORE` on the button's `Text` does not fix it. **Needs**: which entity `bevy_picking` reports as the hit at that centre. See B's tool. |

## Two traps the passes hit, so they are not hit again

- **`find_by_label` returns the `Text`, not the widget**, when there is no `TabIndex` ancestor — clicking what
  it hands back clicks a label with no observer. Walk to the `Button` ancestor.
- **The injected cursor takes logical px; `ComputedNode`/`UiGlobalTransform` report physical.** Converting once,
  `/ scale_factor`, is correct. Passes 391 and 392 recorded the opposite for a while and it sent two rows down
  a wrong path.
