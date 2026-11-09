# Research — the editor's two halves, and the door between them

Run: 2026-10-20-05. Feature: **an author crosses between the Kit door and the Map door without
going through the menu.**

---

## The measurement that started this

I drove the real editor over BRP against a copy of Sanctuary's project and did the work an author
does: arm a composition, stamp it, save, reload, and then go and work on the thing that was stamped.

### The round trip, as an author pays it

The editor has **two doors**. The **Kit** door authors content — meshes, tiles, compositions. The
**Map** door builds the level out of it. The door is chosen at startup (`--door kit`, `--door map`),
and there is **no key that changes it** — `screen.rs` says so:

> *"Doors are still reached through the menu. No direct Kit↔Map key. It became affordable once the
> process boundary went — the design doc had rejected it when it meant a relaunch — and it was
> declined anyway: with full teardown the saving is the menu round-trip rather than the reload, and
> a second way to do one thing is the pattern this crate spends its refusals on."*

**Measured, the round trip is six keystrokes and a reload:**

| | Keystrokes | What happens |
|---|---|---|
| Map → Kit | `Cmd+O`, `right`, `Enter` | leave the door, the chooser opens, walk to KITS, open the Kit door |
| Kit → Map | `Cmd+O`, `right`, `Enter` | the same, the other way |

**Three keystrokes each way.** Every one of them a *reopen*, and the reopen is the expensive part:
`OnExit(Editor)` runs `close_the_door`, which **drops the `Project`** and reads it back off disk
in the next door.

### What that costs, measured on Sanctuary's town map

| | The round trip | The fix |
|---|---|---|
| `kits.ron`, `vocab.ron`, `rules.ron` | re-read and re-validated | **not re-read** |
| 332 library descriptors, 16 compositions | re-read, re-measured | **not re-read** |
| The GLB triangle counts | re-measured from every file's JSON chunk | **not re-measured** |
| The armed composition | **kept** (last run's `compose::Armed`, `Ownership::Session`) | kept |
| The map's own rows | re-read, re-expanded | re-read, re-expanded |

**The Project is re-read on every crossing, and nothing in it changes.** `Project::open` parses
`kits.ron`, validates every bound kit's `library.ron`, loads `compositions.ron` and `rules.ron`,
and **measures the triangle count of every GLB by reading its JSON chunk** — because, as
`project.rs` says, *"triangle count is a fact about the file, so keeping it in the schema would mean
a number that silently goes stale the first time an artist re-exports."*

That is a good reason to measure it at open. It is not a reason to measure it again when the
author walks from one door to the other **to look at the thing they just made**.

### The workflow the round trip breaks

The last run made the arm cross the door. The workflow that exposes the remaining wall is:

> **Make a house on the Kit door → stamp it on the Map door → go back to the Kit door to see the
> house's members, its derived interface, and what has gone stale under it.**

**The COMPOSE tab is the only place that shows a group's members, its edge tokens, its faults and
its STALE list** — and it is on the Kit door, while the map that holds the stamp is on the Map door.
So the author's loop is: *stamp on the map, walk to the Kit door, read the group, walk back, place
the next one.* **Each of those walks is three keystrokes and a full project reload.**

**And the reload is not free of consequence.** The `Project` is `Ownership::Project` — *"reloaded
when the kit is not"* — so a crossing that keeps the same kit *should* keep it, and the round trip
reloads it anyway. The `Ownership` table already states the rule the door switch needs; the round
trip does not obey it.

---

## The literature

### Task switching: the cost is the thing, and the cost is measurable

Braem & Egner, *Getting a grip on cognitive flexibility* (2018,
[10.1177/0963721418787475](https://doi.org/10.1177/0963721418787475)) — the review that defines
the field's central object:

> "Task sets can be considered a configuration of context-dependent production ('if, then') rules
> that are actively maintained in order to guide our current behavior."

**A door is a task set.** The Kit door's rules are *"to author, `Enter` arms, `M` captures a
selection"*; the Map door's are *"to place, `P` takes the arrows, the click stamps"*. They are
different rule sets, and the author holds one at a time.

> "Using paradigms in which participants have to switch between two or more tasks, task switching
> studies typically focus on the switch cost: slower and less accurate performance on task switches
> than task repetitions. The switch cost has been interpreted as an index of cognitive control
> processes required for reconfiguring the task set."

**The switch cost is the metric, and it is measured in time and errors.** The editor's round trip
is a switch cost: three keystrokes, one reload, and a project re-validated from disk.

### The cues that make a switch cheap

The same paper's most useful finding is about *how* a switch is cued:

> "It has been shown that switch costs can be reduced for stimuli that are presented at a screen
> location associated with a higher likelihood of task switches (relative to repetitions), even when
> people are unaware of this contingency (Crump & Logan, 2010)."

> "By employing three different task sets, we could demonstrate that stimuli (here, specific
> individuals' faces) associated with task switches did indeed facilitate task switching, and that
> they did so irrespective of which task was being switched to. This suggests that what participants
> learned was to associate specific cues with a **general readiness to switch between tasks rather
> than to switch to one particular alternative task**."

**Two design instructions, and they are the whole design:**

1. **The cue is a place, not a menu.** A location that is associated with switching makes the
   switch cheaper. The editor's **door strip** — the row at the top that says `1 MAP` or
   `1 MESHES  2 TILES  3 COMPOSE` — is exactly such a place, and it is where the author already
   looks to see where they are.
2. **The cue is a *readiness* to switch, not a switch *to* something.** A cue that says "you may
   cross" is reusable for both directions. A key that says "open the Map door" is not: it means the
   opposite thing on the other side.

### Awareness without a mechanism

Dourish & Bellotti, *Awareness and coordination in shared workspaces* (1992,
[10.1145/143457.143468](https://doi.org/10.1145/143457.143468)) — 2,548 citations, and the paper
that established that a shared workspace can carry awareness *passively*, through the workspace
itself:

> "Awareness of individual and group activities is critical to successful collaboration and is
> commonly supported in CSCW systems by active, information generation mechanisms separate from the
> shared workspace. These mechanisms penalise information providers, presuppose relevance to the
> recipient, and make access difficult."

**The editor already has the passive mechanism: the door strip.** It says which door is open, in
the place the author looks. What it lacks is a way to *act on* that awareness.

### The one-path rule, and what it costs

Cockburn, Gutwin, Scarr & Malacria, *Supporting Novice to Expert Transitions in User Interfaces*
(2014, [10.1145/2659796](https://doi.org/10.1145/2659796)) — the review's four domains include
**"intermodal improvement, which occurs when users switch from one method to another that has a
higher performance ceiling"** and **"task mapping, which examines the ways in which users perform
their tasks"**.

This project has a strong rule against a second way to do one thing, and this paper is the reason
the rule has a shape: **the exception is a *mode* with a higher ceiling, not a duplicate.** A door
key is not a second way to open a door — it is a way to *change* which door is open, and the menu
still opens a door. Those are different acts, and the census can tell them apart because they are
different actions with different keys.

### The seam, and the cost of crossing it

Ishii, *TeamWorkStation: towards a seamless shared workspace* (1990,
[10.1145/99332.99337](https://doi.org/10.1145/99332.99337)) — on the *seam* between two
workspaces, and the mechanism that removes it:

> "The key ideas are the overlay of individual workspace images in a virtual shared workspace and
> the creation of a shared drawing surface. Because each co-worker can continue to use his/her
> favorite application programs or manual tools in the virtual shared workspace, the **cognitive
> discontinuity (seam) between the individual and shared workspaces is greatly reduced**, and users
> can shuttle smoothly between these two workspaces."

**The editor's seam is the reload.** The two doors are one project, and the author shuttles between
them constantly; the reload makes the shuttle a *process* rather than a *movement*, and the seam is
where the author's model of the project is rebuilt from disk instead of carried.

---

## The candidates, ranked

### 1. **A door switch.** The two halves of the editor cannot be crossed without the menu.

**Measured cost:** three keystrokes and a **full project reload** each way — `kits.ron`,
`vocab.ron`, `rules.ron`, 332 library descriptors, 16 compositions, and every GLB's triangle count
re-read and re-validated — to move between two halves of one project. The `Ownership` table already
says a same-kit crossing should *keep* the `Project`; the round trip does not.

**Why it beats the runner-up:** the runner-up (the six missing editor operations — kit rename,
project rename, project delete, bash creation, policy patch add, composition rename/delete) is
**six separate holes**, and a scout of `bevy_emerge_core` found **no function for any of them** —
not even one without a UI caller. Each needs a new on-disk writer with its own re-parse-before-save
discipline, and the scout noted that a `Patch` has a multi-line `because` and a nested
`Descriptor`, so `LineDoc::append` is not a direct fit. **That is six features.** The door switch is
**one**, and it is the one that every one of the other five would be *done from* — an author renaming
a kit renames it *on* the Kit door, and then wants to see the map that uses it.

### 2. The six missing editor operations.

Real, and six of them. But: six features, six new on-disk writers, and the scout's own note is that
the patch one does not fit the existing comment-preserving tool. **Carve it**: the most valuable
slice is kit rename, which is the one an author does *while authoring* and which the door switch
would then make seamless.

### 3. The 282 hand-placed fences.

Measured, and the largest single number in the map. But `rules.ron` already says the *game* lays
the blocks out, and the fences are the *border* of a block — so this is content, and the last three
iterations called it the owner's.

### 4. `fill.rs` has no test at all.

1,058 lines, the one untested module in the mapper, and the module that lays the bulk of a level. A
flood and a box fill are the two gestures that make a level cheap, and nothing pins their lattice,
their refusals, or their idempotence. **Worth doing, and not a feature an author feels.**

### 5. `vlm.rs:1436-1442` — `probe` returns `Ready` without connecting.

A LAN endpoint that is down is reported ready. Needs a test that calls `probe`, not `is_near`.

---

## What great looks like

Acceptance criteria, phrased so a reviewer could fail the implementation against them.

1. **One key crosses the doors.** From either door, one key opens the other. The author never
   types the chooser's argv, and never re-navigates three columns to get back to where they were.
2. **The cue is a place, not a menu.** The door strip — where the editor already says which door is
   open — is where the switch lives. A key is not a place.
3. **The cue is a readiness, not a direction.** The same key means the same thing on both doors:
   *"you may cross"*, not *"go to the Map"*. A key that named a direction would mean the opposite
   thing on the other side, which is the mode error Braem & Egner's *"general readiness to switch"*
   finding is about.
4. **The crossing does not re-read the project.** The `Project` is `Ownership::Project` — *"reloaded
   when the kit is not"* — and a crossing that keeps the kit **keeps the project**. Measured: the
   library's triangle counts, the compositions and the vocabulary are **not** re-read.
5. **A refusal, not a degradation.** A crossing that cannot complete — a kit that will not bind, a
   map that will not read — says so and **stays where it is**, the way `open_the_selection` does.
6. **Nothing is lost.** A crossing with unsaved edits asks first, exactly as `leave_for_menu` does.
   The question is not new and it is not changed.
7. **The teardown stays a teardown.** Every entity the old door made is gone, and every door-owned
   resource is reset. The `OWNERSHIP` table's `Door` class keeps its meaning.
8. **One path.** The door switch is **one** way to change which door is open. The menu still opens a
   door — that is a different act, and the census can tell them apart because they are different
   actions with different keys.
9. **The census holds.** A new verb adds **one row** to `BINDINGS`, and the key-space and
   badge-budget ratchets are not raised to make room for it.

## Gaps

The literature does not settle three things, and this project must decide them on its own.

- **Whether the crossing keeps the map's unsaved rows.** The `OpenMap` is `Ownership::Project`
  and holds the *map*, not the project — and the Map door's map is the thing the author was editing.
  The crossing has to carry it or lose it, and the two are different: the map's rows may be
  unsaved, and the *project* may be shared by both doors.
- **Whether the Kit door's panel set is the right target.** The Kit door has three panels
  (MESHES, TILES, COMPOSE) and the Map door one (MAP). A crossing has to land on a *specific*
  panel, and "the first one" is a rule the author did not choose.
- **Whether the cue belongs on the door strip or in the chrome bar.** The strip says which door is
  open; the bar says which map is open. A cue on the strip is *where the door is named*, and a cue
  in the bar is *where the map is named*. The literature says a **place** makes the switch cheap,
  and both are places — so the choice is this project's, and it should be made by what the author
  looks at when they want to cross.
