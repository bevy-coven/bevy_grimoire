# Perf — the door switch

Run: 2026-10-20-05. What the change costs, measured.

## The number that matters

**The project is not re-read.** Measured over three crossings, from the editor's own log:

| | before the crossings | after three crossings |
|---|---|---|
| **project reads** | 2 | **2** |
| **map reads** | 2 | **3** |

**Two project reads in the whole session: one at startup, and one for the message about the pieces
past the lattice ceiling.** The round trip used to cost **a full project reload** each way.

## What the reload used to cost

`Project::open` is not a cheap read:

| | what it does |
|---|---|
| `kits.ron` | parsed, and every bound kit validated |
| `vocab.ron` | parsed, and every table and library cross-checked |
| `rules.ron` | parsed, and every block validated against the merged library |
| 332 library descriptors | read from two kits' `library.ron`, and **layered with each kit's policy** |
| 16 compositions | read from `compositions.ron`, and **validated against the library** |
| **every GLB's triangle count** | **measured, by reading the file's JSON chunk** |

The last row is the expensive one, and `project.rs` says why it is done at all: *"triangle count is
a fact about the file, so keeping it in the schema would mean a number that silently goes stale the
first time an artist re-exports."* **A good reason to measure it at open. Not a reason to measure it
again when the author walks from one door to the other to look at the thing they just made.**

**And the `Ownership` table already said so**: `Project` is *"reloaded when the kit is not"*. A
same-kit crossing is not a kit change, so the project should **survive** it. It does now.

## The change's cost

**One key, one resource, one panel block.**

| | before | after |
|---|---|---|
| Keystrokes to cross | **3** (`Cmd+O`, `right`, `Enter`) | **1** (`=`) |
| A full project reload | **yes, each way** | **no** |
| New census rows in `Context::Global` | — | **1** — `SwitchDoor`, on `=`, and the pin says why |
| New census rows in `Context::Map` | — | **0** — the crossing is `Global`, because it must work on both doors |
| The teardown | a full teardown | **a full teardown** — unchanged |
| `bevy_emerge_core` | — | **untouched** |

**The steady-state cost is one `PreUpdate` system and one comparison.** The crossing reads a key and
returns; the strip's cue is rewritten only when the door changes.

## The measurement

`crates/bevy_emerge_mapper/tests/the_door_switch_crosses.rs` — five tests:

| | what it pins |
|---|---|
| `the_crossing_keeps_the_project` | the project **survives** a crossing, by identity — `Project` is not `Clone`, and the marker a re-read would lose is still there |
| `the_crossing_tears_the_old_door_down` | the door's own working state is back at its opening value — **the teardown is a teardown** |
| `a_crossing_that_cannot_complete_stays_put` | a refused crossing says why and **leaves the editor exactly where it was** |
| `the_crossing_and_the_menu_are_different_verbs` | two different actions, two different keys, and the cue is **on the door strip** |
| `the_project_is_a_project_owned_resource` | the `Ownership` table says `Project` is `Ownership::Project` — the thing that decides |

**And the live check**: the editor was started from the command line on Sanctuary's town map, and
three crossings were driven over BRP — **zero panics**, and the log's project-read count did not
move.

## The suite

| | Result |
|---|---|
| `cargo test --workspace --no-fail-fast` | **1,234 pass, 1 fail** — the one failure is `no_two_leaders_cross`, red since 2026-09-03 and not this work's. The baseline measured 1,229; this run adds 5. |
| The suite, run **three times** | the same result each time — **no flaky test** |
| `cargo test -p bevy_emerge_mapper --lib keys::` | 32 pass — **including `the_key_space_has_no_collisions`**, so the new row costs no key |
| `cargo clippy --workspace --all-targets -- -D warnings` | **no finding in any file this change touches** |
| `cargo fmt --check` | every file this change touches keeps its **pre-existing** diff count, and `args.rs` drops from 1 to 0 |
| `cargo doc --no-deps` | **25 unresolved-link warnings, the same 25 as the baseline** — this change adds none |
| Headless boot against Sanctuary's real project | `town` → 12 frames, 8,192 entities, no panic; `town_block` → 4,096 entities, no panic |
| Sanctuary's own suite | 1,014 pass, 4 fail — the same four that were already red |

## What the build found that the plan did not know

**A crossing must happen in `PreUpdate`, and the plan did not say so.** The editor's Map-door
systems are gated by `Door::map_door_is_open` — a **run condition on the tuple** — and Bevy evaluates
a *set's* conditions **once per schedule**
(`bevy_ecs-0.20.0/src/schedule/executor/multi_threaded.rs`: `evaluated_sets`). So a crossing that
ran inside `Update` left that condition evaluated against the *old* door, and four systems that take
a `Res<OpenMap>` ran on the Kit door — which has no map — and Bevy 0.19 **panics a system whose
`Res<T>` is missing** rather than skipping it. **`PreUpdate` runs before `StateTransition`, which
runs before `Update`**, so the crossing has changed the door before anything evaluates its
conditions against it.

**The first version left the world without a `Project` for the rest of the frame**, and **twenty**
editor systems read one. The fix was to move the crossing into `PreUpdate` — where the world holds a
project at every instant — and to mark the transition so the teardown spares the *new* door's
resources.

**The headless harness never registered `ScreenPlugin`**, so a door change had **no test at all** —
which is why the crossing's own test went green while the door's working state *survived* it. The
harness registers it now, and the crossing is testable end to end.

**The backquote was the wrong key, and only the picture said so.** The backquote is the industry's
cue for "the other place" (`Cmd+\`` is VS Code's *toggle terminal*, `Ctrl+\`` is Blender's *toggle
console*) — and **at 11 px it read as a speck**. A grave accent is a small mark at any size, and a
key the author cannot see is a key they will not press. `=` is the widest mark on the keyboard and
the most legible of the free ones, and it is the shape of a toggle.

**Two tests shared a fixture directory, and the tests removed each other's project.** A fixture's
directory is keyed by the process id and the test's name, and the tests run **in parallel** in one
process — so two tests with one name delete each other's project. It was measured as a test that
passed on three runs out of six, and the fix is a name per test.
