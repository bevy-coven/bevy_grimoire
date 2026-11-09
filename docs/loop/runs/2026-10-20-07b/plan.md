# Plan — the editor's refusals reach the author, and no message promises what the tool cannot do

Run: 2026-10-20-07b. Feature: **one visible channel for the editor's refusals, and a fill that
tells the truth when it stops.**

---

## The decision

**One feature, in three parts, each a defect measured in the previous run's own project.**

1. **A refusal reaches the author.** The editor's `Status` has two slots and its own doc says which
   belongs where; **21 refusals across the editor are in the wrong one** — the slot that is a dim
   row, that the next receipt overwrites, and that a tab switch clears. **They go where the crate's
   own doc says they go: `Status::problem`,** which reaches the toast and the journal.
2. **No message promises what the tool cannot do.** The flood's cap says *"fill again to
   continue"*, and the second fill **cannot** continue — it is refused, because the flood will not
   start on a cell that now holds a piece.
3. **The two fills say the same kind of thing.** The box fill says *"stopped at the 4096 cell cap"*
   — true. The flood says something else, and it is the one on the more-used gesture.

**Why these three together.** They are one defect seen three ways: **the editor's account of what
just happened and the truth about it come apart.** A refusal the author cannot see is that, at the
level of the message. A promise the tool cannot keep is that, at the level of the wording. And two
operations answering the same event differently is that, at the level of the vocabulary.

**And the brief's phrase for "most usable" is *"an author gets a real project built faster, with
fewer sharp edges, and **trusts the result**"*.** A tool that says *"fill again to continue"* and
then refuses the second fill, and says nothing the author can see, is not trusted — **and every
other feature the editor has inherits that.**

**Why not the runner-up.** The log's top item is **renaming a kit's namespace** — 744 mentions across
4 files, and no multi-file atomicity. That is a **larger** feature, but a *rarer* one: it happens
**once per project**. **This one fires on every refusal in the editor** — 21 of them, across the
fill, the placement, the drag, the rename, the labeller and the tile operations — **and the author's
trust in every one of those operations is what is at stake.**

## The scope, and the slice

**The survey found 21 refusals in the wrong slot.** That is too big for one iteration to do well, and
the brief says to carve.

**The slice is the fill path, and it is the sharpest because the fill is the operation that lays the
bulk of a level.** The fill has:

- **two** refusal sites in the box path (`editor.rs:3497`, `3527`) and **one** in the flood path
  (`editor.rs:9673`);
- **the cap's false promise** (`editor.rs:9686-9690`);
- **the two fills' disagreement**;
- and it is **the gesture the author uses most** for a floor or a wall.

**And the mechanism is already there and is one line each.** The fix is not a new mechanism; it is
**the crate's own `Status::problem`, used where its own doc says it should be.** So the slice is
small in code and large in effect — and **the remaining 18 sites are the same change, and the log
records them as the next item.**

### The three refusals the slice moves

| site | the message | why it is a refusal |
|---|---|---|
| `editor.rs:3497` | *"`{}` mounts to a `support`, so it cannot be box filled"* / *"a fill needs a positive step"* | the core refused; **the editor could not do what was asked** |
| `editor.rs:3527` | *"blocked: `{}` carries `{}` — remove it first"* | the riders refused; **the editor did only part of what was asked** |
| `editor.rs:9673` | *"there is already something here"* / *"that point is outside the map"* / *"mounts to a `support`"* | the flood refused; **and the author was told nothing** |

### The cap's promise, and what replaces it

**The measurement, from the new test:** a second flood in the **same** cell is refused (*"there is
already something here"*); a second flood in a **different** empty cell **places 4,096 more**,
because the flood stops at the first fill's pieces.

**So the promise is achievable, but only by clicking somewhere new — and nothing says so.**

**And the literature has the exact contract for this.** Google AIP-158 (Pagination) is a primary
standard for a resumable cursor, and its own words are the rule:

> *"Page tokens **must** be limited to providing an indication of where to continue the pagination
> process only."*

> *"If the end of the collection has been reached… the `next_page_token` field **must** be empty.
> This is the *only* way to communicate 'end-of-collection' to users."*

**So the message must say where to continue, and a fill that finished must not look like one that
stopped.** The flood's message already says the second half — *"filled 4096 and STOPPED"* — and
fails the first.

**The choice, and why it is the boring one.** Two options:

| | what it does | cost |
|---|---|---|
| **(a) the flood keeps a cursor** — the unvisited cells survive the cap, and the next fill resumes from them | a true resumable operation, exactly AIP-158's shape | the fill must carry a cursor across the click, and the cursor is **bound to the piece, the map and the region** — a change to any of them must refuse the resume |
| **(b) the message names the route that works** — *"fill from a new cell to continue"* | one sentence | the author clicks a new cell and the flood covers what is left — which **works today**, measured |

**(b) is what this run takes**, for a reason that is not timidity: **the flood's behaviour is
already correct.** A second flood from a new cell covers the remaining open floor, because the
flood stops at anything already placed. **The editor's algorithm is fine; its message is not.** (a)
would be a new mechanism — a cursor across a click — and it would need its own preconditions and its
own refusals, and **the research's criterion 6 is a whole new rule to get right.**

**So the slice is: make the message true, and make the refusal visible.** The cursor is the next
item, and the log says so.

## The shape, in detail

### 1. The channel: `Status::problem`, not `Status::note`

**The crate's own doc, verbatim** (`chrome.rs`):

> **`note`** — *"What just happened, and went as asked… This is the right call for a refusal the
> author is not owed an alarm about — clicking bare ground with the remove tool armed, dragging a
> box round nothing. **Those are answers, not failures**, and a red block for each would teach an
> author to stop reading red blocks."*

> **`problem`** — *"**The editor could not do what was asked, or did only part of it.** The line, in
> practice: a write that did not reach disk, **a refusal handed back by `bevy_emerge_core`**, input
> that would not parse, a piece that changed underneath the author, or an edit that succeeded and
> then failed to redraw. **Anything a person would want to still be on screen a minute later.**"*

**Every one of the three sites is a refusal handed back by the core, and one of them is literally
*"did only part of it"*.** So they are `problem`s, and the code's own comment at those sites —
*"A refusal is the answer, not a failure"* — **contradicts the `Status` doc it is implementing.**

**And the difference is not cosmetic.** A `problem` reaches:

- **the toast** — a timed banner in the chrome bar, which the author cannot miss;
- **the journal** — the session's record of what went wrong, and it survives a tab switch.

A `note` reaches **one dim row**, and is **erased by the next receipt** and **by a tab switch**
(`tiles.rs:4370`, the only `clear_note` call site).

**NN/g's Error-Message Guidelines** name the failure exactly:

> *"The very worst error messages are those that don't exist. When users make a mistake and receive
> no feedback, it can create a cascade of misunderstanding, wasted effort, and frustration."*

**And the research's criterion 2 asks for the truncation to appear in the same channel as the ghost's
refusal** — which is the visible one. The toast is the editor's existing home for an event-shaped
refusal, and **the cap's own message already goes there.**

### 2. The message: no promise the tool cannot keep

**The flood's cap message today:**

> *"filled 4096 and STOPPED at the 4096 cell cap — fill again to continue"*

**What is true, measured:** a second flood **in the same cell** is refused; a second flood **in a
new empty cell** places 4,096 more and truncates again.

**What the message must say**, by the research's criteria:

| criterion | what it demands |
|---|---|
| **1 — no unkept promise** | the message must name an action that produces the stated result |
| **4 — completion is explicit** | a fill that finished and a fill that stopped must be distinguishable — and **the flood's already are**, so this is a matter of keeping them apart |
| **7 — recoverable and terminal are distinguishable** | a recoverable stop says what remains and how to resume; a terminal refusal says what changed |
| **5 — consistency across operations** | the two fills use the same channel, the same count, the same vocabulary |

**So the cap's message becomes: what was laid, that it stopped, and the route that works.** And the
box fill's message — *"filled N — stopped at the 4096 cell cap"* — **is the right shape and needs
the route too**, so the two agree.

### 3. The wording, and the NN/g rules the slice must satisfy

- **"Merely stating the problem is also not enough; offer some potential remedies."** (NN/g) — so
  the message names the next action, not just the stop.
- **"never use exclusively color or animation to indicate errors."** (NN/g) — the toast is **text**;
  the colour is a second channel, not the only one. **Nothing changes here.**
- **"no blame wording"** (NN/g) — *"there is already something here"* names the world, not the
  author's mistake. **Nothing changes here either, and that is worth saying: the crate's messages
  are already well written.** **What is wrong is the channel, not the words.**

## Touched modules

| file | what changes |
|---|---|
| `crates/bevy_emerge_mapper/src/editor.rs` | **three refusal sites** move to `status.problem`; the flood's cap message names the route that works |
| `crates/bevy_emerge_mapper/src/fill.rs` | the new test; **and the message's facts come from here** — what the flood did and did not cover |
| `crates/bevy_emerge_mapper/src/chrome.rs` | **the `Status` doc gains a sentence**, because the code contradicted it in three places and the doc was right |

**`bevy_emerge_core` is untouched** — the writer is engine-free and needs no change. **No schema
change**: a fill lays placements, and a message is not a file. **No new verb**: no key is added, so
`BINDINGS` does not change.

**`bevy_emerge_core/tests/engine_free.rs` is unaffected** — nothing in the core changed.

## Tests

1. **A refused fill reaches the visible channel.** The fill's refusal is a `problem`, and the
   `Status` has one. **This is the test that fails today.**
2. **A refusal is not erased by a later receipt.** A receipt written after a refusal does not take
   the refusal away — the crate's own `Status` contract says a problem survives.
3. **A refusal is not erased by a tab switch.** `clear_note` clears receipts; a refusal is not a
   receipt.
4. **The cap's message names a route that works.** **The message is followed, in a test, and the
   stated result is asserted** — the research's criterion 1, and the *failing* form of it is the test
   this run already wrote.
5. **The two fills say the same kind of thing.** The box fill's and the flood's truncation messages
   use the same vocabulary and the same channel.
6. **A fill that finished and a fill that stopped are distinguishable.** Two different messages.
7. **The flood's refusals are the three the core can produce**, and each is a `problem`.
8. **The rest of the editor's 18 mis-slotted refusals are listed**, so the next iteration starts
   from a measurement rather than a search. — *a doc, not a test.*

## Risk

| risk | how it is answered |
|---|---|
| **The toast becomes noisy.** | The slice moves **three** refusals, and they are refusals the author asked for and did not get. **The habituation literature is the reason not to move all 21 at once** — and the log records the other 18 as the next item, where the noise can be judged with the first ones in place. |
| **A refusal in the toast is not where the author looks.** | **The cap's own message already goes to the toast**, and the author reads it. **The ghost's refusal is in the panel's amber row**, and it stays there. |
| **The message's new wording is wrong.** | **The wording is pinned by a test that performs the action the message names** — so a wrong claim fails the suite rather than reaching an author. |
| **The two fills drift apart again.** | A test compares their messages. |
| **`clear_note` starts clearing problems.** | It must not. A test pins that a refusal survives a tab switch. |

## Not in this run

- **The other 18 mis-slotted refusals** — measured, listed in the log, and the next item. The
  mechanism is one line each; **the reason it is not this run is the habituation literature**, and
  the reason it should be the *next* run is that the first three are the ones on the gesture the
  author uses most.
- **A resumable flood with a real cursor** — AIP-158's shape, bound to the piece, the map and the
  region. The flood's behaviour is already correct; the *cursor* is the new mechanism.
- **Renaming a kit's namespace** — 744 mentions across 4 files, and no multi-file atomicity.
- **The 282 hand-placed fences** — content, and the owner's.
- **`fill.rs` having no test** — **stale**. It has 20 inline tests, and this run corrects the log.
