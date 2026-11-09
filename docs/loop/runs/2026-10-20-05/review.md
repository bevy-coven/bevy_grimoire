# Review — the door switch

Run: 2026-10-20-05. Three passes, judged on the captures in `evidence/`.

Every capture was taken by driving the **real editor** over BRP (`bevy_debugger/screenshot` with
`source: "offscreen"`, so the editor's own surface is read and no window is taken), against a copy
of Sanctuary's project.

---

## Pass 1 — Function: every state of the feature at full frame

**Verdict: GREAT**

### The states

| State | What it showed | Verdict |
|---|---|---|
| **The Map door, on arrival** | The INSPECTOR panel carries the COMPOSITIONS picker; the strip reads `1 MAP` · **the kit door**. | ✅ The cue is on screen without being pressed. |
| **`K` held** | The badge cluster shows **`1`** and **`=`**, the `=` in its own box with a leader line to the strip. | ✅ The key is discoverable, and the two facts are complementary, not two statements of one. |
| **`=` pressed** | The editor is on the **Kit door**: `KIT · town`, MESHES / TILES / COMPOSE, the barrel previewing in the viewport, the PIECES shelf on the right. | ✅ **One key, one crossing.** |
| **The Kit door's own cue** | The strip reads `1 MESHES  2 TILES  3 COMPOSE` · **the map door**. | ✅ The cue names the *other* way, from the other side. |
| **`=` pressed again** | Back on the **Map door**, with the same map: `759 placed (727 hand, 32 group(s))`. | ✅ The round trip is two keys. |

### The proof that the project is not re-read

**Measured over three crossings** (the editor's own log, which prints a line per project read):

| | before the crossings | after three crossings |
|---|---|---|
| **project reads** | 2 | **2** |
| **map reads** | 2 | **3** |

**The project is never re-read.** The map is — once, because one of the three crossings went *to*
the Map door, and **a map is a map**: the Map door's subject is the map the author is editing, and
it is re-read so the crossing shows the file rather than a stale copy.

That is the whole feature in one number: the round trip used to cost **a full project reload** —
`kits.ron`, `vocab.ron`, `rules.ron`, 332 library descriptors, 16 compositions, and **every GLB's
triangle count re-measured from its JSON chunk** — and now it costs **nothing**.

### The proof that the teardown is still a teardown

The crossing **tears the old door down**: every entity it made is gone, and every
`Ownership::Door` resource is back at its opening value. The test
`the_crossing_tears_the_old_door_down` says so, and it **found its own bug**: the first version
passed while the door's working state *survived* — because the headless harness never registered
`ScreenPlugin`, so the teardown had never run. The harness registers it now.

---

## Pass 2 — Zoom: the cue, cropped and upscaled

**Verdict: GREAT**

The strip at 3×, with the keys held:

```
1 MAP        the kit door  ──┤ 1 │ │
```

* **The type is crisp** — FiraMono at 3×, the `k` in "kit" and the `t` in "the" clean.
* **The hierarchy is right.** The panels are the strip's subject: `MAP` is `TAB`-sized and white.
  The cue is a sentence beside them: **the kit door** is `BODY`-sized and in the muted grey, so it
  is *present* without competing.
* **The key is legible.** `=` is the widest mark on the keyboard and it reads as a key at a glance.
  **The backquote was tried first and measured at 11 px, where it read as a speck** — a grave accent
  is a small mark at any size, and a key the author cannot see is a key they will not press. `=` is
  also the shape of a toggle, which is what the crossing is.
* **The two facts are complementary, not duplicated.** The strip states the *name*; the badge states
  the *key*; the leader line joins them. **The first version printed the key in the strip as well,
  and the two sat three pixels apart** — the editor's own *"a fact stated more than once drifts"*
  rule, caught by looking.

---

## Pass 3 — Fresh eyes: a developer who has never seen the editor

**Verdict: GREAT**

A **clean copy** of Sanctuary's project, the editor started **from the command line on a map**.

### Can I discover it?

**Hold `K`.** The badge cluster shows the strip's keys in their own boxes — `1` for the panel slots
and **`=`** for the crossing — with a leader line to the strip. The strip itself says **the kit
door**, so the badge and the strip agree on what the key does and where it goes.

**And it is on screen without being pressed.** The author does not have to know that a crossing
exists to find it: the strip names the other door every minute of every session, and the key is one
`K` away.

### Can I do the task?

**`=`**, and the editor is on the other door. **One key.** No menu, no column walk, no
`Cmd+O`.

### Do I trust it?

Two things say yes, and neither is a promise:

* **The count is honest and it does not move.** `759 placed (727 hand, 32 group(s))` before the
  crossing and after it. The crossing is not a reload dressed up as a shortcut — it is a crossing.
* **The map comes back as the map I left.** The stamp is still there, the group still expands, and
  the editor's own log shows the project was read **twice in the whole session** — once at startup,
  and never again.

### What a fresh eye would still ask

"Can I get from the Map door to the Kit door *and back to the same map*?" — **yes**, and the map is
the one I was on: the project remembers the map's name across the crossing, the way it remembers
the armed composition.

---

## Summary

| Pass | Verdict | Note |
|---|---|---|
| 1 — Function | **GREAT** | One key crosses; the project is never re-read (2 reads before, 2 after three crossings); the teardown is still a teardown. |
| 2 — Zoom | **GREAT** | The cue is one statement in one place; the key is legible; the hierarchy puts the panels first and the cue beside them. |
| 3 — Fresh eyes | **GREAT** | The cue is on screen without being pressed; the key is one `K` away; the map comes back as the map you left. |

Three GREEN reviews. Shipped.

### Evidence

| File | What it shows |
|---|---|
| `evidence/01-the-map-door-with-the-cue.png` | The Map door with the strip reading `1 MAP` · **the kit door**, and the COMPOSITIONS picker below. |
| `evidence/02-one-key-to-the-kit-door.png` | The Kit door after one keypress: `KIT · town`, MESHES / TILES / COMPOSE, the barrel previewing. |
| `evidence/03-the-keys-held.png` | The keys held: the strip's badge cluster showing `1` and **`=`**, the `=` in its own box with a leader line to the strip. |
