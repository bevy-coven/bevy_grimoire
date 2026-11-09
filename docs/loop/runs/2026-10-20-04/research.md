# Research — the editor's two halves, and the arm that cannot cross

Run: 2026-10-20-04. Feature: **an author must be able to put a composition on the map, from
wherever they made it.**

---

## The measurement that started this

I drove the real editor over BRP against a copy of Sanctuary's project, and did the task the
project's own data says an author has to do.

### What the project's data says

`assets/emerge/maps/town.map.ron`, parsed:

| | |
|---|---|
| placements | **727** |
| distinct descriptors | 82 |
| `prop_wooden_fence_single` | **282** |
| `town/garden_bed` | 80 |
| `town/wall_segment` | 76 |
| `town/houses_plaster` | 13 |
| **stamps** | **0** |

And `assets/emerge/compositions.ron` defines **16 compositions** — `town/house_cottage_plaster`,
`town/house_cottage_brick`, `town/house_cottage_wood`, `town/house_town_house_plaster`, … each with
18–38 members, and each carrying the note *"A house in the plain style…"*.

**Sixteen house assemblies are defined. Not one of them is stamped on either map.** The maps hold
727 hand-placed rows and zero stamps.

### What the editor does when an author tries

I armed `town/house_cottage_plaster` on the **Kit** door, in the **COMPOSE** tab. The editor
confirmed it, in the status line at the foot of the panel:

> **`town/house_cottage_plaster` armed — the map tab stamps it**

**There is no map tab in that process.** The Kit door's strip reads `1 MESHES  2 TILES  3 COMPOSE`.
The Map door's strip reads `1 MAP`. The two are different processes.

I then opened the **Map** door and pressed `Enter` where the composition list would be. **Nothing
happened** — the status line did not change. The Map door has no way to arm a composition.

### The structural cause, read from the source

| Fact | Where |
|---|---|
| `arm` — the verb that arms a composition — is gated on `in_compose_mode` | `compose.rs:194-201` |
| `in_compose_mode` means `mode == Mode::Compose` | `compose.rs:376-378` |
| `Mode::Compose` is in `Door::Kit.tabs()` and **only** there | `tiles.rs:113` |
| The door is a `Res<Door>` resource, and a door change is a **full teardown** | `screen.rs:25-33`, `tiles.rs:148-153` |
| `ComposeState` is `Ownership::Door` — **reset** on a door change | `screen.rs:357` |

So the round trip an author must make to stamp a house is:

> **Kit door → COMPOSE tab → `Enter` to arm → `Cmd+0` to the menu → re-open the project → open the
> map → and the arm is gone**, because `ComposeState` was reset by the door change.

**The arm cannot cross the door.** Not "is awkward to carry" — *structurally cannot*.

### But the editor IS one application

This is the part that makes the gap a bug rather than a design:

> *"These were two processes. The chooser built its own `App`, ran to completion, launched the editor
> as a child…"* — `screen.rs:5-12`

> *"**A state change, not a process launch**: this wrote the argv into a mutex the parent process read
> after `AppExit`, because the menu and the editor were two programs. **They are one now**."* —
> `chooser.rs:6290-6296`

> *"**Doors are still reached through the menu** — No direct Kit↔Map key. It became affordable once
> the process boundary went — the design doc had rejected it when it meant a relaunch — **and it was
> declined anyway**: with full teardown the saving is the menu round-trip rather than the reload, and
> a second way to do one thing is the pattern this crate spends its refusals on."* — `screen.rs:31-40`

**The decline was reasoned on a premise that no longer holds.** It was declined because "the saving
is the menu round-trip rather than the reload" — but the menu round-trip is *not* a reload any more,
it is a **state change inside one running application**, and the reload it used to imply is exactly
what makes the arm lossy.

---

## The literature

### Direct manipulation: the state must survive the gesture

Shneiderman, *Direct manipulation for comprehensible, predictable and controllable user
interfaces* (CHI 1997, [10.1145/238218.238281](https://doi.org/10.1145/238218.238281)) — the
canonical statement of the direct-manipulation principles:

> "Direct manipulation depends on visual representation of the objects and actions of interest,
> physical actions or pointing instead of complex syntax, and **rapid incremental reversible
> operations whose effect on the object of interest is immediately visible**. This strategy can lead to
> user interfaces that are comprehensible, predictable and controllable."

The three clauses that matter here: **rapid**, **incremental**, and **immediately visible**. An
author who arms a house, leaves, and comes back to a disarmed editor has got an operation that is
none of the three. The *stamping* is still direct manipulation; the *arming* is a mode change
performed across a process boundary, and the arm is the part that carries the intent.

Shneiderman's *semantic distance* is the measure that names this defect: the number of steps
between the author's intent and the result. Arming on one door and stamping on another has a
semantic distance of "leave, navigate, re-navigate, re-arm" — and the re-arm is not even possible
in the editor's own terms, because nothing in the editor remembers what you armed.

### Slips: the intended action, performed in the wrong place

Reason, *Human Error: Models and Management* (1990) and the *Experimental Slips and Human Error*
volume (1992) — the slip is a **description error**: the plan was right and the *execution* went to
the wrong object. The classic instance is the one that applies here: an author who has spent an hour
in one context, and reaches for a verb that belongs to the other.

**The editor's own words create the slip.** The status line says *"the map tab stamps it"* — and an
author who reads that literally will look for a map tab. There is none. The editor names a place
that does not exist, in the one place the author is guaranteed to read it.

Norman, *The Psychology of Slips* (in *Experimental Slips and Human Error*, 1992) makes the
design consequence precise: a slip is not fixed by telling the user not to slip. It is fixed by
**removing the possibility of the wrong action** — by making the verb available where the intent
is, so there is no second context to be in.

### Prefabs: the reference must be reachable from the instance

The prefab-and-instance literature's consistent finding — from the Unity/Unreal instancing model to
the *Constraint-Based Network Diagram Authoring Tool* (Dwyer, Marriott & Wybrow, 2009,
[10.1007/978-3-642-00219-9_41](https://doi.org/10.1007/978-3-642-00219-9_41)) — is that an
instance must be creatable **from the thing it instances**, at the point of use. Dwyer et al.'s
constraint authoring tool exists precisely because authoring a diagram by referencing a *separate*
library object is the thing authors fight.

This project already made the reference model choice, and the choice was right:

> *"What lands in the file is a reference, so editing the composition later changes this stamp and
> every other one; baking the rows here would have made each stamp an independent copy that
> silently stopped tracking its source."* — `editor.rs:4900-4906`

**The reference model is correct and complete. The only thing missing is a way to reach the
reference from the map.** The 16 compositions are the prefabs; the map door is where instances are
made; and the prefab library is on the other door, with no way to reach it from there.

### Wave function collapse: the author picks the region, the solver picks the contents

Sandhu, Chen & McCoy, *Enhancing wave function collapse with design-level constraints* (2019,
[10.1145/3337722.3337752](https://doi.org/10.1145/3337722.3337752)) — the finding that matters for
the *other* candidate (the 282 fences) is that a design-level constraint is what makes a
procedurally-filled region match the author's intent, and the constraint is expressed **in the terms
the author already has** — the regions and the pieces, not a solver's vocabulary.

Sanctuary's `rules.ron` is already that: `regions: ["town/houses_plaster", …]`, `cell: 2.0`,
`size: (17, 17)`, `weights: [(prefix: "town/house_", times: 100.0)]`, `no_empty: true`. The game
lays the blocks out; the editor cannot. That is a **deliberate division of labour** — the *game*
owns the block layout, the editor owns the pieces.

### The decisive question the literature cannot answer

Whether a door change should be a **teardown** or a **swap** is a project decision, not a research
question. The literature says nothing about it. This project has already decided it, in a comment
that is worth quoting in full because the *reason* has expired:

> *"A door change is a full teardown… The alternative — keep the library, vocabulary and thumbnails
> warm and swap only the map — is faster and is the whole point of one window, but it makes every
> entity, resource and history stack I forget to clear a stale value that survives into the next
> door. A reload cannot be wrong; a partial teardown can be, silently, and the bug lands weeks later
> looking like something else."* — `screen.rs:19-32`

**That reasoning is sound and this run does not overturn it.** A partial teardown is a bug factory.

**But the conclusion it reached — "there is no Kit↔Map key" — rests on a premise that has expired**:
that the alternative to a door key was a *relaunch*. It is not. The alternative is a **full
teardown that is not a reload**: despawn the door's entities, reset the door's own state, and open
the other door — in one application, without the menu, without losing the project.

**And the arm is not door state.** It is the *author's intent*, and it is worth carrying.

---

## The candidates, ranked

### 1. **The arm cannot cross the door** — a composition can be made on one door and not placed on the other.

**Measured cost:** 16 house compositions defined, 0 stamped, 727 hand-placed rows in the map,
13 of them `town/houses_plaster` regions placed by hand. An author who has just built a house
composition must leave the editor, come back through the menu, re-open the map, and **re-arm the
composition by hand** — because `ComposeState` is reset by the door change and the Map door has no
`Enter`-to-arm at all.

**Why it beats the runner-up:** the runner-up (the 282 fences) is a *content* problem — those rows
are the game's own block layout, laid by `rules.ron` at runtime, and the last iteration's log
already recorded that as *"a **content decision for the owner**"*. This one is a **tool** problem:
the editor cannot do the thing it already has the data model for, and no content decision fixes it.
Every future composition an author makes inherits the same wall.

### 2. The 282 hand-placed fences.

Measured, and it is the largest single number in the map. But `rules.ron` already says the game
lays the blocks out, and the fences are the *border* of a block. Converting them to groups is
content work, and the log already called it the owner's.

### 3. Editor operations that do not exist (kit rename, project rename, project delete, bash create, policy patch add, composition delete/rename).

A scout of `bevy_emerge_core` found **no function for any of the six** — not even one without a UI
caller. They are real holes, but each is a *convenience*, and each needs a new on-disk writer with
its own re-parse-before-save discipline. Six small features, none of which unlocks anything.

### 4. `fill.rs` has no test at all.

`fill.rs` is 1,058 lines and the **only** untested module in the mapper. A flood and a box fill
are the two gestures that make a level cheap. This is a *quality* item: it is worth doing, and it
is not a feature an author feels.

### 5. Tokens with no reader (5 of 11 `effects` tokens, `placement.rooms`, `placement.group`).

Real, and the vocabulary file's own prose says so. A `vocab.ron` field naming the reader, checked
at load, would make the file's notes enforceable.

---

## What great looks like

Acceptance criteria, phrased so a reviewer could fail the implementation against them.

1. **The Map door can arm a composition.** The author picks a composition from a list *on the map
   tab*, and the next click stamps it. Measured by: an author on the Map door arms a composition and
   the map takes a stamp, with no other application involved.
2. **The arm survives a door change.** An author arms a composition on the Kit door, crosses to the
   Map door, and it is **still armed** — the same id, by name, with the composition's name shown
   where the author can see it. Measured by: arm on the Kit door, cross, and the Map door's stamp
   gesture stamps *that* composition.
3. **The arm survives the door change because the door change is not a reset of it.** The
   classification in `screen.rs`'s `OWNERSHIP` table moves `ComposeState` from `Ownership::Door` to
   the class that means *"this is the author's intent, and it crosses"*. The ratchet
   `the_door_resets_what_it_says_it_owns` is the table's own guard and it must still pass.
4. **A door change is a teardown, not a reload.** Every entity the old door made is gone, every
   door-owned resource is reset, and the project is **not** dropped and re-read from disk. A
   door change costs a door change, not a project reload — and the proof is that the project's
   loaded state is *not* re-read.
5. **A refusal, not a degradation.** A door change that cannot complete — a project that will not
   open, a kit that will not bind — says so and **stays where it is**, exactly as the chooser's
   `open_the_selection` does.
6. **Nothing is lost on the way.** A door change with unsaved edits asks first, the way
   `leave_for_menu` does. The arm is not the only thing that crosses; the *decision* about unsaved
   work is unchanged.
7. **One path.** The arm is set in one place and read in one place. There is no second way to arm
   a composition, and no fallback that arms it when the first path refuses.
8. **The census holds.** A new verb adds **one row** to `BINDINGS` in `keys.rs`, and the action
   census's ceilings are not raised to make room for it.

## Gaps

The literature does not settle three things, and this project must decide them on its own.

- **Whether the arm should be persisted to disk.** It should not: an arm is a moment, and a file
  on disk that says "the next click stamps this" is a stale intent waiting to surprise someone. It
  belongs in memory, and it belongs in the class of resources that survive a door change.
- **Whether the Map door should show the whole COMPOSE panel, or a picker.** The whole panel is
  the author's own *authoring* surface — the carousel, the members, the stages. The Map door wants
  the **picker**: which compositions exist, which one is armed, and the verb that arms it. A
  **picker on the Map door** is the smaller, more honest slice, and it is the one that makes the
  round trip unnecessary rather than merely shorter.
- **How the door change is invoked.** It cannot be a second way to do one thing — the crate's own
  rule. The door strip already shows which door is open; a **door switch on that strip** is one
  way to do one thing, and it is where the author looks.
