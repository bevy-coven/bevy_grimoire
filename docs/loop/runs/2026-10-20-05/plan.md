# Plan — the door switch

Run: 2026-10-20-05. Feature: **an author crosses between the Kit door and the Map door without
going through the menu, and without the project being re-read.**

---

## The decision

**Two changes, in this order, each small and each independently testable.**

1. **The crossing keeps the `Project`.** A crossing that does not change the kit **keeps the
   project in memory** — the library, the vocabulary, the compositions, the rules and every GLB's
   triangle count. The `Ownership` table already says this: `Project` is *"reloaded when the kit is
   not"*.
2. **One key crosses, and the cue is a place.** The key is on the **door strip** — the row at the
   top that says which door is open — because a *place* that is associated with switching makes the
   switch cheap (Braem & Egner 2018, on Crump & Logan 2010), and the strip is where the author
   already looks to see where they are.

## Why the crossing keeps the project

`Project` is `#[derive(Resource)]` — **not `Clone`**. So the crossing **moves** it out of the world
rather than copying it, and the move has to happen **before** `close_the_door` removes it.

**The route, and why each step is the one it is:**

| step | why |
|---|---|
| **Take the `Project` out of the world.** | `close_the_door` calls `Opened::remove_from(world)`, which removes it. The crossing has to hold it first. |
| **Build the other door's `Opened` with that `Project`.** | The Kit door needs a `Project` and no map; the Map door needs a `Project` and an `OpenMap`. The `Project` is the same object — **not re-read**. |
| **Put the `Opened` in `Chosen`.** | `Chosen` is the one resource that survives `close_the_door` (it is not in the `door_state!` list, and `close_the_door` does not touch it). It is the *existing* handover, used by the menu today. |
| **`NextState::set(Screen::Editor)`.** | **Bevy 0.20 runs `OnExit` and `OnEnter` for a same-state transition** (`NextState::set` is `Pending`, and `Pending` does not suppress the schedules — only `PendingIfDifferent` does). So `OnExit(Editor)` runs `close_the_door` — the teardown — and `OnEnter(Editor)` runs the spawns. |

**And the chooser never appears.** A same-state transition runs neither `OnExit(Menu)` nor
`OnEnter(Menu)`, so the chooser's `build_chooser` / `spawn_screen` / `spawn_menu_bars` never run.
The author sees the editor tear down and the other door build, in one frame.

**The alternative — hopping through `Screen::Menu` — is out**, and the reason is the chooser:
`ChooserPlugin` spawns the whole menu UI on `OnEnter(Menu)`, so a hop would build the chooser and
tear it down — a flash of the wrong screen on every crossing.

### What the crossing costs

| | the round trip (today) | the crossing |
|---|---|---|
| keystrokes | **3** (`Cmd+O`, `right`, `Enter`) | **1** |
| `kits.ron`, `vocab.ron`, `rules.ron` | re-read and re-validated | **not re-read** |
| 332 library descriptors, 16 compositions | re-read, re-validated | **not re-read** |
| every GLB's triangle count | **re-measured from the file's JSON chunk** | **not re-measured** |
| the map's own rows | re-read, re-expanded | re-read, re-expanded (the map is a *map*, not the project) |
| the armed composition | kept | kept |

**The triangle counts are the expensive part.** `project.rs` says: *"triangle count is a fact about
the file, so keeping it in the schema would mean a number that silently goes stale the first time an
artist re-exports."* That is a good reason to measure it at open. It is **not** a reason to measure
it again when the author walks from one door to the other to look at the thing they just made.

## The shape, in detail

### 1. The crossing

**A new module**, `crates/bevy_emerge_mapper/src/door_switch.rs` — or a section of `screen.rs`.
The design prefers **`screen.rs`**: the crossing is a *screen* concern, it touches the teardown, and
`screen.rs` already owns `close_the_door` and the `Ownership` table.

```rust
/// **Cross to the other door, keeping the project.**
pub fn cross(world: &mut World, to: Door) -> Result<(), String>
```

**It cannot fail silently**, and the failure must be *before* anything is torn down — that is the
whole point of the refusal path.

**The order is the design:**

1. **The refusal first.** If the map is dirty and the crossing would lose it, ask — the same
   question `leave_for_menu` asks. If the target door cannot be built, say so and **stay put**.
2. **Take the `Project` out of the world.** `world.remove_resource::<Project>()`.
3. **Build the target's `Opened`.**
   - To the **Kit** door: `{project, open_map: None, door: Kit, mode: Kit.opens_on()}`.
   - To the **Map** door: `{project, open_map: Some(OpenMap::open(&project, name)?), door: Map,
     mode: Map}`. **The map is re-read** — it is a *map*, and the Map door's subject is the map.
4. **Put it in `Chosen`.**
5. **`NextState::set(Screen::Editor)`.**

**One subtlety, and it is the whole feature:** the `Project` must be **put back** if the crossing
cannot complete. Step 3 can fail (`OpenMap::open` refuses a map that will not read), and a failure
after step 2 has taken the `Project` out of the world would leave the editor with no project —
which is the panic the crate's own `open_the_door` note is about. **So the `Project` is re-inserted
on the failure path**, and the editor carries on exactly as it was.

### 2. The key

**One row in `BINDINGS`.** The rule is *"A new verb adds one row to `BINDINGS` in `keys.rs`,
nowhere else."*

**Which key?** The scout measured the budget:

| context | bindings | pin | free bare letters |
|---|---|---|---|
| `Context::Map` | 29 | 29 | **C, I, L** |
| `Context::Global` | 17 | 17 | **none** — every letter is bare-bound in some tab, and Global overlaps all of them |

**A `Global` row is the only shape that works**, because the crossing must be live on **both** doors
and `Context::Map` is only live on the Map one. That costs one row in every context's budget, which
is what the pin is for — and it means **every letter is taken**, so the key is a bare non-letter.

**The free bare keys are `Digit4`–`Digit0`, `Minus`, `Equal`, `Semicolon`, `Quote`, `Backquote`,
`Backslash` and `Tab`.** Each is ruled out for one reason: `Tab` is Bevy's focus traversal, the
digits are **the most learned keys in this editor** (`1`/`2`/`3` are the door-relative panel
slots), `;` and `-` are punctuation the author reads as punctuation, and `\` is the shell's "leave
this level" — a meaning this key does not have.

**`Quote` and `Backquote` fail on legibility, and only the picture said so.** The backquote was the
first choice — it is the industry's cue for "the other place" (`Cmd+\`` is VS Code's *toggle
terminal*, `Ctrl+\`` is Blender's *toggle console*) — and **at 11 px it read as a speck**: a grave
accent is a small mark at any size, and a key the author cannot see is a key they will not press.

**`=` is the widest mark on the keyboard and the most legible of the free ones**, and it is the
shape of a toggle — which is what the crossing is: the same key, the other door.

**And the direction problem.** Braem & Egner's finding is that a cue should be *"a general readiness
to switch between tasks rather than to switch to one particular alternative task"*. So the key says
**"the other door"** — the same word on both sides — and **the strip shows which way is other**.

**The refusal, and the announcement.** The crossing says what it did, in the status line, because a
door change is exactly the moment an author needs to know where they are: *"the kit door"*.

### 3. The strip grows the other door

**The cue is a place, and the place says what the key does.** The door strip currently reads
`1 MAP` on the Map door and `1 MESHES  2 TILES  3 COMPOSE` on the Kit door — it says which door is
open and which panel is live.

**The strip should also say the other door is one key away.** So the strip gains a second mark: the
**other door's name**, in the dimmer ink.

**And the key is the badge's, not the strip's.** The strip's own convention is that a control states
its key in its own text and the badge cluster repeats it — the tab chips print `1`, `2`, `3` and the
badges print them again. So the strip prints the *name* and the badge prints the *key*, and the two
are complementary rather than two statements of the same fact. **The first version printed the key
in the strip as well, and the two sat three pixels apart** — the editor's own *"a fact stated more
than once drifts"* rule, caught by looking.

**The Rigs door names nothing.** It is the rig bench — a character asset rather than level content —
and `screen.rs` says it *"crosses with neither"*. A cue that leads nowhere is a lie, so the mark
hides itself there.

### 4. Where the crossing runs

**In `PreUpdate`, and that is the whole design.** The editor's Map-door systems are gated by
`Door::map_door_is_open` — a **run condition on the tuple** — and Bevy evaluates a *set's* conditions
**once per schedule** (`bevy_ecs-0.20.0/src/schedule/executor/multi_threaded.rs`:
`evaluated_sets`). So a crossing that ran inside `Update` left that condition evaluated against the
*old* door, and four systems that take a `Res<OpenMap>` ran on the Kit door — which has no map —
and Bevy 0.19 **panics a system whose `Res<T>` is missing** rather than skipping it.

**`PreUpdate` runs before `StateTransition`, which runs before `Update`**, so the crossing has
changed the door before anything evaluates its conditions against it.

**And the world holds a project at every instant of the crossing.** The first version took the
`Project` out of the world and put it back in `Chosen` — which is what `close_the_door` removes and
`open_the_door` restores — and **twenty** editor systems read one in the frame the crossing left it
missing. The crossing now puts the project back **before it returns**, and a `Crossing` marker tells
the teardown that the door's own resources are already the *new* door's.

**And the map's name is carried across**, because the Kit door has no map: without it, crossing back
to the Map door has nothing to open, and the refusal it gives is *true* and useless — the author was
on a map a moment ago. `Project::open_map_name` is that memory, and it is the same argument the
armed composition makes.

## Touched modules

| file | what changes |
|---|---|
| `crates/bevy_emerge_mapper/src/screen.rs` | the crossing — it owns the teardown and the `Ownership` table |
| `crates/bevy_emerge_mapper/src/keys.rs` | **one row** in `BINDINGS`; the `Context::Global` pin 17 → 18 |
| `crates/bevy_emerge_mapper/src/tiles.rs` | the door strip grows the other door's mark |
| `crates/bevy_emerge_mapper/src/chooser.rs` | nothing — `Chosen` is the existing handover |
| `crates/bevy_emerge_core/` | **nothing.** No file changes, no schema version, no on-disk format |

**`bevy_emerge_core` is untouched.** The crossing is an editor concern: it changes which *panel set*
is on screen, and no file format is involved.

## Schema impact

**None.** No file changes, no version moves, no `deny_unknown_fields` surprises. Nothing is written
to disk by a crossing except the map's own save, which is the existing path.

## Tests

1. **The crossing keeps the project.** Arm nothing, cross from the Map door to the Kit door, and
   assert that the `Project` in the world is **the same value** — not a re-read. The proof: put a
   marker on the project that a re-read would lose. *(`Project` is not `Clone`, so the identity is
   observable.)*
2. **The crossing tears the old door down.** After a crossing, every entity the old door made is
   gone, and every `Ownership::Door` resource is at its opening value. **The teardown stays a
   teardown.**
3. **A crossing that cannot complete says so and stays put.** A crossing whose target map will not
   read leaves the editor **exactly as it was** — the same door, the same project, the same map, the
   same arm. **A refusal, not a degradation.**
4. **The unsaved question.** A crossing with unsaved map edits asks first.
5. **The key is one row**, and the census's collision, badge-budget and key-count ratchets hold.
6. **The crossing is not the menu's route.** The menu still opens a door; the crossing *changes* one.
   A test that asserts the two are different actions, and that both reach a door.
7. **The round trip still works.** `Cmd+O` → the chooser → the other door. **The crossing is added,
   not substituted** — the menu's route is not removed.

## Risk

| risk | how it is answered |
|---|---|
| **A partial teardown.** `screen.rs` warns that keeping state across a door change is how stale values survive into the next door. | The teardown **stays a teardown** — `close_the_door` runs exactly as it does today, and the `OWNERSHIP` table's `Door` class keeps its meaning. Only the `Project` is carried, and it is `Ownership::Project` — the class that already says a same-kit crossing keeps it. |
| **The `Project` is taken out and cannot be put back.** | The refusal runs **before** the `Project` is taken, and the failure path **re-inserts** it. A crossing that cannot complete leaves the editor untouched — and that is tested. |
| **A key that means opposite things on the two doors** — a mode error. | The key says *"the other door"*, not *"the Map door"*, and the strip says which way is other. Braem & Egner's *"a general readiness to switch rather than to switch to one particular alternative task"* is the instruction. |
| **A `Global` key costs every context's budget.** | One row, and the pin goes 17 → 18 with the reason stated. The row is `Context::Global` because the key must work on **both** doors, and `Context::Map` is only live on one. |
| **The core is engine-free.** | `bevy_emerge_core` is not touched. `engine_free.rs` passes because nothing changed. |

## Not in this run

- **Kit rename, project rename, project delete, bash creation, policy patch add, composition
  rename/delete.** Six holes, six new on-disk writers, and the scout's note is that a `Patch` has a
  multi-line `because` and a nested `Descriptor`, so `LineDoc::append` is not a direct fit. **The
  crossing is the one that makes them all easier** — an author renaming a kit renames it *on* the
  Kit door, and then wants to see the map that uses it.
- **The 282 hand-placed fences.** Content, and the game's own `rules.ron` lays the blocks out.
- **`fill.rs`'s missing tests.** Worth doing, and not a feature an author feels.
- **`vlm.rs`'s `probe` that never connects.** A test that calls `probe`, not `is_near`.
