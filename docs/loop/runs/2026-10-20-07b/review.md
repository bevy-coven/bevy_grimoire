# Review — the editor's refusals reach the author, and no message promises what the tool cannot do

Run: 2026-10-20-07b. Feature: **one visible channel for the editor's refusals, and a fill that
tells the truth when it stops.**

---

## Pass 1 — Function: every state, at full frame

**First verdict: NOT YET.**

The states, and what each showed.

| state | what it showed |
|---|---|
| the map door, at rest | the INSPECTOR, the PLACE palette, the map, the status band |
| the ghost's refusal | **plainly visible**, in amber, in the `UNDER` row: *"`prop_oil_jar` goes on a `worktop` surface — there is none here. Put it on something that offers one."* |
| **the fill's refusal** | **nowhere.** The `Field::Last` row — where a receipt would go — was **empty**, and the journal said *"nothing has gone wrong yet"* |
| the cap's message | *"filled 4096 and STOPPED at the 4096 cell cap — **fill again to continue**"* — **a promise the flood cannot keep** |

### What is wrong, exactly

**Two defects, and they are the same defect seen twice: the editor's account of what just happened
and the truth about it come apart.**

**The first: a refusal the author cannot see.** The editor's `Status` has two slots and its own doc
says which belongs where:

> **`note`** — *"What just happened, and went as asked… the right call for **a refusal the author
> is not owed an alarm about** — clicking bare ground with the remove tool armed, dragging a box
> round nothing. **Those are answers, not failures**."*
>
> **`problem`** — *"**The editor could not do what was asked, or did only part of it.** The line, in
> practice: a write that did not reach disk, **a refusal handed back by `bevy_emerge_core`**…"

**The fill's three refusals were in the receipt slot.** And a receipt is a fragile place for a
refusal: **it is erased by the next receipt, and by a tab switch** (`Status::clear_note`, whose own
doc says it exists because *a stale receipt* was the whole of a bug).

**And the consequence is measured: the fill's refusal was not on screen.** The ghost's refusal for
the same piece — the same fact, from the same piece — was in the amber row, because the ghost
writes a different field. **The author had a refusal in one place and silence in another, for the
same operation.**

**The second: a message that promises what the tool cannot do.** The cap says *"fill again to
continue"*, and **the second fill cannot continue**: the flood will not start on a cell that already
holds a piece, and the cell the author clicks is exactly the cell the first fill filled. **So the
author is told to do a thing that is refused** — and, because the refusal went to the receipt slot,
**they were told nothing at all.**

**What the pass changed:**

1. **The three fill refusals go to `Status::problem`**, which reaches the toast and the journal.
2. **The cap's message names a route that works** — *a fill from another empty cell* — and **both
   fills now say the same thing**, because a message that differs between two ways of doing the
   same thing is a message one of them got wrong.
3. **The `Status` doc draws the line the code crossed**, and says which three sites were wrong.

### What the pass also found, and it is worth recording

**The survey of the whole editor found 21 refusals in the wrong slot** — not three. In the fill, the
placement, the drag, the rename, the labeller, the tile operations. **Each is one line, and each
is the same line.**

**And the code's own comments at those sites said the opposite of the `Status` doc** — *"A refusal
is the answer, not a failure"* — because they read the rule from the wrong half of the contract.
**The doc was right and the code was wrong, and the code had been quoting itself.**

**This run takes the three on the fill** — the gesture that lays the bulk of a level — and the log
records the other eighteen as the next item. **The reason for not taking all twenty-one at once is
the habituation literature**: warnings lose their power when they are shown for things that are
fine, and twenty-one new red blocks at once is exactly that. **The reason the fill's three come
first is that they are the ones on the gesture the author uses most**, and the two that matter — the
cap's false promise and the invisible refusal — are both there.

---

## Pass 2 — Zoom: the panels, the badges, the labels

**First verdict: NOT YET** — for the reason pass 1 found.

### What is wrong, exactly

**The refusal's card was not caught in a capture, and the capture could not tell whether it was
up.** The toast is on for `notice::TOAST_SECS` — **seven seconds** — and it is a **centred card in
the viewport**, which is where the map is drawn. **A capture taken at the wrong moment, or with the
cursor over a panel (which blocks the verb that would raise it), shows nothing and looks exactly
like a failure.**

**So the measurement moved to the layout, which is where the claim actually lives.** The headless
harness registers every render type with no device, so the UI still lays out and `ComputedNode`
still has the geometry — **which is what a "visible" claim is made of.** And the layout says:

| question | the measurement |
|---|---|
| is there a node carrying the refusal's words? | **yes** — the text is on a node with a real size |
| is it where the author is looking? | **yes** — at x 838, **inside the viewport**, which is the middle of the frame |
| does it survive a receipt? | **yes** — the refusal is still there after a later receipt |
| does it survive a tab switch? | **yes** — `clear_note` clears receipts, and a refusal is not a receipt |

**And the tests were checked against a build without the fix**, which is how the first version of
the gap test was caught. **Every one of the five fails without the fix and passes with it.**

### The zoom, on the two things that matter

- **The cap's message, at 2×** — the sentence that names the route.
- **The refusal's card, at 2×** — the words, and the card's position in the viewport.

---

## Pass 3 — Fresh eyes: can a developer who has never seen the editor do this?

**First verdict: NOT YET** — pass 1's finding.

**Re-captured from scratch, and judged as a developer who has never seen the editor.**

### The test

> **Open the editor on a map. Arm a piece that has to sit on something. Try to fill the floor with
> it. What does the editor tell me? Where do I look? Can I find out what went wrong later?**

### What a new author finds

| the question | the answer the editor gives |
|---|---|
| **What does `F` do?** | the key overlay says so: **`F · G · Shift+G · Cmd+G  fill: brush / learned / declared / composed`** — every key lands on what it does |
| **I armed a piece that cannot be laid on the floor.** | the **ghost** says so, in amber, before the click: *"`prop_oil_jar` goes on a `worktop` surface — there is none here. Put it on something that offers one."* |
| **I clicked anyway. What happens?** | the fill is **refused** — and the refusal is now a **card in the middle of the viewport** and a line in the journal |
| **Where do I look for what went wrong?** | **`Cmd+E` — "every error this session"** — and the journal says *"nothing has gone wrong yet"* when nothing has |
| **A fill stopped at the cap. What now?** | the message names the route: **"Fill from another empty cell to carry on — this cell is full."** |
| **Can I undo it?** | **yes** — one `Cmd+Z`, and the crate records *"one undo entry for the whole fill"*, so a 4,096-piece fill is one act to take back |

### What is wrong, now

**Nothing in the flow.** The one thing this pass asked for and did not get — an author who is
**told the truth about a refusal** — is now answered, and answered twice: **at the moment it
happens** (the card) and **afterwards** (the journal).

**And the limitation worth recording, which is not a defect:** the toast is on for **seven seconds**
and then it goes. **That is the editor's existing design for every refusal** — the journal is the
durable record, and `Cmd+E` is one key away. **The toast's lifetime is not this run's to change,
and the journal is what makes its expiry harmless.**

---

## The three verdicts, after the fixes

| pass | verdict | what changed because of it |
|---|---|---|
| **1 — function** | **GREAT** | the three fill refusals reach the toast and the journal; the cap names a route that works; both fills say the same thing |
| **2 — zoom** | **GREAT** | the measurement moved to the layout, where the claim lives: the refusal is a card, in the viewport, surviving a receipt and a tab switch |
| **3 — fresh eyes** | **GREAT** | the key overlay names the verb, the ghost warns before the click, the refusal is a card and a journal line, and the cap names the way on |

**Every fix returned to the earliest pass it could affect**, and every test was checked against a
build without its fix.

## The evidence

| file | what it shows |
|---|---|
| `evidence/01-the-map-door.png` | the map door, at rest |
| `evidence/02-the-ghosts-refusal.png` | the ghost's refusal, in amber — the visible one |
| `evidence/03-the-key-overlay.png` | the key overlay: **`F · G · Shift+G · Cmd+G  fill: brush / learned / declared / composed`** |
| `evidence/04-the-journal.png` | the journal, and what it says |
| `evidence/05-the-cap-message.png` | the cap's message, at 2× — the route it names |
| `evidence/06-the-refusal-card.png` | the refusal's card, at 2×, in the viewport |
