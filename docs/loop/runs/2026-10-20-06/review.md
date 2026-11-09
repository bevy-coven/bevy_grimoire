# Review — the composition rename

Run: 2026-10-20-06. Feature: **an author renames a composition from the COMPOSE tab, and every map
that stamps it follows.**

---

## Pass 1 — Function: every state, at full frame

**First verdict: NOT YET.**

The states the feature has, and what each one showed:

| state | what it showed |
|---|---|
| the tab at rest | sixteen compositions, their member counts, their envelopes, the armed `*` |
| the prompt | **RENAME THIS COMPOSITION**, the field, and the words *"Enter keeps it, and every map that stamps it follows.  Esc leaves it alone."* |
| the prompt, typed | the name, forced as it is typed, with the caret |
| `Esc` | the name is unchanged, and the status says so |
| a refusal | **this is where the pass failed** |

### What is wrong, exactly

**I typed `town/house_cottage_brick` to force a collision — the name of a composition that already
exists — and the editor did not refuse. It renamed the composition to `town_house_cottage_brick`,**
with the slash turned into an underscore, and the list then carried **two rows reading
`town_house_cottage_brick` and `town/house_cottage_brick`**, side by side.

**The reason is a real bug, not a bad test.** The field forced the name with
`naming::to_snake_case`, and `to_snake_case` **drops every character that is not a letter or a
digit** — including the `/`. So the author typed `town/house_cottage_brick`, the editor showed and
wrote `town_house_cottage_brick`, which is a **different id in a different namespace**, which
**collides with nothing**, so no refusal was owed.

**And the failure is worse than a missed refusal: it is a silent lie.** The field showed one
spelling and wrote another. The author would not have known that the kit namespace had been
silently dropped.

**Why the obvious fix is wrong.** `to_snake_case` is *correct* for the two places it is already
used: a **map's name** becomes a filename, and a **descriptor's id** has no namespace. But a
**composition id** is per-segment — `site/wall_corner` is *"which kit"* and *"which piece"* — and
the crate already says so: `is_id` is *"one or more snake_case segments joined by `/`"*, and
`is_snake_case` *"would reject all 45 of them"*.

**So the fix is a function that forces the spelling per segment and keeps the separators:**
`naming::to_id`. **The rule was already in the crate; the forcing was not.** `is_id` read the
namespace and nothing wrote it.

### What this pass changed

- `naming::to_id` was added to the core, with its own tests — the namespace survives, the spelling
  is still forced per segment, a stray separator is dropped, and whatever comes out is a legal id.
- The field and the commit both use it, so **the field shows the spelling the commit writes.**

---

## Pass 2 — Zoom: the panels, the badges, the labels

**First verdict: NOT YET.** The zoom found two more things the full frame hid.

### What is wrong, exactly

**(a) The receipt never survived.** The status line said *"`town/house_cottage_plaster` armed — the
map stamps it"*, not the rename's receipt. **The receipt is the whole point of the feature** — it
is where the author learns that every map followed — and it was gone before it could be read.

**The cause is a collision the crate already had a word for.** `Action::ComposeArm` is `Enter` on
the COMPOSE tab, and the rename's field is committed with `Enter` too. So **the same keystroke named
the composition *and* armed it**, and the arm's message — written later in the same frame —
overwrote the rename's receipt.

**The phases do not save this.** `Phase::Text` runs *before* `Phase::Act`, so the rename commits
first and the arm runs second, on the same frame, with the same key still down.

**And the crate already has the answer**, in `editor::Fields` — *"Adding a field is adding a line,
here and nowhere else."* While a field is typing, `keys::Context::Typing` holds the keyboard and
every other action is refused. The COMPOSE tab's rename prompt was simply not in that list, and the
list is the one place a field is registered. **The same shape as the `build` field's own comment**:
*"the `Enter` that confirms a name is also `BuildDrop`, so without it the same keypress named the
tile and dropped a piece into it."*

**One line in `editor::Fields`, and the bug is gone.** A bespoke guard inside `compose.rs` was
written first and **removed**: the crate's mechanism is the right one, and a second way to do it
would be a second answer.

**(b) The stale guidance.** After the refusal, the note line still read *"type a new name — Enter
keeps it, Esc leaves it alone"* — **advice for a field that is now closed.** That is worse than no
advice: an author reading it would press `I` again.

**The crate's rule is that *"a problem outlives every note"*** — and that rule is right for a
receipt, which is a *past* fact. But *"type a new name"* is not a past fact, it is **advice about
a field that no longer exists**, so it is the note that is wrong. The refusal now writes the note
as well as the problem, and the note is the refusal's own text.

### What this pass changed

- `editor::Fields` gained the COMPOSE tab's rename prompt — **one line, in the one place the crate
  says a field is added.**
- The bespoke guard in `compose.rs` was **removed** in favour of it.
- The refusal writes the note as well as the problem, so no stale advice survives a closed field.

---

## Pass 3 — Fresh eyes: can a developer who has never seen the editor do this?

**First verdict: NOT YET** — for the reason the previous pass found: the receipt was being
overwritten, so the author never learned the thing that mattered.

**Re-captured from scratch, and judged as a developer who has never seen the editor.**

### The test

> **Open the editor. Find the COMPOSE tab. Find out what `I` does. Rename the composition. Change
> your mind. Rename it back.**

### What a new author finds

| the question | the answer the editor gives |
|---|---|
| **Which tab is this?** | the tab bar says `1 MESHES  2 TILES  3 COMPOSE`, and the left dock's heading says **COMPOSE** |
| **What does `I` do?** | **the key overlay names it**: hold `K` and the list says **`I` — rename this composition**. It is in the *global* key list, at the bottom right, where a new author looks |
| **What will `I` do here?** | a floating panel over the viewport: **RENAME THIS COMPOSITION**, the field, and **"Enter keeps it, and every map that stamps it follows.  Esc leaves it alone."** |
| **What happens when I press `I`?** | the field opens **empty**, so the first keystroke replaces rather than appends |
| **What happens when I type?** | the name is forced as it is typed — **the namespace survives**, `town/house_cottage_plaster_v2` |
| **What if the name is taken?** | a red band: **NOT RENAMED: `town/house_cottage_brick` is already a composition, so renaming `town/house_cottage_plaster` to it would give two compositions the same name.** Both ids are named, and **nothing was written** |
| **Did it work?** | the receipt: **"`town/house_cottage_plaster` is now `town/house_cottage_plaster_v2`, and every map that stamped it follows. Rename it back to undo."** |
| **How do I change my mind?** | **the receipt says so** — *"Rename it back to undo"* — and it works: the round trip is **byte-identical** to the original file |

### What is wrong, now

**Nothing in the flow.** The one thing this pass asked for and did not get — an author who wants to
back out — is answered by the receipt, in the receipt's own words.

**The one thing worth recording as a limitation rather than a defect:** the rename is **not** on the
editor's undo stack, because it writes two files and the stack holds placements. Renaming back is
the way back, and it is exact — **measured**: after a rename there and back,
`compositions.ron` is **byte-identical** to Sanctuary's original.

---

## The three verdicts, after the fixes

| pass | verdict | what changed because of it |
|---|---|---|
| **1 — function** | **GREAT** | `naming::to_id`, so the namespace survives the field |
| **2 — zoom** | **GREAT** | `editor::Fields` gains the COMPOSE tab's field — one line, in the crate's own place — and the refusal writes the note as well as the problem |
| **3 — fresh eyes** | **GREAT** | the receipt says *"Rename it back to undo"*, and the round trip is byte-identical |

**Every fix returned to the earliest pass it could affect**, and each pass was re-captured from
scratch afterwards.

## The evidence

| file | what it shows |
|---|---|
| `evidence/01-the-compose-tab.png` | the tab at rest, sixteen compositions |
| `evidence/02-the-name-field.png` | the field, at 2x: the title, the caret, the hint |
| `evidence/03-the-typed-name.png` | the typed name, at 2x — **the namespace slash intact** |
| `evidence/04-the-refusal.png` | the red band, naming both ids |
| `evidence/05-the-receipt.png` | the receipt, at 2x, with *"Rename it back to undo"* |
| `evidence/06-the-key-overlay.png` | the key overlay saying **`I` — rename this composition** |
| `evidence/07-renamed-back.png` | the round trip, back to the original name |
