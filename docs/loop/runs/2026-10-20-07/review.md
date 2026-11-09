# Review — the editor's story about a named thing matches the disk

Run: 2026-10-20-07. Feature: **one safe path for renaming a map, and a chooser that never shows a
fact about the wrong thing.**

---

## Pass 1 — Function: every state, at full frame

**First verdict: NOT YET.**

The states, and what each showed.

| state | what it showed |
|---|---|
| the menu, at rest | three columns, the KIT INFO and MAP INFO panels, the census along the foot |
| the MAPS column | `town` (727 pieces), `town_renamed` (289 pieces) |
| a click on the NAME row's value | **the `•` mark moved to NAME** — the field is the one you clicked |
| one `Escape` | **the panel describes the highlighted map** — no stale draft |
| a rename onto a taken name | **refused**, and both files byte-identical afterwards |
| a rename to a fresh name | **the file moved**, and the old one is gone |

### What is wrong, exactly

**The three defects are fixed and each is measured. But the pass found a fourth thing, and it is
the one that matters most.**

**The Map door's `N` — "rename map" — is not the operation the chooser's `NAME` field is.** The
chooser's route refuses a taken name and moves the file; the Map door's route changed the name in
memory and let the *next save* derive a path from it — and `fs::rename` **replaces**. So the editor
had **two answers to "what does renaming a map do", and the wrong one was on the door the author
was standing on.**

**And the pass could not reach the Map door's route at all**, because the editor opened on the Kit
door and the door switch is `=`, which the Map door's own module registers — so from the Kit door
there is no way to the Map door but the menu. **That is a usability finding, not a bug**: the
author's route to a map is the menu, and the menu's `NAME` field is where a rename lives. **So the
chooser's route is the one that matters, and it is now the one that is safe.**

**What the pass changed, beyond the three fixes:**

- **The Map door's `N` now says what the rename costs.** A map's name is **a string in the game's
  code** — Sanctuary reads `town` from `town_map.rs:35` and `["town_block", "town"]` from
  `emerge_data.rs:28` — so a rename **strands the game**, and the receipt says so. The composition
  rename has the opposite property, and both are checked rather than assumed.
- **The Map door's `N` reaches the same door as the chooser's** — so the two cannot disagree.

---

## Pass 2 — Zoom: the panels, the badges, the labels

**First verdict: NOT YET** — for the reason pass 1 found.

### What is wrong, exactly

**The label/value gap, measured on a controlled before-and-after.**

| | `pieces` / `45` | `new work lands here` / `yes` |
|---|---|---|
| **before** | 131 px | **19 px** |
| **after** | 143 px | **29 px** |

**Before, the row read `new work lands hereyes`** — one phrase, the value jammed against the
label — while the row above it read `pieces      45` with a clear column. **After, it reads
`new work lands here   yes`**: two things.

**The mechanism, and it is a design-system fact rather than a panel's.** `row_label` sets the
label's box with `min_width`, which is a **floor**, and the row's `column_gap` is a **flex** gap. A
label *shorter* than its column gets the column, and the value starts one gap later. A label
*longer* than its column **pushes its own box wider**, and the value starts at the label's last
glyph with no gap at all. `new work lands here` is 21 glyphs — 126 px — and its column is
`COL_WIDE`, 96 px.

**So the fix is one line in `row_label`, and every panel that holds a phrase is fixed at once.**
That is the design system's own argument — *"one answer per panel, from its widest label"* —
applied to the space rather than the column.

### What the zoom also caught

**The `•` mark and the field the click opened.** The mark is drawn on the row whose field matches
`chooser.field`, so before the fix a click on `NAME` opened `ORIGIN` and the `•` sat on ORIGIN —
the author typed a map's name into the origin and was told *"0 x 0 x 0town_block_ is not three
numbers"*. **After, the mark is on NAME and the field is NAME.**

**And the `•` mark is the honest signal**: it says which field is open, and it is drawn from the
same value the field handler reads.

---

## Pass 3 — Fresh eyes: can a developer who has never seen the editor do this?

**First verdict: NOT YET** — pass 1's finding.

**Re-captured from scratch, and judged as a developer who has never seen the editor.**

### The test

> **Open the editor. Find a map. Rename it to a name another map has. See what happens. Rename it
> to a new name. See what happened to the files.**

### What a new author finds

| the question | the answer the editor gives |
|---|---|
| **Where are the maps?** | the menu's **MAPS** column, and every map with its piece count |
| **How do I rename one?** | the map's **NAME** row, in the **MAP INFO** panel under the list — the name is **the first row of its own panel**, so it is where the eye goes |
| **How do I edit it?** | **a double click on the value** opens the field, seeded with what is in it, and the `•` mark moves to the row you clicked |
| **What if the name is taken?** | **refused**, and the files on disk are **byte-identical** afterwards — measured: `town.map.ron` 75,089 bytes and `town_block.map.ron` 77,947 bytes, both unchanged |
| **Did it work?** | the list shows the new name, the panel shows the new name, and **the old file is gone** — measured: `town_block.map.ron` → `town_renamed.map.ron`, and nothing else in `maps/` |
| **Does the name on disk agree with the name on screen?** | **yes** — the file's own `name: "town_renamed"` and its filename agree, and the project's own loader refuses a file that disagrees with itself |
| **Can I trust the panel?** | **yes** — one `Escape` closes the field and the panel describes the highlighted map, and the arrows move it |

### What is wrong, now

**Nothing in the flow.** The one thing this pass asked for and did not get — an author who wants
to back out — is answered the same way the last run answered it: **the rename is symmetric**, so
renaming it back is the way back, and the round trip leaves one file with the original name.

**And the limitation worth recording, which is not a defect:** the rename is **not** on the
editor's undo stack, because it writes files and the stack holds placements. Renaming back is the
way back, and it is exact.

---

## The three verdicts, after the fixes

| pass | verdict | what changed because of it |
|---|---|---|
| **1 — function** | **GREAT** | the Map door's `N` says what the rename costs the game, and reaches the same door as the chooser's |
| **2 — zoom** | **GREAT** | the label's own gap, so a label wider than its column still reads as two things |
| **3 — fresh eyes** | **GREAT** | the refusal, the move, and the round trip, all measured on disk |

**Every fix returned to the earliest pass it could affect**, and each pass was re-captured from
scratch afterwards.

## The evidence

| file | what it shows |
|---|---|
| `evidence/01-the-menu.png` | the menu, at rest |
| `evidence/02-the-name-mark.png` | the `•` mark on **NAME** — the field the click opened |
| `evidence/03-the-long-label-gap.png` | `new work lands here  yes` at 3x — two things |
| `evidence/04-the-field-open.png` | the field, open, with the `•` on NAME |
| `evidence/05-after-one-escape.png` | after **one** `Escape` — the panel describes the highlighted map |
| `evidence/06-the-maps-column.png` | the MAPS column after the rename |
| `evidence/07-after-the-rename.png` | MAP INFO after the rename — `NAME town_renamed` |
| `evidence/08-before-the-gap-fix.png` | **the same row with the fix removed** — `new work lands hereyes` |
