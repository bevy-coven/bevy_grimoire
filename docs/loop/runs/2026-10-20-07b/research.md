# Research — the editor's refusals must reach the author

Run: 2026-10-20-07b. Feature: **a fill that cannot be done is refused loudly, and a truncated
operation says what the author does next.**

---

## The measurement that started this

I drove the real editor over BRP against a copy of Sanctuary's project and asked the Map door to
**lay the bulk of a level** — the gesture the editor has for exactly that.

### The cap works, and it is loud about itself

`F` is a **flood fill**: it spreads from the cell under the cursor, stopping at anything already
placed and at the map's edge. One click on open floor laid **4,096 pieces** and the editor said so,
in the chrome bar's red banner:

> **filled 4096 and STOPPED at the 4096 cell cap — fill again to continue**

**And the promise is false.** Measured, with a new test in the crate (`fill.rs`):

| what the author does | what happens |
|---|---|
| **fills again in the same cell** | **refused** — *"there is already something here"*, because the flood will not start on a cell that now holds a piece |
| **fills again in a *different* empty cell** | **works** — it places 4,096 more and truncates again, because the flood stops at the first fill's pieces |

**So "fill again to continue" is achievable, but only by clicking somewhere new, and nothing says
so.** The author is told to do a thing that is refused, and is not told the thing that works.

### And the refusal is invisible

**The refusal the author gets for following the promise does not reach the screen.** Measured on a
capture, zoomed 2× on the INSPECTOR:

- the ghost's refusal for the same piece is **plainly visible**, in amber, in the `UNDER` row:
  > `prop_bread_basket` goes on a `support` surface — there is none here. Put it on something that offers one.
- the fill's own refusal — *"mounts to a `support`, so it cannot be flood filled"* — **is not on
  screen at all**. The `Field::Last` row, where a receipt would go, is empty.

### Why: the editor has two slots, and the fill used the wrong one

**`Status` has two slots, and its own doc says which belongs where** (`chrome.rs`):

| slot | the doc's own words |
|---|---|
| `Status::note` | *"What just happened, and went as asked… This is the right call for **a refusal the author is not owed an alarm about** — clicking bare ground with the remove tool armed, dragging a box round nothing. **Those are answers, not failures**, and a red block for each would teach an author to stop reading red blocks."* |
| `Status::problem` | *"**The editor could not do what was asked, or did only part of it.** The line, in practice: a write that did not reach disk, **a refusal handed back by `bevy_emerge_core`**, input that would not parse, a piece that changed underneath the author, or an edit that succeeded and then failed to redraw. **Anything a person would want to still be on screen a minute later.**"*

**The fill's refusals are `note`s.** They are *not* answers — they are refusals handed back by
`bevy_emerge_core`, and one of them is literally *"did only part of it"*, which is the `problem`
slot's own first clause.

**And a note is a fragile place to put a refusal.** Three mechanisms lose it:

1. **`Status::clear_note()`** exists — and its doc says why: *"a **stale receipt** was the whole of
   the bug"*, because a receipt that survives a tab switch misleads. **A refusal is not a stale
   receipt, and clearing it loses the only record that the operation did not happen.**
2. **The note is one `String` that is only ever overwritten** — so a receipt written a moment later
   erases the refusal.
3. **The note is a dim row.** A `problem` becomes a **toast** — a timed banner in the chrome bar,
   which the author cannot miss — and an entry in the **journal**, which is the session's record of
   what went wrong.

### And the two fills already disagree with each other

| | the message |
|---|---|
| **the box fill** (a drag) | *"filled N — stopped at the 4096 cell cap"* — **true**, and it promises nothing |
| **the flood** (`F`) | *"filled 4096 and STOPPED at the 4096 cell cap — **fill again to continue**"* — **a promise the algorithm cannot keep** |

**So the editor has two answers to "the cap stopped me", and the one on the more-used gesture is
the false one.**

### The scope, measured

**`fill.rs` is 1,058 lines with 20 inline tests** — the log's claim that it *"has no test at all"*
is **stale**, and this run corrects it. **But two of those twenty bracket the defect exactly and
neither asks the question between them:**

- `a_huge_region_truncates_and_says_so` — a fill truncates and reports it.
- `starting_on_something_is_refused` — a fill starting on something is refused.

**Neither asks: after a truncating fill, what does the *next* fill do?** That is the author's only
route past the cap, and the question had never been asked. **The new test
`the_caps_promise_cannot_be_kept_by_a_second_fill` asks it and proves the promise false.**

---

## The literature

### The literature on progress and on the interrupted operation

**Myers, *A Visualization of Long-Term Program Usage* (CHP 1986,
[10.1145/792550.792563](https://doi.org/10.1145/792550.792563))** — the paper that named the
"percent done" display, and with it the problem the fill's cap is an instance of: **a long
operation needs to say where it is, so the author can see it is not stuck.**

**The systems answer to "a long operation hit a limit" is a resumable cursor.** Dean & Boddy, *A
Practical, General-Purpose Algorithm for Generating Anytime Computations* (AAAI 1988) and the
*anytime algorithm* literature that followed: **an anytime computation is one that can be
interrupted and returned to**, and the *contract* between the algorithm and the user is that the
state needed to resume is part of the result. **The fill's "fill again to continue" is an anytime
contract the fill does not honour** — it offers resumption without a cursor.

**O'Brien & Shapiro, *Undo for anyone, anywhere, anytime*** (CHI 2004,
[10.1145/1133572.1133579](https://doi.org/10.1145/1133572.1133579)) — the abstract's claim is that
an author can *"navigate, visualize, edit and experiment with the history of the system safe in the
knowledge that any history change will not have unforeseen and irreversible effects."* **The
editor's undo already gives the author the way back from a truncated fill** — one `Cmd+Z`, and the
crate records *"one undo entry for the whole fill"*, precisely so a 4,096-piece fill is one act to
take back. **The cap's message points at the wrong route.**

**Rossmy, Terzimehić, Döring, Buschek & Wiethoff, *Point of no Undo: Irreversible Interactions as a
Design Strategy*** (CHI 2023, [10.1145/3544548.3581433](https://doi.org/10.1145/3544548.3581433))
— the counter-position, and it is the right one here: **irreversibility can be a deliberate design
element, but only where it is intended.** A message that promises a route the tool does not have is
an *unintended* irreversibility, and the fix is to name the real one.

### The literature on severity and on the silent failure

**Nielsen, *10 Usability Heuristics* (Heuristic 5: Error Prevention)**
([nngroup.com](https://www.nngroup.com/articles/ten-usability-heuristics/)):

> "Good error messages are important, but **the best designs carefully prevent problems from
> occurring in the first place**."

> "Either eliminate error-prone conditions, or check for them and present users with a confirmation
> option before they commit to the action."

**So: eliminate it where it can be eliminated.** The cap's false promise *is* the preventable
condition — a message that cannot be kept should not be written.

**Norman, *Design rules based on analyses of human error*** (CACM 1983,
[10.1145/2163.358092](https://doi.org/10.1145/2163.358092)) — the **slip** classification:

> "Slips are defined to be situations in which the user's intention was proper, but the results did
> not conform to that intention."

**The author who clicks "fill again" after the cap has a proper intention — continue the fill — and
the editor does something else.** That is a slip, and the editor's own message invited it.

**Apple HIG, Alerts** ([developer.apple.com](https://developer.apple.com/design/human-interface-guidelines/alerts)):

> "Avoid displaying alerts for common, undoable actions, even when they're destructive."

**And the habituation literature** (*Tuning Out Security Warnings*, MISQ 2018,
[10.25300/misq/2018/14124](https://doi.org/10.25300/misq/2018/14124); *Polymorphic Warnings*, CHI
2015, [10.1145/2702123.2702322](https://doi.org/10.1145/2702123.2702322)) — **warnings lose their
power when they are shown for things that are fine.** **So the editor's own rule — and its `Status`
doc — is right: a refusal earns a red block, and an answer does not.** The problem is not that the
editor is quiet; it is that the editor is quiet *about the wrong things*.

**Bravo-Lillo et al., *Your Attention Please: Designing Security-Decision UIs to Make Genuine Risks
Harder to Ignore*** (SOUPS 2013) — on making a message that matters actually reach the eye.

**The silent-failure hazard.** The failure this run found is a **silent failure**: the operation did
not happen, the editor said nothing where the author looks, and **the map on screen and the map on
disk agree** — so nothing looks wrong. A silent failure is worse than a loud one, because the
author's trust is calibrated by what they can see.

### The literature on the tool never lying about its state

**Hutchins, Hollan & Norman, *Direct Manipulation Interfaces*** (HCI 1(4), 1985,
[10.1207/s15327051hci0104_2](https://doi.org/10.1207/s15327051hci0104_2)) — the **gulf of
evaluation**: the distance between the system's state and what the person can tell from the
display. **A fill that was refused and said nothing is that gulf, in full** — the screen looks
exactly as it did before the click, and the author has no way to know the operation did not happen.

**Shneiderman, *Direct Manipulation: A Step Beyond Programming Languages*** (IEEE Computer 16(8),
1983, [10.1109/MC.1983.1654471](https://doi.org/10.1109/MC.1983.1654471)) — *"Permit easy
reversal of actions. This feature relieves anxiety, since the user knows that errors can be
undone."* **The editor's undo is that relief** — so the *message* should point at it.

---

## What great looks like

Acceptance criteria, phrased so a reviewer could fail the implementation against them.

1. **No message promises what the tool cannot do.** The cap's message names a route that exists.
2. **The route past the cap is named and is the one that works.** One sentence, and it is the
   sentence an author can act on.
3. **A refusal reaches the author.** A fill that cannot be done is a **problem** — a toast and a
   journal entry — and not a dim receipt that a later receipt can erase.
4. **A refusal is never cleared by the receipt-clearing rule.** `clear_note` clears receipts; a
   refusal is not a receipt.
5. **The two fills say the same kind of thing.** The flood and the box fill both report a cap the
   same way, and neither promises what it cannot deliver.
6. **A partial operation says so.** A truncated fill is *"did only part of it"*, which the
   `Status::problem` doc names as its first clause.
7. **The way back is named.** The receipt says how to take the fill back — one undo entry for the
   whole fill.
8. **The module's own tests ask the question between the two that bracket it.** The pair
   *"a fill truncates and says so"* and *"a fill starting on something is refused"* must be joined
   by *"and the next fill does this"*.

## Gaps

The literature settles three things and leaves two to this project.

- **Whether the cap should raise the limit, or the fill should be chunked with a real cursor.**
  The anytime literature's answer is a resumable cursor, and the fill's flood **has** one — the
  queue of unvisited cells is thrown away at the cap. **So the machinery for a resumable flood is
  already there and is being discarded.** Whether to keep it or to lower the cap is the design
  question, and the answer is bounded by what the editor can undo.
- **Whether the refusal should be the chrome's toast or the panel's amber row.** The ghost's
  refusal is in the panel's amber `Field::Hint` row; a fill's refusal is an *event*, not a
  *state*, so a row that is rewritten every frame is the wrong home for it. **The toast is the
  editor's existing home for an event-shaped refusal, and it is where the cap's own message
  already goes.**
