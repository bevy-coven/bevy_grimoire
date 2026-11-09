# Review — the stamped group the game never sees

Run: 2026-10-20-01. Three passes, judged on the captures in `evidence/`.

Every capture was taken by driving the **real editor** over BRP (`bevy_debugger/input`,
`bevy_debugger/screenshot`) against a copy of Sanctuary's own project — never a real keyboard,
never a real display.

---

## Pass 1 — Function: every state of the feature at full frame

**Verdict: GREAT**

### What the feature does, in the editor

An author arms a group on the Compose tab (`Enter`), clicks the map, and the group is stamped:
**one reference** goes into the map file, and the group's **18 rows** are drawn in the world.

### The states, and what each one showed

| State | What it showed | Verdict |
|---|---|---|
| **No group stamped** | `MAP  727 placed` — the plain count, unchanged from before the feature. The map file held `0 stamp(s) standing for 0 row(s)`. | ✅ An ordinary map is untouched by the change. |
| **A group armed** | The ghost follows the cursor, translucent, showing the group's real contents — the same 18 members, in the same places, the stamp would land them. | ✅ The preview and the commit are one calculation. |
| **A group stamped** | `MAP  745 placed (727 by hand, 18 from 1 group(s))`, the group's 18 rows drawn solid in the world, and `tris drawn` rising from **201,388 → 203,878** — the 18 rows really are geometry, not a picture of geometry. | ✅ The count and the picture agree. |
| **A group that cannot expand** | The stamp's group renamed to one that does not exist. The editor **refuses**, by name: `composition: house_cottage_plaster@1 refers to composition town/no_such_group, which does not exist`. The map still opens, the hand-placed rows still draw, and the refusal is in the inspector rather than swallowed. | ✅ A loud refusal, not a degraded map. |
| **The file on disk** | `stamps: [ ( id: "house_cottage_plaster@1", of: "town/house_cottage_plaster", at: (0.0, 0.0), … ) ]` — a **reference**, and the rows are *not* in `placements`. | ✅ Editing the group still moves every map that stamped it. |

### The proof that the game agrees

The load path, measured with the consumer's own door
(`bevy_emerge_core::project_data::load`):

```
before:  map.placements.len() = 728   map.stamps.len() = 1   → the game got 728 rows
after:   map.placements.len() = 746   map.stamps.len() = 1   → the game got 746 rows
```

The 18 rows the editor drew are the 18 rows the game now has. **That is the whole feature.**

### A false alarm, checked and cleared

The first capture of the broken-group state showed an **empty viewport**, which looked like the
refusal had taken the whole map with it. It had not: the untouched baseline capture
(`evidence/` from before any change) has the **same** 1,363 distinct colours in the viewport as
the refusal capture, and the broken capture has the same 1,363. The camera opens at a zoom where
the town's centre is off-frame; scrolling out (`evidence/03`) shows all 727 hand-placed rows and
the stamped group among them. The blank viewport is the camera's starting framing, not a
consequence of the broken stamp.

---

## Pass 2 — Zoom: the panel, cropped and upscaled

**Verdict: GREAT** (after one fix, below)

### What the zoom showed

The inspector's `MAP` row, cropped and upscaled 3x:

* The text is **crisp** — the FiraMono face renders the digits cleanly at 3x, no mush, no
  anti-aliasing artefacts.
* The **colour** is right: the value is amber when the map is unsaved and neutral when it is
  saved. That is the editor's existing `open.dirty()` rule, applied to the new count rather than
  reinvented.
* The **alignment** is right: the value starts at the same column as `NAME`, `BRUSH`, `YAW` and
  `UNDER` above it, and the label sits in the same fixed column as every other label.

### The defect the zoom found, and the fix

The first wording was:

```
745 placed (1 stamped, 18 row(s)), unsaved
```

which reads as though the 18 were **on top of** the 745 — they are not; the 745 already counts
them. So the breakdown was rewritten to say what is *included*.

The second wording was:

```
745 placed (727 by hand, 18 from 1 group(s)), unsaved
```

and **this one wrapped to two lines** in the narrower panel (the viewport zoomed in). The `MAP`
label sat beside two lines of value, which reads as two rows and is neither. Measured: the value
column holds about **45 characters** at the narrowest the panel gets.

The wording that ships:

```
745 placed (727 hand, 18 group(s)), unsaved
```

43 characters — inside the budget, and the two numbers now add up to the number in front of them.

### One thing the zoom could not settle

Whether "hand" and "group(s)" are the right words. They are short enough to fit, they are
distinct from one another, and they match the vocabulary the Compose tab already uses
("18 member(s)" for the group's size, "1 of 16" for the list). They are terser than the panel's
usual prose, and the reason is on the row: this is a **count**, and a count that wraps is a count
that has become a paragraph.

---

## Pass 3 — Fresh eyes: a developer who has never seen the editor

**Verdict: GREAT**

A **clean copy** of Sanctuary's project, the editor started from scratch, and the whole flow
driven as a first-time user would drive it.

### Can I discover it?

Yes, and the path is the one the editor already teaches:

1. `kits & maps` → the KIT door → the **COMPOSE** tab (`3`).
2. The panel lists the project's 16 groups by name, each with its member count and whether it is
   anchored or bounded: `town/house_cottage_plaster — 18 member(s), anchored`. A 3D preview of
   the focused group stands to the right, at three sizes, with the focused one picked out.
3. `Enter` arms it. The footer says so, and the Map tab's next click becomes a stamp.
4. Back to the map, a click stamps it.

### Can I do it?

Yes. The stamp landed on the first click, drew its 18 rows, and the inspector said what happened:

```
MAP    745 placed (727 hand, 18 group(s)), unsaved
UNDER  stamped `town/house_cottage_plaster` as house_cottage_plaster@1
```

The `UNDER` line names the group **and** the reference it created, which is the one thing an
author needs to know to find it again.

### Can I undo it?

Yes — `Undo` takes the reference back, and because the rows were never stored, they go with it.
The existing test `stamping_writes_a_reference_and_undo_takes_it_back` covers it and still
passes.

### Is the number honest?

This is the question a fresh eye asks first, because the number is the thing that decides whether
the author trusts the tool. **745** is the number the game will spawn. Before the change the
panel said 727 and the game had 728 — the panel was wrong by 18 in the direction that matters,
and the author had no way to see it. Now the panel says what the game has, and the breakdown says
where it came from.

---

## Summary

| Pass | Verdict | Note |
|---|---|---|
| 1 — Function | **GREAT** | Every state refuses loudly or counts honestly; the game now gets the rows the editor drew. |
| 2 — Zoom | **GREAT** | One defect found (a wrapping value) and fixed; the re-capture fits. |
| 3 — Fresh eyes | **GREAT** | Discoverable, usable, undoable, and the number is the number. |

Three GREEN reviews. Shipped.

### Evidence

| File | What it shows |
|---|---|
| `evidence/01-map-with-a-stamped-group.png` | The whole map, zoomed out, with the group stamped and the count in the inspector. |
| `evidence/02-stamped-group-in-the-world.png` | The group's 18 rows drawn solid in the world. |
| `evidence/03-a-broken-group-refuses.png` | A group that cannot expand: the refusal, by name. |
| `evidence/04-the-compose-tab.png` | The Compose tab — where a group is found and armed. |
