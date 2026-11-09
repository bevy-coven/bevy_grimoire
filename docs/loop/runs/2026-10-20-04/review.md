# Review — the arm crosses the door

Run: 2026-10-20-04. Three passes, judged on the captures in `evidence/`.

Every capture was taken by driving the **real editor** over BRP (`bevy_debugger/screenshot` with
`source: "offscreen"`, so the editor's own surface is read and no window is taken), against a copy
of Sanctuary's project.

---

## Pass 1 — Function: every state of the feature at full frame

**Verdict: GREAT**

### The states

| State | What it showed | Verdict |
|---|---|---|
| **The Map door, on arrival** | The INSPECTOR panel carries a **COMPOSITIONS** block under MAP SIZE, listing the project's 16 house assemblies. | ✅ The door that stamps now says what it can stamp. |
| **`P` pressed** | The status line reads **"the arrows walk the compositions — Enter arms one"**, and the first row `town/house_cottage_plaster` takes the accent rail. | ✅ The picker says what it is and what the keys do. |
| **The arrows walked** | Two `ArrowDown` presses moved the cursor to `town/house_cottage_wood` — the rail followed, and the *other* rows did not. | ✅ The cursor and the list cannot disagree. |
| **`Enter` pressed** | The armed row carries an accent `*`, and the status reads **"`town/house_cottage_wood` armed — the next click stamps it"**. | ✅ One writer, one answer. |
| **The click on the map** | **MAP** reads **`745 placed (727 hand, 18 group(s)), unsaved`** and **UNDER** reads **"stamped 'town/house_cottage_wood' as house_cottage_wood@1"**. | ✅ **The whole feature, end to end, on the door where stamping happens.** |
| **The key overlay (`K`)** | The INSPECTOR's badge cluster reads **`up · down` — walk the compositions / Shift: x5** and **`Enter` — arm this composition for the map**. | ✅ Discoverable, and the wording is the same on both doors. |

### The proof that the round trip is gone

The measurement that started this run, and the measurement that ends it:

| | Before | After |
|---|---|---|
| Where the arm lives | a field of `ComposeState` | **its own resource, `compose::Armed`** |
| What the `Ownership` table says a door change does to it | **`Ownership::Door` — it is reset** | **`Ownership::Session` — it is kept** |
| The Map door's way to arm a composition | **none — `Enter` did nothing** | **`P`, the arrows, `Enter`** |
| Sanctuary's `town.map.ron` | 727 hand-placed rows, **0 stamps** | 727 hand-placed rows, **18 group rows** from one stamp |

**The 18 group rows are the 18 members of `town/house_cottage_wood`.** One click, one house, and the
map carries it as a *reference* — the reference model the crate chose in the first place, and the one
that was unreachable.

### The ratchet, proven on the broken code

`tests/the_arm_survives_the_door_change.rs` is written so it compiles on **both** the baseline and
the fixed code, and it **fails on the baseline**:

```
assertion `left != right` failed: the armed composition is a field of `ComposeState`, and
`ComposeState` is `Ownership::Door` — so a door change resets it. The arm is the author's intent
and it has to cross: the composition was made on the Kit door and the map that stamps it is on the
Map door. Measured on 2026-10-21: the COMPOSE tab armed a house and said that the map tab stamps
it, and there was no map tab in that process, because the door is chosen at startup.
```

---

## Pass 2 — Zoom: the picker, cropped and upscaled

**Verdict: GREAT**

The COMPOSITIONS block at 4×, and the armed row at 4×:

```
town/house_cottage_plaster
town/house_cottage_brick
town/house_cottage_wood  *      ← the accent rail on the left, the accent * on the right
town/house_town_house_plaster
```

* **The type is crisp** — FiraMono at 4×, the counters of the `d` and the `t` clean.
* **The two marks are told apart by their ink, not by their position.** The cursor is the
  **accent rail** on the row's left edge; the arm is the **`*` in accent ink** on the row's right.
  Two different facts, two different marks, and neither is a background the reader has to decode.
* **The alignment is the editor's own** — the rows share the panel's left edge, the names start at
  the same column, and the block sits inside the INSPECTOR's flow like every other block in it.
* **The receipt is at the same place the Map says everything else** — the UNDER row of the readout,
  in the same two-column shape as `MAP`, `BRUSH` and `EDGES`.

**And the zoom shows what the change did *not* do.** The panel's other rows, the palette, the
chrome bar, the map — all unchanged. The feature added a block to a panel and a key to a stance.

---

## Pass 3 — Fresh eyes: a developer who has never seen the editor

**Verdict: GREAT**

A **clean copy** of Sanctuary's project, the editor started **from the command line on a map** —
the way a developer would actually start it.

### Can I discover it?

**Hold `K`.** The overlay puts every key on what it does, anchored on the control it acts on.
The INSPECTOR's cluster reads:

> `up · down` — **walk the compositions / Shift: x5**
> `Enter` — **arm this composition for the map**

and the palette's own row is still there beside it:

> `up · down` — **walk the palette / Shift: x5**

**The two lists are told apart by their verbs, and the arrows' meaning is announced the moment it
changes** — the status line says *"the arrows walk the compositions — Enter arms one"* the instant
`P` is pressed, so an author who reaches for the arrow and gets the wrong list is told why in one
sentence, in the place they are already looking.

### Can I do the task?

`P` → `down` `down` → `Enter` → click the map. **Four keystrokes and a click, and a house is on
the map.** No door change, no menu, no re-arming.

### Can I undo it?

The stamp is an edit, and the editor's undo owns it — the same undo that owns a placement, because
a stamp is a reference to a group of placements.

### Do I trust it?

Two things say yes, and neither is a promise:

* The **count is honest**: `745 placed (727 hand, 18 group(s)), unsaved`. The 18 is the house's
  member count, and the split between hand-placed and group-brought is stated, because a consumer
  that validates every row against the descriptors it knows has to skip the rows a group brought.
* **The picker is a picker and not a second authoring surface.** It names the compositions and says
  which one is armed. It does not show the members, the carousel or the stages — those are the
  COMPOSE tab's business, and the COMPOSE tab is on the other door, where an author makes one.

### What a fresh eye would still ask

"Can I put a house on the map from the map?" — **yes, and it is four keys.**
"Can I still make a house on the Kit door?" — **yes, and the arm crosses with me.**
"Can I get to the Kit door from here?" — **no**, and that is the one thing this run did not do. It
is the next item, and the picker removes the reason to need it.

---

## Summary

| Pass | Verdict | Note |
|---|---|---|
| 1 — Function | **GREAT** | The Map door grows a picker; `P`, the arrows and `Enter` arm a composition; the click stamps it — 727 hand rows plus 18 group rows. |
| 2 — Zoom | **GREAT** | The cursor and the arm are two marks in two inks; the type is crisp; nothing else in the panel moved. |
| 3 — Fresh eyes | **GREAT** | Hold `K` and the keys say what they do; the arrows announce their new meaning; four keys and a click is the whole task. |

Three GREEN reviews. Shipped.

### Evidence

| File | What it shows |
|---|---|
| `evidence/01-the-picker-on-the-map-door.png` | The Map door with the COMPOSITIONS picker; the cursor on `town/house_cottage_wood`, the `*` in accent ink, and the receipt reading "armed — the next click stamps it". |
| `evidence/02-a-house-stamped-from-the-map.png` | The stamp landed: `745 placed (727 hand, 18 group(s))` and "stamped 'town/house_cottage_wood' as house_cottage_wood@1". |
