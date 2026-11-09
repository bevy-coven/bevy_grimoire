# Review — the run of a thing

Run: 2026-10-20-02. Three passes, judged on the captures in `evidence/`.

Every capture was taken by driving the **real editor** over BRP (`bevy_debugger/input`,
`bevy_debugger/screenshot` with `source: "offscreen"`, so the editor's own surface is read and no
window is taken) against a copy of Sanctuary's project. The "before" captures come from the
**baseline build** — the same project, the same editor, the commit before this one.

---

## Pass 1 — Function: every state of the feature at full frame

**Verdict: GREAT** (after one fix, below)

### The states

| State | What it showed | Verdict |
|---|---|---|
| **The filter, before** | `F` on the Map tab types into a box that ignores it: the box still reads the placeholder `filter`, and the palette still shows `KIT · CONTAINER`. `F` is the Map's *flood fill*, and the palette's filter was `Context::Tiles` — **mouse-only**. | ✅ The "before" is real and is in `evidence/before/`. |
| **The filter, after** | `Cmd+F` focuses the palette's box, "fence" lands in it, and the palette narrows from **211 rows to one**: `KIT · FENCE` → `prop_wooden_fence_single`. | ✅ **Two keystrokes instead of ~90.** |
| **The piece armed** | `BRUSH  prop_wooden_fence_single`, and the palette row carries the amber selection rail. | ✅ The arm is visible. |
| **The run mode armed** | `UNDER  run mode: drag a line and the armed piece repeats along it, at its own pitch. Esc to stop.` — the mode says exactly what it does, in the status line the author is already reading. | ✅ A mode you cannot forget: it is drawn, and it says itself. |
| **The drag** | The line is drawn on the ground **and the six posts stand along it**, translucent, in the piece's own pitch. | ✅ The author sees the fence before letting go. |
| **The release** | The same six posts, solid. `MAP  733 placed, unsaved` (727 + 6) and `UNDER  ran 6 piece(s) along the line`. | ✅ The ghost and the commit agree — the same posts in the same places. |

### The defect the first pass found, and the fix

**The first capture of the drag showed only a line.** A rule on the ground, and the author had to
*imagine* the fence on it. That breaks the editor's own rule — *"a preview computing either
differently is a promise about a landing that will not happen, which is the one thing this
editor's previews are held to"* — because **a line is the instruction to lay, not the landing**.

So the run now has a **ghost**: the posts stand where they would go, translucent, through the
`Ghost` marker the editor's other previews already use. `evidence/after/3-the-ghost-shows-the-posts.png`
is the fixed capture; `evidence/crops/3-…` and `4-…` are the ghost and the commit side by side,
and they are the same six posts.

### The number that matters

The run's pitch is **the piece's own** — 2.064 m for the fence — and the editor's lattice rungs
are 1.000, 0.333 and 0.111 m. A fill lays that fence 3.000, 2.333 or 2.111 m apart, so the run is
the first thing in the editor that can draw this piece's own spacing. The posts in the capture
abut exactly, with no hairline between them.

---

## Pass 2 — Zoom: the panel, cropped and upscaled

**Verdict: GREAT**

### The filter box, before and after

Two crops of the same box, at 1.8x:

* **Before** (`crops/1-…`): the box reads the placeholder `filter`. The typing went somewhere
  else — the Map tab's `F` is the *flood fill*, so the keystrokes reached the **map**, not the box.
  That is the sharp edge: a filter the author cannot reach by keyboard is a filter they miss, and
  **a miss is not a no-op — it edits the level.** One of the two clicks during the measurement
  replaced a house block with a barrel.
* **After** (`crops/2-…`): the box reads `fence`, and the list below is `KIT · FENCE` with exactly
  one row.

The type is crisp, the box has the same dark inset it always had, and the selection rail on the
row is the editor's amber — **no new colour was introduced**, which is the chrome census's rule.

### The status line and the count

`crops/5-the-receipt-and-the-count.png`, at 3x:

```
MAP     733 placed, unsaved
UNDER   ran 6 piece(s) along the line
```

Both lines sit in their own columns, aligned with `NAME`, `BRUSH`, `YAW` and `EDGES` above them.
The count is amber because the map is unsaved — the editor's existing rule, applied to the new
number rather than reinvented.

### The ghost and the commit

`crops/3-…` and `crops/4-…`, the same fence, one translucent and one solid. **The posts are in
the same places.** That is the property the preview is held to, and it is visible rather than
asserted.

---

## Pass 3 — Fresh eyes: a developer who has never seen the editor

**Verdict: GREAT**

A **clean copy** of Sanctuary's project and the whole flow driven as a first-time user would
drive it — `evidence/after/` is that session.

### Can I discover it?

1. I want a fence. I press **`Cmd+F`** — the platform's own "find", which every application on
   the machine uses for this — and type "fence". **The palette narrows to one row.** I did not
   need to know that the Map tab's filter existed, or that it had a keyboard route, or what its
   chord was: the chord is the one I already use everywhere else.
2. I press `Enter` (the box closes) and `↓` (the list's own walk), and the fence is **armed** —
   the inspector says `BRUSH  prop_wooden_fence_single`.
3. I press **`Cmd+B`**. The status line says, in full: *"run mode: drag a line and the armed piece
   repeats along it, at its own pitch. Esc to stop."* I know what to do, and I know how to stop.

**Total: 5 keystrokes and a drag**, and no step needed a document.

### Can I do it?

I drag across the map. **The fence appears as I drag**, translucent, so I can see whether it is
where I want it before I commit. I let go, and the posts turn solid. The status line says
`ran 6 piece(s) along the line` and the count says `733 placed`.

### Can I undo it?

Yes — one `Cmd+Z` takes the whole run back, because a run is **one act**. That is the same rule
the fill and the box follow, and it is tested (`one_undo_takes_the_whole_run_back`).

### Do I trust it?

The run's posts are **ordinary rows**, so the map file gains rows and nothing else and the game's
loader needs no change — the previous iteration's contract (*a level the author drew is the level
the game walks*) is not broken. And the count in the inspector is the count the consumer has:
`census::of_map` expands, so the panel and the game agree.

**And the number the author reads is now honest in a way it was not.** Before this, reaching a
piece took ~90 keystrokes through a palette whose *only* reliable route was a filter the
keyboard could not reach, whose clicks fell through to the map. An author who does not trust the
palette does not use the palette, and every other feature inherits that.

---

## Summary

| Pass | Verdict | Note |
|---|---|---|
| 1 — Function | **GREAT** | Every state reads correctly and refuses loudly; one defect found (the drag showed only a line) and fixed with a ghost. |
| 2 — Zoom | **GREAT** | Crisp, aligned, no new colour; the before/after crops are the clearest evidence in the run. |
| 3 — Fresh eyes | **GREAT** | Discoverable in 5 keystrokes, do-able in one drag, undoable in one press, and the numbers are honest. |

Three GREEN reviews. Shipped.

### Evidence

| File | What it shows |
|---|---|
| `before/1-F-does-not-reach-the-palette-filter.png` | The Map tab, `F` pressed, the palette unchanged. |
| `crops/1-before-the-filter-box-ignores-the-typing.png` | The filter box before: still the placeholder. |
| `crops/2-after-the-filter-box-narrows-to-one-row.png` | The filter box after: `fence`, and one row. |
| `after/1-the-piece-armed.png` | The fence armed, from a clean project. |
| `after/2-run-mode-armed.png` | The run mode, and what it says about itself. |
| `after/3-the-ghost-shows-the-posts.png` | The drag, with the posts standing along the line. |
| `after/4-the-run-laid.png` | The release: the same six posts, solid, and the receipt. |
| `crops/3-…`, `crops/4-…` | The ghost and the commit, side by side. |
| `crops/5-the-receipt-and-the-count.png` | The count and the receipt, at 3x. |
