# Plan — the arm crosses the door

Run: 2026-10-20-04. Feature: **a composition can be armed on the map, and an armed composition
survives a door change.**

---

## The decision

**Two changes, in this order, each small and each independently testable.**

1. **The arm becomes its own resource, and it is `Session`-owned.** It leaves `ComposeState` and
   becomes `compose::Armed`. The `Ownership` table says it is *"true for as long as the application
   runs"* — which is exactly what an arm is.
2. **The Map door grows a COMPOSITIONS picker** in its INSPECTOR panel: the project's
   compositions, the armed one starred, and the same `Enter` verb that arms one on the Kit door.

**The door switch is not this run** — see *Why a door switch key — and why it is NOT this run*
below. The measurement said the round trip is not what makes the arm unusable; the **door boundary**
is, and the picker removes the need to cross it at all.

### Why the arm becomes its own resource

`ComposeState` bundles five things, and they do not belong together:

| field | what it is | whose |
|---|---|---|
| `selected`, `member`, `focus` | the author's cursors, in a list | **door** — a different kit's ids mean nothing here |
| `staged` | what the carousel last stood up | **door** — the entities are despawned |
| `status` | what the panel last said | **door** — a toast about a door the author has left |
| **`armed`** | **which composition the next click stamps** | **session** — it is the intent, and the intent crosses |

`screen.rs` already has the class: `Ownership::Session` is *"True for as long as the application
runs: caches, generations, input, the surface. Kept."* The arm is the author's intent, it is
in-memory, and it must outlive a door change.

**Splitting it out is the whole trick.** A door change resets `ComposeState` wholesale, and it will
keep doing so — the cursors *should* reset. But the arm is not in `ComposeState` any more, so the
door change does not reach it.

### Why the Map door grows a section rather than the whole COMPOSE panel

The COMPOSE panel is an *authoring* surface: the carousel, the members, the stages, the budget. The
Map door wants a **picker**: which compositions exist, which one is armed, and the verb that arms
it. Shipping the whole panel onto the map door would put an authoring surface where an authoring
action is not — and the panel's own docs say its members list is *"the second cursor, under the one
above"*, which means nothing on a map.

**A section in the INSPECTOR panel** is the smaller, more honest slice, and it is where the author
already looks: the panel that tells them the brush, the yaw, and what is under the cursor.

### Why a door switch key — and why it is NOT this run

**This was in the plan, and the measurement said otherwise.**

`screen.rs` records why there is no such key today:

> *"It became affordable once the process boundary went — the design doc had rejected it when it
> meant a relaunch — and it was declined anyway: with full teardown the saving is the menu
> round-trip rather than the reload, and a second way to do one thing is the pattern this crate
> spends its refusals on."*

**That reason has expired.** The "reload" it weighed a menu round-trip against **no longer exists**:
the menu and the editor are one application, and a door change is a state change inside it.

**But the door switch is a different feature, and it is not this one.** It touches the most delicate
machinery in the crate — the teardown, the refusal path, the unsaved-edit question — and it buys
*convenience* (skip the menu) rather than *capability* (the arm exists at all on the map).

The capability is the two changes below. Once the Map door has a picker, **the author never has to
cross a door to stamp a composition** — and the arm that *does* cross is the one the author armed
on the Kit door while authoring, which the menu round trip still carries.

**So the door switch is carved out and logged as the next item.** It is the most valuable remaining
thing after this run, and it is worth its own iteration with its own refusals.

### The key budget, measured

`no_context_gains_a_key` pins the **number of bindings per context**, and it is a ratchet: adding
one is a deliberate edit with the reason in the commit message. Measured from the table:

| context | bindings | pinned |
|---|---|---|
| `Global` | 17 | 17 |
| `Map` | **25** | **25** |
| `Meshes` | 32 | 32 |
| `Tiles` | 30 | 31 |
| `Anim` | 11 | 11 |
| `Compose` | **7** | **7** |

`Context::Map` is **at** its ceiling and `Context::Compose` is **at** its ceiling. So the arm
**cannot be a new row in either**, and it cannot be two rows — `every_action_has_exactly_one_binding`
allows exactly one binding per action.

**The answer the constraints force, and it is also the best answer:**

`Context::Map` and `Context::Compose` **never overlap** (a tab overlaps only itself,
`keys.rs:107-120`). So one row can serve both — and the census has a place for a row that serves
everywhere: **`Context::Global`**, which overlaps everything.

**So the arm's binding *moves* from `Context::Compose` to `Context::Global`.** It is a **move, not
an addition**:

* `Context::Global` 17 → **18** (a deliberate edit to the pin, with the reason here).
* `Context::Compose` 7 → **6** (the pin goes *down*, which the ratchet allows for free).
* **Zero new keys to learn.** `Enter` is already the arm's key on the Compose tab, and `Enter` is
  already *"the obvious thing to this row"* on every other tab — `Accept` on Meshes, `TileOpen` on
  Tiles, `BuildDrop` on Tiles, `AdoptMeasured` on Anim. `Enter` is **free** in `Context::Global`.

### **The census refused it, and it was right**

The plan above was written from the key table, and the table says `Enter` is bound in **every tab
context** and in **none** of `Global`. The census's collision test
(`the_key_space_has_no_collisions`) measures the overlap, and `Context::Global` overlaps
everything — so the move produces **seven collisions**:

```
AcceptProposal (Map, Only(Proposed)) and ComposeArm (Global, Any) both take `Enter`
Accept (Meshes, Only(Idle)) and ComposeArm (Global, Any) both take `Enter`
AcceptEdges (Meshes, Only(Proposed)) and ComposeArm (Global, Any) both take `Enter`
TileOpen (Tiles, Only(Browsing)) and ComposeArm (Global, Any) both take `Enter`
BuildDrop (Tiles, Only(Idle)) and ComposeArm (Global, Any) both take `Enter`
BuildSlot (Tiles, Only(Idle)) and ComposeArm (Global, Any) both take `Enter`
AdoptMeasured (Anim, Any) and ComposeArm (Global, Any) both take `Enter`
```

**A `Global` arm would have taken `Enter` away from six other verbs.** The plan was wrong and the
census was right, and the fix is to keep the arm's row in `Context::Compose` and give the **Map
door's picker** its own route in.

### The route the Map door's picker takes

**The message stream, not the census** — and the crate already does this. `crate::tiles`'s
`take_the_one_match` reads `Key::Enter` off the `KeyboardInput` stream for the *tag block*, whose
`Enter` **takes the one match** rather than committing a walk. The census has no way to say that,
because a narrowed list's `Enter` is not a tab verb.

**So the Map door's picker is a narrowed list, and its `Enter` takes the one match.** The picker
is the Map door's INSPECTOR panel — the panel that says *what is in hand* (the brush, the yaw, the
thing under the cursor) — and the arm is exactly that: what the next click will put down.

**And it costs no census row at all.** The arm's row stays where it is; the picker is driven by
the mouse, which is how every list in this editor is driven. The keyboard's route into the picker
is the census's own, and it is the one the author already knows.

### The badge budget

The cap is 8 badges per home per context (`no_home_carries_more_than_it_can_show`). Measured, and
**unchanged by this run** — the arm's row did not move:

| home | Global | Map | Compose | total |
|---|---|---|---|---|
| `ControlId::Detail` | 1 (`CopyInfo`) | 2 (`Undo`, `Redo`) | 7 | Map: 3, Compose: 8 |

Both are inside the cap of 8. And the arm's badge belongs on `ControlId::Detail` — the INSPECTOR
panel, which is where the *brush* is named, and which is the same control the COMPOSE panel's
badge sits on.

**The picker carries no `ControlId` of its own**, and the ratchet is why: the panel's status block
already claims `ControlId::Detail`, and
`every_control_the_census_homes_a_verb_at_is_on_screen` **fails on two visible nodes claiming one
id**. That failure is real — it is what the first build of the picker produced.

## The shape, in detail

### 1. `compose::Armed` — a resource of its own

```rust
/// **Which composition the next click stamps** — the author's intent, and the one thing in the
/// editor that outlives a door change.
#[derive(Resource, Default, Clone, PartialEq, Eq)]
pub struct Armed(pub Option<String>);
```

- `ComposeState::armed` **goes away**. Every read and write goes to the new resource.
- `screen::OWNERSHIP` gets `("bevy_emerge_mapper::compose::Armed", Ownership::Session)`.
- The `Ownership` ratchet (`every_resource_says_what_a_door_does_to_it.rs`) checks both halves of
  the table — classified-but-not-registered and registered-but-not-classified — so the new resource
  **must** be in the table, and it must not be in `door_state!`.

### 2. The Map door's COMPOSITIONS picker

**Where:** the INSPECTOR panel (`spawn_panel`, `editor.rs:1412`), inside the scrolling body, below
the readout and the map-size fields.

**What it draws:** a `COMPOSITIONS` heading, then one row per composition in
`project.compositions.compositions` — the same list, the same order, the same `*` for the armed
one that the COMPOSE tab uses. The row is **clickable** (`chrome::list_row` — a row, not a bare
`Button`), and a click arms.

**What it must not be:** a second authoring surface. No members, no carousel, no stages. The
picker names the compositions and shows which is armed; that is the whole of it.

**No scroll list of its own.** The panel's body is already a scroll area, and a scroll inside a
scroll is two wheels fighting over one gesture — the author scrolls the picker and the panel does
not move, which reads as a stuck panel.

**No `ControlId` of its own.** The panel's status block already claims `ControlId::Detail`, and
`every_control_the_census_homes_a_verb_at_is_on_screen` fails on two visible nodes claiming one
id. The picker's verbs are the panel's verbs; one control, one badge, one answer to "what can I do
here".

### 2a. The picker needs a **keyboard** route, and the build proved why

The first build of the picker was **mouse-only**, and the mouse does not work over BRP:
`bevy_picking` reads the mouse from the `WindowEvent` stream
(`bevy_picking-0.20.0/src/input.rs:153`), and BRP injects into `ButtonInput<MouseButton>` — a
different stream. **No** `Button` in this editor receives a click over BRP: the palette's rows and
the chrome bar's back button are equally unaffected. It was measured, not assumed.

**And a mouse-only picker would be wrong here anyway.** The editor's own brief says the work is to
be done *"by the keyboard, as key strokes are faster"*, and the crate's own palette test drives the
palette from the keyboard for the same reason.

**So the picker is keyboard-first**, and the shape is the crate's own:

* **`P` takes the arrows to the picker** — and the *announcement* is a status line, because a
  gesture that changes what the arrows mean must say so.
* **The arrows walk the compositions** — the palette's own `up`/`down` rows, re-bound at
  `Stance::Picking`. **No new arrow.**
* **`Enter` arms** — free at `Stance::Picking`, because `AcceptProposal` holds it at
  `Stance::Proposed` and the two stances never hold together.

**One new key, `P`.** `Context::Map` goes 25 → 29, stated in the pin with the reason, which is
what the ratchet asks for. `P` is free on the Map tab: it belongs to Meshes (`RotateMeshZ`) and
Compose (`CarouselNext`), and neither context ever overlaps Map.

**The picker is a `Stance` and not a context** because it is a *list on the Map tab*, and the Map
tab's arrows are already bound — to the palette. That is the axis's whole job, and it is why the
collision test passed with no change: `Stances::overlaps` asks `Stance::ALL` rather than
case-analysing.

### 3. The writer

**One writer, and it is `Armed::toggle`.** Three ways to arm — the COMPOSE tab's `Enter`, the Map
door's picker, and the devshot driver — and one writer, so there is one answer to "what is armed".
A second writer would be a second answer, and the crate spends its refusals on exactly that shape.

**The receipt goes to the Map's own status line**, not to a panel of its own: the picker is a block
in the INSPECTOR panel, and the panel's status block is where the Map says anything.

### 4. The door switch — **not this run**

See *Why a door switch key — and why it is NOT this run* above. The measurement said the round trip
is not what makes the arm unusable; the **door boundary** is, and the picker removes the need to
cross it at all.

## Touched modules

| file | what changes |
|---|---|
| `crates/bevy_emerge_mapper/src/compose.rs` | `Armed` is a resource of its own; `ComposeState::armed` goes; `arm` writes the new resource; the COMPOSE tab's rows read it; the picker module |
| `crates/bevy_emerge_mapper/src/screen.rs` | `OWNERSHIP` gains `Armed` as `Session`; `close_the_door` becomes `pub` so a test can drive the teardown |
| `crates/bevy_emerge_mapper/src/editor.rs` | the Map door's COMPOSITIONS picker; `stamp_snap` and `stamp_here` read the new resource |
| `crates/bevy_emerge_mapper/src/devshot.rs` | the sentinel driver arms through the same writer |
| `crates/bevy_emerge_mapper/src/keys.rs` | **the arm's row does not move.** The census refused a `Global` move, and the picker needs no census row |
| `crates/bevy_emerge_core/` | **nothing.** The arm is an editor concept; `compositions.ron` does not change; no schema version moves |

**`bevy_emerge_core` is untouched.** That is a design requirement, not a convenience: the arm is
the editor's memory of what the author is about to do, and it has no business in the file format.

## Schema impact

**None.** No file changes, no version moves, no `deny_unknown_fields` surprises. `compositions.ron`
is read exactly as it is read today, and the map's `stamps` are written exactly as they are written
today.

## Tests

The tests are the point. A feature that is proven only by "it works now" is a comment.

**Two files, and the split is deliberate.**

`tests/the_arm_crosses_the_door.rs` — the new API, six tests:

1. **The arm is a `Session` resource, and the table says so.** The `Ownership` table is the only
   thing that decides what a door change does to a resource, so the test asks the table directly.
   **A test that fails if the arm is classified `Door`.**
2. **The arm is not in the reset list.** The table's word and the code's word have to agree, and
   the code's word is `reset_door_state`.
3. **The arm survives a door change.** Arm a composition, run the teardown, and the arm is **the
   same id**. This is the regression test for the whole feature.
4. **The Map door can arm, and the next click stamps.** The picker's own writer, then the stamp
   verb — a stamp lands in `map.stamps` and names the composition that was armed.
5. **One writer.** The picker and the COMPOSE tab's `Enter` land on the same answer.
6. **A refusal, not a degradation.** An arm naming a composition the project no longer defines is
   refused **by name**, and the arm is **cleared** — so the next click does not stamp nothing
   without saying so.

`tests/the_arm_survives_the_door_change.rs` — **the baseline proof**, two tests, written so they
compile on **both** the baseline and the fixed code:

- They ask **where the arm lives**, which is the question the fix answered. On the baseline the
  answer is *"a field of `ComposeState`, which is `Ownership::Door`"* — and the test **fails**,
  with the defect named in the table's own words. On the fixed code the answer is *"its own
  resource, classified `Session`"*, and it passes.

**Why two files.** `the_arm_crosses_the_door.rs` **cannot compile on the baseline** — the baseline
has no `compose::Armed` and no `compose::picker`. *A regression test that cannot compile against
the broken code cannot be run against the broken code*, and a test nobody has run against the
broken code is a comment. The proof file is the one that runs against the broken code.

### What the build found that the plan did not know

**`close_the_door` had no test at all.** The headless harness does **not** register
`ScreenPlugin` — it inserts the state directly, because a harness that stands up a door is not a
harness that changes one — so `OnExit(Editor)` never runs in a test, and the crate's most delicate
machinery was unreachable. `close_the_door` is now `pub`, and the arm's test drives it directly.

**The first attempt at the door change went through the state machine** (`NextState::set`), and
**passed on the baseline** — because the `OnExit` schedule never ran, so nothing was torn down and
the arm survived trivially. The test was green and proved nothing. Calling the teardown directly
is what made it a real test.

## Risk

| risk | how it is answered |
|---|---|
| **A partial teardown.** `screen.rs` warns that keeping state across a door change is how stale values survive into the next door. | The arm is **one resource with one field**, and it is a **string id** — a composition id. A stale arm is *named*, so it either resolves against the project or the stamp is refused by name. It cannot be a silently-wrong index. |
| **A `Global` arm row collides with six other verbs.** | **It happened, and the census caught it.** The plan moved the arm's row to `Context::Global`; `the_key_space_has_no_collisions` refused it with seven collisions, because `Enter` is bound in every tab context. The row stayed in `Context::Compose` and the picker took the message stream instead — the shape `tiles::take_the_one_match` already uses for the tag block. |
| **Two nodes claiming one `ControlId`.** | The first build gave the picker its own `ControlId::Detail`, and `every_control_the_census_homes_a_verb_at_is_on_screen` failed on it. The picker has **no** `ControlId` of its own; the panel's status block keeps it. |
| **The core is engine-free.** | `bevy_emerge_core` is not touched. `engine_free.rs` passes because nothing changed. |

## Not in this run

- **Renaming or deleting a kit, a project, or a composition.** The scout found **no core function
  for any of them**; each needs a new writer with its own re-parse-before-save discipline. A
  separate run.
- **A bash creator.** Same: a new on-disk writer for `kits.ron`.
- **A policy patch adder.** The scout's note is that a `Patch` has a multi-line `because` and a
  nested `Descriptor`, so `LineDoc::append` — the one-line-record tool — is not a direct fit. It
  needs span insertion.
- **The 282 fences.** Content, and the game's own `rules.ron` lays the blocks out.
- **A door switch that keeps the project's library warm.** The teardown stays a teardown. This run
  only stops the *reload* from being needed — the project is not re-read from disk, because the
  door change is a state change inside one application.
