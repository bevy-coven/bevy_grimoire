# Perf — the arm crosses the door

Run: 2026-10-20-04. What the change costs, measured.

## The number that matters

**A house is on the map in four keystrokes and a click.** Before, the arm was reachable only from
the COMPOSE tab, which is on the Kit door, while the click that stamps is on the Map door — and the
two never met. The author's only route was to leave, come back through the menu, re-open the map,
and **re-arm the composition by hand**, because the arm had been reset with the rest of the door's
working state.

**Sixteen house compositions were defined in Sanctuary's project and not one was stamped on either
map.** The maps held 727 hand-placed rows and zero stamps. The `town.map.ron` had
`13 × town/houses_plaster` — a *region marker*, which the game's own `rules.ron` fills at runtime —
and no composition at all.

## The change's cost

**One resource, one stance, one key, and a block in a panel that already existed.**

| | Before | After |
|---|---|---|
| The arm | a field of `ComposeState` | its own resource, `compose::Armed` |
| A door change | reset the arm with it | **kept it** — the table says `Ownership::Session` |
| New census rows in `Context::Map` | — | **4**: `PickComposition`, `CompositionPrev`, `CompositionNext`, `ArmComposition` |
| New **keys** the author learns | — | **1**: `P` |
| New arrows | — | **0** — the palette's own `up`/`down`, re-bound at `Stance::Picking` |
| `bevy_emerge_core` | — | **untouched** |

**Three of the four census rows cost no key.** `Enter` is free at `Stance::Picking` because
`AcceptProposal` holds it at `Stance::Proposed` and the two stances never hold together; the two
arrow rows are the palette's own, re-bound. **The collision test is the proof** — it passed with no
change, because `Stances::overlaps` asks `Stance::ALL` rather than case-analysing.

**The steady-state cost is zero.** The picker's rows are rebuilt only when the project, the arm or
the cursor changes. The two key systems are gated on `in_map_mode`, so the other four doors do not
run them at all.

## The measurement

`crates/bevy_emerge_mapper/tests/the_arm_crosses_the_door.rs` — six tests, and the baseline proof
is a seventh file that **compiles on both the broken and the fixed code**.

| | Baseline | Fixed |
|---|---|---|
| The arm's home | a field of `ComposeState` | `compose::Armed` |
| The table's answer for it | **`Ownership::Door` — reset** | **`Ownership::Session` — kept** |
| The proof's verdict | **FAILS**, naming the defect | passes |
| The Map door's way to arm | **none** | `P`, `up`/`down`, `Enter` |

**The live check** — the editor was started from the command line on Sanctuary's town map, and the
whole flow was driven over BRP:

* `P` → the status reads *"the arrows walk the compositions — Enter arms one"*, and the first row
  takes the cursor's rail.
* `ArrowDown` `ArrowDown` → the cursor is on `town/house_cottage_wood`.
* `Enter` → the row carries the accent `*`, and the status reads *"`town/house_cottage_wood` armed
  — the next click stamps it"*.
* A click on the map → **MAP** reads `745 placed (727 hand, 18 group(s)), unsaved`, and **UNDER**
  reads *"stamped 'town/house_cottage_wood' as house_cottage_wood@1"*.

**The 18 group rows are the 18 members of that composition.** One click, one house, and the map
carries it as a *reference*.

## The suite

| | Result |
|---|---|
| `cargo test --workspace --no-fail-fast` | **1,229 pass, 1 fail** — the one failure is `no_two_leaders_cross`, red since 2026-09-03 and not this work's. The baseline measured 1,221; this run adds 8. |
| `cargo test -p bevy_emerge_mapper --lib keys::` | 32 pass — **including `the_key_space_has_no_collisions`**, which is what says the four new rows cost no key |
| `cargo clippy --workspace --all-targets -- -D warnings` | **no finding in any file this change touches** |
| `cargo fmt --check` | `compose.rs` and `keys.rs` keep their **pre-existing** diff counts (94 and 22); the new test files are clean |
| `cargo doc --no-deps` | **24 unresolved-link warnings, the same 24 as the baseline** — this change adds none |
| Headless boot against Sanctuary's real project | `town` → 12 frames, 8,192 entities, no panic |
| Sanctuary's own suite | 1,014 pass, 4 fail — the same four that were already red |

## What the build found that the plan did not know

**`close_the_door` had no test at all.** The headless harness does **not** register
`ScreenPlugin` — it inserts the state directly, because a harness that stands up a door is not a
harness that changes one — so `OnExit(Editor)` never runs in a test, and the crate's most delicate
machinery was unreachable. `close_the_door` is now `pub`, and the arm's test drives it directly.

**The first attempt at the door change went through the state machine** (`NextState::set`), and
**passed on the baseline** — because the `OnExit` schedule never ran, so nothing was torn down and
the arm survived trivially. **The test was green and proved nothing.** Calling the teardown
directly is what made it a real test.

**The mouse path does not work over BRP, and that is a crate-wide fact, not a defect in this
change.** `bevy_picking` reads the mouse from the `WindowEvent` stream
(`bevy_picking-0.20.0/src/input.rs:153`), and BRP injects into `ButtonInput<MouseButton>` — a
different stream. So **no** `Button` in this editor receives a click over BRP: the palette's rows
and the chrome bar's back button are equally unaffected. It was measured, not assumed, and it is
why the picker has a **keyboard** route — which is also what the editor's own brief asks for
(*"this should be done by the keyboard, as key strokes are faster"*, and the crate's own palette
test drives the palette from the keyboard for the same reason).

**The first build of the picker gave it its own `ControlId::Detail`**, and
`every_control_the_census_homes_a_verb_at_is_on_screen` failed on it: the panel's status block
already claims that id, and two visible nodes claiming one id is a bug rather than a tie to break.
The picker has **no** `ControlId` of its own.
