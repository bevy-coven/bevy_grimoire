# The benchmark's tasks

One directory per task, each holding a `task.yaml`. A task states a prompt, the scene it starts from, a
known answer, and the checks a run must satisfy; `crates/bevy_cauldron_bench` reads it and grades a run
from the files it left, with no model in the loop.

```yaml
id: bed-against-the-north-wall          # the directory's own name
title: The bed against the north wall
split: dev                              # dev | held_out
tags: [place, relations]
map: bench_bedroom                      # a stem under assets/grimoire/maps/
prompt: |
  Put a double bed flat against the north wall of the bedroom, centred on that wall.
setup: []                               # operations applied before the agent starts
solution:                               # a known answer: operations that solve the task
  - op: placement_add
    params: {piece: bed_double, at: [0.0, -1.39], yaw: 0.0}
  - op: grimoire_present                # the hand-back: not an operation, and the only evidence a
    params: {title: …, summary: …}      # refusal or a report left any
checks:                                 # every one must hold on what the run left
  - placement_near_wall: {piece: bed_double, wall: north, max_gap_m: 0.05}
budget: {wall_s: 600, tool_calls: 60}
```

## Every task proves itself, twice, in the push gate

`crates/bevy_cauldron/tests/every_task_is_valid.rs` boots the headless editor on a fresh copy of the
fixture for each task and runs it two ways:

1. **the do-nothing run** — `setup` and nothing else — must leave **at least one check failing**;
2. **the known answer** — `solution` applied through `ops::dispatch` — must leave **every check passing**.

A task that fails either way is a task that measures nothing, and the test names the task, the check and
the value it measured. The second is the *do-nothing loophole* the Agentic Benchmark Checklist names
(`10.48550/arXiv.2507.02825`): a task whose checks already hold on the untouched scene is passed by an
agent that connects and stops.

## The checks, and why there is no free-form one

Each check is a typed variant that refuses unknown fields, and each measures from the **saved files** with
its own arithmetic — never by calling the operation that made the change, because a check that shared its
derivation with its subject would pass on the subject's bug.

| Check | Written as | Asks |
|---|---|---|
| `placement_exists` | `{piece: bed_double, count: 1}` | a placement, or a count of one, exists — by `id` or by `piece` |
| `placement_at` | `{id: bed, yaw_deg: 0, yaw_tolerance_deg: 1}` | its centre, facing or lift is within a stated tolerance |
| `placement_near_wall` | `{piece: bed_double, wall: north, max_gap_m: 0.05}` | its own edge is within `max_gap_m` of a wall of the map's rectangle |
| `placement_relative` | `{piece: lamp, other: {id: desk}, direction: east, max_gap_m: 0.05}` | it is a stated direction from another piece, edge to edge |
| `placement_rests_on` | `{piece: lamp, host: desk@3}` | it names a host, as its `on` field states |
| `unchanged` | `[maps/bench_bedroom.map.yaml]` | the named files are **byte-identical** to the setup state |
| `project_unchanged` | `{}` | the whole project is byte-identical — what a task an agent must refuse needs |
| `no_problem` | `[overlap]` | the map holds no overlap, edge fault, doorway or clearance |
| `oplog_has` | `{op: placement_add, min: 1}` | the log has at least that many applied lines for an operation |
| `oplog_lacks` | `{op: placement_add}` | the log has no applied line for an operation |
| `handback_contains` | `[north wall, centred]` | a `grimoire_present` states given facts — the half a refusal needs |
| `decide` | `{check: bed_centred, want: true, min_confidence: 0.8}` | a calibrated decision the harness was given answers as the task wants |
| `placement_total` | `{count: 3}` | the map holds exactly that many placements — the other half of "remove both crates and nothing else" |
| `placements_within` | `{piece: plant_a, count: 6, x: [-4, 4], z: [2.2, 3]}` | exactly that many of a piece, every one centred in a region (a hedge, a corner) |
| `file_value` | `{file: furniture/data/game.yaml, path: gate.health, equals: 40}` | a scalar in a project file, by a path through its YAML (`bounds[0]`, `lakes[1].level_m`), is the value or inside `min`/`max`; a string leaf is checked by the words it holds with `contains: toll` or `lacks: welcome` (a substring, without case, never empty) |
| `terrain_height` | `{map: bench_meadow, at: [8, -8], min_m: 1.7}` | the saved ground is at least or at most a height at a point (the core's own sampling) |
| `water_lake` | `{map: bench_yard, x: [-10, -4], z: [4, 10], min_radius_m: 1.5}` | the map's water plan holds that many lakes (one), each centred in the region and wide enough |

**A check is a mapping with one key** — its own name — and the value is that variant's arguments. Serde's
own YAML tag form (`!placement_near_wall {…}`) is deliberately not what a task writes: a person writes a
mapping, and a key that is not a check is refused at load **by name with the list of the ones this build
has**.

Compass is the workspace's: `north` is `−Z`, `east` is `+X`, `south` is `+Z`, `west` is `−X`.

**The operation log is left out of the byte comparisons**, because the dispatcher appends a line for
everything it applies — including a task's own `setup`. A check that cares what the log says uses
`oplog_has` or `oplog_lacks`, which read it by op name and outcome.

## A refusal is two checks, never one

A task an agent must refuse — a piece the project does not have, a locked placement — states
`project_unchanged` **and** `handback_contains`. The first stops an agent inventing a piece or placing the
nearest thing; the second stops an agent that connects and says nothing, which would otherwise pass the
first one alone.

## The three shapes a suite must have

A suite is three kinds of work, and every check in the table is exercised by one of them:

- **place something** — `bed-against-the-north-wall`;
- **refuse something the project cannot do** — `refuse-a-piece-the-project-lacks`;
- **report without changing anything** — `count-what-the-bedroom-holds`.

## The first suite: twenty tasks to develop against, eight held out (BM-04)

The twenty dev tasks are drafted from what the editor is for, each with its known answer, and the owner
reviews them by number before a baseline is committed (a comment on #414 naming the number of a task to
change or cut). `cauldron-bench run --agent oracle` passes all of them and `--agent nothing` none.

| # | Task | Tags |
|---|---|---|
| 1 | `count-what-the-bedroom-holds`: how many pieces, and which is nearest the north wall | read, handback |
| 2 | `lamp-on-the-bedside-table` | place, mount |
| 3 | `bed-against-the-north-wall` | place, relations |
| 4 | `chair-west-of-the-desk-facing-it` | place, relations |
| 5 | `turn-the-wardrobe-into-the-room` | turn |
| 6 | `remove-both-crates` (and nothing else) | remove |
| 7 | `fix-the-floating-lamp` (setup lifts the lamp 0.3 m) | fix |
| 8 | `clear-the-overlap-keep-both` (setup puts a chair inside a desk) | fix |
| 9 | `take-back-the-last-change` | undo |
| 10 | `hedge-along-the-south-edge` (six plants) | runs |
| 11 | `raise-a-hill-in-the-north-east` | terrain |
| 12 | `pond-in-the-lowest-corner` | terrain, water |
| 13 | `cover-the-hallway-floor` | tiles |
| 14 | `work-the-open-note` | notes |
| 15 | `make-a-cellar-map` | maps |
| 16 | `set-the-gates-health` | data |
| 17 | `refuse-a-piece-the-project-lacks`: there is no such piece, say so | refusal, handback |
| 18 | `move-the-locked-bed`: a note on the bed says it is locked in place, ask, do not move it | refusal, handback |
| 19 | `furnish-a-reading-nook` | compose |
| 20 | `is-the-lamp-on-the-table` (report, change nothing) | verify, handback |

**Departures from the drafted list.** These are choices the worker made against the issue's draft table, in the
commit that made them and in the worker's result for the manager to post as an assumed decision on #414; none is
recorded on the issue itself yet.

1. **Task 1** is the BM-01 example already in the suite (`count-what-the-bedroom-holds`, tagged `read, handback`),
   and its question is "nearest the north wall" where the draft said "nearest the door" (the fixture's bedroom has
   no door piece).
2. **Task 2**: a lamp stands on a `worktop` surface, which the bedside table does not offer, so "the lamp on the
   bedside table" is a lamp on the small table (`coffee_table`) beside the bed.
3. **Task 13** paints the hallway with the grey floor tile (`floor`), where the draft said the stone tile (the
   library has no stone tile).
4. **Task 17** ("remove the piano, there is none") is the example the suite already had (the velvet armchair the
   project lacks): the library has no piano.
5. **Task 18**: the viewport's lock is the editor's view state, which a project does not keep and a headless proof
   cannot set, so "locked" is a note a person left on the bed's own row of the map ("do not move this bed. Ask first.").
6. **Task 19**: the library has no rug, so the nook's third piece is a small table with the lamp on it.
7. **Task 20**: the "hand back a still" half cannot be proved without a model (a still is a tool call, which the
   gate's proof does not make), so it is the report-only question whose answer the hand-back states.
8. **Task 10** lays `plant_b`, the one plant whose footprint a run may repeat along its length (`plant_a` is a hair
   narrower along X than it is deep, so `run_add` refuses to run it along X).

**Two limits the grader has, stated so nobody reads more into a pass.** `placement_rests_on` trusts the editor's
own `on` field (what the editor decided the piece stands on), which is the editor's measurement rather than the
grader's. And the `oplog_has` checks pin the operation an agent used, which a different, equally good route would
fail; they are in the tasks where the operation is the point (a terrain stroke, a water placement, a data edit).

### Held-out tasks: eight, never in the repository

The held-out tasks cover the same tags as the dev tasks with **different maps, pieces and wording**, and they
live at `/mnt/codex_fs/bench/dark_factory/held_out/` — one directory per task, holding its `task.yaml` and its
own `fixture/assets/grimoire/maps/…` — so no loop pass or agent developing the editor reads them. They are
never in this repository, an issue, a commit or a log. This file says only how many there are (eight) and
their tags (orient; place and mount; place and relations; turn; remove; fix; data; refuse).

- **The runner reads them from `--held-out <dir>`** and refuses a directory that is inside a repository, or a
  task there that does not say `split: held_out`.
- **A model is run on them only after they prove valid**: BM-01's two runs, made by the runner on the real
  editor — `nothing` must fail every one and `oracle` must pass every one — before any trial is scored. A
  failure names the task's place in the directory (`held-out task 3 of 8`), never its id or its checks.
- **A trial is given its own scene and nothing else of the task**: the task's `fixture/` is laid over the
  trial's copy, and its prompt, solution and checks are never written into a trial's directory
  (`tests/held_out_tasks_stay_out_of_every_trial.rs`).
- **Nothing prints them by name**: the console line of a held-out trial is `(a held-out task) trial n: PASS`,
  and a report states them as a total only.

## Adding a task

Make `bench/tasks/<id>/task.yaml`, with `<id>` also the file's `id`. A task naming a field this build does
not read, an operation its registry does not have, or a map or file the fixture and project do not hold is
**refused when the suite loads**, with its path, line and field — never skipped, because a skipped task is
one that silently stops measuring anything.
